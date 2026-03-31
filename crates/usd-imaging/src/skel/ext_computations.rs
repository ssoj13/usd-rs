//! UsdSkelImaging ext computations - CPU skinning invoke, callback, GLSL kernel.
//!
//! Port of pxr/usdImaging/usdSkelImaging/extComputations.h/cpp
//!
//! Provides the skinning ext computation: CPU callback that invokes UsdSkel
//! skinning, and GLSL kernel loading for GPU path.

use super::tokens::{
    EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS, EXT_COMPUTATION_INPUT_TOKENS,
    EXT_COMPUTATION_LEGACY_INPUT_TOKENS, EXT_COMPUTATION_OUTPUT_TOKENS,
};
use std::sync::Arc;
use usd_gf::matrix3::Matrix3d;
use usd_gf::matrix4::Matrix4d;
use usd_gf::matrix4::Matrix4f;
use usd_gf::vec2::Vec2f;
use usd_gf::vec3::Vec3f;
use usd_hd::data_source::{HdRetainedTypedSampledDataSource, HdValueExtract};
use usd_hd::ext_computation_context::HdExtComputationContext;
use usd_hd::ext_computation_cpu_callback::{
    HdExtComputationCpuCallback, HdExtComputationCpuCallbackValue,
};
use usd_hd::schema::HdStringDataSourceHandle;
use usd_skel::utils::{
    SKINNING_METHOD_DQS, SKINNING_METHOD_LBS, skin_face_varying_normals_interleaved,
    skin_normals_interleaved, skin_points_interleaved, skin_transform,
};
use usd_tf::Token;
use usd_vt::{Array, Value};

/// Env setting: force CPU compute for skinning (skip GLSL GPU path).
///
/// Matches C++ USDSKELIMAGING_FORCE_CPU_COMPUTE.
pub fn force_cpu_compute() -> bool {
    std::env::var("USDSKELIMAGING_FORCE_CPU_COMPUTE").map_or(false, |v| {
        v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes")
    })
}

/// Invoke the skinning ext computation.
///
/// Reads inputs from the context, performs CPU skinning (points and/or normals),
/// writes outputs. Matches C++ `UsdSkelImagingInvokeExtComputation`.
pub fn invoke_ext_computation(skinning_method: &Token, ctx: &mut dyn HdExtComputationContext) {
    let rest_points = ctx
        .get_optional_input_value(&EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS.rest_points)
        .cloned();
    let rest_normals = ctx
        .get_optional_input_value(&EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS.rest_normals)
        .cloned();

    if rest_points.is_none() && rest_normals.is_none() {
        log::error!("No rest points or normals provided");
        ctx.raise_computation_error();
        return;
    }

    if rest_points.is_some() {
        invoke_skinning_computation_points(skinning_method, ctx);
    } else {
        invoke_skinning_computation_normals(skinning_method, ctx);
    }
}

fn matrix4f_to_matrix4d(m: &Matrix4f) -> Matrix4d {
    Matrix4d::new(
        m[0][0] as f64,
        m[0][1] as f64,
        m[0][2] as f64,
        m[0][3] as f64,
        m[1][0] as f64,
        m[1][1] as f64,
        m[1][2] as f64,
        m[1][3] as f64,
        m[2][0] as f64,
        m[2][1] as f64,
        m[2][2] as f64,
        m[2][3] as f64,
        m[3][0] as f64,
        m[3][1] as f64,
        m[3][2] as f64,
        m[3][3] as f64,
    )
}

fn read_input<T>(ctx: &mut dyn HdExtComputationContext, token: &Token) -> Option<T>
where
    T: HdValueExtract + std::fmt::Debug,
{
    T::extract(&ctx.get_input_value(token))
}

fn read_input_i32_vec(ctx: &mut dyn HdExtComputationContext, token: &Token) -> Option<Vec<i32>> {
    Array::<i32>::extract(&ctx.get_input_value(token)).map(|array| array.to_vec())
}

fn read_input_vec4f(
    ctx: &mut dyn HdExtComputationContext,
    token: &Token,
) -> Option<Vec<usd_gf::vec4::Vec4f>> {
    let value = ctx.get_input_value(token);

    if let Some(values) = value.get::<Vec<usd_gf::vec4::Vec4f>>() {
        return Some(values.clone());
    }
    if let Some(values) = value.get::<Array<usd_gf::vec4::Vec4f>>() {
        return Some(values.to_vec());
    }
    if let Some(values) = value.get::<Vec<Value>>() {
        let mut result = Vec::with_capacity(values.len());
        for value in values {
            result.push(usd_gf::vec4::Vec4f::extract(value)?);
        }
        return Some(result);
    }

    None
}

fn apply_packed_blend_shapes(
    offsets: &[usd_gf::vec4::Vec4f],
    ranges: &[usd_gf::vec2::Vec2i],
    weights: &[f32],
    points: &mut [Vec3f],
) {
    let end = ranges.len().min(points.len());
    for i in 0..end {
        let range = ranges[i];
        let mut p = points[i];
        for j in range.x..range.y {
            let j = j as usize;
            if j < offsets.len() {
                let offset = &offsets[j];
                let shape_index = offset.w as usize;
                let weight = weights.get(shape_index).copied().unwrap_or(0.0);
                p.x += offset.x * weight;
                p.y += offset.y * weight;
                p.z += offset.z * weight;
            }
        }
        points[i] = p;
    }
}

fn transform_points(points: &mut [Vec3f], xform: &Matrix4d) {
    for p in points.iter_mut() {
        let pd = usd_gf::vec3::Vec3d::new(p.x as f64, p.y as f64, p.z as f64);
        let t = xform.transform_point(&pd);
        *p = Vec3f::new(t.x as f32, t.y as f32, t.z as f32);
    }
}

fn transform_normals(normals: &mut [Vec3f], xform_inv_transpose: &Matrix3d) {
    for n in normals.iter_mut() {
        let nd = usd_gf::vec3::Vec3d::new(n.x as f64, n.y as f64, n.z as f64);
        let t = nd * *xform_inv_transpose;
        *n = Vec3f::new(t.x as f32, t.y as f32, t.z as f32);
    }
}

fn invoke_skinning_computation_points(
    skinning_method: &Token,
    ctx: &mut dyn HdExtComputationContext,
) {
    use usd_gf::vec2::Vec2i;
    use usd_gf::vec4::Vec4f;

    let rest_points: Vec<Vec3f> =
        match read_input(ctx, &EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS.rest_points) {
            Some(v) => v,
            None => {
                ctx.raise_computation_error();
                return;
            }
        };
    let geom_bind_xform: Matrix4f = match ctx
        .get_input_value(&EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS.geom_bind_xform)
        .get::<Matrix4f>()
    {
        Some(m) => *m,
        None => {
            ctx.raise_computation_error();
            return;
        }
    };
    let influences: Vec<Vec2f> =
        match read_input(ctx, &EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS.influences) {
            Some(v) => v,
            None => {
                ctx.raise_computation_error();
                return;
            }
        };
    let num_influences_per_component: i32 = match ctx
        .get_input_value(&EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS.num_influences_per_component)
        .get::<i32>()
    {
        Some(&n) => n,
        None => {
            ctx.raise_computation_error();
            return;
        }
    };
    let has_constant_influences: bool = match ctx
        .get_input_value(&EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS.has_constant_influences)
        .get::<bool>()
    {
        Some(&b) => b,
        None => {
            ctx.raise_computation_error();
            return;
        }
    };
    let prim_world_to_local: Matrix4d = match ctx
        .get_input_value(&EXT_COMPUTATION_LEGACY_INPUT_TOKENS.prim_world_to_local)
        .get::<Matrix4d>()
    {
        Some(m) => *m,
        None => {
            ctx.raise_computation_error();
            return;
        }
    };
    let blend_shape_offsets: Vec<Vec4f> = match read_input_vec4f(
        ctx,
        &EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS.blend_shape_offsets,
    ) {
        Some(v) => v,
        None => {
            ctx.raise_computation_error();
            return;
        }
    };
    let blend_shape_offset_ranges: Vec<Vec2i> = match read_input(
        ctx,
        &EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS.blend_shape_offset_ranges,
    ) {
        Some(v) => v,
        None => {
            ctx.raise_computation_error();
            return;
        }
    };
    let blend_shape_weights: Vec<f32> =
        match read_input(ctx, &EXT_COMPUTATION_INPUT_TOKENS.blend_shape_weights) {
            Some(v) => v,
            None => {
                ctx.raise_computation_error();
                return;
            }
        };
    let skinning_xforms: Vec<Matrix4f> =
        match read_input(ctx, &EXT_COMPUTATION_INPUT_TOKENS.skinning_xforms) {
            Some(v) => v,
            None => {
                ctx.raise_computation_error();
                return;
            }
        };
    let skel_local_to_world: Matrix4d = match ctx
        .get_input_value(&EXT_COMPUTATION_LEGACY_INPUT_TOKENS.skel_local_to_world)
        .get::<Matrix4d>()
    {
        Some(m) => *m,
        None => {
            ctx.raise_computation_error();
            return;
        }
    };

    let mut skinned_points = rest_points;

    apply_packed_blend_shapes(
        &blend_shape_offsets,
        &blend_shape_offset_ranges,
        &blend_shape_weights,
        &mut skinned_points,
    );

    let num_influences = num_influences_per_component as usize;
    if num_influences <= 0 {
        ctx.set_output_value(
            &EXT_COMPUTATION_OUTPUT_TOKENS.skinned_points,
            Value::from(skinned_points),
        );
        return;
    }

    let geom_bind_d = matrix4f_to_matrix4d(&geom_bind_xform);
    let joint_xforms: Vec<Matrix4d> = skinning_xforms.iter().map(matrix4f_to_matrix4d).collect();

    let skinning_method_str = if skinning_method == SKINNING_METHOD_LBS {
        Token::new(SKINNING_METHOD_LBS)
    } else if skinning_method == SKINNING_METHOD_DQS {
        Token::new(SKINNING_METHOD_DQS)
    } else {
        Token::new(SKINNING_METHOD_LBS)
    };

    if has_constant_influences {
        let mut skinned_xform = Matrix4d::identity();
        let joint_indices: Vec<i32> = influences
            .iter()
            .take(num_influences)
            .map(|i| i.x as i32)
            .collect();
        let joint_weights: Vec<f32> = influences
            .iter()
            .take(num_influences)
            .map(|i| i.y)
            .collect();
        if skin_transform(
            &skinning_method_str,
            &geom_bind_d,
            &joint_xforms,
            &joint_indices,
            &joint_weights,
            &mut skinned_xform,
        ) {
            let rest_to_prim_local_skinned =
                skinned_xform * skel_local_to_world * prim_world_to_local;
            transform_points(&mut skinned_points, &rest_to_prim_local_skinned);
        }
    } else {
        let _ = skin_points_interleaved(
            &skinning_method_str,
            &geom_bind_d,
            &joint_xforms,
            &influences,
            num_influences,
            &mut skinned_points,
        );
        let skel_to_prim_local = skel_local_to_world * prim_world_to_local;
        transform_points(&mut skinned_points, &skel_to_prim_local);
    }

    ctx.set_output_value(
        &EXT_COMPUTATION_OUTPUT_TOKENS.skinned_points,
        Value::from(skinned_points),
    );
}

fn invoke_skinning_computation_normals(
    skinning_method: &Token,
    ctx: &mut dyn HdExtComputationContext,
) {
    use usd_gf::matrix3::Matrix3d;

    let rest_normals: Vec<Vec3f> =
        match read_input(ctx, &EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS.rest_normals) {
            Some(v) => v,
            None => {
                ctx.raise_computation_error();
                return;
            }
        };
    let geom_bind_xform: Matrix4f = match ctx
        .get_input_value(&EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS.geom_bind_xform)
        .get::<Matrix4f>()
    {
        Some(m) => *m,
        None => {
            ctx.raise_computation_error();
            return;
        }
    };
    let influences: Vec<Vec2f> =
        match read_input(ctx, &EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS.influences) {
            Some(v) => v,
            None => {
                ctx.raise_computation_error();
                return;
            }
        };
    let num_influences_per_component: i32 = match ctx
        .get_input_value(&EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS.num_influences_per_component)
        .get::<i32>()
    {
        Some(&n) => n,
        None => {
            ctx.raise_computation_error();
            return;
        }
    };
    let has_constant_influences: bool = match ctx
        .get_input_value(&EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS.has_constant_influences)
        .get::<bool>()
    {
        Some(&b) => b,
        None => {
            ctx.raise_computation_error();
            return;
        }
    };
    let prim_world_to_local: Matrix4d = match ctx
        .get_input_value(&EXT_COMPUTATION_LEGACY_INPUT_TOKENS.prim_world_to_local)
        .get::<Matrix4d>()
    {
        Some(m) => *m,
        None => {
            ctx.raise_computation_error();
            return;
        }
    };
    let skinning_xforms: Vec<Matrix4f> =
        match read_input(ctx, &EXT_COMPUTATION_INPUT_TOKENS.skinning_xforms) {
            Some(v) => v,
            None => {
                ctx.raise_computation_error();
                return;
            }
        };
    let skel_local_to_world: Matrix4d = match ctx
        .get_input_value(&EXT_COMPUTATION_LEGACY_INPUT_TOKENS.skel_local_to_world)
        .get::<Matrix4d>()
    {
        Some(m) => *m,
        None => {
            ctx.raise_computation_error();
            return;
        }
    };
    let face_vertex_indices: Vec<i32> = match read_input_i32_vec(
        ctx,
        &EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS.face_vertex_indices,
    ) {
        Some(v) => v,
        None => {
            ctx.raise_computation_error();
            return;
        }
    };
    let has_face_varying_normals: bool = match ctx
        .get_input_value(&EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS.has_face_varying_normals)
        .get::<bool>()
    {
        Some(&b) => b,
        None => {
            ctx.raise_computation_error();
            return;
        }
    };

    let num_influences = num_influences_per_component as usize;
    if num_influences <= 0 {
        ctx.set_output_value(
            &EXT_COMPUTATION_OUTPUT_TOKENS.skinned_normals,
            Value::from(rest_normals),
        );
        return;
    }

    let mut skinned_normals = rest_normals;

    let geom_bind_d = matrix4f_to_matrix4d(&geom_bind_xform);
    let geom_bind_inv_transpose = geom_bind_d
        .extract_rotation_matrix()
        .inverse()
        .map(|inv| inv.transpose())
        .unwrap_or_else(Matrix3d::identity);

    let joint_xforms: Vec<Matrix4d> = skinning_xforms.iter().map(matrix4f_to_matrix4d).collect();
    let skinning_inv_transpose: Vec<Matrix3d> = joint_xforms
        .iter()
        .map(|m| {
            m.extract_rotation_matrix()
                .inverse()
                .map(|inv| inv.transpose())
                .unwrap_or_else(Matrix3d::identity)
        })
        .collect();

    let skinning_method_str =
        if skinning_method == SKINNING_METHOD_LBS || skinning_method == SKINNING_METHOD_DQS {
            Token::new(skinning_method.as_str())
        } else {
            Token::new(SKINNING_METHOD_LBS)
        };

    if has_constant_influences {
        let mut skinned_xform = Matrix4d::identity();
        let joint_indices: Vec<i32> = influences
            .iter()
            .take(num_influences)
            .map(|i| i.x as i32)
            .collect();
        let joint_weights: Vec<f32> = influences
            .iter()
            .take(num_influences)
            .map(|i| i.y)
            .collect();
        if skin_transform(
            &skinning_method_str,
            &geom_bind_d,
            &joint_xforms,
            &joint_indices,
            &joint_weights,
            &mut skinned_xform,
        ) {
            let rest_to_prim_local = skinned_xform * skel_local_to_world * prim_world_to_local;
            let inv_transpose = rest_to_prim_local
                .extract_rotation_matrix()
                .inverse()
                .map(|m| m.transpose())
                .unwrap_or_else(Matrix3d::identity);
            transform_normals(&mut skinned_normals, &inv_transpose);
        }
    } else {
        if has_face_varying_normals {
            let _ = skin_face_varying_normals_interleaved(
                &skinning_method_str,
                &geom_bind_inv_transpose,
                &skinning_inv_transpose,
                &influences,
                num_influences,
                &face_vertex_indices,
                &mut skinned_normals,
            );
        } else {
            let _ = skin_normals_interleaved(
                &skinning_method_str,
                &geom_bind_inv_transpose,
                &skinning_inv_transpose,
                &influences,
                num_influences,
                &mut skinned_normals,
            );
        }
        let skel_to_prim_local = skel_local_to_world * prim_world_to_local;
        let skel_to_gprim_inv_transpose = skel_to_prim_local
            .extract_rotation_matrix()
            .inverse()
            .map(|m| m.transpose())
            .unwrap_or_else(Matrix3d::identity);
        transform_normals(&mut skinned_normals, &skel_to_gprim_inv_transpose);
    }

    ctx.set_output_value(
        &EXT_COMPUTATION_OUTPUT_TOKENS.skinned_normals,
        Value::from(skinned_normals),
    );
}

/// CPU callback that invokes skinning. Matches C++ `_SkinningComputationCpuCallback`.
struct SkinningComputationCpuCallback {
    skinning_method: Token,
}

impl HdExtComputationCpuCallback for SkinningComputationCpuCallback {
    fn compute(&self, ctx: &mut dyn HdExtComputationContext) {
        invoke_ext_computation(&self.skinning_method, ctx);
    }
}

/// Data source for skinning CPU computation.
///
/// Matches C++ `UsdSkelImagingExtComputationCpuCallback`.
pub fn ext_computation_cpu_callback(
    skinning_method: &Token,
) -> Option<usd_hd::HdDataSourceBaseHandle> {
    if skinning_method == SKINNING_METHOD_LBS {
        let cb: Arc<dyn HdExtComputationCpuCallback> = Arc::new(SkinningComputationCpuCallback {
            skinning_method: Token::new(SKINNING_METHOD_LBS),
        });
        Some(
            HdRetainedTypedSampledDataSource::new(HdExtComputationCpuCallbackValue::from(cb))
                as usd_hd::HdDataSourceBaseHandle,
        )
    } else if skinning_method == SKINNING_METHOD_DQS {
        let cb: Arc<dyn HdExtComputationCpuCallback> = Arc::new(SkinningComputationCpuCallback {
            skinning_method: Token::new(SKINNING_METHOD_DQS),
        });
        Some(
            HdRetainedTypedSampledDataSource::new(HdExtComputationCpuCallbackValue::from(cb))
                as usd_hd::HdDataSourceBaseHandle,
        )
    } else {
        log::warn!("Unknown skinning method {}", skinning_method.as_str());
        None
    }
}

/// Data source for skinning GPU computation (GLSL kernel).
///
/// Returns None if USDSKELIMAGING_FORCE_CPU_COMPUTE is set or if kernel cannot be loaded.
/// Matches C++ `UsdSkelImagingExtComputationGlslKernel`.
pub fn ext_computation_glsl_kernel(
    skinning_method: &Token,
    computation_type: &Token,
) -> Option<HdStringDataSourceHandle> {
    if force_cpu_compute() {
        return None;
    }
    let kernel_key = match (skinning_method.as_str(), computation_type.as_str()) {
        (SKINNING_METHOD_LBS, "points") => "skinPointsLBSKernel",
        (SKINNING_METHOD_LBS, "normals") => "skinNormalsLBSKernel",
        (SKINNING_METHOD_DQS, "points") => "skinPointsDQSKernel",
        (SKINNING_METHOD_DQS, "normals") => "skinNormalsDQSKernel",
        _ => {
            log::warn!(
                "Unknown skinning method {:?} or computation type {:?}",
                skinning_method.as_str(),
                computation_type.as_str()
            );
            return None;
        }
    };
    let shader_source = super::glslfx::load_skinning_kernel(kernel_key)?;
    Some(HdRetainedTypedSampledDataSource::new(shader_source) as HdStringDataSourceHandle)
}
