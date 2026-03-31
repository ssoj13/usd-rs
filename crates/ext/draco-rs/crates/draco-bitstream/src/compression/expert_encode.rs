//! Draco bitstream expert encoder.
//! Reference: `_ref/draco/src/draco/compression/expert_encode.h|cc`.
//!
//! Provides attribute-id-based options and encoding controls.

use crate::compression::config::compression_shared::{MeshEncoderMethod, PointCloudEncodingMethod};
use crate::compression::config::encoder_options::EncoderOptions;
use crate::compression::encode_base::EncoderBase;
use crate::compression::mesh::edgebreaker_shared::EdgebreakerTopologyBitPattern;
use crate::compression::mesh::{MeshEdgebreakerEncoder, MeshEncoder, MeshSequentialEncoder};
use crate::compression::point_cloud::{
    PointCloudEncoder, PointCloudKdTreeEncoder, PointCloudSequentialEncoder,
};
use draco_core::attributes::geometry_attribute::GeometryAttributeType;
use draco_core::core::bit_utils::most_significant_bit;
use draco_core::core::encoder_buffer::EncoderBuffer;
use draco_core::core::status::{ok_status, Status, StatusCode};
use draco_core::core::vector_d::Vector3f;
use draco_core::mesh::mesh::Mesh;
use draco_core::point_cloud::point_cloud::PointCloud;

pub struct ExpertEncoder<'a> {
    base: EncoderBase<EncoderOptions>,
    point_cloud: Option<&'a PointCloud>,
    mesh: Option<&'a Mesh>,
}

enum PointCloudEncoderKind {
    Sequential(PointCloudSequentialEncoder),
    KdTree(PointCloudKdTreeEncoder),
}

impl PointCloudEncoderKind {
    fn set_point_cloud(&mut self, pc: &PointCloud) {
        match self {
            Self::Sequential(enc) => enc.set_point_cloud(pc),
            Self::KdTree(enc) => enc.set_point_cloud(pc),
        }
    }

    fn encode(&mut self, options: &EncoderOptions, out_buffer: &mut EncoderBuffer) -> Status {
        match self {
            Self::Sequential(enc) => enc.encode(options, out_buffer),
            Self::KdTree(enc) => enc.encode(options, out_buffer),
        }
    }

    fn num_encoded_points(&self) -> usize {
        match self {
            Self::Sequential(enc) => enc.num_encoded_points(),
            Self::KdTree(enc) => enc.num_encoded_points(),
        }
    }
}

enum MeshEncoderKind {
    Sequential(MeshSequentialEncoder),
    Edgebreaker(MeshEdgebreakerEncoder),
}

impl MeshEncoderKind {
    fn set_mesh(&mut self, mesh: &Mesh) {
        match self {
            Self::Sequential(enc) => enc.set_mesh(mesh),
            Self::Edgebreaker(enc) => enc.set_mesh(mesh),
        }
    }

    fn encode(&mut self, options: &EncoderOptions, out_buffer: &mut EncoderBuffer) -> Status {
        match self {
            Self::Sequential(enc) => enc.encode(options, out_buffer),
            Self::Edgebreaker(enc) => enc.encode(options, out_buffer),
        }
    }

    fn num_encoded_points(&self) -> usize {
        match self {
            Self::Sequential(enc) => enc.num_encoded_points(),
            Self::Edgebreaker(enc) => enc.num_encoded_points(),
        }
    }

    fn num_encoded_faces(&self) -> usize {
        match self {
            Self::Sequential(enc) => enc.num_encoded_faces(),
            Self::Edgebreaker(enc) => enc.num_encoded_faces(),
        }
    }
}

impl<'a> ExpertEncoder<'a> {
    pub fn new_point_cloud(point_cloud: &'a PointCloud) -> Self {
        Self {
            base: EncoderBase::new(),
            point_cloud: Some(point_cloud),
            mesh: None,
        }
    }

    pub fn new_mesh(mesh: &'a Mesh) -> Self {
        Self {
            base: EncoderBase::new(),
            point_cloud: Some(mesh),
            mesh: Some(mesh),
        }
    }

    pub fn encode_to_buffer(&mut self, out_buffer: &mut EncoderBuffer) -> Status {
        self.encode_to_buffer_with_symbols(out_buffer, None)
    }

    /// Same as encode_to_buffer; when out_symbols is Some, fills it with the traversal symbol
    /// sequence (Standard EdgeBreaker only; for parity debugging).
    pub fn encode_to_buffer_with_symbols(
        &mut self,
        out_buffer: &mut EncoderBuffer,
        out_symbols: Option<&mut Vec<EdgebreakerTopologyBitPattern>>,
    ) -> Status {
        let pc = match self.point_cloud {
            Some(pc) => pc,
            None => {
                return Status::new(StatusCode::DracoError, "Invalid input geometry.");
            }
        };
        if self.mesh.is_none() {
            return self.encode_point_cloud_to_buffer(pc, out_buffer);
        }
        let mesh = self.mesh.unwrap();
        self.encode_mesh_to_buffer(mesh, out_buffer, out_symbols)
    }

    pub fn reset(&mut self, options: EncoderOptions) {
        self.base.reset(&options);
    }

    pub fn reset_default(&mut self) {
        self.base.reset_default();
    }

    pub fn set_speed_options(&mut self, encoding_speed: i32, decoding_speed: i32) {
        self.base.set_speed_options(encoding_speed, decoding_speed);
    }

    pub fn set_track_encoded_properties(&mut self, flag: bool) {
        self.base.set_track_encoded_properties(flag);
    }

    pub fn set_attribute_quantization(&mut self, attribute_id: i32, bits: i32) {
        self.base
            .options_mut()
            .set_attribute_int(&attribute_id, "quantization_bits", bits);
    }

    pub fn set_attribute_explicit_quantization(
        &mut self,
        attribute_id: i32,
        quantization_bits: i32,
        num_dims: i32,
        origin: &[f32],
        range: f32,
    ) {
        self.base.options_mut().set_attribute_int(
            &attribute_id,
            "quantization_bits",
            quantization_bits,
        );
        self.base.options_mut().set_attribute_vector(
            &attribute_id,
            "quantization_origin",
            num_dims,
            origin,
        );
        self.base
            .options_mut()
            .set_attribute_float(&attribute_id, "quantization_range", range);
    }

    pub fn set_use_built_in_attribute_compression(&mut self, enabled: bool) {
        self.base
            .options_mut()
            .set_global_bool("use_built_in_attribute_compression", enabled);
    }

    pub fn set_encoding_method(&mut self, encoding_method: i32) {
        self.base.set_encoding_method(encoding_method);
    }

    pub fn set_encoding_submethod(&mut self, encoding_submethod: i32) {
        self.base.set_encoding_submethod(encoding_submethod);
    }

    pub fn set_attribute_prediction_scheme(
        &mut self,
        attribute_id: i32,
        prediction_scheme_method: i32,
    ) -> Status {
        let pc = match self.point_cloud {
            Some(pc) => pc,
            None => {
                return Status::new(StatusCode::DracoError, "Invalid input geometry.");
            }
        };
        let att = match pc.attribute(attribute_id) {
            Some(att) => att,
            None => return Status::new(StatusCode::DracoError, "Invalid attribute id."),
        };
        let status = self
            .base
            .check_prediction_scheme(att.attribute_type(), prediction_scheme_method);
        if !status.is_ok() {
            return status;
        }
        self.base.options_mut().set_attribute_int(
            &attribute_id,
            "prediction_scheme",
            prediction_scheme_method,
        );
        status
    }

    pub fn options(&self) -> &EncoderOptions {
        self.base.options()
    }

    pub fn options_mut(&mut self) -> &mut EncoderOptions {
        self.base.options_mut()
    }

    pub fn num_encoded_points(&self) -> usize {
        self.base.num_encoded_points()
    }

    pub fn num_encoded_faces(&self) -> usize {
        self.base.num_encoded_faces()
    }

    fn encode_point_cloud_to_buffer(
        &mut self,
        pc: &PointCloud,
        out_buffer: &mut EncoderBuffer,
    ) -> Status {
        if pc.is_compression_enabled() {
            if let Err(status) = self.apply_compression_options(pc) {
                return status;
            }
        }

        let encoding_method = self.base.options().get_global_int("encoding_method", -1);
        let mut encoder: Option<PointCloudEncoderKind> = None;

        if encoding_method == PointCloudEncodingMethod::PointCloudSequentialEncoding as i32 {
            encoder = Some(PointCloudEncoderKind::Sequential(
                PointCloudSequentialEncoder::new(),
            ));
        } else if encoding_method == -1 && self.base.options().get_speed() == 10 {
            encoder = Some(PointCloudEncoderKind::Sequential(
                PointCloudSequentialEncoder::new(),
            ));
        } else {
            let mut kd_tree_possible = true;
            for i in 0..pc.num_attributes() {
                let att = match pc.attribute(i) {
                    Some(att) => att,
                    None => continue,
                };
                if kd_tree_possible
                    && att.data_type() != draco_core::core::draco_types::DataType::Float32
                    && att.data_type() != draco_core::core::draco_types::DataType::Uint32
                    && att.data_type() != draco_core::core::draco_types::DataType::Uint16
                    && att.data_type() != draco_core::core::draco_types::DataType::Uint8
                    && att.data_type() != draco_core::core::draco_types::DataType::Int32
                    && att.data_type() != draco_core::core::draco_types::DataType::Int16
                    && att.data_type() != draco_core::core::draco_types::DataType::Int8
                {
                    kd_tree_possible = false;
                }
                if kd_tree_possible
                    && att.data_type() == draco_core::core::draco_types::DataType::Float32
                    && self
                        .base
                        .options()
                        .get_attribute_int(&i, "quantization_bits", -1)
                        <= 0
                {
                    kd_tree_possible = false;
                }
                if !kd_tree_possible {
                    break;
                }
            }
            if kd_tree_possible {
                encoder = Some(PointCloudEncoderKind::KdTree(PointCloudKdTreeEncoder::new()));
            } else if encoding_method == PointCloudEncodingMethod::PointCloudKdTreeEncoding as i32 {
                return Status::new(StatusCode::DracoError, "Invalid encoding method.");
            }
        }

        if encoder.is_none() {
            encoder = Some(PointCloudEncoderKind::Sequential(
                PointCloudSequentialEncoder::new(),
            ));
        }
        let mut encoder = encoder.unwrap();
        encoder.set_point_cloud(pc);
        let status = encoder.encode(self.base.options(), out_buffer);
        if !status.is_ok() {
            return status;
        }
        self.base
            .set_num_encoded_points(encoder.num_encoded_points());
        self.base.set_num_encoded_faces(0);
        ok_status()
    }

    fn encode_mesh_to_buffer(
        &mut self,
        mesh: &Mesh,
        out_buffer: &mut EncoderBuffer,
        out_symbols: Option<&mut Vec<EdgebreakerTopologyBitPattern>>,
    ) -> Status {
        if mesh.is_compression_enabled() {
            if let Err(status) = self.apply_compression_options(mesh) {
                return status;
            }
        }
        let mut encoding_method = self.base.options().get_global_int("encoding_method", -1);
        if encoding_method == -1 {
            if self.base.options().get_speed() == 10 {
                encoding_method = MeshEncoderMethod::MeshSequentialEncoding as i32;
            } else {
                encoding_method = MeshEncoderMethod::MeshEdgebreakerEncoding as i32;
            }
        }
        let mut encoder = if encoding_method == MeshEncoderMethod::MeshEdgebreakerEncoding as i32 {
            MeshEncoderKind::Edgebreaker(MeshEdgebreakerEncoder::new())
        } else {
            MeshEncoderKind::Sequential(MeshSequentialEncoder::new())
        };
        encoder.set_mesh(mesh);
        let status = encoder.encode(self.base.options(), out_buffer);
        if !status.is_ok() {
            return status;
        }
        if let (Some(symb_out), MeshEncoderKind::Edgebreaker(ref enc)) = (out_symbols, &encoder) {
            if let Some(symbols) = enc.get_traversal_symbols() {
                symb_out.clear();
                symb_out.extend_from_slice(&symbols);
            }
        }
        self.base
            .set_num_encoded_points(encoder.num_encoded_points());
        self.base.set_num_encoded_faces(encoder.num_encoded_faces());
        ok_status()
    }

    fn apply_compression_options(&mut self, pc: &PointCloud) -> Result<(), Status> {
        if !pc.is_compression_enabled() {
            return Ok(());
        }
        let compression_options = pc.compression_options();
        if !self.base.options().is_speed_set() {
            let speed = 10 - compression_options.compression_level;
            self.base.options_mut().set_speed(speed, speed);
        }
        for ai in 0..pc.num_attributes() {
            if self
                .base
                .options()
                .is_attribute_option_set(&ai, "quantization_bits")
            {
                continue;
            }
            let att = match pc.attribute(ai) {
                Some(att) => att,
                None => continue,
            };
            let mut quantization_bits = 0;
            match att.attribute_type() {
                GeometryAttributeType::Position => {
                    if compression_options
                        .quantization_position
                        .are_quantization_bits_defined()
                    {
                        quantization_bits = compression_options
                            .quantization_position
                            .quantization_bits();
                    } else {
                        self.apply_grid_quantization(pc, ai, compression_options)?;
                        continue;
                    }
                }
                GeometryAttributeType::TexCoord => {
                    quantization_bits = compression_options.quantization_bits_tex_coord;
                }
                GeometryAttributeType::Normal => {
                    quantization_bits = compression_options.quantization_bits_normal;
                }
                GeometryAttributeType::Color => {
                    quantization_bits = compression_options.quantization_bits_color;
                }
                GeometryAttributeType::Tangent => {
                    quantization_bits = compression_options.quantization_bits_tangent;
                }
                GeometryAttributeType::Weights => {
                    quantization_bits = compression_options.quantization_bits_weight;
                }
                GeometryAttributeType::Generic => {
                    quantization_bits = compression_options.quantization_bits_generic;
                }
                _ => {}
            }
            if quantization_bits > 0 {
                self.base.options_mut().set_attribute_int(
                    &ai,
                    "quantization_bits",
                    quantization_bits,
                );
            }
        }
        Ok(())
    }

    fn apply_grid_quantization(
        &mut self,
        pc: &PointCloud,
        attribute_index: i32,
        compression_options: &draco_core::compression::draco_compression_options::DracoCompressionOptions,
    ) -> Result<(), Status> {
        let spacing = compression_options.quantization_position.spacing();
        let status = self.set_attribute_grid_quantization(pc, attribute_index, spacing);
        if status.is_ok() {
            Ok(())
        } else {
            Err(status)
        }
    }

    pub fn set_attribute_grid_quantization(
        &mut self,
        pc: &PointCloud,
        attribute_index: i32,
        spacing: f32,
    ) -> Status {
        let att = match pc.attribute(attribute_index) {
            Some(att) => att,
            None => {
                return Status::new(StatusCode::DracoError, "Invalid attribute index.");
            }
        };
        if att.attribute_type() != GeometryAttributeType::Position {
            return Status::new(
                StatusCode::DracoError,
                "Invalid attribute type: Grid quantization is currently supported only for positions.",
            );
        }
        if att.num_components() != 3 {
            return Status::new(
                StatusCode::DracoError,
                "Invalid number of components: Grid quantization is currently supported only for 3D positions.",
            );
        }
        let bbox = pc.compute_bounding_box();
        let mut min_pos = Vector3f::new3(0.0, 0.0, 0.0);
        let mut num_values = 0i32;
        for c in 0..3 {
            let min_grid_pos = (bbox.min_point()[c] / spacing).floor();
            let max_grid_pos = (bbox.max_point()[c] / spacing).ceil();
            min_pos[c] = min_grid_pos * spacing;
            let component_num_values = max_grid_pos as i32 - min_grid_pos as i32 + 1;
            if component_num_values > num_values {
                num_values = component_num_values;
            }
        }
        let mut bits = most_significant_bit(num_values as u32);
        if (1u32 << bits) < num_values as u32 {
            bits += 1;
        }
        let range = ((1u32 << bits) as f32 - 1.0) * spacing;
        self.set_attribute_explicit_quantization(
            attribute_index,
            bits as i32,
            3,
            &[min_pos[0], min_pos[1], min_pos[2]],
            range,
        );
        ok_status()
    }
}
