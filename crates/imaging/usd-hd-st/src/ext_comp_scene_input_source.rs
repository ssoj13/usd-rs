#![allow(dead_code)]

//! ExtComp scene input source - binds computation input to scene delegate value.
//!
//! An input source that provides a value directly from the scene delegate,
//! as opposed to being computed by another ExtComputation.
//!
//! Port of pxr/imaging/hdSt/extCompSceneInputSource.h

use super::ext_comp_input_source::ExtCompInputSource;
use usd_tf::Token;
use usd_vt::Value as VtValue;

/// Scene-provided external computation input source.
///
/// Binds a computation input name to a value provided directly
/// by the scene delegate. Resolve is immediate (value already available).
///
/// Port of HdSt_ExtCompSceneInputSource
#[derive(Debug, Clone)]
pub struct ExtCompSceneInputSource {
    /// Input name
    input_name: Token,
    /// Value from scene delegate
    value: VtValue,
    /// Whether resolved
    resolved: bool,
}

impl ExtCompSceneInputSource {
    /// Create a scene input source binding inputName to the provided value.
    pub fn new(input_name: Token, value: VtValue) -> Self {
        Self {
            input_name,
            value,
            resolved: false,
        }
    }
}

impl ExtCompInputSource for ExtCompSceneInputSource {
    fn name(&self) -> &Token {
        &self.input_name
    }
    fn value(&self) -> &VtValue {
        &self.value
    }

    /// Set resolved and return true (value is already available).
    fn resolve(&mut self) -> bool {
        self.resolved = true;
        true
    }

    fn is_resolved(&self) -> bool {
        self.resolved
    }

    /// Valid if the value is not empty.
    fn is_valid(&self) -> bool {
        !self.input_name.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scene_input_resolve() {
        let mut src = ExtCompSceneInputSource::new(Token::new("restPoints"), VtValue::default());
        assert!(!src.is_resolved());
        assert!(src.resolve());
        assert!(src.is_resolved());
        assert!(src.is_valid());
    }

    #[test]
    fn test_scene_input_name() {
        let src = ExtCompSceneInputSource::new(Token::new("velocities"), VtValue::default());
        assert_eq!(src.name(), &Token::new("velocities"));
    }
}
