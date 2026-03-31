//! Encoder base utilities.
//! Reference: `_ref/draco/src/draco/compression/encode_base.h`.
//!
//! Provides shared encoder options and validation for prediction schemes.

use crate::compression::config::compression_shared::PredictionSchemeMethod;
use crate::compression::config::draco_options::DracoOptions;
use crate::compression::config::encoder_options::EncoderOptionsBase;
use draco_core::attributes::geometry_attribute::GeometryAttributeType;
use draco_core::core::status::{ok_status, Status, StatusCode};

pub trait EncoderOptionsExt: Clone {
    fn create_default_options() -> Self;
    fn set_speed(&mut self, encoding_speed: i32, decoding_speed: i32);
    fn set_global_int(&mut self, name: &str, val: i32);
    fn set_global_bool(&mut self, name: &str, val: bool);
}

impl<AttributeKeyT> EncoderOptionsExt for EncoderOptionsBase<AttributeKeyT>
where
    AttributeKeyT: Ord + Clone,
{
    fn create_default_options() -> Self {
        EncoderOptionsBase::create_default_options()
    }

    fn set_speed(&mut self, encoding_speed: i32, decoding_speed: i32) {
        EncoderOptionsBase::set_speed(self, encoding_speed, decoding_speed)
    }

    fn set_global_int(&mut self, name: &str, val: i32) {
        DracoOptions::set_global_int(&mut *self, name, val);
    }

    fn set_global_bool(&mut self, name: &str, val: bool) {
        DracoOptions::set_global_bool(&mut *self, name, val);
    }
}

pub struct EncoderBase<EncoderOptionsT: EncoderOptionsExt> {
    options: EncoderOptionsT,
    num_encoded_points: usize,
    num_encoded_faces: usize,
}

impl<EncoderOptionsT: EncoderOptionsExt> EncoderBase<EncoderOptionsT> {
    pub fn new() -> Self {
        Self {
            options: EncoderOptionsT::create_default_options(),
            num_encoded_points: 0,
            num_encoded_faces: 0,
        }
    }

    pub fn options(&self) -> &EncoderOptionsT {
        &self.options
    }

    pub fn options_mut(&mut self) -> &mut EncoderOptionsT {
        &mut self.options
    }

    pub fn set_track_encoded_properties(&mut self, flag: bool) {
        self.options
            .set_global_bool("store_number_of_encoded_points", flag);
        self.options
            .set_global_bool("store_number_of_encoded_faces", flag);
    }

    pub fn num_encoded_points(&self) -> usize {
        self.num_encoded_points
    }

    pub fn num_encoded_faces(&self) -> usize {
        self.num_encoded_faces
    }

    pub fn reset(&mut self, options: &EncoderOptionsT) {
        self.options = options.clone();
    }

    pub fn reset_default(&mut self) {
        self.options = EncoderOptionsT::create_default_options();
    }

    pub fn set_speed_options(&mut self, encoding_speed: i32, decoding_speed: i32) {
        self.options.set_speed(encoding_speed, decoding_speed);
    }

    pub fn set_encoding_method(&mut self, encoding_method: i32) {
        self.options
            .set_global_int("encoding_method", encoding_method);
    }

    pub fn set_encoding_submethod(&mut self, encoding_submethod: i32) {
        self.options
            .set_global_int("encoding_submethod", encoding_submethod);
    }

    pub fn check_prediction_scheme(
        &self,
        att_type: GeometryAttributeType,
        prediction_scheme: i32,
    ) -> Status {
        if prediction_scheme < PredictionSchemeMethod::PredictionNone as i32 {
            return Status::new(
                StatusCode::DracoError,
                "Invalid prediction scheme requested.",
            );
        }
        if prediction_scheme >= PredictionSchemeMethod::NumPredictionSchemes as i32 {
            return Status::new(
                StatusCode::DracoError,
                "Invalid prediction scheme requested.",
            );
        }
        if prediction_scheme == PredictionSchemeMethod::MeshPredictionTexCoordsDeprecated as i32 {
            return Status::new(
                StatusCode::DracoError,
                "MESH_PREDICTION_TEX_COORDS_DEPRECATED is deprecated.",
            );
        }
        if prediction_scheme == PredictionSchemeMethod::MeshPredictionMultiParallelogram as i32 {
            return Status::new(
                StatusCode::DracoError,
                "MESH_PREDICTION_MULTI_PARALLELOGRAM is deprecated.",
            );
        }
        if prediction_scheme == PredictionSchemeMethod::MeshPredictionTexCoordsPortable as i32 {
            if att_type != GeometryAttributeType::TexCoord {
                return Status::new(
                    StatusCode::DracoError,
                    "Invalid prediction scheme for attribute type.",
                );
            }
        }
        if prediction_scheme == PredictionSchemeMethod::MeshPredictionGeometricNormal as i32 {
            if att_type != GeometryAttributeType::Normal {
                return Status::new(
                    StatusCode::DracoError,
                    "Invalid prediction scheme for attribute type.",
                );
            }
        }
        if att_type == GeometryAttributeType::Normal {
            if prediction_scheme != PredictionSchemeMethod::PredictionDifference as i32
                && prediction_scheme != PredictionSchemeMethod::MeshPredictionGeometricNormal as i32
            {
                return Status::new(
                    StatusCode::DracoError,
                    "Invalid prediction scheme for attribute type.",
                );
            }
        }
        ok_status()
    }

    pub fn set_num_encoded_points(&mut self, num: usize) {
        self.num_encoded_points = num;
    }

    pub fn set_num_encoded_faces(&mut self, num: usize) {
        self.num_encoded_faces = num;
    }
}

impl<EncoderOptionsT: EncoderOptionsExt> Default for EncoderBase<EncoderOptionsT> {
    fn default() -> Self {
        Self::new()
    }
}
