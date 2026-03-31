//! Wrap prediction transform base.
//! Reference: `_ref/draco/src/draco/compression/attributes/prediction_schemes/prediction_scheme_wrap_transform_base.h`.

use crate::compression::config::compression_shared::PredictionSchemeTransformType;

#[derive(Clone)]
pub struct PredictionSchemeWrapTransformBase {
    num_components: i32,
    min_value: i32,
    max_value: i32,
    max_dif: i32,
    max_correction: i32,
    min_correction: i32,
    clamped_value: Vec<i32>,
}

impl PredictionSchemeWrapTransformBase {
    pub fn new() -> Self {
        Self {
            num_components: 0,
            min_value: 0,
            max_value: 0,
            max_dif: 0,
            max_correction: 0,
            min_correction: 0,
            clamped_value: Vec::new(),
        }
    }

    pub fn get_type() -> PredictionSchemeTransformType {
        PredictionSchemeTransformType::PredictionTransformWrap
    }

    pub fn init(&mut self, num_components: i32) {
        self.num_components = num_components;
        self.clamped_value.resize(num_components as usize, 0);
    }

    pub fn are_corrections_positive(&self) -> bool {
        false
    }

    pub fn clamp_predicted_value(&self, predicted_val: &[i32]) -> Vec<i32> {
        let mut out = self.clamped_value.clone();
        for i in 0..self.num_components as usize {
            if predicted_val[i] > self.max_value {
                out[i] = self.max_value;
            } else if predicted_val[i] < self.min_value {
                out[i] = self.min_value;
            } else {
                out[i] = predicted_val[i];
            }
        }
        out
    }

    pub fn init_correction_bounds(&mut self) -> bool {
        let dif = (self.max_value as i64) - (self.min_value as i64);
        if dif < 0 || dif >= i64::from(i32::MAX) {
            return false;
        }
        self.max_dif = 1 + dif as i32;
        self.max_correction = self.max_dif / 2;
        self.min_correction = -self.max_correction;
        if (self.max_dif & 1) == 0 {
            self.max_correction -= 1;
        }
        true
    }

    pub fn num_components(&self) -> i32 {
        self.num_components
    }

    pub fn min_value(&self) -> i32 {
        self.min_value
    }

    pub fn set_min_value(&mut self, v: i32) {
        self.min_value = v;
    }

    pub fn max_value(&self) -> i32 {
        self.max_value
    }

    pub fn set_max_value(&mut self, v: i32) {
        self.max_value = v;
    }

    pub fn max_dif(&self) -> i32 {
        self.max_dif
    }

    pub fn min_correction(&self) -> i32 {
        self.min_correction
    }

    pub fn max_correction(&self) -> i32 {
        self.max_correction
    }
}

impl Default for PredictionSchemeWrapTransformBase {
    fn default() -> Self {
        Self::new()
    }
}
