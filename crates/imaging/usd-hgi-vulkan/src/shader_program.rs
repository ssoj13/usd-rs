// Port of pxr/imaging/hgiVulkan/shaderProgram

use usd_hgi::{HgiShaderProgram, HgiShaderProgramDesc};

/// Vulkan shader program — holds a set of compiled shader functions.
///
/// Unlike OpenGL/Metal, Vulkan does not have a monolithic program object;
/// the actual pipeline is assembled later at draw/dispatch time.
/// This struct therefore owns no VkPipeline; it only aggregates the
/// function handles and exposes them to pipeline-creation code.
#[derive(Debug)]
pub struct HgiVulkanShaderProgram {
    desc: HgiShaderProgramDesc,
    /// Bitmask tracking which in-flight command buffers reference this resource,
    /// so the garbage collector knows when it is safe to destroy it.
    inflight_bits: u64,
}

impl HgiVulkanShaderProgram {
    /// Creates a new shader program from the given descriptor.
    ///
    /// Matches C++ `HgiVulkanShaderProgram(device, desc)`.  The device
    /// reference is not stored here because it is only needed during
    /// pipeline creation, which happens elsewhere in the Vulkan backend.
    pub fn new(desc: HgiShaderProgramDesc) -> Self {
        Self {
            desc,
            inflight_bits: 0,
        }
    }

    /// Returns the bitmask of in-flight command buffers that reference this program.
    pub fn inflight_bits(&self) -> u64 {
        self.inflight_bits
    }

    /// Returns a mutable reference to the inflight-bits field.
    ///
    /// The garbage collector writes to this to mark which frames own the resource.
    pub fn inflight_bits_mut(&mut self) -> &mut u64 {
        &mut self.inflight_bits
    }

    /// Sets the inflight bits directly.
    pub fn set_inflight_bits(&mut self, bits: u64) {
        self.inflight_bits = bits;
    }
}

impl HgiShaderProgram for HgiVulkanShaderProgram {
    fn descriptor(&self) -> &HgiShaderProgramDesc {
        &self.desc
    }

    /// Returns true when all attached shader function handles are non-null.
    ///
    /// The C++ Vulkan backend always returns `true` here; the real validity
    /// check is deferred to pipeline creation.  We follow the same contract:
    /// a program is considered valid as long as every function slot is occupied
    /// (handle is non-null), matching what `HgiShaderProgramDesc::is_valid()`
    /// would express for the handles.
    fn is_valid(&self) -> bool {
        !self.desc.shader_functions.is_empty()
            && self.desc.shader_functions.iter().all(|f| f.is_valid())
    }

    /// Returns any link-time errors.
    ///
    /// Vulkan has no separate link step; errors are reported per-function at
    /// SPIR-V compile time.  This method therefore always returns an empty
    /// string, mirroring the C++ implementation.
    fn link_errors(&self) -> &str {
        ""
    }

    /// Returns the sum of byte sizes of all attached shader functions.
    fn byte_size_of_resource(&self) -> usize {
        self.desc
            .shader_functions
            .iter()
            .map(|f| f.byte_size_of_resource())
            .sum()
    }

    /// Returns 0 — there is no single VkPipeline at the program level.
    fn raw_resource(&self) -> u64 {
        0
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
