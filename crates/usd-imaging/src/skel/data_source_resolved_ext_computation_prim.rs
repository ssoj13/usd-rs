//! DataSourceResolvedExtComputationPrim - Ext computation prim for skinned geometry.
//!
//! Port of pxr/usdImaging/usdSkelImaging/dataSourceResolvedExtComputationPrim.h/cpp
//!
//! Returns data source for skinning points/normals ext computation prim with
//! inputValues, inputComputations, outputs, glslKernel, cpuCallback, elementCount.

use super::data_source_resolved_points_based_prim::DataSourceResolvedPointsBasedPrim;
use super::ext_computations::{ext_computation_cpu_callback, ext_computation_glsl_kernel};
use super::tokens::{
    EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS, EXT_COMPUTATION_INPUT_TOKENS,
    EXT_COMPUTATION_NAME_TOKENS, EXT_COMPUTATION_OUTPUT_TOKENS, EXT_COMPUTATION_TYPE_TOKENS,
};
use std::sync::Arc;
use usd_hd::data_source::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdRetainedContainerDataSource, HdRetainedTypedSampledDataSource, HdSampledDataSource,
    HdTypedSampledDataSource,
};
use usd_hd::schema::{
    HdExtComputationInputComputationSchemaBuilder, HdExtComputationOutputSchemaBuilder,
    HdExtComputationSchema, HdExtComputationSchemaBuilder, HdPrimvarsSchema,
    HdSizetDataSourceHandle,
};
use usd_hd::types::{HdTupleType, HdType};
use usd_sdf::Path;
use usd_tf::Token;

/// Input names for points aggregator (excludes restNormals, faceVertexIndices, hasFaceVaryingNormals).
fn ext_computation_input_names_for_points() -> Vec<Token> {
    vec![
        EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS.rest_points.clone(),
        EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS
            .geom_bind_xform
            .clone(),
        EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS
            .has_constant_influences
            .clone(),
        EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS
            .num_influences_per_component
            .clone(),
        EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS.influences.clone(),
        EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS
            .blend_shape_offsets
            .clone(),
        EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS
            .blend_shape_offset_ranges
            .clone(),
        EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS
            .num_blend_shape_offset_ranges
            .clone(),
    ]
}

/// Input names for normals aggregator (excludes restPoints, blendShape*).
fn ext_computation_input_names_for_normals() -> Vec<Token> {
    vec![
        EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS
            .geom_bind_xform
            .clone(),
        EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS.influences.clone(),
        EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS
            .num_influences_per_component
            .clone(),
        EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS
            .has_constant_influences
            .clone(),
        EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS.rest_normals.clone(),
        EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS
            .face_vertex_indices
            .clone(),
        EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS
            .has_face_varying_normals
            .clone(),
    ]
}

/// Input names for classic linear skinning (excludes skinningScaleXforms, skinningDualQuats).
fn ext_computation_input_names_for_classic_linear() -> Vec<Token> {
    vec![
        EXT_COMPUTATION_INPUT_TOKENS
            .common_space_to_prim_local
            .clone(),
        EXT_COMPUTATION_INPUT_TOKENS.blend_shape_weights.clone(),
        EXT_COMPUTATION_INPUT_TOKENS.skinning_xforms.clone(),
        EXT_COMPUTATION_INPUT_TOKENS
            .skel_local_to_common_space
            .clone(),
    ]
}

/// Aggregator input values container - delegates to resolved prim.
#[derive(Debug)]
struct ExtAggregatorComputationInputValuesDataSource {
    resolved_prim: Arc<DataSourceResolvedPointsBasedPrim>,
    computation_type: Token,
}

impl HdContainerDataSource for ExtAggregatorComputationInputValuesDataSource {
    fn get_names(&self) -> Vec<Token> {
        if self.computation_type == EXT_COMPUTATION_TYPE_TOKENS.points {
            ext_computation_input_names_for_points()
        } else {
            ext_computation_input_names_for_normals()
        }
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if *name == EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS.rest_points {
            self.resolved_prim.get_points()
        } else if *name == EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS.geom_bind_xform {
            Some(self.resolved_prim.get_geom_bind_transform())
        } else if *name == EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS.has_constant_influences {
            Some(self.resolved_prim.get_has_constant_influences())
        } else if *name == EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS.num_influences_per_component {
            Some(self.resolved_prim.get_num_influences_per_component())
        } else if *name == EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS.influences {
            Some(self.resolved_prim.get_influences())
        } else if *name == EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS.blend_shape_offsets {
            Some(self.resolved_prim.get_blend_shape_offsets())
        } else if *name == EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS.blend_shape_offset_ranges {
            Some(self.resolved_prim.get_blend_shape_offset_ranges())
        } else if *name == EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS.num_blend_shape_offset_ranges {
            Some(self.resolved_prim.get_num_blend_shape_offset_ranges())
        } else if *name == EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS.rest_normals {
            self.resolved_prim.get_normals()
        } else if *name == EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS.face_vertex_indices {
            self.resolved_prim.get_face_vertex_indices()
        } else if *name == EXT_AGGREGATOR_COMPUTATION_INPUT_TOKENS.has_face_varying_normals {
            self.resolved_prim.get_has_face_varying_normals()
        } else {
            None
        }
    }
}

impl HdDataSourceBase for ExtAggregatorComputationInputValuesDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            resolved_prim: Arc::clone(&self.resolved_prim),
            computation_type: self.computation_type.clone(),
        }) as HdDataSourceBaseHandle
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn sample_at_zero(&self) -> Option<usd_vt::Value> {
        None
    }

    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(Self {
            resolved_prim: Arc::clone(&self.resolved_prim),
            computation_type: self.computation_type.clone(),
        }))
    }
}

/// Skin computation input values container.
#[derive(Debug)]
struct ExtComputationInputValuesDataSource {
    resolved_prim: Arc<DataSourceResolvedPointsBasedPrim>,
}

fn is_dual_quat_skinning(method: &Token) -> bool {
    method == "dualQuaternion"
}

impl HdContainerDataSource for ExtComputationInputValuesDataSource {
    fn get_names(&self) -> Vec<Token> {
        if is_dual_quat_skinning(self.resolved_prim.get_skinning_method()) {
            vec![
                EXT_COMPUTATION_INPUT_TOKENS
                    .common_space_to_prim_local
                    .clone(),
                EXT_COMPUTATION_INPUT_TOKENS.blend_shape_weights.clone(),
                EXT_COMPUTATION_INPUT_TOKENS.skinning_xforms.clone(),
                EXT_COMPUTATION_INPUT_TOKENS.skinning_scale_xforms.clone(),
                EXT_COMPUTATION_INPUT_TOKENS.skinning_dual_quats.clone(),
                EXT_COMPUTATION_INPUT_TOKENS
                    .skel_local_to_common_space
                    .clone(),
            ]
        } else {
            ext_computation_input_names_for_classic_linear()
        }
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if *name == EXT_COMPUTATION_INPUT_TOKENS.common_space_to_prim_local {
            self.resolved_prim
                .get_common_space_to_prim_local()
                .map(|ds| ds as HdDataSourceBaseHandle)
        } else if *name == EXT_COMPUTATION_INPUT_TOKENS.blend_shape_weights {
            self.resolved_prim
                .get_blend_shape_weights()
                .map(|ds| ds as HdDataSourceBaseHandle)
        } else if *name == EXT_COMPUTATION_INPUT_TOKENS.skinning_xforms {
            self.resolved_prim
                .get_skinning_transforms()
                .map(|ds| ds as HdDataSourceBaseHandle)
        } else if *name == EXT_COMPUTATION_INPUT_TOKENS.skinning_scale_xforms {
            if is_dual_quat_skinning(self.resolved_prim.get_skinning_method()) {
                self.resolved_prim
                    .get_skinning_scale_transforms()
                    .map(|ds| ds as HdDataSourceBaseHandle)
            } else {
                None
            }
        } else if *name == EXT_COMPUTATION_INPUT_TOKENS.skinning_dual_quats {
            if is_dual_quat_skinning(self.resolved_prim.get_skinning_method()) {
                self.resolved_prim
                    .get_skinning_dual_quats()
                    .map(|ds| ds as HdDataSourceBaseHandle)
            } else {
                None
            }
        } else if *name == EXT_COMPUTATION_INPUT_TOKENS.skel_local_to_common_space {
            self.resolved_prim
                .get_resolved_skeleton_schema()
                .get_skel_local_to_common_space()
                .map(|ds| ds as HdDataSourceBaseHandle)
        } else {
            None
        }
    }
}

impl HdDataSourceBase for ExtComputationInputValuesDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            resolved_prim: Arc::clone(&self.resolved_prim),
        }) as HdDataSourceBaseHandle
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn sample_at_zero(&self) -> Option<usd_vt::Value> {
        None
    }

    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(Self {
            resolved_prim: Arc::clone(&self.resolved_prim),
        }))
    }
}

/// NumElements data source - array size of points/normals primvar.
#[derive(Debug)]
struct NumElementsDataSource {
    primvars: HdPrimvarsSchema,
    primvar_name: Token,
}

impl HdTypedSampledDataSource<usize> for NumElementsDataSource {
    fn get_typed_value(&self, _shutter_offset: f32) -> usize {
        let pv = self.primvars.get_primvar(&self.primvar_name);
        let Some(primvar_cont) = pv else {
            return 0;
        };
        let value_token = &*super::data_source_utils::PRIMVAR_VALUE;
        let Some(ds_handle) = primvar_cont.get(value_token) else {
            return 0;
        };
        // Use sample_at_zero for retained sources; fallback to 0 if unavailable
        let value = ds_handle.as_ref().sample_at_zero().unwrap_or_default();
        value.array_size()
    }
}

impl HdSampledDataSource for NumElementsDataSource {
    fn get_value(&self, shutter_offset: f32) -> usd_vt::Value {
        usd_vt::Value::from(self.get_typed_value(shutter_offset))
    }

    fn get_contributing_sample_times(
        &self,
        _start_time: f32,
        _end_time: f32,
        _out_sample_times: &mut Vec<f32>,
    ) -> bool {
        false
    }
}

impl HdDataSourceBase for NumElementsDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            primvars: self.primvars.clone(),
            primvar_name: self.primvar_name.clone(),
        }) as HdDataSourceBaseHandle
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }

    fn sample_at_zero(&self) -> Option<usd_vt::Value> {
        Some(usd_vt::Value::from(self.get_typed_value(0.0)))
    }
}

fn build_ext_aggregator_computation_prim_data_source(
    resolved_prim: Arc<DataSourceResolvedPointsBasedPrim>,
    computation_type: &Token,
) -> HdContainerDataSourceHandle {
    let input_values = Arc::new(ExtAggregatorComputationInputValuesDataSource {
        resolved_prim,
        computation_type: computation_type.clone(),
    }) as Arc<dyn HdContainerDataSource>;
    HdRetainedContainerDataSource::from_entries(&[(
        (**HdExtComputationSchema::get_schema_token()).clone(),
        HdExtComputationSchemaBuilder::new()
            .set_input_values(input_values)
            .build(),
    )])
}

fn build_ext_computation_input_computations(
    prim_path: &Path,
    computation_type: &Token,
) -> HdContainerDataSourceHandle {
    use usd_hd::schema::HdPathDataSourceHandle;

    let (names, computation_name) = if computation_type == EXT_COMPUTATION_TYPE_TOKENS.points {
        (
            ext_computation_input_names_for_points(),
            EXT_COMPUTATION_NAME_TOKENS
                .points_aggregator_computation
                .clone(),
        )
    } else {
        (
            ext_computation_input_names_for_normals(),
            EXT_COMPUTATION_NAME_TOKENS
                .normals_aggregator_computation
                .clone(),
        )
    };

    let Some(child_path) = prim_path.append_child(computation_name.as_str()) else {
        return HdRetainedContainerDataSource::new_empty();
    };
    let path_src = HdRetainedTypedSampledDataSource::new(child_path) as HdPathDataSourceHandle;

    let values: Vec<HdDataSourceBaseHandle> = names
        .iter()
        .map(|name| {
            let schema = HdExtComputationInputComputationSchemaBuilder::new()
                .set_source_computation(path_src.clone())
                .set_source_computation_output_name(HdRetainedTypedSampledDataSource::new(
                    name.clone(),
                ))
                .build();
            schema as HdDataSourceBaseHandle
        })
        .collect();

    let entries: Vec<(Token, HdDataSourceBaseHandle)> = names
        .iter()
        .zip(values.iter())
        .map(|(n, v)| (n.clone(), v.clone()))
        .collect();
    HdRetainedContainerDataSource::from_entries(&entries)
}

fn build_ext_computation_outputs(computation_type: &Token) -> HdContainerDataSourceHandle {
    let (output_name, _) = if computation_type == EXT_COMPUTATION_TYPE_TOKENS.points {
        (
            EXT_COMPUTATION_OUTPUT_TOKENS.skinned_points.clone(),
            EXT_COMPUTATION_TYPE_TOKENS.points.clone(),
        )
    } else {
        (
            EXT_COMPUTATION_OUTPUT_TOKENS.skinned_normals.clone(),
            EXT_COMPUTATION_TYPE_TOKENS.normals.clone(),
        )
    };

    let value_type = HdRetainedTypedSampledDataSource::new(HdTupleType::new(HdType::FloatVec3, 1));
    let output_schema = HdExtComputationOutputSchemaBuilder::new()
        .set_value_type(value_type)
        .build();

    HdRetainedContainerDataSource::from_entries(&[(output_name, output_schema)])
}

fn build_ext_computation_prim_data_source(
    resolved_prim: Arc<DataSourceResolvedPointsBasedPrim>,
    computation_type: &Token,
) -> HdContainerDataSourceHandle {
    let primvar_name = if computation_type == EXT_COMPUTATION_TYPE_TOKENS.points {
        Token::new("points")
    } else {
        Token::new("normals")
    };

    let element_count = Arc::new(NumElementsDataSource {
        primvars: resolved_prim.get_primvars().clone(),
        primvar_name: primvar_name.clone(),
    }) as HdSizetDataSourceHandle;

    let input_values = Arc::new(ExtComputationInputValuesDataSource {
        resolved_prim: Arc::clone(&resolved_prim),
    }) as Arc<dyn HdContainerDataSource>;

    let input_computations =
        build_ext_computation_input_computations(resolved_prim.get_prim_path(), computation_type);
    let outputs = build_ext_computation_outputs(computation_type);

    let glsl_kernel =
        ext_computation_glsl_kernel(resolved_prim.get_skinning_method(), computation_type);
    let cpu_callback = ext_computation_cpu_callback(resolved_prim.get_skinning_method());

    let schema = HdExtComputationSchemaBuilder::new()
        .set_input_values(input_values)
        .set_input_computations(input_computations)
        .set_outputs(outputs)
        .set_dispatch_count(element_count.clone())
        .set_element_count(element_count);

    let mut schema = schema;
    if let Some(gl) = glsl_kernel {
        schema = schema.set_glsl_kernel(gl);
    }
    if let Some(cb) = cpu_callback {
        schema = schema.set_cpu_callback(cb);
    }

    HdRetainedContainerDataSource::from_entries(&[(
        (**HdExtComputationSchema::get_schema_token()).clone(),
        schema.build(),
    )])
}

/// Create data source for ext computation prim of a skinned prim.
///
/// Used by points resolving scene index. Adds ext computations as children
/// of skinned prim with given computation_name.
///
/// Supported: pointsComputation, normalsComputation,
/// pointsAggregatorComputation, normalsAggregatorComputation.
pub fn data_source_resolved_ext_computation_prim(
    resolved_prim_source: Arc<DataSourceResolvedPointsBasedPrim>,
    computation_name: &Token,
) -> Option<HdContainerDataSourceHandle> {
    if *computation_name == EXT_COMPUTATION_NAME_TOKENS.points_computation {
        Some(build_ext_computation_prim_data_source(
            resolved_prim_source,
            &EXT_COMPUTATION_TYPE_TOKENS.points,
        ))
    } else if *computation_name == EXT_COMPUTATION_NAME_TOKENS.normals_computation {
        Some(build_ext_computation_prim_data_source(
            resolved_prim_source,
            &EXT_COMPUTATION_TYPE_TOKENS.normals,
        ))
    } else if *computation_name == EXT_COMPUTATION_NAME_TOKENS.points_aggregator_computation {
        Some(build_ext_aggregator_computation_prim_data_source(
            resolved_prim_source,
            &EXT_COMPUTATION_TYPE_TOKENS.points,
        ))
    } else if *computation_name == EXT_COMPUTATION_NAME_TOKENS.normals_aggregator_computation {
        Some(build_ext_aggregator_computation_prim_data_source(
            resolved_prim_source,
            &EXT_COMPUTATION_TYPE_TOKENS.normals,
        ))
    } else {
        None
    }
}
