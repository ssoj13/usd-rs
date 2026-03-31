//! Quantization utilities.
//! Reference: `_ref/draco/src/draco/core/quantization_utils.h` + `.cc`.

/// Quantizer for single precision floating point values.
#[derive(Clone, Copy, Debug)]
pub struct Quantizer {
    inverse_delta: f32,
}

impl Quantizer {
    pub fn new() -> Self {
        Self { inverse_delta: 1.0 }
    }

    pub fn init_range(&mut self, range: f32, max_quantized_value: i32) {
        self.inverse_delta = max_quantized_value as f32 / range;
    }

    pub fn init_delta(&mut self, delta: f32) {
        self.inverse_delta = 1.0 / delta;
    }

    pub fn quantize_float(&self, mut val: f32) -> i32 {
        val *= self.inverse_delta;
        (val + 0.5).floor() as i32
    }
}

impl Default for Quantizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Dequantizer for values quantized by Quantizer.
#[derive(Clone, Copy, Debug)]
pub struct Dequantizer {
    delta: f32,
}

impl Dequantizer {
    pub fn new() -> Self {
        Self { delta: 1.0 }
    }

    pub fn init_range(&mut self, range: f32, max_quantized_value: i32) -> bool {
        if max_quantized_value <= 0 {
            return false;
        }
        self.delta = range / max_quantized_value as f32;
        true
    }

    pub fn init_delta(&mut self, delta: f32) -> bool {
        self.delta = delta;
        true
    }

    pub fn dequantize_float(&self, val: i32) -> f32 {
        val as f32 * self.delta
    }
}

impl Default for Dequantizer {
    fn default() -> Self {
        Self::new()
    }
}
