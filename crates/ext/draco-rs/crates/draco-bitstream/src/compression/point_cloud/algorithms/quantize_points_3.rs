//! 3D point quantization helpers.
//! Reference: `_ref/draco/src/draco/compression/point_cloud/algorithms/quantize_points_3.h`.
//!
//! These helpers quantize/dequantize 3D point clouds for legacy float-based
//! kD-tree encoding paths.

use draco_core::core::quantization_utils::{Dequantizer, Quantizer};
use draco_core::draco_dcheck_ge;

use crate::compression::point_cloud::algorithms::point_cloud_types::{Point3f, Point3ui};

/// Output sink used by decoding helpers.
pub trait PointOutput<CoordT> {
    fn write_point(&mut self, point: &[CoordT]);
}

/// Quantization parameters for 3D point clouds.
#[derive(Clone, Copy, Debug, Default)]
pub struct QuantizationInfo {
    pub quantization_bits: u32,
    pub range: f32,
}

/// Quantizes 3D float points into uint32 points.
pub fn quantize_points_3(points: &[Point3f], info: &mut QuantizationInfo, out: &mut Vec<Point3ui>) {
    draco_dcheck_ge!(info.quantization_bits as i32, 0);

    let mut max_range = 0.0f32;
    for p in points {
        max_range = max_range.max(p[0].abs());
        max_range = max_range.max(p[1].abs());
        max_range = max_range.max(p[2].abs());
    }

    let max_quantized_value = (1u32 << info.quantization_bits) - 1;
    let mut quantizer = Quantizer::new();
    quantizer.init_range(max_range, max_quantized_value as i32);
    info.range = max_range;

    out.clear();
    out.reserve(points.len());
    for p in points {
        let qx = quantizer.quantize_float(p[0]) as u32 + max_quantized_value;
        let qy = quantizer.quantize_float(p[1]) as u32 + max_quantized_value;
        let qz = quantizer.quantize_float(p[2]) as u32 + max_quantized_value;
        out.push(Point3ui::new3(qx, qy, qz));
    }
}

/// Dequantizes uint32 points into float points.
pub fn dequantize_points_3<O: PointOutput<f32>>(
    points: &[Point3ui],
    info: &QuantizationInfo,
    out: &mut O,
) {
    draco_dcheck_ge!(info.quantization_bits as i32, 0);
    draco_dcheck_ge!(info.range as i32, 0);

    let max_quantized_value = (1u32 << info.quantization_bits) - 1;
    let mut dequantizer = Dequantizer::new();
    let _ = dequantizer.init_range(info.range, max_quantized_value as i32);

    for p in points {
        let x = dequantizer.dequantize_float((p[0] - max_quantized_value) as i32);
        let y = dequantizer.dequantize_float((p[1] - max_quantized_value) as i32);
        let z = dequantizer.dequantize_float((p[2] - max_quantized_value) as i32);
        let coords = [x, y, z];
        out.write_point(&coords);
    }
}
