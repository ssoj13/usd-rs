//! Draco bitstream encoder (basic API).
//! Reference: `_ref/draco/src/draco/compression/encode.h|cc`.
//!
//! Provides the simple Encoder wrapper that maps type-based options to
//! attribute-id options used by ExpertEncoder.

use crate::compression::config::encoder_options::{EncoderOptions, EncoderOptionsBase};
use crate::compression::encode_base::EncoderBase;
use crate::compression::expert_encode::ExpertEncoder;
use draco_core::attributes::geometry_attribute::GeometryAttributeType;
use draco_core::core::encoder_buffer::EncoderBuffer;
use draco_core::core::status::{ok_status, Status};
use draco_core::mesh::mesh::Mesh;
use draco_core::point_cloud::point_cloud::PointCloud;

pub struct Encoder {
    base: EncoderBase<EncoderOptionsBase<GeometryAttributeType>>,
}

impl Encoder {
    pub fn new() -> Self {
        Self {
            base: EncoderBase::new(),
        }
    }

    pub fn encode_point_cloud_to_buffer(
        &mut self,
        pc: &PointCloud,
        out_buffer: &mut EncoderBuffer,
    ) -> Status {
        let mut encoder = ExpertEncoder::new_point_cloud(pc);
        encoder.reset(self.create_expert_encoder_options(pc));
        let status = encoder.encode_to_buffer(out_buffer);
        if !status.is_ok() {
            return status;
        }
        self.base
            .set_num_encoded_points(encoder.num_encoded_points());
        self.base.set_num_encoded_faces(0);
        ok_status()
    }

    pub fn encode_mesh_to_buffer(&mut self, mesh: &Mesh, out_buffer: &mut EncoderBuffer) -> Status {
        let mut encoder = ExpertEncoder::new_mesh(mesh);
        encoder.reset(self.create_expert_encoder_options(mesh));
        let status = encoder.encode_to_buffer(out_buffer);
        if !status.is_ok() {
            return status;
        }
        self.base
            .set_num_encoded_points(encoder.num_encoded_points());
        self.base.set_num_encoded_faces(encoder.num_encoded_faces());
        ok_status()
    }

    pub fn reset(&mut self, options: &EncoderOptionsBase<GeometryAttributeType>) {
        self.base.reset(options);
    }

    pub fn reset_default(&mut self) {
        self.base.reset_default();
    }

    pub fn set_speed_options(&mut self, encoding_speed: i32, decoding_speed: i32) {
        self.base.set_speed_options(encoding_speed, decoding_speed);
    }

    pub fn set_attribute_quantization(
        &mut self,
        att_type: GeometryAttributeType,
        quantization_bits: i32,
    ) {
        self.base.options_mut().set_attribute_int(
            &att_type,
            "quantization_bits",
            quantization_bits,
        );
    }

    pub fn set_attribute_explicit_quantization(
        &mut self,
        att_type: GeometryAttributeType,
        quantization_bits: i32,
        num_dims: i32,
        origin: &[f32],
        range: f32,
    ) {
        self.base.options_mut().set_attribute_int(
            &att_type,
            "quantization_bits",
            quantization_bits,
        );
        self.base.options_mut().set_attribute_vector(
            &att_type,
            "quantization_origin",
            num_dims,
            origin,
        );
        self.base
            .options_mut()
            .set_attribute_float(&att_type, "quantization_range", range);
    }

    pub fn set_attribute_prediction_scheme(
        &mut self,
        att_type: GeometryAttributeType,
        prediction_scheme_method: i32,
    ) -> Status {
        let status = self
            .base
            .check_prediction_scheme(att_type, prediction_scheme_method);
        if !status.is_ok() {
            return status;
        }
        self.base.options_mut().set_attribute_int(
            &att_type,
            "prediction_scheme",
            prediction_scheme_method,
        );
        status
    }

    pub fn set_encoding_method(&mut self, encoding_method: i32) {
        self.base.set_encoding_method(encoding_method);
    }

    pub fn set_track_encoded_properties(&mut self, flag: bool) {
        self.base.set_track_encoded_properties(flag);
    }

    pub fn options(&self) -> &EncoderOptionsBase<GeometryAttributeType> {
        self.base.options()
    }

    pub fn options_mut(&mut self) -> &mut EncoderOptionsBase<GeometryAttributeType> {
        self.base.options_mut()
    }

    pub fn num_encoded_points(&self) -> usize {
        self.base.num_encoded_points()
    }

    pub fn num_encoded_faces(&self) -> usize {
        self.base.num_encoded_faces()
    }

    pub fn create_expert_encoder_options(&self, pc: &PointCloud) -> EncoderOptions {
        let mut ret_options = EncoderOptions::create_empty_options();
        ret_options.set_global_options(self.base.options().global_options().clone());
        ret_options.set_feature_options(self.base.options().feature_options().clone());

        for i in 0..pc.num_attributes() {
            let att = match pc.attribute(i) {
                Some(att) => att,
                None => continue,
            };
            if let Some(att_options) = self
                .base
                .options()
                .find_attribute_options(&att.attribute_type())
            {
                ret_options.set_attribute_options(&i, att_options.clone());
            }
        }
        ret_options
    }
}

impl Default for Encoder {
    fn default() -> Self {
        Self::new()
    }
}
