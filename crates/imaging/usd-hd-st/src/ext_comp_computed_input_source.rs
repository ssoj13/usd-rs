#![allow(dead_code)]

//! ExtComp computed input source - binds input to another computation's output.
//!
//! An input source that obtains its value from a specific output of another
//! ExtCompCpuComputation, enabling chained computations.
//!
//! Port of pxr/imaging/hdSt/extCompComputedInputSource.h

use super::ext_comp_cpu_computation::{ExtCompCpuComputationSharedPtr, INVALID_OUTPUT_INDEX};
use super::ext_comp_input_source::ExtCompInputSource;
use usd_tf::Token;
use usd_vt::Value as VtValue;

/// Computed external computation input source.
///
/// Binds an input name to a specific output of another CPU computation,
/// creating a dependency chain between computations.
///
/// Port of HdSt_ExtCompComputedInputSource
#[derive(Debug)]
pub struct ExtCompComputedInputSource {
    /// Input name
    input_name: Token,
    /// Source computation producing the value
    source: ExtCompCpuComputationSharedPtr,
    /// Index of the output in the source computation
    source_output_idx: usize,
    /// Whether resolved
    resolved: bool,
}

impl ExtCompComputedInputSource {
    /// Create a computed input source.
    ///
    /// Binds `input_name` to `source_output_name` on the source computation.
    pub fn new(
        input_name: Token,
        source: ExtCompCpuComputationSharedPtr,
        source_output_name: &Token,
    ) -> Self {
        let source_output_idx = source.get_output_index(source_output_name);
        Self {
            input_name,
            source,
            source_output_idx,
            resolved: false,
        }
    }
}

impl ExtCompInputSource for ExtCompComputedInputSource {
    fn name(&self) -> &Token {
        &self.input_name
    }

    /// Get the value from the source computation's output.
    fn value(&self) -> &VtValue {
        static EMPTY: VtValue = VtValue::empty();
        self.source
            .get_output_by_index(self.source_output_idx)
            .unwrap_or(&EMPTY)
    }

    /// Resolved once the source computation is resolved.
    fn resolve(&mut self) -> bool {
        if self.source.is_resolved() {
            self.resolved = true;
            true
        } else {
            false
        }
    }

    fn is_resolved(&self) -> bool {
        self.resolved
    }

    /// Valid if the output index is valid.
    fn is_valid(&self) -> bool {
        self.source_output_idx != INVALID_OUTPUT_INDEX
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ext_comp_cpu_computation::ExtCompCpuComputation;
    use std::sync::Arc;
    use usd_sdf::Path as SdfPath;

    #[test]
    fn test_computed_input_binding() {
        let id = SdfPath::from_string("/comp").unwrap();
        let source = Arc::new(ExtCompCpuComputation::new(
            id,
            vec![],
            vec![Token::new("points"), Token::new("normals")],
            10,
        ));

        let input = ExtCompComputedInputSource::new(
            Token::new("inputPoints"),
            source.clone(),
            &Token::new("points"),
        );
        assert!(input.is_valid());
        assert_eq!(input.name(), &Token::new("inputPoints"));
    }

    #[test]
    fn test_invalid_output_name() {
        let id = SdfPath::from_string("/comp").unwrap();
        let source = Arc::new(ExtCompCpuComputation::new(
            id,
            vec![],
            vec![Token::new("out")],
            10,
        ));

        let input =
            ExtCompComputedInputSource::new(Token::new("in"), source, &Token::new("nonexistent"));
        assert!(!input.is_valid());
    }
}
