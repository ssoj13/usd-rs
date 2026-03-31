//! Prediction scheme decoder factory.
//! Reference: `_ref/draco/src/draco/compression/attributes/prediction_schemes/prediction_scheme_decoder_factory.h`.

use num_traits::{NumCast, ToPrimitive};
use std::ops::Div;

use draco_core::core::math_utils::AddAsUnsigned;
use draco_core::mesh::corner_table_iterators::CornerTableTraversal;

use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_constrained_multi_parallelogram_decoder::
    MeshPredictionSchemeConstrainedMultiParallelogramDecoder;
use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_corner_table::
    MeshPredictionCornerTable;
use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_data::
    MeshPredictionSchemeData;
use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_geometric_normal_decoder::
    MeshPredictionSchemeGeometricNormalDecoder;
use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_multi_parallelogram_decoder::
    MeshPredictionSchemeMultiParallelogramDecoder;
use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_parallelogram_decoder::
    MeshPredictionSchemeParallelogramDecoder;
use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_tex_coords_decoder::
    MeshPredictionSchemeTexCoordsDecoder;
use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_tex_coords_portable_decoder::
    MeshPredictionSchemeTexCoordsPortableDecoder;
use crate::compression::attributes::prediction_schemes::prediction_scheme_decoder_interface::
    PredictionSchemeTypedDecoderInterface;
use crate::compression::attributes::prediction_schemes::prediction_scheme_decoding_transform::
    DecodingTransform;
use crate::compression::attributes::prediction_schemes::prediction_scheme_delta_decoder::
    PredictionSchemeDeltaDecoder;
use crate::compression::config::compression_shared::{
    EncodedGeometryType, PredictionSchemeMethod, PredictionSchemeTransformType,
};
use crate::compression::mesh::MeshDecoder;
use crate::compression::point_cloud::PointCloudDecoder;
use draco_core::attributes::point_attribute::PointAttribute;
use draco_core::mesh::corner_table::CornerTable;
use draco_core::mesh::mesh_attribute_corner_table::MeshAttributeCornerTable;

fn create_mesh_prediction_scheme<DataTypeT, TransformT, MeshDataT>(
    method: PredictionSchemeMethod,
    attribute: &PointAttribute,
    transform: TransformT,
    mesh_data: MeshDataT,
    bitstream_version: u16,
) -> Option<Box<dyn PredictionSchemeTypedDecoderInterface<DataTypeT, TransformT::CorrType>>>
where
    DataTypeT: Copy + Default + NumCast + ToPrimitive + AddAsUnsigned + Div<Output = DataTypeT> + 'static,
    TransformT: DecodingTransform<DataTypeT> + Clone + 'static,
    TransformT::CorrType: Copy + 'static,
    MeshDataT: crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_data::
        MeshPredictionSchemeDataRef
        + Copy
        + 'static,
    MeshDataT::CornerTable: MeshPredictionCornerTable + CornerTableTraversal + 'static,
{
    let transform_type = transform.get_type();
    if transform_type == PredictionSchemeTransformType::PredictionTransformNormalOctahedron
        || transform_type
            == PredictionSchemeTransformType::PredictionTransformNormalOctahedronCanonicalized
    {
        if method == PredictionSchemeMethod::MeshPredictionGeometricNormal {
            return Some(Box::new(MeshPredictionSchemeGeometricNormalDecoder::new(
                attribute, transform, mesh_data,
            )));
        }
        return None;
    }

    match method {
        PredictionSchemeMethod::MeshPredictionParallelogram => Some(Box::new(
            MeshPredictionSchemeParallelogramDecoder::new(attribute, transform, mesh_data),
        )),
        PredictionSchemeMethod::MeshPredictionMultiParallelogram => Some(Box::new(
            MeshPredictionSchemeMultiParallelogramDecoder::new(attribute, transform, mesh_data),
        )),
        PredictionSchemeMethod::MeshPredictionConstrainedMultiParallelogram => Some(Box::new(
            MeshPredictionSchemeConstrainedMultiParallelogramDecoder::new(
                attribute, transform, mesh_data,
            ),
        )),
        PredictionSchemeMethod::MeshPredictionTexCoordsDeprecated => {
            Some(Box::new(MeshPredictionSchemeTexCoordsDecoder::new(
                attribute,
                transform,
                mesh_data,
                bitstream_version,
            )))
        }
        PredictionSchemeMethod::MeshPredictionTexCoordsPortable => Some(Box::new(
            MeshPredictionSchemeTexCoordsPortableDecoder::new(attribute, transform, mesh_data),
        )),
        _ => None,
    }
}

fn create_mesh_prediction_scheme_from_decoder<DataTypeT, TransformT>(
    method: PredictionSchemeMethod,
    att_id: i32,
    decoder: &dyn MeshDecoder,
    transform: TransformT,
    bitstream_version: u16,
) -> Option<Box<dyn PredictionSchemeTypedDecoderInterface<DataTypeT, TransformT::CorrType>>>
where
    DataTypeT:
        Copy + Default + NumCast + ToPrimitive + AddAsUnsigned + Div<Output = DataTypeT> + 'static,
    TransformT: DecodingTransform<DataTypeT> + Clone + 'static,
    TransformT::CorrType: Copy + 'static,
{
    if !matches!(
        method,
        PredictionSchemeMethod::MeshPredictionParallelogram
            | PredictionSchemeMethod::MeshPredictionMultiParallelogram
            | PredictionSchemeMethod::MeshPredictionConstrainedMultiParallelogram
            | PredictionSchemeMethod::MeshPredictionTexCoordsDeprecated
            | PredictionSchemeMethod::MeshPredictionTexCoordsPortable
            | PredictionSchemeMethod::MeshPredictionGeometricNormal
    ) {
        return None;
    }

    let att = decoder.point_cloud()?.attribute(att_id)?;
    let mesh = decoder.mesh()?;
    let corner_table = decoder.get_corner_table()?;
    let encoding_data = decoder.get_attribute_encoding_data(att_id)?;

    if let Some(att_corner_table) = decoder.get_attribute_corner_table(att_id) {
        // SAFETY: att_corner_table lives in the decoder's attribute_data; the prediction scheme
        // is stored in the decoder's attributes_decoders. Both are owned by the decoder, so the
        // corner table outlives the prediction scheme. We transmute to 'static to satisfy
        // create_mesh_prediction_scheme's MeshDataT::CornerTable: 'static bound; the actual
        // lifetime is the decoder's, which outlives all decode use.
        let att_corner_table_static: &MeshAttributeCornerTable<'static> =
            unsafe { std::mem::transmute(att_corner_table) };
        let mut mesh_data = MeshPredictionSchemeData::<MeshAttributeCornerTable<'static>>::new();
        mesh_data.set(
            mesh,
            att_corner_table_static,
            &encoding_data.encoded_attribute_value_index_to_corner_map,
            &encoding_data.vertex_to_encoded_attribute_value_index_map,
        );
        return create_mesh_prediction_scheme(method, att, transform, mesh_data, bitstream_version);
    }

    let mut mesh_data = MeshPredictionSchemeData::<CornerTable>::new();
    mesh_data.set(
        mesh,
        corner_table,
        &encoding_data.encoded_attribute_value_index_to_corner_map,
        &encoding_data.vertex_to_encoded_attribute_value_index_map,
    );
    create_mesh_prediction_scheme(method, att, transform, mesh_data, bitstream_version)
}

pub fn create_prediction_scheme_for_decoder<DataTypeT, TransformT>(
    method: PredictionSchemeMethod,
    att_id: i32,
    decoder: &dyn PointCloudDecoder,
    transform: TransformT,
) -> Option<Box<dyn PredictionSchemeTypedDecoderInterface<DataTypeT, TransformT::CorrType>>>
where
    DataTypeT:
        Copy + Default + NumCast + ToPrimitive + AddAsUnsigned + Div<Output = DataTypeT> + 'static,
    TransformT: DecodingTransform<DataTypeT> + Clone + 'static,
    TransformT::CorrType: Copy + 'static,
{
    if method == PredictionSchemeMethod::PredictionNone {
        return None;
    }
    let att = decoder.point_cloud()?.attribute(att_id)?;

    if decoder.get_geometry_type() == EncodedGeometryType::TriangularMesh {
        if let Some(mesh_decoder) = decoder.as_mesh_decoder() {
            if let Some(mesh_scheme) = create_mesh_prediction_scheme_from_decoder(
                method,
                att_id,
                mesh_decoder,
                transform.clone(),
                decoder.bitstream_version(),
            ) {
                return Some(mesh_scheme);
            }
        }
    }

    Some(Box::new(PredictionSchemeDeltaDecoder::new(att, transform)))
}

pub fn create_prediction_scheme_for_decoder_default<DataTypeT, TransformT>(
    method: PredictionSchemeMethod,
    att_id: i32,
    decoder: &dyn PointCloudDecoder,
) -> Option<Box<dyn PredictionSchemeTypedDecoderInterface<DataTypeT, TransformT::CorrType>>>
where
    DataTypeT:
        Copy + Default + NumCast + ToPrimitive + AddAsUnsigned + Div<Output = DataTypeT> + 'static,
    TransformT: DecodingTransform<DataTypeT> + Default + Clone + 'static,
    TransformT::CorrType: Copy + 'static,
{
    create_prediction_scheme_for_decoder(method, att_id, decoder, TransformT::default())
}
