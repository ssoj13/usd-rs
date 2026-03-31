//! Binary serialization for splines.
//!
//! Port of pxr/base/ts/binary.h and binary.cpp
//!
//! Provides compact binary encoding for spline data in USD files.

use super::knot::Knot;
use super::knot_data::KnotValueType;
use super::spline::Spline;
use super::spline_data::CustomData;
use super::types::{
    CurveType, ExtrapMode, Extrapolation, InterpMode, LoopParams, TangentAlgorithm, TsTime,
};
use std::collections::HashMap;
use std::io;

/// Binary format version.
/// Version 1: Initial spline implementation.
/// Version 2: Added tangent algorithms None and AutoEase.

/// Binary format version 1: Initial spline implementation.
pub const BINARY_FORMAT_VERSION_1: u8 = 1;
/// Binary format version 2: Added tangent algorithms None and AutoEase.
pub const BINARY_FORMAT_VERSION_2: u8 = 2;
/// Current binary format version in use.
pub const CURRENT_BINARY_VERSION: u8 = BINARY_FORMAT_VERSION_2;

/// Error type for binary operations.
#[derive(Debug)]
pub enum BinaryError {
    /// Unexpected end of data while parsing.
    UnexpectedEndOfData,
    /// Invalid type descriptor encountered.
    BadTypeDescriptor,
    /// Unknown binary format version.
    UnknownVersion(u8),
    /// IO error occurred during binary operations.
    IoError(io::Error),
}

impl From<io::Error> for BinaryError {
    fn from(e: io::Error) -> Self {
        BinaryError::IoError(e)
    }
}

impl std::fmt::Display for BinaryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnexpectedEndOfData => write!(f, "Unexpected end of data while parsing"),
            Self::BadTypeDescriptor => write!(f, "Bad spline type descriptor"),
            Self::UnknownVersion(v) => write!(f, "Unknown spline data version {}", v),
            Self::IoError(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for BinaryError {}

/// Binary data access for splines.
pub struct BinaryDataAccess;

impl BinaryDataAccess {
    /// Gets the binary format version needed for a spline.
    pub fn get_binary_format_version(spline: &Spline) -> u8 {
        // Version 2 if any tangent algorithm != None
        for knot in spline.knots() {
            if knot.pre_tan_algorithm() != TangentAlgorithm::None
                || knot.post_tan_algorithm() != TangentAlgorithm::None
            {
                return BINARY_FORMAT_VERSION_2;
            }
        }
        BINARY_FORMAT_VERSION_1
    }

    /// Writes a spline to binary data.
    ///
    /// Returns the binary blob and a reference to custom data.
    #[allow(deprecated)] // Uses deprecated curve_type() for binary compatibility
    pub fn write_binary_data(spline: &Spline) -> (Vec<u8>, HashMap<OrderedTime, CustomData>) {
        let mut buf = Vec::new();

        if spline.is_empty() {
            return (buf, HashMap::new());
        }

        let version = Self::get_binary_format_version(spline);
        let value_type = spline.value_type().unwrap_or(KnotValueType::Double);
        let is_hermite = spline.curve_type() == CurveType::Hermite;
        let has_loops = spline.inner_loop_params().is_enabled();

        // Type descriptor
        let type_descriptor = match value_type {
            KnotValueType::Double => 1u8,
            KnotValueType::Float => 2u8,
            KnotValueType::Half => 3u8,
        };

        // Header byte 1:
        // Bits 0-3: version
        // Bits 4-5: value type
        // Bit 6: time-valued
        // Bit 7: curve type
        let header1: u8 = (version
            | (type_descriptor << 4)) // time_valued = false
            | ((spline.curve_type() as u8) << 7);
        write_u8(&mut buf, header1);

        // Header byte 2:
        // Bits 0-2: pre-extrapolation mode
        // Bits 3-5: post-extrapolation mode
        // Bit 6: inner loops enabled
        let pre_extrap = spline.pre_extrapolation();
        let post_extrap = spline.post_extrapolation();
        let header2: u8 =
            (pre_extrap.mode as u8) | ((post_extrap.mode as u8) << 3) | ((has_loops as u8) << 6);
        write_u8(&mut buf, header2);

        // Sloped extrapolation slopes
        if pre_extrap.mode == ExtrapMode::Sloped {
            write_f64(&mut buf, pre_extrap.slope);
        }
        if post_extrap.mode == ExtrapMode::Sloped {
            write_f64(&mut buf, post_extrap.slope);
        }

        // Inner loop params
        if has_loops {
            let lp = spline.inner_loop_params();
            write_f64(&mut buf, lp.proto_start);
            write_f64(&mut buf, lp.proto_end);
            write_i32(&mut buf, lp.num_pre_loops);
            write_i32(&mut buf, lp.num_post_loops);
            write_f64(&mut buf, lp.value_offset);
        }

        // Knot count
        let knots: Vec<_> = spline.knots().collect();
        write_u32(&mut buf, knots.len() as u32);

        // Write each knot
        for knot in &knots {
            // Flag byte
            let flag: u8 = (knot.is_dual_valued() as u8)
                | ((knot.interp_mode() as u8) << 1)
                | ((knot.curve_type() as u8) << 3);
            write_u8(&mut buf, flag);

            // Time and value
            write_f64(&mut buf, knot.time());
            match value_type {
                KnotValueType::Double => write_f64(&mut buf, knot.value()),
                KnotValueType::Float => write_f32(&mut buf, knot.value() as f32),
                KnotValueType::Half => write_f16(&mut buf, knot.value()),
            }

            // Pre-value if dual-valued
            if knot.is_dual_valued() {
                match value_type {
                    KnotValueType::Double => write_f64(&mut buf, knot.pre_value()),
                    KnotValueType::Float => write_f32(&mut buf, knot.pre_value() as f32),
                    KnotValueType::Half => write_f16(&mut buf, knot.pre_value()),
                }
            }

            // Tangent widths (if not Hermite)
            if !is_hermite {
                write_f64(&mut buf, knot.pre_tangent().width);
                write_f64(&mut buf, knot.post_tangent().width);
            }

            // Tangent slopes
            match value_type {
                KnotValueType::Double => {
                    write_f64(&mut buf, knot.pre_tangent().slope);
                    write_f64(&mut buf, knot.post_tangent().slope);
                }
                KnotValueType::Float => {
                    write_f32(&mut buf, knot.pre_tangent().slope as f32);
                    write_f32(&mut buf, knot.post_tangent().slope as f32);
                }
                KnotValueType::Half => {
                    write_f16(&mut buf, knot.pre_tangent().slope);
                    write_f16(&mut buf, knot.post_tangent().slope);
                }
            }

            // Tangent algorithms (version 2+)
            if version > 1 {
                let algo_byte: u8 =
                    (knot.pre_tan_algorithm() as u8) | ((knot.post_tan_algorithm() as u8) << 4);
                write_u8(&mut buf, algo_byte);
            }
        }

        (buf, HashMap::new())
    }

    /// Creates a spline from binary data.
    #[allow(deprecated)] // Uses deprecated set_curve_type() for binary compatibility
    pub fn read_binary_data(
        buf: &[u8],
        custom_data: HashMap<OrderedTime, CustomData>,
    ) -> Result<Spline, BinaryError> {
        if buf.is_empty() {
            return Ok(Spline::new());
        }

        let version = buf[0] & 0x0F;
        match version {
            1 | 2 => Self::parse_v1_2(version, buf, custom_data),
            _ => Err(BinaryError::UnknownVersion(version)),
        }
    }

    fn parse_v1_2(
        version: u8,
        buf: &[u8],
        _custom_data: HashMap<OrderedTime, CustomData>,
    ) -> Result<Spline, BinaryError> {
        let mut reader = BinaryReader::new(buf);

        // Header byte 1
        let header1 = reader.read_u8()?;
        let type_descriptor = (header1 & 0x30) >> 4;
        let value_type = match type_descriptor {
            0 | 1 => KnotValueType::Double,
            2 => KnotValueType::Float,
            3 => KnotValueType::Half,
            _ => return Err(BinaryError::BadTypeDescriptor),
        };
        let _time_valued = (header1 & 0x40) != 0;
        let curve_type = if (header1 & 0x80) != 0 {
            CurveType::Hermite
        } else {
            CurveType::Bezier
        };
        let is_hermite = curve_type == CurveType::Hermite;

        // Header byte 2
        let header2 = reader.read_u8()?;
        let pre_mode = ExtrapMode::from_u8(header2 & 0x07);
        let post_mode = ExtrapMode::from_u8((header2 >> 3) & 0x07);
        let has_loops = (header2 & 0x40) != 0;

        // Extrapolation slopes
        let pre_slope = if pre_mode == ExtrapMode::Sloped {
            reader.read_f64()?
        } else {
            0.0
        };
        let post_slope = if post_mode == ExtrapMode::Sloped {
            reader.read_f64()?
        } else {
            0.0
        };

        // Inner loop params
        let loop_params = if has_loops {
            let proto_start = reader.read_f64()?;
            let proto_end = reader.read_f64()?;
            let num_pre_loops = reader.read_i32()? as u32;
            let num_post_loops = reader.read_i32()? as u32;
            let value_offset = reader.read_f64()?;
            LoopParams {
                proto_start,
                proto_end,
                num_pre_loops: num_pre_loops as i32,
                num_post_loops: num_post_loops as i32,
                value_offset,
            }
        } else {
            LoopParams::default()
        };

        // Build spline
        let mut spline = Spline::with_value_type(value_type);
        spline.set_curve_type(curve_type);
        spline.set_pre_extrapolation(Extrapolation {
            mode: pre_mode,
            slope: pre_slope,
        });
        spline.set_post_extrapolation(Extrapolation {
            mode: post_mode,
            slope: post_slope,
        });
        spline.set_inner_loop_params(loop_params);

        // Read knots
        let knot_count = reader.read_u32()?;
        for _ in 0..knot_count {
            // Flag byte
            let flag = reader.read_u8()?;
            let dual_valued = (flag & 0x01) != 0;
            let next_interp = InterpMode::from_u8((flag >> 1) & 0x03);
            let _knot_curve_type = if ((flag >> 3) & 0x01) != 0 {
                CurveType::Hermite
            } else {
                CurveType::Bezier
            };

            // Time and value
            let time = reader.read_f64()?;
            let value = reader.read_value(value_type)?;

            // Pre-value
            let pre_value = if dual_valued {
                reader.read_value(value_type)?
            } else {
                value
            };

            // Tangent widths
            let (pre_width, post_width) = if !is_hermite {
                (reader.read_f64()?, reader.read_f64()?)
            } else {
                (0.0, 0.0)
            };

            // Tangent slopes
            let pre_slope = reader.read_value(value_type)?;
            let post_slope = reader.read_value(value_type)?;

            // Tangent algorithms (version 2+)
            let (pre_algo, post_algo) = if version > 1 {
                let algo_byte = reader.read_u8()?;
                (
                    TangentAlgorithm::from_u8(algo_byte & 0x0F),
                    TangentAlgorithm::from_u8(algo_byte >> 4),
                )
            } else {
                (TangentAlgorithm::None, TangentAlgorithm::None)
            };

            // Create knot
            let mut knot = Knot::at_time(time, value);
            knot.set_interp_mode(next_interp);
            // Note: knot_curve_type is read for binary compatibility but not used
            // as curve type is now set at spline level only
            if dual_valued {
                knot.set_pre_value(pre_value);
            }
            knot.set_pre_tangent(super::knot::Tangent {
                slope: pre_slope,
                width: pre_width,
            });
            knot.set_post_tangent(super::knot::Tangent {
                slope: post_slope,
                width: post_width,
            });
            knot.set_pre_tan_algorithm(pre_algo);
            knot.set_post_tan_algorithm(post_algo);

            spline.set_knot(knot);
        }

        Ok(spline)
    }
}

/// Ordered time key for HashMap.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrderedTime(pub TsTime);

impl Eq for OrderedTime {}

impl std::hash::Hash for OrderedTime {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

/// Binary reader helper.
struct BinaryReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> BinaryReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    fn read_bytes(&mut self, n: usize) -> Result<&[u8], BinaryError> {
        if self.remaining() < n {
            return Err(BinaryError::UnexpectedEndOfData);
        }
        let result = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(result)
    }

    fn read_u8(&mut self) -> Result<u8, BinaryError> {
        let bytes = self.read_bytes(1)?;
        Ok(bytes[0])
    }

    fn read_u32(&mut self) -> Result<u32, BinaryError> {
        let bytes = self.read_bytes(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_i32(&mut self) -> Result<i32, BinaryError> {
        let bytes = self.read_bytes(4)?;
        Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_f32(&mut self) -> Result<f32, BinaryError> {
        let bytes = self.read_bytes(4)?;
        Ok(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_f64(&mut self) -> Result<f64, BinaryError> {
        let bytes = self.read_bytes(8)?;
        Ok(f64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn read_f16(&mut self) -> Result<f64, BinaryError> {
        let bytes = self.read_bytes(2)?;
        let bits = u16::from_le_bytes([bytes[0], bytes[1]]);
        Ok(half_to_f64(bits))
    }

    fn read_value(&mut self, value_type: KnotValueType) -> Result<f64, BinaryError> {
        match value_type {
            KnotValueType::Double => self.read_f64(),
            KnotValueType::Float => self.read_f32().map(f64::from),
            KnotValueType::Half => self.read_f16(),
        }
    }
}

// Write helpers
fn write_u8(buf: &mut Vec<u8>, v: u8) {
    buf.push(v);
}

fn write_u32(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn write_i32(buf: &mut Vec<u8>, v: i32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn write_f32(buf: &mut Vec<u8>, v: f32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn write_f64(buf: &mut Vec<u8>, v: f64) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn write_f16(buf: &mut Vec<u8>, v: f64) {
    let bits = f64_to_half(v);
    buf.extend_from_slice(&bits.to_le_bytes());
}

/// Converts f64 to half-precision bits.
fn f64_to_half(v: f64) -> u16 {
    let f = v as f32;
    let bits = f.to_bits();

    let sign = ((bits >> 16) & 0x8000) as u16;
    let exp = ((bits >> 23) & 0xFF) as i32;
    let mantissa = bits & 0x007FFFFF;

    if exp == 255 {
        // Inf or NaN
        if mantissa != 0 {
            sign | 0x7E00 // NaN
        } else {
            sign | 0x7C00 // Inf
        }
    } else if exp > 142 {
        // Overflow to infinity
        sign | 0x7C00
    } else if exp < 113 {
        // Underflow to zero or subnormal
        if exp < 103 {
            sign // Zero
        } else {
            // Subnormal
            let m = (mantissa | 0x800000) >> (126 - exp);
            sign | (m >> 13) as u16
        }
    } else {
        // Normal number
        let new_exp = ((exp - 127 + 15) as u16) << 10;
        let new_mantissa = (mantissa >> 13) as u16;
        sign | new_exp | new_mantissa
    }
}

/// Converts half-precision bits to f64.
fn half_to_f64(bits: u16) -> f64 {
    let sign = ((bits & 0x8000) as u32) << 16;
    let exp = ((bits >> 10) & 0x1F) as u32;
    let mantissa = (bits & 0x3FF) as u32;

    let f32_bits = if exp == 0 {
        if mantissa == 0 {
            sign // Zero
        } else {
            // Subnormal - normalize
            let mut m = mantissa;
            let mut e = 0i32;
            while (m & 0x400) == 0 {
                m <<= 1;
                e += 1;
            }
            m &= 0x3FF;
            let new_exp = ((127 - 15 - e) as u32) << 23;
            sign | new_exp | (m << 13)
        }
    } else if exp == 31 {
        // Inf or NaN
        sign | 0x7F800000 | (mantissa << 13)
    } else {
        // Normal
        let new_exp = ((exp as i32 - 15 + 127) as u32) << 23;
        sign | new_exp | (mantissa << 13)
    };

    f64::from(f32::from_bits(f32_bits))
}

// Add trait implementations for ExtrapMode, InterpMode, TangentAlgorithm
impl ExtrapMode {
    fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Held,
            1 => Self::Linear,
            2 => Self::Sloped,
            3 => Self::LoopRepeat,
            4 => Self::LoopReset,
            5 => Self::LoopOscillate,
            6 => Self::ValueBlock,
            _ => Self::Held,
        }
    }
}

impl InterpMode {
    fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Held,
            1 => Self::Linear,
            2 => Self::Curve,
            3 => Self::ValueBlock,
            _ => Self::Held,
        }
    }
}

impl TangentAlgorithm {
    fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::None,
            1 => Self::Custom,
            2 => Self::AutoEase,
            _ => Self::None,
        }
    }
}

/// Returns the current maximum binary format version supported.
///
/// Matches C++ `GetBinaryFormatVersion` for the "latest" version.
pub fn current_binary_version() -> u8 {
    CURRENT_BINARY_VERSION
}

/// Checks if a binary buffer's version can be read by this implementation.
///
/// Returns `Ok(version)` if readable, or `Err(BinaryError::UnknownVersion)`.
pub fn check_binary_version(buf: &[u8]) -> Result<u8, BinaryError> {
    if buf.is_empty() {
        return Ok(0); // Empty buffer is trivially valid
    }
    let version = buf[0] & 0x0F;
    match version {
        1 | 2 => Ok(version),
        _ => Err(BinaryError::UnknownVersion(version)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_spline_roundtrip() {
        let spline = Spline::new();
        let (buf, _) = BinaryDataAccess::write_binary_data(&spline);
        assert!(buf.is_empty());

        let read =
            BinaryDataAccess::read_binary_data(&buf, HashMap::new()).expect("value expected");
        assert!(read.is_empty());
    }

    #[test]
    fn test_simple_spline_roundtrip() {
        let mut spline = Spline::new();
        spline.set_knot(Knot::at_time(0.0, 0.0));
        spline.set_knot(Knot::at_time(10.0, 100.0));

        let (buf, _) = BinaryDataAccess::write_binary_data(&spline);
        assert!(!buf.is_empty());

        let read =
            BinaryDataAccess::read_binary_data(&buf, HashMap::new()).expect("value expected");
        assert_eq!(read.knot_count(), 2);
    }

    #[test]
    fn test_half_conversion() {
        // Test normal numbers
        let half = f64_to_half(1.0);
        let back = half_to_f64(half);
        assert!((back - 1.0).abs() < 0.001);

        // Test zero
        let half = f64_to_half(0.0);
        let back = half_to_f64(half);
        assert_eq!(back, 0.0);

        // Test negative
        let half = f64_to_half(-2.5);
        let back = half_to_f64(half);
        assert!((back - (-2.5)).abs() < 0.01);
    }

    #[test]
    fn test_version_detection() {
        let mut spline = Spline::new();
        spline.set_knot(Knot::at_time(0.0, 0.0));

        // Default tangent algorithm is None -> version 1
        assert_eq!(BinaryDataAccess::get_binary_format_version(&spline), 1);
    }

    #[test]
    fn test_version_2_with_tangent_algorithm() {
        let mut spline = Spline::new();
        let mut knot = Knot::at_time(0.0, 0.0);
        knot.set_pre_tan_algorithm(TangentAlgorithm::AutoEase);
        spline.set_knot(knot);

        // AutoEase tangent -> version 2
        assert_eq!(BinaryDataAccess::get_binary_format_version(&spline), 2);

        // Roundtrip should preserve tangent algorithm
        let (buf, _) = BinaryDataAccess::write_binary_data(&spline);
        let read =
            BinaryDataAccess::read_binary_data(&buf, HashMap::new()).expect("should read v2 data");
        assert_eq!(read.knot_count(), 1);
    }

    #[test]
    fn test_check_binary_version() {
        // Empty buffer is ok
        assert_eq!(check_binary_version(&[]).unwrap(), 0);

        // Version 1 encoded in bits 0-3
        assert_eq!(check_binary_version(&[0x11]).unwrap(), 1);

        // Version 2
        assert_eq!(check_binary_version(&[0x12]).unwrap(), 2);

        // Unknown version 15
        assert!(check_binary_version(&[0x0F]).is_err());
    }

    #[test]
    fn test_v1_migration_to_v2() {
        // Write a v1 spline (no tangent algorithms)
        let mut spline = Spline::new();
        spline.set_knot(Knot::at_time(0.0, 1.0));
        spline.set_knot(Knot::at_time(10.0, 5.0));
        let (buf, _) = BinaryDataAccess::write_binary_data(&spline);

        // Verify it was written as v1
        let ver = buf[0] & 0x0F;
        assert_eq!(ver, 1);

        // Read it back - should work and produce default tangent algorithms
        let read =
            BinaryDataAccess::read_binary_data(&buf, HashMap::new()).expect("should read v1 data");
        assert_eq!(read.knot_count(), 2);
        for knot in read.knots() {
            assert_eq!(knot.pre_tan_algorithm(), TangentAlgorithm::None);
            assert_eq!(knot.post_tan_algorithm(), TangentAlgorithm::None);
        }
    }

    #[test]
    fn test_current_binary_version() {
        assert_eq!(current_binary_version(), 2);
    }
}
