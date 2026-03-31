//! Compute command buffer interface

use super::cmds::HgiCmds;
use super::compute_pipeline::HgiComputePipelineHandle;
use super::enums::HgiMemoryBarrier;
use super::resource_bindings::HgiResourceBindingsHandle;

/// Compute dispatch parameters
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HgiComputeDispatchOp {
    /// Number of work groups in X dimension
    pub work_group_count_x: u32,
    /// Number of work groups in Y dimension
    pub work_group_count_y: u32,
    /// Number of work groups in Z dimension
    pub work_group_count_z: u32,
}

impl HgiComputeDispatchOp {
    /// Create a new compute dispatch
    pub fn new(x: u32, y: u32, z: u32) -> Self {
        Self {
            work_group_count_x: x,
            work_group_count_y: y,
            work_group_count_z: z,
        }
    }

    /// Create a 1D dispatch
    pub fn new_1d(x: u32) -> Self {
        Self::new(x, 1, 1)
    }

    /// Create a 2D dispatch
    pub fn new_2d(x: u32, y: u32) -> Self {
        Self::new(x, y, 1)
    }
}

impl Default for HgiComputeDispatchOp {
    fn default() -> Self {
        Self::new(1, 1, 1)
    }
}

/// Compute command buffer for compute shader operations
///
/// Used to record compute shader dispatches that will be submitted to the GPU.
pub trait HgiComputeCmds: HgiCmds {
    /// Bind compute pipeline state
    fn bind_pipeline(&mut self, pipeline: &HgiComputePipelineHandle);

    /// Bind resource bindings (buffers, textures, samplers)
    fn bind_resources(&mut self, resources: &HgiResourceBindingsHandle);

    /// Set push constant / function constant values for compute shader
    ///
    /// `pipeline` is the compute pipeline that you are binding before the dispatch.
    /// `bind_index` is the binding point index in the pipeline's shader.
    /// `data` is the data you are copying into the push constants block.
    fn set_constant_values(
        &mut self,
        _pipeline: &HgiComputePipelineHandle,
        _bind_index: u32,
        _data: &[u8],
    ) {
        // Default: no-op. Backends override for push constant support.
    }

    /// Dispatch compute work groups
    fn dispatch(&mut self, dispatch: &HgiComputeDispatchOp);

    /// Insert a memory barrier
    fn memory_barrier(&mut self, barrier: HgiMemoryBarrier);

    /// Returns the dispatch method for this compute encoder.
    ///
    /// Mirrors C++ `HgiComputeCmds::GetDispatchMethod()` const.
    /// Backends that support concurrent dispatch return `Concurrent`.
    fn get_dispatch_method(&self) -> super::enums::HgiComputeDispatch {
        super::enums::HgiComputeDispatch::Serial
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_dispatch() {
        let dispatch = HgiComputeDispatchOp::new(16, 16, 1);
        assert_eq!(dispatch.work_group_count_x, 16);
        assert_eq!(dispatch.work_group_count_y, 16);
        assert_eq!(dispatch.work_group_count_z, 1);
    }

    #[test]
    fn test_compute_dispatch_helpers() {
        let dispatch_1d = HgiComputeDispatchOp::new_1d(256);
        assert_eq!(dispatch_1d.work_group_count_x, 256);
        assert_eq!(dispatch_1d.work_group_count_y, 1);
        assert_eq!(dispatch_1d.work_group_count_z, 1);

        let dispatch_2d = HgiComputeDispatchOp::new_2d(32, 32);
        assert_eq!(dispatch_2d.work_group_count_x, 32);
        assert_eq!(dispatch_2d.work_group_count_y, 32);
        assert_eq!(dispatch_2d.work_group_count_z, 1);
    }

    #[test]
    fn test_compute_dispatch_default() {
        let dispatch = HgiComputeDispatchOp::default();
        assert_eq!(dispatch.work_group_count_x, 1);
        assert_eq!(dispatch.work_group_count_y, 1);
        assert_eq!(dispatch.work_group_count_z, 1);
    }
}
