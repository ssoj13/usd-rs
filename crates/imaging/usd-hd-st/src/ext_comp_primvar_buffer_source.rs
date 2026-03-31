#![allow(dead_code)]

//! ExtComp primvar buffer source - binds primvar to computation output.
//!
//! A buffer source that binds a primvar to an ExtComputation output,
//! making computation results available as geometry primvars for rendering.
//! Compatible with being bound to a BAR (Buffer Array Range).
//!
//! Port of pxr/imaging/hdSt/extCompPrimvarBufferSource.h

use super::ext_comp_cpu_computation::{ExtCompCpuComputationSharedPtr, INVALID_OUTPUT_INDEX};
use usd_tf::Token;
use usd_vt::Value as VtValue;

/// Tuple type for primvar data (component type + count).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HdTupleType {
    /// Component data type
    pub data_type: Token,
    /// Number of components per element
    pub count: usize,
}

impl Default for HdTupleType {
    fn default() -> Self {
        Self {
            data_type: Token::empty(),
            count: 1,
        }
    }
}

/// Primvar buffer source bound to an ExtComputation output.
///
/// Extracts a named output from a CPU computation and presents it
/// as a primvar that can be bound to a Buffer Array Range for rendering.
///
/// Port of HdStExtCompPrimvarBufferSource
#[derive(Debug)]
pub struct ExtCompPrimvarBufferSource {
    /// Primvar name
    primvar_name: Token,
    /// Source computation
    source: ExtCompCpuComputationSharedPtr,
    /// Index of the output in the source computation
    source_output_idx: usize,
    /// Data type descriptor
    tuple_type: HdTupleType,
    /// Whether resolved
    resolved: bool,
}

impl ExtCompPrimvarBufferSource {
    /// Create a primvar buffer source.
    ///
    /// Binds `primvar_name` to `source_output_name` from the source computation.
    /// `value_type` provides type information for the primvar.
    pub fn new(
        primvar_name: Token,
        source: ExtCompCpuComputationSharedPtr,
        source_output_name: &Token,
        value_type: HdTupleType,
    ) -> Self {
        let source_output_idx = source.get_output_index(source_output_name);
        Self {
            primvar_name,
            source,
            source_output_idx,
            tuple_type: value_type,
            resolved: false,
        }
    }

    /// Get the primvar name.
    pub fn name(&self) -> &Token {
        &self.primvar_name
    }

    /// Get the tuple type descriptor.
    pub fn tuple_type(&self) -> &HdTupleType {
        &self.tuple_type
    }

    /// Get the number of output elements.
    pub fn num_elements(&self) -> usize {
        self.source.num_elements()
    }

    /// Get the primvar value (valid after resolve).
    pub fn value(&self) -> Option<&VtValue> {
        if self.resolved {
            self.source.get_output_by_index(self.source_output_idx)
        } else {
            None
        }
    }

    /// Compute a hash of the underlying data.
    pub fn compute_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        Hash::hash(&self.primvar_name, &mut h);
        self.source_output_idx.hash(&mut h);
        h.finish()
    }

    /// Resolve: extract primvar from the source computation.
    pub fn resolve(&mut self) -> bool {
        if !self.source.is_resolved() {
            return false;
        }
        self.resolved = true;
        true
    }

    /// Whether resolved.
    pub fn is_resolved(&self) -> bool {
        self.resolved
    }

    /// Whether the binding is valid.
    pub fn is_valid(&self) -> bool {
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
    fn test_primvar_buffer_source() {
        let id = SdfPath::from_string("/comp").unwrap();
        let source = Arc::new(ExtCompCpuComputation::new(
            id,
            vec![],
            vec![Token::new("points")],
            100,
        ));

        let pbs = ExtCompPrimvarBufferSource::new(
            Token::new("points"),
            source,
            &Token::new("points"),
            HdTupleType {
                data_type: Token::new("float3"),
                count: 3,
            },
        );

        assert!(pbs.is_valid());
        assert_eq!(pbs.name(), &Token::new("points"));
        assert_eq!(pbs.num_elements(), 100);
        assert_eq!(pbs.tuple_type().count, 3);
    }

    #[test]
    fn test_invalid_binding() {
        let id = SdfPath::from_string("/comp").unwrap();
        let source = Arc::new(ExtCompCpuComputation::new(
            id,
            vec![],
            vec![Token::new("out")],
            10,
        ));

        let pbs = ExtCompPrimvarBufferSource::new(
            Token::new("pv"),
            source,
            &Token::new("missing"),
            HdTupleType::default(),
        );
        assert!(!pbs.is_valid());
    }
}
