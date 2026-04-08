//! HdExtComputation - External computation state primitive.
//!
//! Represents a client-defined computation for procedural primvar generation.
//! Follows an Input -> Processing -> Output model where:
//! - Inputs come from scene delegate or other computations (chaining)
//! - Results are in SOA form (parallel arrays with same element count)
//! - Uses pull model: processing only triggers when downstream pulls output
//!
//! Port of pxr/imaging/hd/extComputation.h

use super::{HdSceneDelegate, HdSprim};
use crate::scene_delegate::{
    HdExtComputationInputDescriptorVector, HdExtComputationOutputDescriptorVector,
};
use crate::types::HdDirtyBits;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// Dirty bits for HdExtComputation change tracking.
pub struct HdExtComputationDirtyBits;

impl HdExtComputationDirtyBits {
    /// No changes.
    pub const CLEAN: HdDirtyBits = 0;
    /// Input descriptors or bindings changed.
    pub const DIRTY_INPUT_DESC: HdDirtyBits = 1 << 0;
    /// Output descriptors changed.
    pub const DIRTY_OUTPUT_DESC: HdDirtyBits = 1 << 1;
    /// Number of output elements changed.
    pub const DIRTY_ELEMENT_COUNT: HdDirtyBits = 1 << 2;
    /// A scene input value changed.
    pub const DIRTY_SCENE_INPUT: HdDirtyBits = 1 << 3;
    /// A computation input value changed.
    pub const DIRTY_COMP_INPUT: HdDirtyBits = 1 << 4;
    /// Compute kernel binding changed.
    pub const DIRTY_KERNEL: HdDirtyBits = 1 << 5;
    /// Dispatch count (kernel invocations) changed.
    pub const DIRTY_DISPATCH_COUNT: HdDirtyBits = 1 << 6;
    /// All bits.
    pub const ALL_DIRTY: HdDirtyBits = Self::DIRTY_INPUT_DESC
        | Self::DIRTY_OUTPUT_DESC
        | Self::DIRTY_ELEMENT_COUNT
        | Self::DIRTY_SCENE_INPUT
        | Self::DIRTY_COMP_INPUT
        | Self::DIRTY_KERNEL
        | Self::DIRTY_DISPATCH_COUNT;
}

/// Hydra external computation state primitive.
///
/// Provides procedural primvar generation. Computations can be chained
/// (output of one feeds input of another).
///
/// Port of C++ `HdExtComputation`.
#[derive(Debug)]
pub struct HdExtComputation {
    /// Prim path identifier.
    id: SdfPath,
    /// Dirty bits for change tracking.
    dirty_bits: HdDirtyBits,
    /// Number of kernel invocations.
    dispatch_count: usize,
    /// Number of elements in output arrays.
    element_count: usize,
    /// Scene input names (inputs from scene delegate).
    scene_input_names: Vec<Token>,
    /// Computation input descriptors (inputs from other computations).
    computation_inputs: HdExtComputationInputDescriptorVector,
    /// Computation output descriptors.
    computation_outputs: HdExtComputationOutputDescriptorVector,
    /// GPU kernel source code.
    gpu_kernel_source: String,
}

impl HdExtComputation {
    /// Create a new external computation prim.
    pub fn new(id: SdfPath) -> Self {
        Self {
            id,
            dirty_bits: HdExtComputationDirtyBits::ALL_DIRTY,
            dispatch_count: 0,
            element_count: 0,
            scene_input_names: Vec::new(),
            computation_inputs: Vec::new(),
            computation_outputs: Vec::new(),
            gpu_kernel_source: String::new(),
        }
    }

    /// Get number of kernel invocations.
    ///
    /// Falls back to element_count if dispatch_count is 0.
    /// Matches C++ `HdExtComputation::GetDispatchCount()` (extComputation.cpp:123-126).
    pub fn get_dispatch_count(&self) -> usize {
        if self.dispatch_count > 0 {
            self.dispatch_count
        } else {
            self.element_count
        }
    }

    /// Get number of elements in output arrays.
    pub fn get_element_count(&self) -> usize {
        self.element_count
    }

    /// Get scene input names.
    pub fn get_scene_input_names(&self) -> &[Token] {
        &self.scene_input_names
    }

    /// Get output names from output descriptors.
    pub fn get_output_names(&self) -> Vec<Token> {
        self.computation_outputs
            .iter()
            .map(|desc| desc.name.clone())
            .collect()
    }

    /// Get computation input descriptors.
    pub fn get_computation_inputs(&self) -> &HdExtComputationInputDescriptorVector {
        &self.computation_inputs
    }

    /// Get computation output descriptors.
    pub fn get_computation_outputs(&self) -> &HdExtComputationOutputDescriptorVector {
        &self.computation_outputs
    }

    /// Get GPU kernel source code.
    pub fn get_gpu_kernel_source(&self) -> &str {
        &self.gpu_kernel_source
    }

    /// Returns true if this computation only aggregates inputs (no kernel).
    ///
    /// Computations with no outputs act as input aggregators, i.e.
    /// schedule inputs for resolution, but don't directly schedule
    /// execution of a computation.
    ///
    /// Matches C++ `HdExtComputation::IsInputAggregation()` (extComputation.cpp:141-147).
    pub fn is_input_aggregation(&self) -> bool {
        self.computation_outputs.is_empty()
    }
}

impl HdSprim for HdExtComputation {
    fn get_id(&self) -> &SdfPath {
        &self.id
    }

    fn get_dirty_bits(&self) -> HdDirtyBits {
        self.dirty_bits
    }

    fn set_dirty_bits(&mut self, bits: HdDirtyBits) {
        self.dirty_bits = bits;
    }

    fn sync(
        &mut self,
        delegate: &dyn HdSceneDelegate,
        _render_param: Option<&dyn crate::prim::HdRenderParam>,
        dirty_bits: &mut HdDirtyBits,
    ) {
        let id = self.id.clone();

        if *dirty_bits & HdExtComputationDirtyBits::DIRTY_INPUT_DESC != 0 {
            self.scene_input_names = delegate.get_ext_computation_scene_input_names(&id);
            self.computation_inputs = delegate.get_ext_computation_input_descriptors(&id);
        }

        if *dirty_bits & HdExtComputationDirtyBits::DIRTY_OUTPUT_DESC != 0 {
            self.computation_outputs = delegate.get_ext_computation_output_descriptors(&id);
        }

        if *dirty_bits & HdExtComputationDirtyBits::DIRTY_ELEMENT_COUNT != 0 {
            // C++ calls GetExtComputationInput, NOT generic Get (extComputation.cpp:89-98).
            let count_val = delegate.get_ext_computation_input(&id, &Token::new("elementCount"));
            // C++: reset to 0 when value is empty (backward compat)
            self.element_count = count_val.get::<usize>().copied().unwrap_or(0);
        }

        if *dirty_bits & HdExtComputationDirtyBits::DIRTY_DISPATCH_COUNT != 0 {
            // C++ calls GetExtComputationInput, NOT generic Get (extComputation.cpp:77-87).
            let count_val = delegate.get_ext_computation_input(&id, &Token::new("dispatchCount"));
            // C++: reset to 0 when value is empty (backward compat)
            self.dispatch_count = count_val.get::<usize>().copied().unwrap_or(0);
        }

        if *dirty_bits & HdExtComputationDirtyBits::DIRTY_KERNEL != 0 {
            self.gpu_kernel_source = delegate.get_ext_computation_kernel(&id);
        }

        // Clear specific bits, preserving DIRTY_SCENE_INPUT (matches C++).
        let clear_mask = HdExtComputationDirtyBits::DIRTY_INPUT_DESC
            | HdExtComputationDirtyBits::DIRTY_OUTPUT_DESC
            | HdExtComputationDirtyBits::DIRTY_DISPATCH_COUNT
            | HdExtComputationDirtyBits::DIRTY_ELEMENT_COUNT
            | HdExtComputationDirtyBits::DIRTY_KERNEL
            | HdExtComputationDirtyBits::DIRTY_COMP_INPUT;
        *dirty_bits &= !clear_mask;
        self.dirty_bits &= !clear_mask;
    }

    fn get_initial_dirty_bits_mask() -> HdDirtyBits
    where
        Self: Sized,
    {
        HdExtComputationDirtyBits::ALL_DIRTY
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ext_computation_creation() {
        let id = SdfPath::from_string("/World/MyComputation").unwrap();
        let comp = HdExtComputation::new(id.clone());
        assert_eq!(comp.get_id(), &id);
        assert_eq!(comp.get_element_count(), 0);
        assert_eq!(comp.get_dispatch_count(), 0);
        assert!(comp.get_scene_input_names().is_empty());
        assert!(comp.get_output_names().is_empty());
        assert!(comp.is_dirty());
    }

    #[test]
    fn test_ext_computation_dirty_bits() {
        let id = SdfPath::from_string("/World/Comp").unwrap();
        let mut comp = HdExtComputation::new(id);

        assert!(comp.is_dirty());
        comp.mark_clean(HdExtComputationDirtyBits::ALL_DIRTY);
        assert!(!comp.is_dirty());

        comp.mark_dirty(HdExtComputationDirtyBits::DIRTY_KERNEL);
        assert!(comp.is_dirty_bits(HdExtComputationDirtyBits::DIRTY_KERNEL));
        assert!(!comp.is_dirty_bits(HdExtComputationDirtyBits::DIRTY_INPUT_DESC));
    }

    #[test]
    fn test_input_aggregation() {
        let id = SdfPath::from_string("/World/Aggregator").unwrap();
        let mut comp = HdExtComputation::new(id);

        // No outputs = input aggregation (C++ checks only outputs.empty())
        assert!(comp.is_input_aggregation());

        // Add outputs = no longer aggregation
        comp.computation_outputs
            .push(crate::scene_delegate::HdExtComputationOutputDescriptor {
                name: Token::new("outA"),
                value_type: Default::default(),
            });
        assert!(!comp.is_input_aggregation());
    }
}
