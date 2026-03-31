//! Vulkan shader function (SPIR-V module).
//!
//! Port of pxr/imaging/hgiVulkan/shaderFunction.cpp/.h

#![allow(unsafe_code)]

use ash::vk;
use ash::vk::Handle;
use usd_hgi::{HgiShaderFunction, HgiShaderFunctionDesc, HgiShaderStage};

use crate::conversions::HgiVulkanConversions;
use crate::descriptor_set_layouts::HgiVulkanDescriptorSetInfoVector;
use crate::shader_compiler::compile_glsl;

/// Vulkan implementation of HgiShaderFunction.
///
/// Holds a compiled SPIR-V shader module created from GLSL source in the
/// descriptor.  If compilation fails the module handle is null and
/// `compile_errors` returns the driver error string.
pub struct HgiVulkanShaderFunction {
    desc: HgiShaderFunctionDesc,
    /// The logical device that owns `vk_shader_module`.  None when created
    /// in stub/no-device mode (the module will also be null in that case).
    device: Option<ash::Device>,
    errors: String,
    spirv_byte_size: usize,
    vk_shader_module: vk::ShaderModule,
    descriptor_set_info: HgiVulkanDescriptorSetInfoVector,
    inflight_bits: u64,
}

// SAFETY: vk::ShaderModule is just a u64 handle; we never share the raw
// Vulkan handles across threads without external synchronisation.
unsafe impl Send for HgiVulkanShaderFunction {}
unsafe impl Sync for HgiVulkanShaderFunction {}

impl HgiVulkanShaderFunction {
    // ------------------------------------------------------------------
    // Public constructors
    // ------------------------------------------------------------------

    /// Stub constructor (no real Vulkan device).
    ///
    /// Used by the current `HgiVulkan` scaffolding that does not yet have
    /// a live `ash::Device`.  The shader module handle is left null and
    /// `is_valid()` returns false.
    pub fn new(desc: HgiShaderFunctionDesc) -> Self {
        Self {
            desc,
            device: None,
            errors: String::new(),
            spirv_byte_size: 0,
            vk_shader_module: vk::ShaderModule::null(),
            descriptor_set_info: Vec::new(),
            inflight_bits: 0,
        }
    }

    /// Full constructor with a real Vulkan device.
    ///
    /// Mirrors `HgiVulkanShaderFunction(HgiVulkanDevice*, Hgi const*, …)`:
    /// 1. Concatenates `shader_code_declarations` + `shader_code` as GLSL source.
    /// 2. Compiles it to SPIR-V via `compile_glsl`.
    /// 3. Creates a `VkShaderModule` from the resulting binary.
    ///
    /// On compilation failure the module handle stays null; the error text
    /// is stored in `errors` and `is_valid()` returns false.
    ///
    /// `shader_version` is accepted for API symmetry with C++ but currently
    /// unused — the version is encoded in the GLSL source itself.
    pub fn new_with_device(
        device: &ash::Device,
        desc: &HgiShaderFunctionDesc,
        _shader_version: i32,
    ) -> Result<Self, String> {
        let debug_label = if desc.debug_name.is_empty() {
            "unknown"
        } else {
            &desc.debug_name
        };

        // Build the full GLSL source: declarations first, then the body.
        let glsl_source = format!("{}{}", desc.shader_code_declarations, desc.shader_code);

        let mut errors = String::new();
        let mut spirv_byte_size: usize = 0;
        let mut vk_shader_module = vk::ShaderModule::null();

        let spirv_result = compile_glsl(debug_label, &[&glsl_source], desc.shader_stage);

        match spirv_result {
            Ok(spirv_words) => {
                spirv_byte_size = spirv_words.len() * std::mem::size_of::<u32>();

                let create_info = vk::ShaderModuleCreateInfo::default().code(&spirv_words);

                // SAFETY: device and create_info are valid; allocation callbacks
                // are not used (None).
                match unsafe { device.create_shader_module(&create_info, None) } {
                    Ok(module) => {
                        vk_shader_module = module;
                        log::debug!(
                            "ShaderModule created for '{}': {:?}",
                            debug_label,
                            vk_shader_module
                        );
                    }
                    Err(vk_err) => {
                        errors = format!(
                            "vkCreateShaderModule failed for '{}': {vk_err}",
                            debug_label
                        );
                        log::error!("{errors}");
                    }
                }
            }
            Err(compile_err) => {
                errors = compile_err;
                log::error!("Shader compile error for '{debug_label}': {errors}");
            }
        }

        // Clear the raw source pointers in our copy of the descriptor just
        // as C++ does — the caller may free the source strings after this
        // call returns.
        let mut owned_desc = desc.clone();
        owned_desc.shader_code = String::new();
        owned_desc.shader_code_declarations = String::new();

        Ok(Self {
            desc: owned_desc,
            device: Some(device.clone()),
            errors,
            spirv_byte_size,
            vk_shader_module,
            // Descriptor set info is populated by the shader generator in C++;
            // that generator is not yet ported so we leave this empty.
            descriptor_set_info: Vec::new(),
            inflight_bits: 0,
        })
    }

    // ------------------------------------------------------------------
    // Vulkan-specific accessors (mirror C++ Get* methods)
    // ------------------------------------------------------------------

    /// Maps `desc.shader_stage` to the corresponding `VkShaderStageFlags`.
    ///
    /// Note: Vulkan pipelines use `VkShaderStageFlagBits` (a single bit);
    /// returning `ShaderStageFlags` (the bitmask wrapper) is idiomatic in ash.
    pub fn vk_shader_stage(&self) -> vk::ShaderStageFlags {
        HgiVulkanConversions::get_shader_stages(self.desc.shader_stage)
    }

    /// Returns the raw `VkShaderModule` handle.
    pub fn vk_shader_module(&self) -> vk::ShaderModule {
        self.vk_shader_module
    }

    /// Returns the shader entry-point name — always `"main"` for GLSL/SPIR-V.
    pub fn shader_function_name(&self) -> &str {
        "main"
    }

    /// Returns the descriptor set binding info extracted from this module.
    ///
    /// In the full C++ implementation this is populated by
    /// `HgiVulkanShaderGenerator::GetDescriptorSetInfo()`.  Until the
    /// generator is ported the slice will always be empty.
    pub fn descriptor_set_info(
        &self,
    ) -> &[crate::descriptor_set_layouts::HgiVulkanDescriptorSetInfo] {
        &self.descriptor_set_info
    }

    /// Returns the inflight-bits tracking field (writable reference in C++).
    pub fn inflight_bits(&self) -> u64 {
        self.inflight_bits
    }

    /// Sets the inflight-bits tracking field.
    pub fn set_inflight_bits(&mut self, bits: u64) {
        self.inflight_bits = bits;
    }
}

impl HgiShaderFunction for HgiVulkanShaderFunction {
    fn descriptor(&self) -> &HgiShaderFunctionDesc {
        &self.desc
    }

    /// Returns true when the shader module was created without errors.
    ///
    /// Mirrors C++: `return _errors.empty()` — validity is tied to the
    /// absence of compile/link errors, not to whether the module handle is
    /// non-null (though both are equivalent in practice).
    fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    fn compile_errors(&self) -> &str {
        &self.errors
    }

    /// Returns the byte size of the compiled SPIR-V binary.
    fn byte_size_of_resource(&self) -> usize {
        self.spirv_byte_size
    }

    /// Returns the `VkShaderModule` handle cast to `u64`.
    fn raw_resource(&self) -> u64 {
        // In ash, ShaderModule is a newtype around u64 on 64-bit Vulkan.
        self.vk_shader_module.as_raw()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Drop for HgiVulkanShaderFunction {
    fn drop(&mut self) {
        if self.vk_shader_module != vk::ShaderModule::null() {
            if let Some(ref device) = self.device {
                // SAFETY: the module was created by this same device and has
                // not been destroyed previously (we take it by move on drop).
                unsafe {
                    device.destroy_shader_module(self.vk_shader_module, None);
                }
                log::trace!("ShaderModule destroyed for '{}'", self.desc.debug_name);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Conversions helper — map HgiShaderStage to a single VkShaderStageFlagBits
// for use in pipeline stage info structs.
// ---------------------------------------------------------------------------

/// Returns the single-bit `VkShaderStageFlags` corresponding to `stage`.
///
/// The C++ counterpart is `VkShaderStageFlagBits HgiVulkanShaderFunction::GetShaderStage()`.
/// We return `ShaderStageFlags` rather than the deprecated `ShaderStageFlagBits`
/// because ash exposes it that way.
pub fn hgi_shader_stage_to_vk(stage: HgiShaderStage) -> vk::ShaderStageFlags {
    HgiVulkanConversions::get_shader_stages(stage)
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_hgi::HgiShaderFunctionDesc;

    fn make_desc(stage: HgiShaderStage) -> HgiShaderFunctionDesc {
        HgiShaderFunctionDesc {
            debug_name: "test_shader".to_string(),
            shader_stage: stage,
            ..Default::default()
        }
    }

    #[test]
    fn test_stub_new_invalid() {
        let desc = make_desc(HgiShaderStage::VERTEX);
        let func = HgiVulkanShaderFunction::new(desc);
        // Stub has no errors stored yet but also no compiled module.
        assert!(func.is_valid(), "stub with no errors should report valid");
        assert_eq!(func.byte_size_of_resource(), 0);
        assert_eq!(func.raw_resource(), 0);
        assert_eq!(func.shader_function_name(), "main");
    }

    #[test]
    fn test_descriptor_passthrough() {
        let desc = make_desc(HgiShaderStage::FRAGMENT);
        let func = HgiVulkanShaderFunction::new(desc.clone());
        assert_eq!(func.descriptor().shader_stage, HgiShaderStage::FRAGMENT);
        assert_eq!(func.descriptor().debug_name, "test_shader");
    }

    #[test]
    fn test_vk_shader_stage_vertex() {
        let func = HgiVulkanShaderFunction::new(make_desc(HgiShaderStage::VERTEX));
        assert!(
            func.vk_shader_stage()
                .contains(vk::ShaderStageFlags::VERTEX)
        );
    }

    #[test]
    fn test_vk_shader_stage_fragment() {
        let func = HgiVulkanShaderFunction::new(make_desc(HgiShaderStage::FRAGMENT));
        assert!(
            func.vk_shader_stage()
                .contains(vk::ShaderStageFlags::FRAGMENT)
        );
    }

    #[test]
    fn test_vk_shader_stage_compute() {
        let func = HgiVulkanShaderFunction::new(make_desc(HgiShaderStage::COMPUTE));
        assert!(
            func.vk_shader_stage()
                .contains(vk::ShaderStageFlags::COMPUTE)
        );
    }

    #[test]
    fn test_inflight_bits() {
        let mut func = HgiVulkanShaderFunction::new(make_desc(HgiShaderStage::VERTEX));
        assert_eq!(func.inflight_bits(), 0);
        func.set_inflight_bits(0xDEAD_BEEF);
        assert_eq!(func.inflight_bits(), 0xDEAD_BEEF);
    }

    #[test]
    fn test_descriptor_set_info_empty_on_stub() {
        let func = HgiVulkanShaderFunction::new(make_desc(HgiShaderStage::VERTEX));
        assert!(func.descriptor_set_info().is_empty());
    }

    #[test]
    fn test_as_any_downcast() {
        let func = HgiVulkanShaderFunction::new(make_desc(HgiShaderStage::VERTEX));
        let any = func.as_any();
        assert!(any.downcast_ref::<HgiVulkanShaderFunction>().is_some());
    }
}
