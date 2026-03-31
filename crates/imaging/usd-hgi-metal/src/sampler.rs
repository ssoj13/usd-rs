//! Metal sampler resource. Port of pxr/imaging/hgiMetal/sampler

use usd_hgi::{HgiSampler, HgiSamplerDesc};

/// Metal sampler resource for texture sampling.
/// Mirrors C++ HgiMetalSampler.
#[derive(Debug)]
pub struct HgiMetalSampler {
    desc: HgiSamplerDesc,
    // On real Metal: sampler_id: id<MTLSamplerState>
    // label: String
}

impl HgiMetalSampler {
    /// Creates a new Metal sampler from the given descriptor.
    /// On real Metal, this would create via [device newSamplerStateWithDescriptor:].
    pub fn new(desc: HgiSamplerDesc) -> Self {
        Self { desc }
    }

    /// Returns the Metal sampler state handle.
    /// Mirrors C++ GetSamplerId().
    /// Stub: returns 0 (no real Metal sampler).
    pub fn get_sampler_id(&self) -> u64 {
        0
    }
}

impl HgiSampler for HgiMetalSampler {
    fn descriptor(&self) -> &HgiSamplerDesc {
        &self.desc
    }
    fn raw_resource(&self) -> u64 {
        self.get_sampler_id()
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
