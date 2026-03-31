//! Float point cloud tree encoder (legacy).
//! Reference: `_ref/draco/src/draco/compression/point_cloud/algorithms/float_points_tree_encoder.h|cc`.
//!
//! Encodes float point clouds by quantizing to integers and applying the
//! dynamic kD-tree encoder. Used for backwards compatibility paths.

use crate::compression::point_cloud::algorithms::dynamic_integer_points_kd_tree_encoder::DynamicIntegerPointsKdTreeEncoder;
use crate::compression::point_cloud::algorithms::point_cloud_compression_method::PointCloudCompressionMethod;
use crate::compression::point_cloud::algorithms::point_cloud_types::{Point3f, Point3ui};
use crate::compression::point_cloud::algorithms::quantize_points_3::{
    quantize_points_3, QuantizationInfo,
};
use draco_core::core::encoder_buffer::EncoderBuffer;
use draco_core::draco_dcheck_le;

/// Legacy float point cloud encoder wrapper.
pub struct FloatPointsTreeEncoder {
    qinfo: QuantizationInfo,
    method: PointCloudCompressionMethod,
    num_points: u32,
    buffer: EncoderBuffer,
    compression_level: u32,
}

impl FloatPointsTreeEncoder {
    const VERSION: u32 = 3;

    pub fn new(method: PointCloudCompressionMethod) -> Self {
        Self {
            qinfo: QuantizationInfo {
                quantization_bits: 16,
                range: 0.0,
            },
            method,
            num_points: 0,
            buffer: EncoderBuffer::new(),
            compression_level: 6,
        }
    }

    pub fn new_with_params(
        method: PointCloudCompressionMethod,
        quantization_bits: u32,
        compression_level: u32,
    ) -> Self {
        Self {
            qinfo: QuantizationInfo {
                quantization_bits,
                range: 0.0,
            },
            method,
            num_points: 0,
            buffer: EncoderBuffer::new(),
            compression_level,
        }
    }

    pub fn buffer(&mut self) -> &mut EncoderBuffer {
        &mut self.buffer
    }

    pub fn quantization_bits(&self) -> u32 {
        self.qinfo.quantization_bits
    }

    pub fn compression_level(&self) -> u32 {
        self.compression_level
    }

    pub fn range(&self) -> f32 {
        self.qinfo.range
    }

    pub fn num_points(&self) -> u32 {
        self.num_points
    }

    pub fn encode_point_cloud(&mut self, points: &[Point3f]) -> bool {
        self.buffer.clear();
        self.num_points = points.len() as u32;

        // Quantize input points to unsigned integer grid.
        let mut qpoints: Vec<Point3ui> = Vec::new();
        quantize_points_3(points, &mut self.qinfo, &mut qpoints);

        // Encode header.
        self.buffer.encode(Self::VERSION);
        self.buffer.encode(self.method as i8);
        self.buffer.encode(self.qinfo.quantization_bits);
        self.buffer.encode(self.qinfo.range);
        self.buffer.encode(self.num_points);
        if self.method == PointCloudCompressionMethod::KdTree {
            self.buffer.encode(self.compression_level);
        }

        if self.num_points == 0 {
            return true;
        }

        if self.method == PointCloudCompressionMethod::KdTree {
            return self.encode_point_cloud_kd_tree_internal(&mut qpoints);
        }
        false
    }

    fn encode_point_cloud_kd_tree_internal(&mut self, qpoints: &mut [Point3ui]) -> bool {
        draco_dcheck_le!(self.compression_level as i32, 6);
        let bit_length = self.qinfo.quantization_bits + 1;
        match self.compression_level {
            0 => {
                let mut enc = DynamicIntegerPointsKdTreeEncoder::<0>::new(3);
                enc.encode_points(&mut crate::compression::point_cloud::algorithms::dynamic_integer_points_kd_tree_encoder::PointSlice::new(qpoints), bit_length, &mut self.buffer)
            }
            1 => {
                let mut enc = DynamicIntegerPointsKdTreeEncoder::<1>::new(3);
                enc.encode_points(&mut crate::compression::point_cloud::algorithms::dynamic_integer_points_kd_tree_encoder::PointSlice::new(qpoints), bit_length, &mut self.buffer)
            }
            2 => {
                let mut enc = DynamicIntegerPointsKdTreeEncoder::<2>::new(3);
                enc.encode_points(&mut crate::compression::point_cloud::algorithms::dynamic_integer_points_kd_tree_encoder::PointSlice::new(qpoints), bit_length, &mut self.buffer)
            }
            3 => {
                let mut enc = DynamicIntegerPointsKdTreeEncoder::<3>::new(3);
                enc.encode_points(&mut crate::compression::point_cloud::algorithms::dynamic_integer_points_kd_tree_encoder::PointSlice::new(qpoints), bit_length, &mut self.buffer)
            }
            4 => {
                let mut enc = DynamicIntegerPointsKdTreeEncoder::<4>::new(3);
                enc.encode_points(&mut crate::compression::point_cloud::algorithms::dynamic_integer_points_kd_tree_encoder::PointSlice::new(qpoints), bit_length, &mut self.buffer)
            }
            5 => {
                let mut enc = DynamicIntegerPointsKdTreeEncoder::<5>::new(3);
                enc.encode_points(&mut crate::compression::point_cloud::algorithms::dynamic_integer_points_kd_tree_encoder::PointSlice::new(qpoints), bit_length, &mut self.buffer)
            }
            _ => {
                let mut enc = DynamicIntegerPointsKdTreeEncoder::<6>::new(3);
                enc.encode_points(&mut crate::compression::point_cloud::algorithms::dynamic_integer_points_kd_tree_encoder::PointSlice::new(qpoints), bit_length, &mut self.buffer)
            }
        }
    }
}
