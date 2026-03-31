//! Interpolation types and settings for USD attribute values.

use std::fmt;
use std::sync::atomic::{AtomicU8, Ordering};

// ============================================================================
// InterpolationType
// ============================================================================

/// Interpolation type for attribute values between time samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum InterpolationType {
    /// Return the value of the nearest time sample (step function).
    Held = 0,
    /// Linearly interpolate between time samples.
    #[default]
    Linear = 1,
}

impl From<u8> for InterpolationType {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Held,
            _ => Self::Linear,
        }
    }
}

impl fmt::Display for InterpolationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InterpolationType::Held => write!(f, "held"),
            InterpolationType::Linear => write!(f, "linear"),
        }
    }
}

// Global default interpolation type
static DEFAULT_INTERPOLATION: AtomicU8 = AtomicU8::new(InterpolationType::Linear as u8);

/// Gets the global default interpolation type for stages.
pub fn get_stage_interpolation_type() -> InterpolationType {
    DEFAULT_INTERPOLATION.load(Ordering::Relaxed).into()
}

/// Sets the global default interpolation type for stages.
pub fn set_stage_interpolation_type(interp: InterpolationType) {
    DEFAULT_INTERPOLATION.store(interp as u8, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolation_default() {
        assert_eq!(InterpolationType::default(), InterpolationType::Linear);
    }

    #[test]
    fn test_global_interpolation() {
        // Save original
        let original = get_stage_interpolation_type();

        // Change to held
        set_stage_interpolation_type(InterpolationType::Held);
        assert_eq!(get_stage_interpolation_type(), InterpolationType::Held);

        // Restore
        set_stage_interpolation_type(original);
    }
}
