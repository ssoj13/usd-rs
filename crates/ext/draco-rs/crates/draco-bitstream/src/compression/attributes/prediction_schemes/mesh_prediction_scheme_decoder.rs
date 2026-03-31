//! Base class for mesh prediction scheme decoders.
//! Reference: `_ref/draco/src/draco/compression/attributes/prediction_schemes/mesh_prediction_scheme_decoder.h`.
//!
//! Wraps PredictionSchemeDecoder and stores mesh connectivity data required by
//! mesh prediction schemes.

use crate::compression::attributes::prediction_schemes::prediction_scheme_decoder::PredictionSchemeDecoder;
use crate::compression::attributes::prediction_schemes::prediction_scheme_decoding_transform::DecodingTransform;
use draco_core::attributes::point_attribute::PointAttribute;

pub struct MeshPredictionSchemeDecoder<DataTypeT, TransformT, MeshDataT>
where
    TransformT: DecodingTransform<DataTypeT>,
{
    base: PredictionSchemeDecoder<DataTypeT, TransformT>,
    mesh_data: MeshDataT,
}

impl<DataTypeT, TransformT, MeshDataT> MeshPredictionSchemeDecoder<DataTypeT, TransformT, MeshDataT>
where
    TransformT: DecodingTransform<DataTypeT>,
{
    pub fn new(attribute: &PointAttribute, transform: TransformT, mesh_data: MeshDataT) -> Self {
        Self {
            base: PredictionSchemeDecoder::new(attribute, transform),
            mesh_data,
        }
    }

    pub fn base(&self) -> &PredictionSchemeDecoder<DataTypeT, TransformT> {
        &self.base
    }

    pub fn base_mut(&mut self) -> &mut PredictionSchemeDecoder<DataTypeT, TransformT> {
        &mut self.base
    }

    pub fn mesh_data(&self) -> &MeshDataT {
        &self.mesh_data
    }

    pub fn mesh_data_mut(&mut self) -> &mut MeshDataT {
        &mut self.mesh_data
    }
}
