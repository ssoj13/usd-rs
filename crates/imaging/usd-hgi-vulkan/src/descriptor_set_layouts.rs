//! Descriptor set layout creation for Vulkan shader programs.
//!
//! Port of pxr/imaging/hgiVulkan/descriptorSetLayouts

use ash::vk;
use std::collections::HashMap;

// In ash 0.38, VkDescriptorSetLayoutBinding<'a> carries a PhantomData for the
// optional p_immutable_samplers pointer. We only ever store bindings with null
// immutable samplers, so 'static is correct here.
type Binding = vk::DescriptorSetLayoutBinding<'static>;

/// Information about one descriptor set in a shader program.
///
/// Mirrors C++ `HgiVulkanDescriptorSetInfo`.
#[derive(Debug, Clone)]
pub struct HgiVulkanDescriptorSetInfo {
    pub set_number: u32,
    pub bindings: Vec<Binding>,
}

/// A collection of descriptor set infos for one shader stage module.
pub type HgiVulkanDescriptorSetInfoVector = Vec<HgiVulkanDescriptorSetInfo>;

/// Returns true if the descriptor type is a texture or sampler variant.
///
/// Texture types receive narrower stage visibility (geometry + fragment only)
/// compared to buffer/uniform types that are visible across all graphics stages.
fn is_descriptor_texture_type(desc_type: vk::DescriptorType) -> bool {
    matches!(
        desc_type,
        vk::DescriptorType::SAMPLER
            | vk::DescriptorType::COMBINED_IMAGE_SAMPLER
            | vk::DescriptorType::SAMPLED_IMAGE
            | vk::DescriptorType::STORAGE_IMAGE
    )
}

/// Maps HgiShaderStage bitflags to the equivalent `VkShaderStageFlags`.
///
/// Mirrors `HgiVulkanConversions::GetShaderStages`. Only the stages relevant
/// to descriptor visibility are handled here.
fn get_shader_stages(hgi_stages: u32) -> vk::ShaderStageFlags {
    // HgiShaderStage bit constants (from usd_hgi::enums::HgiShaderStage)
    const VERTEX: u32 = 1 << 0;
    const FRAGMENT: u32 = 1 << 1;
    const COMPUTE: u32 = 1 << 2;
    const TESSELLATION_CONTROL: u32 = 1 << 3;
    const TESSELLATION_EVAL: u32 = 1 << 4;
    const GEOMETRY: u32 = 1 << 5;

    let mut flags = vk::ShaderStageFlags::empty();
    if hgi_stages & VERTEX != 0 {
        flags |= vk::ShaderStageFlags::VERTEX;
    }
    if hgi_stages & FRAGMENT != 0 {
        flags |= vk::ShaderStageFlags::FRAGMENT;
    }
    if hgi_stages & COMPUTE != 0 {
        flags |= vk::ShaderStageFlags::COMPUTE;
    }
    if hgi_stages & TESSELLATION_CONTROL != 0 {
        flags |= vk::ShaderStageFlags::TESSELLATION_CONTROL;
    }
    if hgi_stages & TESSELLATION_EVAL != 0 {
        flags |= vk::ShaderStageFlags::TESSELLATION_EVALUATION;
    }
    if hgi_stages & GEOMETRY != 0 {
        flags |= vk::ShaderStageFlags::GEOMETRY;
    }
    flags
}

/// Stage flags for texture/sampler bindings: geometry + fragment.
///
/// Mirrors: `HgiVulkanConversions::GetShaderStages(HgiShaderStageGeometry | HgiShaderStageFragment)`
fn texture_stage_flags() -> vk::ShaderStageFlags {
    get_shader_stages((1 << 5) | (1 << 1))
}

/// Stage flags for buffer/uniform bindings: all graphics stages.
///
/// Mirrors: `HgiVulkanConversions::GetShaderStages(vertex | tess_ctrl | tess_eval | geometry | fragment)`
fn buffer_stage_flags() -> vk::ShaderStageFlags {
    get_shader_stages((1 << 0) | (1 << 1) | (1 << 3) | (1 << 4) | (1 << 5))
}

/// Stage flags for compute-only bindings.
fn compute_stage_flags() -> vk::ShaderStageFlags {
    get_shader_stages(1 << 2)
}

/// Given descriptor set infos from all shader modules in a program, merges
/// them and creates the `VkDescriptorSetLayout` objects needed for pipeline
/// layout creation.
///
/// The caller takes ownership of the returned layouts and must destroy them
/// via `device.destroy_descriptor_set_layout()`.
///
/// # Merge rules (matching C++ `HgiVulkanMakeDescriptorSetLayouts`)
/// - Infos from all shader stages are merged by `set_number`.
/// - Within a set, bindings with the same `binding` index are deduplicated.
/// - Stage flags are overridden to cover all relevant graphics stages (or left
///   alone for compute), matching `HgiVulkanResourceBindings` expectations.
/// - Layouts are returned sorted by ascending set number.
///
/// # Safety
/// Calls `vkCreateDescriptorSetLayout`. The returned `VkDescriptorSetLayout`
/// handles are owned by the caller and must be destroyed with the same device.
pub fn make_descriptor_set_layouts(
    device: &ash::Device,
    infos: &[HgiVulkanDescriptorSetInfoVector],
    debug_name: &str,
) -> Result<Vec<vk::DescriptorSetLayout>, vk::Result> {
    // Merge bindings from all shader stage modules, keyed by set number.
    let mut merged: HashMap<u32, HgiVulkanDescriptorSetInfo> = HashMap::new();

    for info_vec in infos {
        for info in info_vec {
            let target =
                merged
                    .entry(info.set_number)
                    .or_insert_with(|| HgiVulkanDescriptorSetInfo {
                        set_number: info.set_number,
                        bindings: Vec::new(),
                    });

            for binding in &info.bindings {
                // Find existing entry for this binding index, or insert a new one.
                let dst = match target
                    .bindings
                    .iter_mut()
                    .find(|b| b.binding == binding.binding)
                {
                    Some(existing) => existing,
                    None => {
                        target.bindings.push(*binding);
                        target.bindings.last_mut().expect("just pushed")
                    }
                };

                // Force stage flags to match HgiVulkanResourceBindings expectations,
                // unless this is a compute-only binding (left as-is).
                if dst.stage_flags != compute_stage_flags() {
                    if is_descriptor_texture_type(dst.descriptor_type) {
                        dst.stage_flags = texture_stage_flags();
                    } else {
                        dst.stage_flags = buffer_stage_flags();
                    }
                }
            }
        }
    }

    // Sort by set number so layouts are returned in ascending set order.
    let mut sorted: Vec<HgiVulkanDescriptorSetInfo> = merged.into_values().collect();
    sorted.sort_by_key(|info| info.set_number);

    // Create one VkDescriptorSetLayout per merged set.
    let mut layouts = Vec::with_capacity(sorted.len());
    for info in &sorted {
        let layout = create_descriptor_set_layout(device, &info.bindings, debug_name)?;
        layouts.push(layout);
    }

    Ok(layouts)
}

/// Creates a single `VkDescriptorSetLayout` from the provided bindings.
///
/// Logs the debug name at debug level; no Vulkan debug utils integration here
/// since that requires the extension loader (not available in this stub).
fn create_descriptor_set_layout(
    device: &ash::Device,
    bindings: &[Binding],
    debug_name: &str,
) -> Result<vk::DescriptorSetLayout, vk::Result> {
    let create_info = vk::DescriptorSetLayoutCreateInfo::default().bindings(bindings);

    // SAFETY: create_info and bindings are valid for the duration of this call.
    let layout = unsafe { device.create_descriptor_set_layout(&create_info, None)? };

    if !debug_name.is_empty() {
        log::debug!("DescriptorSetLayout {debug_name}: created {layout:?}");
    }

    Ok(layout)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_binding(index: u32, desc_type: vk::DescriptorType) -> Binding {
        vk::DescriptorSetLayoutBinding::default()
            .binding(index)
            .descriptor_type(desc_type)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::VERTEX)
    }

    #[test]
    fn test_is_descriptor_texture_type() {
        assert!(is_descriptor_texture_type(vk::DescriptorType::SAMPLER));
        assert!(is_descriptor_texture_type(
            vk::DescriptorType::COMBINED_IMAGE_SAMPLER
        ));
        assert!(is_descriptor_texture_type(
            vk::DescriptorType::SAMPLED_IMAGE
        ));
        assert!(is_descriptor_texture_type(
            vk::DescriptorType::STORAGE_IMAGE
        ));
        assert!(!is_descriptor_texture_type(
            vk::DescriptorType::UNIFORM_BUFFER
        ));
        assert!(!is_descriptor_texture_type(
            vk::DescriptorType::STORAGE_BUFFER
        ));
    }

    #[test]
    fn test_get_shader_stages_compute() {
        let flags = get_shader_stages(1 << 2);
        assert_eq!(flags, vk::ShaderStageFlags::COMPUTE);
    }

    #[test]
    fn test_get_shader_stages_vertex_fragment() {
        let flags = get_shader_stages((1 << 0) | (1 << 1));
        assert_eq!(
            flags,
            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT
        );
    }

    /// Drives the merge logic inline to verify deduplication without a real device.
    fn run_merge(
        infos: &[HgiVulkanDescriptorSetInfoVector],
    ) -> HashMap<u32, HgiVulkanDescriptorSetInfo> {
        let mut merged: HashMap<u32, HgiVulkanDescriptorSetInfo> = HashMap::new();
        for info_vec in infos {
            for info in info_vec {
                let target =
                    merged
                        .entry(info.set_number)
                        .or_insert_with(|| HgiVulkanDescriptorSetInfo {
                            set_number: info.set_number,
                            bindings: Vec::new(),
                        });
                for binding in &info.bindings {
                    if !target.bindings.iter().any(|b| b.binding == binding.binding) {
                        target.bindings.push(*binding);
                    }
                }
            }
        }
        merged
    }

    #[test]
    fn test_merge_deduplicates_same_binding_index() {
        // Two shader modules both declare binding 0 in set 0 — should produce one entry.
        let binding = make_binding(0, vk::DescriptorType::UNIFORM_BUFFER);
        let infos = vec![
            vec![HgiVulkanDescriptorSetInfo {
                set_number: 0,
                bindings: vec![binding],
            }],
            vec![HgiVulkanDescriptorSetInfo {
                set_number: 0,
                bindings: vec![binding],
            }],
        ];
        let merged = run_merge(&infos);
        assert_eq!(
            merged[&0].bindings.len(),
            1,
            "duplicate binding index must be merged to one"
        );
    }

    #[test]
    fn test_merge_keeps_distinct_binding_indices() {
        // Bindings at index 0 and 1 in set 0 — both must survive the merge.
        let infos = vec![vec![HgiVulkanDescriptorSetInfo {
            set_number: 0,
            bindings: vec![
                make_binding(0, vk::DescriptorType::UNIFORM_BUFFER),
                make_binding(1, vk::DescriptorType::STORAGE_BUFFER),
            ],
        }]];
        let merged = run_merge(&infos);
        assert_eq!(merged[&0].bindings.len(), 2);
    }

    #[test]
    fn test_merge_separate_set_numbers() {
        // Set 0 from one module and set 1 from another must remain independent.
        let infos = vec![
            vec![HgiVulkanDescriptorSetInfo {
                set_number: 0,
                bindings: vec![make_binding(0, vk::DescriptorType::UNIFORM_BUFFER)],
            }],
            vec![HgiVulkanDescriptorSetInfo {
                set_number: 1,
                bindings: vec![make_binding(0, vk::DescriptorType::SAMPLED_IMAGE)],
            }],
        ];
        let merged = run_merge(&infos);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[&0].bindings.len(), 1);
        assert_eq!(merged[&1].bindings.len(), 1);
    }

    #[test]
    fn test_texture_stage_flags_is_geometry_fragment() {
        let flags = texture_stage_flags();
        assert!(flags.contains(vk::ShaderStageFlags::GEOMETRY));
        assert!(flags.contains(vk::ShaderStageFlags::FRAGMENT));
        assert!(!flags.contains(vk::ShaderStageFlags::VERTEX));
        assert!(!flags.contains(vk::ShaderStageFlags::COMPUTE));
    }

    #[test]
    fn test_buffer_stage_flags_covers_all_graphics() {
        let flags = buffer_stage_flags();
        assert!(flags.contains(vk::ShaderStageFlags::VERTEX));
        assert!(flags.contains(vk::ShaderStageFlags::FRAGMENT));
        assert!(flags.contains(vk::ShaderStageFlags::GEOMETRY));
        assert!(flags.contains(vk::ShaderStageFlags::TESSELLATION_CONTROL));
        assert!(flags.contains(vk::ShaderStageFlags::TESSELLATION_EVALUATION));
        assert!(!flags.contains(vk::ShaderStageFlags::COMPUTE));
    }
}
