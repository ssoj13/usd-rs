#![allow(dead_code)]

//! ExtComp input source base - abstract binding for computation inputs.
//!
//! Base trait for buffer sources that represent bindings to external
//! computation inputs. Concrete implementations bind to either scene
//! delegate values or outputs of other computations.
//!
//! Port of pxr/imaging/hdSt/extCompInputSource.h

use std::sync::Arc;
use usd_tf::Token;
use usd_vt::Value as VtValue;

/// Abstract base for external computation input sources.
///
/// Represents a named binding to a computation input. Subclasses provide
/// the actual value (from scene delegate or another computation's output).
///
/// Port of HdSt_ExtCompInputSource
pub trait ExtCompInputSource: std::fmt::Debug + Send + Sync {
    /// Returns the name of the input.
    fn name(&self) -> &Token;

    /// Returns the value associated with the input.
    fn value(&self) -> &VtValue;

    /// Resolve the input (compute or fetch the value).
    /// Returns true on success.
    fn resolve(&mut self) -> bool;

    /// Whether the input has been resolved.
    fn is_resolved(&self) -> bool;

    /// Check if the input binding is valid.
    fn is_valid(&self) -> bool;
}

/// Shared pointer to an input source.
pub type ExtCompInputSourceSharedPtr = Arc<dyn ExtCompInputSource>;

/// Vector of shared input sources.
pub type ExtCompInputSourceSharedPtrVector = Vec<ExtCompInputSourceSharedPtr>;

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct MockInput {
        name: Token,
        value: VtValue,
        resolved: bool,
    }

    impl ExtCompInputSource for MockInput {
        fn name(&self) -> &Token {
            &self.name
        }
        fn value(&self) -> &VtValue {
            &self.value
        }
        fn resolve(&mut self) -> bool {
            self.resolved = true;
            true
        }
        fn is_resolved(&self) -> bool {
            self.resolved
        }
        fn is_valid(&self) -> bool {
            true
        }
    }

    #[test]
    fn test_input_source_trait() {
        let input = MockInput {
            name: Token::new("points"),
            value: VtValue::default(),
            resolved: false,
        };
        assert_eq!(input.name(), &Token::new("points"));
        assert!(!input.is_resolved());
    }
}
