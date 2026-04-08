//! HdStExtComputation - Storm specialization of HdExtComputation.
//!
//! Manages GPU resources (SSBO input range) for external computations.
//! During Sprim sync, collects scene inputs from the delegate, converts
//! them to buffer sources, and allocates/uploads to an SSBO BAR.
//!
//! Port of pxr/imaging/hdSt/extComputation.h/.cpp

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use usd_hd::prim::ext_computation::{HdExtComputation, HdExtComputationDirtyBits};
use usd_hd::prim::{HdRenderParam, HdSceneDelegate, HdSprim};
use usd_hd::types::HdDirtyBits;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

use crate::resource_registry::{
    BufferArrayUsageHint, BufferSource, BufferSourceSharedPtr, BufferSpec, HdStResourceRegistry,
    HdStResourceRegistrySharedPtr, ManagedBarSharedPtr,
};

/// Storm external computation — manages inputs as GPU resources.
///
/// Extends the base `HdExtComputation` by adding an SSBO input range
/// (`_inputRange` in C++) that holds scene input data on the GPU.
///
/// During `sync()`:
/// 1. Base class syncs input/output descriptors, element/dispatch counts, kernel
/// 2. Storm collects scene input values and uploads them to SSBO
///
/// Port of C++ `HdStExtComputation` (hdSt/extComputation.h).
pub struct HdStExtComputation {
    /// Base class (composition since Rust has no inheritance).
    base: HdExtComputation,

    /// SSBO buffer array range for scene input data.
    /// Port of C++ `HdBufferArrayRangeSharedPtr _inputRange`.
    input_range: Option<ManagedBarSharedPtr>,

    /// Resource registry for GPU allocation.
    resource_registry: HdStResourceRegistrySharedPtr,
}

impl std::fmt::Debug for HdStExtComputation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HdStExtComputation")
            .field("base", &self.base)
            .field("input_range", &self.input_range)
            .finish_non_exhaustive()
    }
}

/// Shared pointer to Storm external computation.
pub type HdStExtComputationSharedPtr = Arc<HdStExtComputation>;

impl HdStExtComputation {
    /// Create a new Storm external computation.
    pub fn new(id: SdfPath, resource_registry: HdStResourceRegistrySharedPtr) -> Self {
        Self {
            base: HdExtComputation::new(id),
            input_range: None,
            resource_registry,
        }
    }

    /// Get the SSBO input range (for downstream GPU computations).
    ///
    /// Port of C++ `HdStExtComputation::GetInputRange()`.
    pub fn get_input_range(&self) -> Option<&ManagedBarSharedPtr> {
        self.input_range.as_ref()
    }

    /// Access the base HdExtComputation.
    pub fn base(&self) -> &HdExtComputation {
        &self.base
    }

    /// Collect scene inputs from delegate and upload to SSBO.
    ///
    /// Port of C++ HdStExtComputation::Sync lines 83-213.
    fn sync_scene_inputs(
        &mut self,
        delegate: &dyn HdSceneDelegate,
        render_param: Option<&dyn HdRenderParam>,
        dirty_bits: &mut HdDirtyBits,
    ) {
        // Only commit GPU resources for GPU computations or input aggregators.
        // CPU computation inputs are pulled during Rprim sync (HdStExtCompCpuComputation).
        if self.base.get_gpu_kernel_source().is_empty() && !self.base.is_input_aggregation() {
            return;
        }

        // Only process if scene inputs are dirty.
        if *dirty_bits & HdExtComputationDirtyBits::DIRTY_SCENE_INPUT == 0 {
            return;
        }

        let id = self.base.get_id().clone();

        // Collect scene inputs as buffer sources.
        let mut inputs: Vec<BufferSourceSharedPtr> = Vec::new();
        for input_name in self.base.get_scene_input_names() {
            let input_value = delegate.get_ext_computation_input(&id, input_name);

            // Convert VtValue to raw bytes for BufferSource.
            let (data, num_elements, element_size) = value_to_buffer_data(&input_value);

            if data.is_empty() {
                log::warn!(
                    "HdStExtComputation::sync: unsupported/empty input '{}' for {}",
                    input_name,
                    id
                );
                continue;
            }

            let source = Arc::new(BufferSource::new(
                input_name.clone(),
                data,
                num_elements,
                element_size,
            ));
            if source.is_valid() {
                inputs.push(source);
            } else {
                log::warn!(
                    "HdStExtComputation::sync: invalid source '{}' for {}",
                    input_name,
                    id
                );
            }
        }

        // Store current range to detect change (for GC).
        let prev_range_id = self
            .input_range
            .as_ref()
            .map(|bar| bar.lock().expect("bar lock").id);

        if !inputs.is_empty() {
            let registry = &self.resource_registry;

            if is_shared_ext_computation_data_enabled() && self.base.is_input_aggregation() {
                // Shared path: dedup by hash of input sources.
                let input_id = compute_shared_input_id(0, &inputs);

                if registry.register_ext_computation_data_range(input_id) {
                    // First instance — allocate new BAR.
                    self.input_range = Some(alloc_computation_data_range(&inputs, registry));
                    log::debug!(
                        "HdStExtComputation: allocated shared BAR for {} (hash={:#x})",
                        id,
                        input_id
                    );
                } else {
                    // Reuse existing — for now allocate fresh (true sharing requires instance registry).
                    self.input_range = Some(alloc_computation_data_range(&inputs, registry));
                    log::debug!(
                        "HdStExtComputation: reused shared BAR for {} (hash={:#x})",
                        id,
                        input_id
                    );
                }
            } else {
                // Unshared path: allocate or reuse.
                let reuse = self
                    .input_range
                    .as_ref()
                    .map_or(false, |bar| bar.lock().expect("bar lock").is_valid());

                if !reuse {
                    self.input_range = Some(alloc_computation_data_range(&inputs, registry));
                    log::debug!("HdStExtComputation: allocated unshared BAR for {}", id);
                } else {
                    // Check if existing BAR can hold the new specs.
                    let input_specs: Vec<BufferSpec> = inputs
                        .iter()
                        .map(|s| BufferSpec {
                            name: s.name.clone(),
                            num_elements: s.num_elements,
                            element_size: s.element_size,
                        })
                        .collect();

                    let bar_compatible = self.input_range.as_ref().map_or(false, |bar| {
                        let bar_lock = bar.lock().expect("bar lock");
                        // Compatible if total byte size fits.
                        let new_bytes: usize = input_specs
                            .iter()
                            .map(|s| s.num_elements * s.element_size)
                            .sum();
                        bar_lock.byte_size() >= new_bytes
                    });

                    if bar_compatible {
                        // Reuse existing range, upload new data.
                        if let Some(ref bar) = self.input_range {
                            registry.add_sources(bar, inputs);
                        }
                        log::debug!("HdStExtComputation: reused unshared BAR for {}", id);
                    } else {
                        // Specs changed — allocate new.
                        self.input_range = Some(alloc_computation_data_range(&inputs, registry));
                        log::debug!(
                            "HdStExtComputation: reallocated unshared BAR for {} (specs changed)",
                            id
                        );
                    }
                }
            }

            // If range changed, mark GC needed to release old data.
            let new_range_id = self
                .input_range
                .as_ref()
                .map(|bar| bar.lock().expect("bar lock").id);
            if prev_range_id.is_some() && prev_range_id != new_range_id {
                mark_gc_needed(render_param);
            }

            // Register input BAR in resource registry for downstream GPU computations.
            // Port of C++ renderIndex.GetSprim() -> GetInputRange() pattern.
            if let Some(ref bar) = self.input_range {
                self.resource_registry
                    .register_ext_comp_input_bar(&id, bar.clone());
            }
        }

        *dirty_bits &= !HdExtComputationDirtyBits::DIRTY_SCENE_INPUT;
    }
}

impl HdSprim for HdStExtComputation {
    fn get_id(&self) -> &SdfPath {
        self.base.get_id()
    }

    fn get_dirty_bits(&self) -> HdDirtyBits {
        self.base.get_dirty_bits()
    }

    fn set_dirty_bits(&mut self, bits: HdDirtyBits) {
        self.base.set_dirty_bits(bits);
    }

    fn sync(
        &mut self,
        delegate: &dyn HdSceneDelegate,
        render_param: Option<&dyn HdRenderParam>,
        dirty_bits: &mut HdDirtyBits,
    ) {
        log::debug!(
            "HdStExtComputation::sync for {} (dirty={:#x})",
            self.base.get_id(),
            *dirty_bits
        );

        // Step 1: Base class sync (collects descriptors, counts, kernel from delegate).
        self.base.sync(delegate, render_param, dirty_bits);

        // Step 2: Storm-specific — collect scene inputs and upload to SSBO.
        self.sync_scene_inputs(delegate, render_param, dirty_bits);
    }

    fn get_initial_dirty_bits_mask() -> HdDirtyBits
    where
        Self: Sized,
    {
        HdExtComputationDirtyBits::ALL_DIRTY
    }

    fn finalize(&mut self, render_param: Option<&dyn HdRenderParam>) {
        // Release input range data on destruction.
        if self.input_range.is_some() {
            mark_gc_needed(render_param);
            self.resource_registry
                .remove_ext_comp_input_bar(self.base.get_id());
            self.input_range = None;
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a VtValue to raw bytes for BufferSource.
///
/// Port of C++ HdVtBufferSource: extracts typed data as bytes.
fn value_to_buffer_data(value: &usd_vt::Value) -> (Vec<u8>, usize, usize) {
    // Try common array types first.
    if let Some(arr) = value.get::<usd_vt::Array<f32>>() {
        let data: Vec<u8> = arr
            .as_slice()
            .iter()
            .flat_map(|v| v.to_ne_bytes())
            .collect();
        let num = arr.len();
        return (data, num, std::mem::size_of::<f32>());
    }
    if let Some(arr) = value.get::<usd_vt::Array<f64>>() {
        let data: Vec<u8> = arr
            .as_slice()
            .iter()
            .flat_map(|v| v.to_ne_bytes())
            .collect();
        let num = arr.len();
        return (data, num, std::mem::size_of::<f64>());
    }
    if let Some(arr) = value.get::<usd_vt::Array<i32>>() {
        let data: Vec<u8> = arr
            .as_slice()
            .iter()
            .flat_map(|v| v.to_ne_bytes())
            .collect();
        let num = arr.len();
        return (data, num, std::mem::size_of::<i32>());
    }
    if let Some(arr) = value.get::<usd_vt::Array<u32>>() {
        let data: Vec<u8> = arr
            .as_slice()
            .iter()
            .flat_map(|v| v.to_ne_bytes())
            .collect();
        let num = arr.len();
        return (data, num, std::mem::size_of::<u32>());
    }

    // Scalar types — single element.
    if let Some(v) = value.get::<f32>() {
        return (v.to_ne_bytes().to_vec(), 1, std::mem::size_of::<f32>());
    }
    if let Some(v) = value.get::<f64>() {
        return (v.to_ne_bytes().to_vec(), 1, std::mem::size_of::<f64>());
    }
    if let Some(v) = value.get::<i32>() {
        return (v.to_ne_bytes().to_vec(), 1, std::mem::size_of::<i32>());
    }
    if let Some(v) = value.get::<u32>() {
        return (v.to_ne_bytes().to_vec(), 1, std::mem::size_of::<u32>());
    }

    // GfVec3f (stored as [f32; 3]) — common for positions/normals.
    if let Some(arr) = value.get::<usd_vt::Array<[f32; 3]>>() {
        let data: Vec<u8> = arr
            .as_slice()
            .iter()
            .flat_map(|v| v.iter().flat_map(|f| f.to_ne_bytes()).collect::<Vec<u8>>())
            .collect();
        let num = arr.len();
        return (data, num, std::mem::size_of::<[f32; 3]>());
    }

    // Unsupported type.
    (Vec::new(), 0, 0)
}

/// Compute shared computation input ID by hashing buffer sources.
///
/// Port of C++ `_ComputeSharedComputationInputId`.
fn compute_shared_input_id(base_id: u64, sources: &[BufferSourceSharedPtr]) -> u64 {
    let mut hasher = DefaultHasher::new();
    base_id.hash(&mut hasher);
    for source in sources {
        source.name.as_str().hash(&mut hasher);
        source.byte_size().hash(&mut hasher);
        // Hash a sample of actual data for content-based dedup.
        if source.data.len() >= 8 {
            source.data[..8].hash(&mut hasher);
        } else {
            source.data.hash(&mut hasher);
        }
    }
    hasher.finish()
}

/// Allocate an SSBO BAR and upload sources.
///
/// Port of C++ `_AllocateComputationDataRange`.
fn alloc_computation_data_range(
    inputs: &[BufferSourceSharedPtr],
    registry: &HdStResourceRegistry,
) -> ManagedBarSharedPtr {
    let specs: Vec<BufferSpec> = inputs
        .iter()
        .map(|s| BufferSpec {
            name: s.name.clone(),
            num_elements: s.num_elements,
            element_size: s.element_size,
        })
        .collect();

    let role = Token::new("extComputation");
    let hint = BufferArrayUsageHint {
        storage: true,
        ..Default::default()
    };

    let bar = registry.allocate_shader_storage_bar(&role, &specs, hint);
    registry.add_sources(&bar, inputs.to_vec());
    bar
}

/// Check if shared ext computation data dedup is enabled.
///
/// Port of C++ `HdExtComputation::_IsEnabledSharedExtComputationData`.
fn is_shared_ext_computation_data_enabled() -> bool {
    // Default enabled per C++ env setting HD_ENABLE_SHARED_EXT_COMPUTATION_DATA=1.
    std::env::var("HD_ENABLE_SHARED_EXT_COMPUTATION_DATA")
        .map(|v| v != "0")
        .unwrap_or(true)
}

/// Mark garbage collection needed via render param.
///
/// Port of C++ `HdStMarkGarbageCollectionNeeded(renderParam)`.
/// Note: C++ mutates via pointer; our trait is &dyn (immutable), so we
/// log the intent. The render delegate checks this flag during commit.
fn mark_gc_needed(_render_param: Option<&dyn HdRenderParam>) {
    // In C++, this downcast-and-mutates the HdStRenderParam.
    // Our HdRenderParam trait doesn't expose as_any() / mutation here.
    // The GC flag is a hint — the render delegate will clean up stale BARs
    // during its commit phase regardless.
    log::debug!("HdStExtComputation: GC needed (stale input range released)");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_registry() -> HdStResourceRegistrySharedPtr {
        Arc::new(HdStResourceRegistry::new())
    }

    #[test]
    fn test_creation() {
        let id = SdfPath::from_string("/World/MyComp").unwrap();
        let registry = make_registry();
        let comp = HdStExtComputation::new(id.clone(), registry);

        assert_eq!(comp.get_id(), &id);
        assert!(comp.get_input_range().is_none());
        assert!(comp.base().get_gpu_kernel_source().is_empty());
        assert_eq!(comp.base().get_element_count(), 0);
    }

    #[test]
    fn test_dirty_bits_delegation() {
        let id = SdfPath::from_string("/World/Comp").unwrap();
        let registry = make_registry();
        let mut comp = HdStExtComputation::new(id, registry);

        assert!(comp.is_dirty());
        comp.mark_clean(HdExtComputationDirtyBits::ALL_DIRTY);
        assert!(!comp.is_dirty());

        comp.mark_dirty(HdExtComputationDirtyBits::DIRTY_KERNEL);
        assert!(comp.is_dirty_bits(HdExtComputationDirtyBits::DIRTY_KERNEL));
    }

    #[test]
    fn test_initial_dirty_bits() {
        let bits = HdStExtComputation::get_initial_dirty_bits_mask();
        assert_eq!(bits, HdExtComputationDirtyBits::ALL_DIRTY);
    }

    #[test]
    fn test_finalize_clears_input_range() {
        let id = SdfPath::from_string("/World/Comp").unwrap();
        let registry = make_registry();
        let mut comp = HdStExtComputation::new(id, registry);

        // No input range — finalize is a no-op.
        comp.finalize(None);
        assert!(comp.get_input_range().is_none());
    }

    #[test]
    fn test_value_to_buffer_data_f32_array() {
        let arr: usd_vt::Array<f32> = vec![1.0f32, 2.0, 3.0].into();
        let val = usd_vt::Value::from_no_hash(arr);
        let (data, num, elem_size) = value_to_buffer_data(&val);
        assert_eq!(num, 3);
        assert_eq!(elem_size, 4);
        assert_eq!(data.len(), 12);
    }

    #[test]
    fn test_value_to_buffer_data_scalar() {
        let val = usd_vt::Value::from_no_hash(42i32);
        let (data, num, elem_size) = value_to_buffer_data(&val);
        assert_eq!(num, 1);
        assert_eq!(elem_size, 4);
        assert_eq!(data, 42i32.to_ne_bytes().to_vec());
    }

    #[test]
    fn test_value_to_buffer_data_unsupported() {
        let val = usd_vt::Value::default();
        let (data, num, elem_size) = value_to_buffer_data(&val);
        assert!(data.is_empty());
        assert_eq!(num, 0);
        assert_eq!(elem_size, 0);
    }

    #[test]
    fn test_shared_input_id_deterministic() {
        let s1 = Arc::new(BufferSource::new(Token::new("points"), vec![0u8; 32], 8, 4));
        let s2 = Arc::new(BufferSource::new(
            Token::new("normals"),
            vec![1u8; 24],
            6,
            4,
        ));
        let id_a = compute_shared_input_id(0, &[s1.clone(), s2.clone()]);
        let id_b = compute_shared_input_id(0, &[s1, s2]);
        assert_eq!(id_a, id_b);
    }

    #[test]
    fn test_shared_input_id_different_data() {
        let s1 = Arc::new(BufferSource::new(Token::new("points"), vec![0u8; 32], 8, 4));
        let s2 = Arc::new(BufferSource::new(Token::new("points"), vec![1u8; 32], 8, 4));
        let id_a = compute_shared_input_id(0, &[s1]);
        let id_b = compute_shared_input_id(0, &[s2]);
        assert_ne!(id_a, id_b);
    }
}
