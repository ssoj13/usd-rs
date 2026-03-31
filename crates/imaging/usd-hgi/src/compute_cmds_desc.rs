//! Compute command buffer descriptor
//!
//! Mirrors C++ HgiComputeCmdsDesc from computeCmdsDesc.h

use super::enums::HgiComputeDispatch;

/// Describes the properties to construct a HgiComputeCmds
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HgiComputeCmdsDesc {
    /// The dispatch method for compute encoders
    pub dispatch_method: HgiComputeDispatch,
}

impl Default for HgiComputeCmdsDesc {
    fn default() -> Self {
        Self {
            dispatch_method: HgiComputeDispatch::Serial,
        }
    }
}

impl HgiComputeCmdsDesc {
    /// Create a new compute commands descriptor
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the dispatch method
    pub fn with_dispatch_method(mut self, method: HgiComputeDispatch) -> Self {
        self.dispatch_method = method;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        let desc = HgiComputeCmdsDesc::default();
        assert_eq!(desc.dispatch_method, HgiComputeDispatch::Serial);
    }

    #[test]
    fn test_builder() {
        let desc = HgiComputeCmdsDesc::new().with_dispatch_method(HgiComputeDispatch::Concurrent);
        assert_eq!(desc.dispatch_method, HgiComputeDispatch::Concurrent);
    }
}
