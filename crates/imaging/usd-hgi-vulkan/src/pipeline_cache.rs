//! Vulkan pipeline cache.
//!
//! Port of pxr/imaging/hgiVulkan/pipelineCache
//!
//! NOTE: The C++ implementation is intentionally a stub — `_vkPipelineCache`
//! is left as `VK_NULL_HANDLE` with a TODO to wire up actual cache creation.
//! A pipeline cache avoids recompiling SPIR-V shader micro-code for every
//! pipeline combination, but has not been implemented upstream yet.
//! This port faithfully mirrors that state.

use ash::vk;

/// Wrapper for a Vulkan pipeline cache handle.
///
/// Currently mirrors the upstream C++ stub: the underlying `VkPipelineCache`
/// is `VK_NULL_HANDLE` and no cache is created. When the upstream adds actual
/// cache creation this struct should be updated to match.
#[derive(Debug)]
pub struct HgiVulkanPipelineCache {
    // VkPipelineCache handle — VK_NULL_HANDLE until upstream wires this up.
    vk_pipeline_cache: vk::PipelineCache,
}

impl HgiVulkanPipelineCache {
    /// Creates a new pipeline cache wrapper.
    ///
    /// Mirrors C++ `HgiVulkanPipelineCache(HgiVulkanDevice*)` which stores a
    /// null handle and defers actual cache creation (upstream TODO).
    pub fn new() -> Self {
        Self {
            vk_pipeline_cache: vk::PipelineCache::null(),
        }
    }

    /// Returns the `VkPipelineCache` handle (currently `VK_NULL_HANDLE`).
    pub fn vk_pipeline_cache(&self) -> vk::PipelineCache {
        self.vk_pipeline_cache
    }
}

impl Default for HgiVulkanPipelineCache {
    fn default() -> Self {
        Self::new()
    }
}
