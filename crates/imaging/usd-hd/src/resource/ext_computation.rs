//! External computation primitive for client-defined procedural data.

use crate::{
    HdDirtyBits,
    prim::{HdSceneDelegate, HdSprim},
};
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// Dirty bits for external computations.
pub struct HdExtComputationDirtyBits;

impl HdExtComputationDirtyBits {
    /// No changes
    pub const CLEAN: HdDirtyBits = 0;

    /// Input descriptors or bindings changed
    pub const DIRTY_INPUT_DESC: HdDirtyBits = 1 << 0;

    /// Output descriptors changed
    pub const DIRTY_OUTPUT_DESC: HdDirtyBits = 1 << 1;

    /// Number of output elements changed
    pub const DIRTY_ELEMENT_COUNT: HdDirtyBits = 1 << 2;

    /// Scene input value changed
    pub const DIRTY_SCENE_INPUT: HdDirtyBits = 1 << 3;

    /// Computation input value changed
    pub const DIRTY_COMP_INPUT: HdDirtyBits = 1 << 4;

    /// Compute kernel binding changed
    pub const DIRTY_KERNEL: HdDirtyBits = 1 << 5;

    /// Dispatch count changed
    pub const DIRTY_DISPATCH_COUNT: HdDirtyBits = 1 << 6;

    /// All bits dirty
    pub const ALL_DIRTY: HdDirtyBits = Self::DIRTY_INPUT_DESC
        | Self::DIRTY_OUTPUT_DESC
        | Self::DIRTY_ELEMENT_COUNT
        | Self::DIRTY_SCENE_INPUT
        | Self::DIRTY_COMP_INPUT
        | Self::DIRTY_KERNEL
        | Self::DIRTY_DISPATCH_COUNT;
}

/// Input descriptor for external computation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HdExtComputationInputDescriptor {
    /// Name of the input
    pub name: Token,

    /// Source computation path (if from another computation)
    pub source_computation: Option<SdfPath>,

    /// Source output name (if from another computation)
    pub source_output_name: Option<Token>,
}

/// Output descriptor for external computation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HdExtComputationOutputDescriptor {
    /// Name of the output
    pub name: Token,

    /// Data type of the output
    pub value_type: Token,
}

/// Vector of input descriptors.
pub type HdExtComputationInputDescriptorVector = Vec<HdExtComputationInputDescriptor>;

/// Vector of output descriptors.
pub type HdExtComputationOutputDescriptorVector = Vec<HdExtComputationOutputDescriptor>;

/// External computation primitive.
///
/// Represents a client-defined computation for procedural primvar generation.
/// Uses an Input -> Processing -> Output model.
///
/// # Model
///
/// - **Inputs**: Provided by scene delegate or other computations
/// - **Processing**: Executed via GPU kernel or CPU code
/// - **Outputs**: SOA (Structure of Arrays) data
///
/// # Pull Model
///
/// Processing is only triggered when downstream consumers pull outputs.
///
/// # Chaining
///
/// Computations can be chained: output from one becomes input to another.
pub struct HdExtComputation {
    /// Prim path identifier
    id: SdfPath,

    /// Current dirty bits
    dirty_bits: HdDirtyBits,

    /// Number of dispatch invocations
    dispatch_count: usize,

    /// Number of elements in output arrays
    element_count: usize,

    /// Names of scene inputs
    scene_input_names: Vec<Token>,

    /// Computation input descriptors
    computation_inputs: HdExtComputationInputDescriptorVector,

    /// Output descriptors
    computation_outputs: HdExtComputationOutputDescriptorVector,

    /// GPU kernel source code
    gpu_kernel_source: String,
}

impl HdExtComputation {
    /// Create a new external computation.
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

    /// Get dispatch count.
    pub fn get_dispatch_count(&self) -> usize {
        self.dispatch_count
    }

    /// Set dispatch count.
    pub fn set_dispatch_count(&mut self, count: usize) {
        self.dispatch_count = count;
    }

    /// Get element count.
    pub fn get_element_count(&self) -> usize {
        self.element_count
    }

    /// Set element count.
    pub fn set_element_count(&mut self, count: usize) {
        self.element_count = count;
    }

    /// Get scene input names.
    pub fn get_scene_input_names(&self) -> &[Token] {
        &self.scene_input_names
    }

    /// Set scene input names.
    pub fn set_scene_input_names(&mut self, names: Vec<Token>) {
        self.scene_input_names = names;
    }

    /// Get output names.
    pub fn get_output_names(&self) -> Vec<Token> {
        self.computation_outputs
            .iter()
            .map(|desc| desc.name.clone())
            .collect()
    }

    /// Get computation inputs.
    pub fn get_computation_inputs(&self) -> &HdExtComputationInputDescriptorVector {
        &self.computation_inputs
    }

    /// Set computation inputs.
    pub fn set_computation_inputs(&mut self, inputs: HdExtComputationInputDescriptorVector) {
        self.computation_inputs = inputs;
    }

    /// Get computation outputs.
    pub fn get_computation_outputs(&self) -> &HdExtComputationOutputDescriptorVector {
        &self.computation_outputs
    }

    /// Set computation outputs.
    pub fn set_computation_outputs(&mut self, outputs: HdExtComputationOutputDescriptorVector) {
        self.computation_outputs = outputs;
    }

    /// Get GPU kernel source.
    pub fn get_gpu_kernel_source(&self) -> &str {
        &self.gpu_kernel_source
    }

    /// Set GPU kernel source.
    pub fn set_gpu_kernel_source(&mut self, source: String) {
        self.gpu_kernel_source = source;
    }

    /// Check if this is an input aggregation computation.
    ///
    /// Computations with no outputs act as input aggregators — they schedule
    /// inputs for resolution but don't directly schedule execution.
    /// Port of C++ `HdExtComputation::IsInputAggregation()`.
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
        _scene_delegate: &dyn HdSceneDelegate,
        _render_param: Option<&dyn crate::prim::HdRenderParam>,
        dirty_bits: &mut HdDirtyBits,
    ) {
        // Scene delegate would populate data based on dirty bits
        // For now, just clear dirty bits
        *dirty_bits = HdExtComputationDirtyBits::CLEAN;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ext_computation_creation() {
        let comp = HdExtComputation::new(SdfPath::from_string("/test").unwrap());

        assert_eq!(comp.get_id().as_str(), "/test");
        assert_eq!(comp.get_dispatch_count(), 0);
        assert_eq!(comp.get_element_count(), 0);
        assert!(comp.get_scene_input_names().is_empty());
        assert!(comp.get_computation_inputs().is_empty());
        assert!(comp.get_computation_outputs().is_empty());
        assert!(comp.get_gpu_kernel_source().is_empty());
    }

    #[test]
    fn test_ext_computation_dispatch_count() {
        let mut comp = HdExtComputation::new(SdfPath::from_string("/test").unwrap());
        comp.set_dispatch_count(10);
        assert_eq!(comp.get_dispatch_count(), 10);
    }

    #[test]
    fn test_ext_computation_element_count() {
        let mut comp = HdExtComputation::new(SdfPath::from_string("/test").unwrap());
        comp.set_element_count(100);
        assert_eq!(comp.get_element_count(), 100);
    }

    #[test]
    fn test_ext_computation_is_input_aggregation() {
        let mut comp = HdExtComputation::new(SdfPath::from_string("/test").unwrap());

        // No outputs -> is aggregation (C++ checks outputs.empty())
        assert!(comp.is_input_aggregation());

        // Has outputs -> not aggregation
        comp.set_computation_outputs(vec![HdExtComputationOutputDescriptor {
            name: Token::new("output1"),
            value_type: Token::new("float"),
        }]);
        assert!(!comp.is_input_aggregation());
    }

    #[test]
    fn test_dirty_bits() {
        assert_eq!(HdExtComputationDirtyBits::CLEAN, 0);
        assert_ne!(HdExtComputationDirtyBits::DIRTY_INPUT_DESC, 0);
        assert_ne!(HdExtComputationDirtyBits::ALL_DIRTY, 0);

        let all = HdExtComputationDirtyBits::ALL_DIRTY;
        assert_ne!(all & HdExtComputationDirtyBits::DIRTY_INPUT_DESC, 0);
        assert_ne!(all & HdExtComputationDirtyBits::DIRTY_KERNEL, 0);
    }
}
