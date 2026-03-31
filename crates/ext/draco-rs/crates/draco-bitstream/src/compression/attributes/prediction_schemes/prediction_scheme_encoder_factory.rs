//! Prediction scheme encoder factory.
//! Reference: `_ref/draco/src/draco/compression/attributes/prediction_schemes/prediction_scheme_encoder_factory.{h,cc}`.

use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_parallelogram_encoder::MeshPredictionSchemeParallelogramEncoder;
use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_tex_coords_portable_encoder::MeshPredictionSchemeTexCoordsPortableEncoder;
use crate::compression::attributes::prediction_schemes::prediction_scheme_delta_encoder::PredictionSchemeDeltaEncoder;
use crate::compression::attributes::prediction_schemes::prediction_scheme_encoder_interface::PredictionSchemeTypedEncoderInterface;
use crate::compression::attributes::prediction_schemes::prediction_scheme_encoding_transform::EncodingTransform;
use crate::compression::config::compression_shared::PredictionSchemeMethod;
use crate::compression::config::encoder_options::EncoderOptions;
use crate::compression::point_cloud::PointCloudEncoder;
use draco_core::attributes::geometry_attribute::GeometryAttributeType;
use draco_core::core::draco_types::is_data_type_integral;

pub fn select_prediction_method(
    att_id: i32,
    encoder: &dyn PointCloudEncoder,
) -> PredictionSchemeMethod {
    select_prediction_method_with_options(att_id, encoder.options(), encoder)
}

pub fn select_prediction_method_with_options(
    att_id: i32,
    options: &EncoderOptions,
    encoder: &dyn PointCloudEncoder,
) -> PredictionSchemeMethod {
    if options.get_speed() >= 10 {
        return PredictionSchemeMethod::PredictionDifference;
    }
    if encoder.get_geometry_type()
        == crate::compression::config::compression_shared::EncodedGeometryType::TriangularMesh
    {
        let att_quant = options.get_attribute_int(&att_id, "quantization_bits", -1);
        let att = encoder.point_cloud().and_then(|pc| pc.attribute(att_id));
        if let Some(att) = att {
            if att_quant != -1
                && att.attribute_type() == GeometryAttributeType::TexCoord
                && att.num_components() == 2
            {
                let pos_att = encoder
                    .point_cloud()
                    .and_then(|pc| pc.get_named_attribute(GeometryAttributeType::Position));
                let mut is_pos_att_valid = false;
                if let Some(pos_att) = pos_att {
                    if is_data_type_integral(pos_att.data_type()) {
                        is_pos_att_valid = true;
                    } else {
                        let pos_att_id = encoder
                            .point_cloud()
                            .map(|pc| pc.get_named_attribute_id(GeometryAttributeType::Position))
                            .unwrap_or(-1);
                        let pos_quant =
                            options.get_attribute_int(&pos_att_id, "quantization_bits", -1);
                        if pos_quant > 0 && pos_quant <= 21 && 2 * pos_quant + att_quant < 64 {
                            is_pos_att_valid = true;
                        }
                    }
                }
                if is_pos_att_valid && options.get_speed() < 4 {
                    return PredictionSchemeMethod::MeshPredictionTexCoordsPortable;
                }
            }
            if att.attribute_type() == GeometryAttributeType::Normal {
                if options.get_speed() < 4 {
                    let pos_att_id = encoder
                        .point_cloud()
                        .map(|pc| pc.get_named_attribute_id(GeometryAttributeType::Position))
                        .unwrap_or(-1);
                    let pos_att = encoder
                        .point_cloud()
                        .and_then(|pc| pc.get_named_attribute(GeometryAttributeType::Position));
                    if let Some(pos_att) = pos_att {
                        if is_data_type_integral(pos_att.data_type())
                            || options.get_attribute_int(&pos_att_id, "quantization_bits", -1) > 0
                        {
                            return PredictionSchemeMethod::MeshPredictionGeometricNormal;
                        }
                    }
                }
                return PredictionSchemeMethod::PredictionDifference;
            }
            if options.get_speed() >= 8 {
                return PredictionSchemeMethod::PredictionDifference;
            }
            if options.get_speed() >= 2
                || encoder.point_cloud().map(|pc| pc.num_points()).unwrap_or(0) < 40
            {
                return PredictionSchemeMethod::MeshPredictionParallelogram;
            }
            return PredictionSchemeMethod::MeshPredictionConstrainedMultiParallelogram;
        }
    }
    PredictionSchemeMethod::PredictionDifference
}

pub fn get_prediction_method_from_options(
    att_id: i32,
    options: &EncoderOptions,
) -> PredictionSchemeMethod {
    let pred_type = options.get_attribute_int(&att_id, "prediction_scheme", -1);
    if pred_type == -1 {
        return PredictionSchemeMethod::PredictionUndefined;
    }
    if pred_type < PredictionSchemeMethod::PredictionNone as i32
        || pred_type >= PredictionSchemeMethod::NumPredictionSchemes as i32
    {
        return PredictionSchemeMethod::PredictionNone;
    }
    match pred_type {
        -2 => PredictionSchemeMethod::PredictionNone,
        0 => PredictionSchemeMethod::PredictionDifference,
        1 => PredictionSchemeMethod::MeshPredictionParallelogram,
        2 => PredictionSchemeMethod::MeshPredictionMultiParallelogram,
        3 => PredictionSchemeMethod::MeshPredictionTexCoordsDeprecated,
        4 => PredictionSchemeMethod::MeshPredictionConstrainedMultiParallelogram,
        5 => PredictionSchemeMethod::MeshPredictionTexCoordsPortable,
        6 => PredictionSchemeMethod::MeshPredictionGeometricNormal,
        _ => PredictionSchemeMethod::PredictionNone,
    }
}

pub fn create_prediction_scheme_for_encoder<DataTypeT, TransformT>(
    method: PredictionSchemeMethod,
    att_id: i32,
    encoder: &dyn PointCloudEncoder,
    transform: TransformT,
) -> Option<Box<dyn PredictionSchemeTypedEncoderInterface<DataTypeT, TransformT::CorrType>>>
where
    DataTypeT: Copy + Default + num_traits::NumCast + 'static,
    TransformT: EncodingTransform<DataTypeT> + Clone + 'static,
    TransformT::CorrType: Copy + Default + 'static,
{
    let mut method = method;
    if method == PredictionSchemeMethod::PredictionUndefined {
        method = select_prediction_method(att_id, encoder);
    }
    if method == PredictionSchemeMethod::PredictionNone {
        return None;
    }
    let att = encoder.point_cloud()?.attribute(att_id)?;
    if method == PredictionSchemeMethod::MeshPredictionParallelogram {
        if let Some(mesh_data) = encoder.mesh_prediction_scheme_data(att_id) {
            if mesh_data.is_initialized() {
                let scheme =
                    MeshPredictionSchemeParallelogramEncoder::new(att, transform, mesh_data);
                return Some(Box::new(scheme));
            }
        }
    }
    if method == PredictionSchemeMethod::MeshPredictionTexCoordsPortable {
        if let Some(mesh_data) = encoder.mesh_prediction_scheme_data(att_id) {
            if mesh_data.is_initialized()
                && att.num_components() == 2
                && encoder
                    .point_cloud()
                    .map(|pc| pc.get_named_attribute(GeometryAttributeType::Position))
                    .is_some()
            {
                let scheme =
                    MeshPredictionSchemeTexCoordsPortableEncoder::new(att, transform, mesh_data);
                return Some(Box::new(scheme));
            }
        }
    }
    if method != PredictionSchemeMethod::PredictionDifference {
        return None;
    }
    Some(Box::new(PredictionSchemeDeltaEncoder::new(att, transform)))
}

pub fn create_prediction_scheme_for_encoder_default<DataTypeT, TransformT>(
    method: PredictionSchemeMethod,
    att_id: i32,
    encoder: &dyn PointCloudEncoder,
) -> Option<Box<dyn PredictionSchemeTypedEncoderInterface<DataTypeT, TransformT::CorrType>>>
where
    DataTypeT: Copy + Default + num_traits::NumCast + 'static,
    TransformT: EncodingTransform<DataTypeT> + Clone + Default + 'static,
    TransformT::CorrType: Copy + Default + 'static,
{
    create_prediction_scheme_for_encoder(method, att_id, encoder, TransformT::default())
}
