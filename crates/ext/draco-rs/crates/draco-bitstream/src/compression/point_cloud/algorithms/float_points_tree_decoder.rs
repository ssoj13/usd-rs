//! Float point cloud tree decoder (legacy).
//! Reference: `_ref/draco/src/draco/compression/point_cloud/algorithms/float_points_tree_decoder.h|cc`.
//!
//! Decodes float point clouds encoded by FloatPointsTreeEncoder.

use crate::compression::point_cloud::algorithms::dynamic_integer_points_kd_tree_decoder::DynamicIntegerPointsKdTreeDecoder;
use crate::compression::point_cloud::algorithms::point_cloud_compression_method::PointCloudCompressionMethod;
use crate::compression::point_cloud::algorithms::point_cloud_types::Point3ui;
use crate::compression::point_cloud::algorithms::quantize_points_3::{
    dequantize_points_3, PointOutput, QuantizationInfo,
};
use draco_core::core::decoder_buffer::DecoderBuffer;
use draco_core::draco_dcheck_le;

struct Point3uiOutput<'a> {
    target: &'a mut Vec<Point3ui>,
}

impl<'a> PointOutput<u32> for Point3uiOutput<'a> {
    fn write_point(&mut self, point: &[u32]) {
        if point.len() >= 3 {
            self.target
                .push(Point3ui::new3(point[0], point[1], point[2]));
        }
    }
}

/// Legacy float point cloud decoder.
pub struct FloatPointsTreeDecoder {
    qinfo: QuantizationInfo,
    method: i8,
    num_points: u32,
    compression_level: u32,
    num_points_from_header: u32,
}

impl FloatPointsTreeDecoder {
    pub fn new() -> Self {
        Self {
            qinfo: QuantizationInfo {
                quantization_bits: 0,
                range: 0.0,
            },
            method: 0,
            num_points: 0,
            compression_level: 0,
            num_points_from_header: 0,
        }
    }

    pub fn set_num_points_from_header(&mut self, num_points: u32) {
        self.num_points_from_header = num_points;
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

    pub fn decode_point_cloud<O: PointOutput<f32>>(
        &mut self,
        buffer: &mut DecoderBuffer,
        out: &mut O,
    ) -> bool {
        let mut decoded_version: u32 = 0;
        if !buffer.decode(&mut decoded_version) {
            return false;
        }

        let mut qpoints: Vec<Point3ui> = Vec::new();
        if decoded_version == 3 {
            let mut method_number: i8 = 0;
            if !buffer.decode(&mut method_number) {
                return false;
            }
            self.method = method_number;
            if self.method == PointCloudCompressionMethod::KdTree as i8 {
                if !self.decode_point_cloud_kd_tree_internal(buffer, &mut qpoints) {
                    return false;
                }
            } else {
                return false;
            }
        } else if decoded_version == 2 {
            if !self.decode_point_cloud_kd_tree_internal(buffer, &mut qpoints) {
                return false;
            }
        } else {
            return false;
        }

        dequantize_points_3(&qpoints, &self.qinfo, out);
        true
    }

    fn decode_point_cloud_kd_tree_internal(
        &mut self,
        buffer: &mut DecoderBuffer,
        qpoints: &mut Vec<Point3ui>,
    ) -> bool {
        if !buffer.decode(&mut self.qinfo.quantization_bits) {
            return false;
        }
        if self.qinfo.quantization_bits > 31 {
            return false;
        }
        if !buffer.decode(&mut self.qinfo.range) {
            return false;
        }
        if !buffer.decode(&mut self.num_points) {
            return false;
        }
        if self.num_points_from_header > 0 && self.num_points != self.num_points_from_header {
            return false;
        }
        if !buffer.decode(&mut self.compression_level) {
            return false;
        }

        // Only allow compression level in [0..6].
        draco_dcheck_le!(self.compression_level as i32, 6);
        if self.compression_level > 6 {
            return false;
        }

        if self.num_points > 0 {
            qpoints.reserve(self.num_points as usize);
        }
        let mut out_it = Point3uiOutput { target: qpoints };
        if self.num_points > 0 {
            let max_points = self.num_points;
            let ok = match self.compression_level {
                0 => {
                    let mut dec = DynamicIntegerPointsKdTreeDecoder::<0>::new(3);
                    dec.decode_points(buffer, &mut out_it, max_points)
                        && dec.num_decoded_points() == max_points
                }
                1 => {
                    let mut dec = DynamicIntegerPointsKdTreeDecoder::<1>::new(3);
                    dec.decode_points(buffer, &mut out_it, max_points)
                        && dec.num_decoded_points() == max_points
                }
                2 => {
                    let mut dec = DynamicIntegerPointsKdTreeDecoder::<2>::new(3);
                    dec.decode_points(buffer, &mut out_it, max_points)
                        && dec.num_decoded_points() == max_points
                }
                3 => {
                    let mut dec = DynamicIntegerPointsKdTreeDecoder::<3>::new(3);
                    dec.decode_points(buffer, &mut out_it, max_points)
                        && dec.num_decoded_points() == max_points
                }
                4 => {
                    let mut dec = DynamicIntegerPointsKdTreeDecoder::<4>::new(3);
                    dec.decode_points(buffer, &mut out_it, max_points)
                        && dec.num_decoded_points() == max_points
                }
                5 => {
                    let mut dec = DynamicIntegerPointsKdTreeDecoder::<5>::new(3);
                    dec.decode_points(buffer, &mut out_it, max_points)
                        && dec.num_decoded_points() == max_points
                }
                _ => {
                    let mut dec = DynamicIntegerPointsKdTreeDecoder::<6>::new(3);
                    dec.decode_points(buffer, &mut out_it, max_points)
                        && dec.num_decoded_points() == max_points
                }
            };
            if !ok {
                return false;
            }
        }

        if qpoints.len() != self.num_points as usize {
            return false;
        }
        true
    }
}
