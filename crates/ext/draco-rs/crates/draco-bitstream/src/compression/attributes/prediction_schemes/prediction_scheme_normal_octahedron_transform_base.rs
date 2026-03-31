//! Octahedron normal prediction transform base.
//! Reference: `_ref/draco/src/draco/compression/attributes/prediction_schemes/prediction_scheme_normal_octahedron_transform_base.h`.

use crate::compression::config::compression_shared::PredictionSchemeTransformType;
use draco_core::compression::attributes::normal_compression_utils::OctahedronToolBox;
use draco_core::core::bit_utils::most_significant_bit;

#[derive(Clone)]
pub struct PredictionSchemeNormalOctahedronTransformBase {
    tool_box: OctahedronToolBox,
}

impl PredictionSchemeNormalOctahedronTransformBase {
    pub fn new() -> Self {
        Self {
            tool_box: OctahedronToolBox::new(),
        }
    }

    pub fn with_max_quantized_value(max_quantized_value: i32) -> Self {
        let mut base = Self::new();
        let _ = base.set_max_quantized_value(max_quantized_value);
        base
    }

    pub fn get_type() -> PredictionSchemeTransformType {
        PredictionSchemeTransformType::PredictionTransformNormalOctahedron
    }

    pub fn are_corrections_positive(&self) -> bool {
        true
    }

    pub fn max_quantized_value(&self) -> i32 {
        self.tool_box.max_quantized_value()
    }

    pub fn center_value(&self) -> i32 {
        self.tool_box.center_value()
    }

    pub fn quantization_bits(&self) -> i32 {
        self.tool_box.quantization_bits()
    }

    pub fn set_max_quantized_value(&mut self, max_quantized_value: i32) -> bool {
        if max_quantized_value % 2 == 0 {
            return false;
        }
        let q = most_significant_bit(max_quantized_value as u32) + 1;
        self.tool_box.set_quantization_bits(q)
    }

    pub fn is_in_diamond(&self, s: i32, t: i32) -> bool {
        self.tool_box.is_in_diamond(s, t)
    }

    pub fn invert_diamond(&self, s: &mut i32, t: &mut i32) {
        self.tool_box.invert_diamond(s, t);
    }

    pub fn mod_max(&self, x: i32) -> i32 {
        self.tool_box.mod_max(x)
    }

    pub fn make_positive(&self, x: i32) -> i32 {
        self.tool_box.make_positive(x)
    }
}

impl Default for PredictionSchemeNormalOctahedronTransformBase {
    fn default() -> Self {
        Self::new()
    }
}
