//! Canonicalized octahedron normal prediction transform base.
//! Reference: `_ref/draco/src/draco/compression/attributes/prediction_schemes/prediction_scheme_normal_octahedron_canonicalized_transform_base.h`.

use crate::compression::attributes::prediction_schemes::prediction_scheme_normal_octahedron_transform_base::PredictionSchemeNormalOctahedronTransformBase;
use crate::compression::config::compression_shared::PredictionSchemeTransformType;

#[derive(Clone)]
pub struct PredictionSchemeNormalOctahedronCanonicalizedTransformBase {
    base: PredictionSchemeNormalOctahedronTransformBase,
}

impl PredictionSchemeNormalOctahedronCanonicalizedTransformBase {
    pub fn new() -> Self {
        Self {
            base: PredictionSchemeNormalOctahedronTransformBase::new(),
        }
    }

    pub fn with_max_quantized_value(max_quantized_value: i32) -> Self {
        Self {
            base: PredictionSchemeNormalOctahedronTransformBase::with_max_quantized_value(
                max_quantized_value,
            ),
        }
    }

    pub fn get_type() -> PredictionSchemeTransformType {
        PredictionSchemeTransformType::PredictionTransformNormalOctahedronCanonicalized
    }

    pub fn base(&self) -> &PredictionSchemeNormalOctahedronTransformBase {
        &self.base
    }

    pub fn base_mut(&mut self) -> &mut PredictionSchemeNormalOctahedronTransformBase {
        &mut self.base
    }

    pub fn get_rotation_count(&self, pred: [i32; 2]) -> i32 {
        let sign_x = pred[0];
        let sign_y = pred[1];
        if sign_x == 0 {
            if sign_y == 0 {
                0
            } else if sign_y > 0 {
                3
            } else {
                1
            }
        } else if sign_x > 0 {
            if sign_y >= 0 {
                2
            } else {
                1
            }
        } else if sign_y <= 0 {
            0
        } else {
            3
        }
    }

    pub fn rotate_point(&self, p: [i32; 2], rotation_count: i32) -> [i32; 2] {
        match rotation_count {
            1 => [p[1], -p[0]],
            2 => [-p[0], -p[1]],
            3 => [-p[1], p[0]],
            _ => p,
        }
    }

    pub fn is_in_bottom_left(&self, p: [i32; 2]) -> bool {
        if p[0] == 0 && p[1] == 0 {
            return true;
        }
        p[0] < 0 && p[1] <= 0
    }
}

impl Default for PredictionSchemeNormalOctahedronCanonicalizedTransformBase {
    fn default() -> Self {
        Self::new()
    }
}
