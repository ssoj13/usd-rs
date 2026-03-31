//! BlendShapeData - Data for blend shape ext computation inputs.
//!
//! Port of pxr/usdImaging/usdSkelImaging/blendShapeData.h/cpp
//!
//! Data for skinned prim to compute blend shape related inputs.

use super::binding_schema::BindingSchema;
use super::blend_shape_schema::BlendShapeSchema;
use super::data_source_utils::get_typed_value_from_container_vec_vec3f;
use super::inbetween_shape_schema::InbetweenShapeSchema;
use std::collections::HashMap;
use usd_gf::vec2::Vec2i;
use usd_gf::vec3::Vec3f;
use usd_gf::vec4::Vec4f;
use usd_hd::data_source::cast_to_container;
use usd_hd::scene_index::{HdSceneIndexHandle, si_ref};
use usd_sdf::Path;
use usd_tf::Token;

const EPS: f32 = 1e-6;

/// Weight and sub-shape index pair.
#[derive(Debug, Clone)]
pub struct WeightAndSubShapeIndex {
    /// Weight for inbetween (1.0 for main offsets).
    pub weight: f32,
    /// Index to sub shape, -1 if none.
    pub sub_shape_index: i32,
}

/// Data for skinned prim blend shape ext computation inputs.
#[derive(Debug, Clone, Default)]
pub struct BlendShapeData {
    /// Path of deformable prim (for warnings/errors).
    pub prim_path: Path,

    /// List of (offset, subShapeIndex).
    pub blend_shape_offsets: Vec<Vec4f>,

    /// For each point, pair of indices into blend_shape_offsets.
    pub blend_shape_offset_ranges: Vec<Vec2i>,

    /// Number of sub shapes.
    pub num_sub_shapes: usize,

    /// For each blend shape name: list of (weight, subShapeIndex).
    pub blend_shape_name_to_weights_and_indices: HashMap<Token, Vec<WeightAndSubShapeIndex>>,
}

impl BlendShapeData {
    /// Create new empty blend shape data.
    pub fn new(prim_path: Path) -> Self {
        Self {
            prim_path,
            blend_shape_offsets: Vec::new(),
            blend_shape_offset_ranges: Vec::new(),
            num_sub_shapes: 0,
            blend_shape_name_to_weights_and_indices: HashMap::new(),
        }
    }
}

#[inline]
fn is_close_f32(a: f32, b: f32, epsilon: f32) -> bool {
    (a - b).abs() < epsilon
}

fn to_vec4f(v: Vec3f, sub_shape: i32) -> Vec4f {
    Vec4f::new(v[0], v[1], v[2], sub_shape as f32)
}

/// Point index and offset pair (point index, Vec4f offset with subShape in w).
type PointIndexAndOffset = (usize, Vec4f);

fn fill_point_indices_and_offsets_dense(
    offsets: &[Vec3f],
    sub_shape: i32,
    point_indices_and_offsets: &mut Vec<PointIndexAndOffset>,
) {
    for (i, o) in offsets.iter().enumerate() {
        point_indices_and_offsets.push((i, to_vec4f(*o, sub_shape)));
    }
}

fn fill_point_indices_and_offsets_sparse(
    _blend_shape_prim_path: &Path,
    _inbetween_name: &Token,
    indices: &[i32],
    offsets: &[Vec3f],
    sub_shape: i32,
    point_indices_and_offsets: &mut Vec<PointIndexAndOffset>,
) {
    let n = indices.len().min(offsets.len());
    for i in 0..n {
        if indices[i] < 0 {
            continue;
        }
        point_indices_and_offsets.push((indices[i] as usize, to_vec4f(offsets[i], sub_shape)));
    }
}

fn fill_point_indices_and_offsets(
    _blend_shape_prim_path: &Path,
    _inbetween_name: &Token,
    indices: &[i32],
    offsets: &[Vec3f],
    sub_shape: i32,
    point_indices_and_offsets: &mut Vec<PointIndexAndOffset>,
) {
    if indices.is_empty() {
        fill_point_indices_and_offsets_dense(offsets, sub_shape, point_indices_and_offsets);
    } else {
        fill_point_indices_and_offsets_sparse(
            _blend_shape_prim_path,
            _inbetween_name,
            indices,
            offsets,
            sub_shape,
            point_indices_and_offsets,
        );
    }
}

struct WeightAndOffsets {
    weight: f32,
    offsets: Vec<Vec3f>,
    inbetween_name: Token,
}

fn process_blend_shape_prim(
    scene_handle: &HdSceneIndexHandle,
    _prim_path: &Path,
    blend_shape_prim_path: &Path,
    num_sub_shapes: &mut usize,
    weights_and_sub_shape_indices: &mut Vec<WeightAndSubShapeIndex>,
    point_indices_and_offsets: &mut Vec<PointIndexAndOffset>,
) {
    let blend_shape_prim = si_ref(&scene_handle).get_prim(blend_shape_prim_path);

    let Some(ref blend_shape_ds) = blend_shape_prim.data_source else {
        return;
    };

    let blend_shape_schema = BlendShapeSchema::get_from_parent(blend_shape_ds);
    if !blend_shape_schema.is_defined() {
        return;
    }

    let indices = blend_shape_schema.get_point_indices();

    let mut weights_and_offsets: Vec<WeightAndOffsets> = vec![
        WeightAndOffsets {
            weight: 0.0,
            offsets: Vec::new(),
            inbetween_name: Token::new(""),
        },
        WeightAndOffsets {
            weight: 1.0,
            offsets: blend_shape_schema.get_offsets(),
            inbetween_name: Token::new(""),
        },
    ];

    if let Some(inbetween_container) = blend_shape_schema.get_inbetween_shapes_container() {
        for name in inbetween_container.get_names() {
            let Some(child) = inbetween_container.get(&name) else {
                continue;
            };
            let Some(ib_container) = cast_to_container(&child) else {
                continue;
            };
            let inbetween_schema = InbetweenShapeSchema::new(ib_container);
            let Some(weight_ds) = inbetween_schema.get_weight() else {
                continue;
            };
            let weight = weight_ds.get_typed_value(0.0);
            if is_close_f32(weight, 0.0, EPS) || is_close_f32(weight, 1.0, EPS) {
                continue;
            }
            let offsets: Vec<Vec3f> = inbetween_schema
                .get_container()
                .and_then(|c| get_typed_value_from_container_vec_vec3f(c, &Token::new("offsets")))
                .unwrap_or_default();
            weights_and_offsets.push(WeightAndOffsets {
                weight,
                offsets,
                inbetween_name: name.clone(),
            });
        }
    }

    weights_and_offsets.sort_by(|a, b| {
        a.weight
            .partial_cmp(&b.weight)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut prev_weight = f32::NEG_INFINITY;

    for wo in &weights_and_offsets {
        let weight = wo.weight;

        if weight == 0.0 {
            weights_and_sub_shape_indices.push(WeightAndSubShapeIndex {
                weight,
                sub_shape_index: -1,
            });
        } else {
            if is_close_f32(prev_weight, weight, EPS) {
                continue;
            }
            prev_weight = weight;

            let sub_shape = *num_sub_shapes as i32;

            weights_and_sub_shape_indices.push(WeightAndSubShapeIndex {
                weight,
                sub_shape_index: sub_shape,
            });

            fill_point_indices_and_offsets(
                blend_shape_prim_path,
                &wo.inbetween_name,
                &indices,
                &wo.offsets,
                sub_shape,
                point_indices_and_offsets,
            );

            *num_sub_shapes += 1;
        }
    }
}

fn compute_blend_shape_offsets(point_indices_and_offsets: &[PointIndexAndOffset]) -> Vec<Vec4f> {
    point_indices_and_offsets
        .iter()
        .map(|(_, offset)| *offset)
        .collect()
}

fn fill_blend_shape_offset_ranges(
    num_offsets: usize,
    point_indices_and_offsets: &[PointIndexAndOffset],
    ranges: &mut [Vec2i],
) {
    let mut current: i32 = -1;

    for i in 0..num_offsets {
        let point_idx = point_indices_and_offsets[i].0 as i32;
        while current < point_idx {
            if current >= 0 {
                ranges[current as usize][1] = i as i32;
            }
            current += 1;
            if (current as usize) < ranges.len() {
                ranges[current as usize] = Vec2i::new(i as i32, i as i32);
            }
        }
    }
    if current >= 0 && (current as usize) < ranges.len() {
        ranges[current as usize][1] = num_offsets as i32;
    }
}

fn compute_blend_shape_offset_ranges(
    point_indices_and_offsets: &[PointIndexAndOffset],
) -> Vec<Vec2i> {
    if point_indices_and_offsets.is_empty() {
        return Vec::new();
    }

    let num_offset_ranges = point_indices_and_offsets
        .last()
        .map(|(idx, _)| *idx + 1)
        .unwrap_or(0);
    let mut ranges = vec![Vec2i::new(0, 0); num_offset_ranges];
    fill_blend_shape_offset_ranges(
        point_indices_and_offsets.len(),
        point_indices_and_offsets,
        &mut ranges,
    );
    ranges
}

/// Compute BlendShapeData for deformable prim with SkelBindingAPI.
pub fn compute_blend_shape_data(
    scene_handle: &HdSceneIndexHandle,
    prim_path: &Path,
) -> BlendShapeData {
    let mut data = BlendShapeData::new(prim_path.clone());

    let prim = si_ref(&scene_handle).get_prim(prim_path);

    let Some(ref prim_ds) = prim.data_source else {
        return data;
    };

    let binding_schema = BindingSchema::get_from_parent(prim_ds);
    let blend_shape_names = binding_schema.get_blend_shapes();
    let blend_shape_prim_paths = binding_schema.get_blend_shape_targets();

    data.num_sub_shapes = 0;

    let n = blend_shape_names.len().min(blend_shape_prim_paths.len());

    let mut point_indices_and_offsets: Vec<PointIndexAndOffset> = Vec::new();

    for i in 0..n {
        let blend_shape_name = &blend_shape_names[i];
        let blend_shape_prim_path = &blend_shape_prim_paths[i];

        let weights_and_indices = data
            .blend_shape_name_to_weights_and_indices
            .entry(blend_shape_name.clone())
            .or_default();

        if !weights_and_indices.is_empty() {
            continue; // Duplicate blend shape
        }

        process_blend_shape_prim(
            scene_handle,
            prim_path,
            blend_shape_prim_path,
            &mut data.num_sub_shapes,
            weights_and_indices,
            &mut point_indices_and_offsets,
        );
    }

    point_indices_and_offsets.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| (a.1[3] as i32).cmp(&(b.1[3] as i32)))
    });

    data.blend_shape_offsets = compute_blend_shape_offsets(&point_indices_and_offsets);
    data.blend_shape_offset_ranges = compute_blend_shape_offset_ranges(&point_indices_and_offsets);

    data
}

/// Blend shape weights for skel ext computation inputs.
/// One weight per sub shape.
pub fn compute_blend_shape_weights(
    data: &BlendShapeData,
    blend_shape_names: &[Token],
    blend_shape_weights: &[f32],
) -> Vec<f32> {
    let mut result = vec![0.0f32; data.num_sub_shapes];

    if blend_shape_names.len() != blend_shape_weights.len() {
        return result;
    }

    let n = blend_shape_names.len().min(blend_shape_weights.len());

    for i in 0..n {
        let blend_shape_name = &blend_shape_names[i];
        let blend_shape_weight = blend_shape_weights[i];

        let Some(weights_and_indices) = data
            .blend_shape_name_to_weights_and_indices
            .get(blend_shape_name)
        else {
            continue;
        };

        if weights_and_indices.len() < 2 {
            continue;
        }

        if weights_and_indices.len() == 2 {
            let sub_shape_index = weights_and_indices[1].sub_shape_index;
            result[sub_shape_index as usize] = blend_shape_weight;
            continue;
        }

        // Binary search for adjacent weights
        let blend_weight = blend_shape_weight;
        let search_slice = &weights_and_indices[1..weights_and_indices.len() - 1];
        let upper = search_slice.partition_point(|w| w.weight < blend_weight);
        let upper_idx = upper + 1;
        let lower_idx = upper_idx.saturating_sub(1);

        let upper = &weights_and_indices[upper_idx];
        let lower = &weights_and_indices[lower_idx];

        let weight_delta = upper.weight - lower.weight;

        if !(weight_delta > EPS) {
            continue;
        }

        let alpha = (blend_shape_weight - lower.weight) / weight_delta;

        let sub_shape_lower = lower.sub_shape_index;
        if sub_shape_lower >= 0 && !is_close_f32(alpha, 1.0, EPS) {
            result[sub_shape_lower as usize] = 1.0 - alpha;
        }

        let sub_shape_upper = upper.sub_shape_index;
        if sub_shape_upper >= 0 && !is_close_f32(alpha, 0.0, EPS) {
            result[sub_shape_upper as usize] = alpha;
        }
    }

    result
}
