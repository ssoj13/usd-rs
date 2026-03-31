//! Compression options.
//! Reference: `_ref/draco/src/draco/compression/draco_compression_options.h` + `.cc`.

use crate::core::status::{Status, StatusCode};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpatialQuantizationMode {
    LocalQuantizationBits,
    GlobalGrid,
}

#[derive(Clone, Copy, Debug)]
pub struct SpatialQuantizationOptions {
    mode: SpatialQuantizationMode,
    quantization_bits: i32,
    spacing: f32,
}

impl SpatialQuantizationOptions {
    pub fn new(quantization_bits: i32) -> Self {
        Self {
            mode: SpatialQuantizationMode::LocalQuantizationBits,
            quantization_bits,
            spacing: 0.0,
        }
    }

    pub fn set_quantization_bits(&mut self, quantization_bits: i32) {
        self.mode = SpatialQuantizationMode::LocalQuantizationBits;
        self.quantization_bits = quantization_bits;
    }

    pub fn are_quantization_bits_defined(&self) -> bool {
        matches!(self.mode, SpatialQuantizationMode::LocalQuantizationBits)
    }

    pub fn quantization_bits(&self) -> i32 {
        self.quantization_bits
    }

    pub fn set_grid(&mut self, spacing: f32) -> &mut Self {
        self.mode = SpatialQuantizationMode::GlobalGrid;
        self.spacing = spacing;
        self
    }

    pub fn spacing(&self) -> f32 {
        self.spacing
    }
}

impl PartialEq for SpatialQuantizationOptions {
    /// Mode-aware equality matching C++ behavior.
    /// Only compares the field relevant to the active mode.
    fn eq(&self, other: &Self) -> bool {
        if self.mode != other.mode {
            return false;
        }
        match self.mode {
            SpatialQuantizationMode::LocalQuantizationBits => {
                self.quantization_bits == other.quantization_bits
            }
            SpatialQuantizationMode::GlobalGrid => self.spacing == other.spacing,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DracoCompressionOptions {
    pub compression_level: i32,
    pub quantization_position: SpatialQuantizationOptions,
    pub quantization_bits_normal: i32,
    pub quantization_bits_tex_coord: i32,
    pub quantization_bits_color: i32,
    pub quantization_bits_generic: i32,
    pub quantization_bits_tangent: i32,
    pub quantization_bits_weight: i32,
    pub find_non_degenerate_texture_quantization: bool,
}

impl Default for DracoCompressionOptions {
    fn default() -> Self {
        Self {
            compression_level: 7,
            quantization_position: SpatialQuantizationOptions::new(11),
            quantization_bits_normal: 8,
            quantization_bits_tex_coord: 10,
            quantization_bits_color: 8,
            quantization_bits_generic: 8,
            quantization_bits_tangent: 8,
            quantization_bits_weight: 8,
            find_non_degenerate_texture_quantization: false,
        }
    }
}

impl DracoCompressionOptions {
    pub fn check(&self) -> Status {
        let status = Self::validate("Compression level", self.compression_level, 0, 10);
        if status.code() != StatusCode::Ok {
            return status;
        }
        if self.quantization_position.are_quantization_bits_defined() {
            let status = Self::validate(
                "Position quantization",
                self.quantization_position.quantization_bits(),
                0,
                30,
            );
            if status.code() != StatusCode::Ok {
                return status;
            }
        } else if self.quantization_position.spacing() <= 0.0 {
            return Status::error("Position quantization spacing is invalid.");
        }
        let status = Self::validate("Normals quantization", self.quantization_bits_normal, 0, 30);
        if status.code() != StatusCode::Ok {
            return status;
        }
        let status = Self::validate(
            "Tex coord quantization",
            self.quantization_bits_tex_coord,
            0,
            30,
        );
        if status.code() != StatusCode::Ok {
            return status;
        }
        let status = Self::validate("Color quantization", self.quantization_bits_color, 0, 30);
        if status.code() != StatusCode::Ok {
            return status;
        }
        let status = Self::validate(
            "Generic quantization",
            self.quantization_bits_generic,
            0,
            30,
        );
        if status.code() != StatusCode::Ok {
            return status;
        }
        let status = Self::validate(
            "Tangent quantization",
            self.quantization_bits_tangent,
            0,
            30,
        );
        if status.code() != StatusCode::Ok {
            return status;
        }
        let status = Self::validate("Weights quantization", self.quantization_bits_weight, 0, 30);
        if status.code() != StatusCode::Ok {
            return status;
        }
        Status::ok()
    }

    pub fn validate(name: &str, value: i32, min: i32, max: i32) -> Status {
        if value < min || value > max {
            let range = format!("[{}-{}].", min, max);
            return Status::error(&format!("{} is out of range {}", name, range));
        }
        Status::ok()
    }
}
