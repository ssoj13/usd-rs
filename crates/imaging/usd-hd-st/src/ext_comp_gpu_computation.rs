#![allow(dead_code)]

//! GPU-executed external computations for Storm.
//!
//! Implements the `input -> GPU compute -> output` model where:
//! 1. Input HdBufferSources are committed into input buffer ranges
//! 2. ExtCompGpuComputationResource holds the compiled shader + bindings
//! 3. This computation dispatches the kernel and writes results to output BAR
//!
//! Matches C++ `HdStExtCompGpuComputation`.

use std::collections::HashMap;
use std::sync::Arc;
use usd_hd::HdInterpolation;
use usd_hd::change_tracker::HdRprimDirtyBits;
use usd_hd::prim::HdSceneDelegate;
use usd_hd::scene_delegate::HdExtComputationPrimvarDescriptor;
use usd_hd::types::HdDirtyBits;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

use crate::ext_comp_gpu_computation_resource::{
    ExtCompGpuComputationResource, ExtCompGpuComputationResourceSharedPtr,
};
use crate::ext_comp_gpu_primvar_buffer::{
    ExtCompGpuPrimvarBufferSource, ExtCompGpuPrimvarBufferSourceSharedPtr,
};
use crate::flat_normals::HdBufferSpec;
use usd_hd::types::HdType;
use usd_hgi::enums::{HgiBindResourceType, HgiShaderStage};
use usd_hgi::resource_bindings::{HgiBufferBindDesc, HgiResourceBindingsDesc};

use crate::resource_registry::{
    BufferSourceSharedPtr, ComputeQueue, HdStResourceRegistry, ManagedBarSharedPtr,
};

// ---------------------------------------------------------------------------
// Primvar descriptor for ext computations
// ---------------------------------------------------------------------------

/// Describes a primvar produced by an external computation.
///
/// Maps a computation output to a primvar on the destination rprim.
#[derive(Debug, Clone)]
pub struct ExtComputationPrimvarDescriptor {
    /// Name of the primvar on the rprim
    pub name: Token,
    /// Source computation output name
    pub source_computation_output_name: Token,
    /// Source computation prim path
    pub source_computation_id: SdfPath,
    /// Value type
    pub value_type: HdType,
}

impl ExtComputationPrimvarDescriptor {
    /// Convert from the Hydra scene-delegate descriptor.
    ///
    /// Maps `HdTupleType.type_` directly (no lossy conversion).
    pub fn from_hd(desc: &HdExtComputationPrimvarDescriptor) -> Self {
        Self {
            name: desc.name.clone(),
            source_computation_output_name: desc.source_computation_output_name.clone(),
            source_computation_id: desc.source_computation_id.clone(),
            value_type: desc.value_type.type_,
        }
    }
}

// ---------------------------------------------------------------------------
// GPU ext computation
// ---------------------------------------------------------------------------

/// GPU-executed external computation.
///
/// Encapsulates the execution of a compute shader that reads from input
/// buffer array ranges and writes results into an output buffer array range.
///
/// The computation is driven by three companion objects:
/// - Input buffer sources (committed during Resolve phase)
/// - `ExtCompGpuComputationResource` (compiled shader + bindings)
/// - This struct (dispatches the kernel during Execute phase)
pub struct ExtCompGpuComputation {
    /// Computation prim path
    id: SdfPath,
    /// GPU resource holder (shader + bindings + inputs)
    resource: ExtCompGpuComputationResourceSharedPtr,
    /// Primvar descriptors for outputs
    comp_primvars: Vec<ExtComputationPrimvarDescriptor>,
    /// Number of GPU kernel invocations
    dispatch_count: i32,
    /// Number of output elements
    element_count: i32,
}

impl ExtCompGpuComputation {
    /// Create a new GPU ext computation.
    ///
    /// `id` - prim path of the source computation
    /// `resource` - GPU resources (shader, inputs, bindings)
    /// `comp_primvars` - primvar descriptors for outputs
    /// `dispatch_count` - number of kernel invocations
    /// `element_count` - number of output elements to allocate
    pub fn new(
        id: SdfPath,
        resource: ExtCompGpuComputationResourceSharedPtr,
        comp_primvars: Vec<ExtComputationPrimvarDescriptor>,
        dispatch_count: i32,
        element_count: i32,
    ) -> Self {
        Self {
            id,
            resource,
            comp_primvars,
            dispatch_count,
            element_count,
        }
    }

    /// Create a GPU computation from a source ExtComputation.
    ///
    /// This factory method:
    /// 1. Creates the compute shader from the source computation's kernel
    /// 2. Maps outputs onto primvar buffer specs
    /// 3. Creates the resource holder with the provided input BARs
    pub fn create_gpu_computation(
        comp_id: SdfPath,
        kernel_source: String,
        comp_primvars: Vec<ExtComputationPrimvarDescriptor>,
        inputs: Vec<ManagedBarSharedPtr>,
        dispatch_count: i32,
        element_count: i32,
    ) -> Arc<Self> {
        // Build output buffer specs from primvar descriptors
        let output_specs: Vec<HdBufferSpec> = comp_primvars
            .iter()
            .map(|pv| HdBufferSpec {
                name: pv.source_computation_output_name.clone(),
                data_type: pv.value_type,
            })
            .collect();

        // Create the compute shader
        let shader = Arc::new(crate::ext_comp_compute_shader::ExtCompComputeShader::new(
            comp_id.clone(),
            kernel_source,
        ));

        // Create the resource
        let resource = Arc::new(ExtCompGpuComputationResource::new(
            output_specs,
            shader,
            inputs,
        ));

        log::debug!(
            "ExtCompGpuComputation::create_gpu_computation: {} (dispatch={}, elements={})",
            comp_id,
            dispatch_count,
            element_count,
        );

        Arc::new(Self::new(
            comp_id,
            resource,
            comp_primvars,
            dispatch_count,
            element_count,
        ))
    }

    /// Get output buffer specs.
    ///
    /// GPU ext computations do not add buffer specs here because the
    /// output BAR is already allocated by the owning rprim.
    pub fn get_buffer_specs(&self) -> Vec<HdBufferSpec> {
        Vec::new()
    }

    /// Execute the computation on the GPU.
    ///
    /// Mirrors C++ `HdStExtCompGpuComputation::Execute()` (reference §5):
    /// 1. Build push-constant uniform buffer:
    ///    [outElementOffset, (outOffset/compSz, outStride/compSz) × N outputs,
    ///     (inByteOffset/compSz, compCount) × M inputs, dispatchCount]
    /// 2. Hash pipeline key: hash(programId, uboByteSize)
    /// 3. Hash resource-bindings key: hash(all buffer handle identifiers)
    /// 4. Register/reuse pipeline + resource bindings in registry caches
    /// 5. Encode via registry: BindResources → BindPipeline → SetConstantValues → Dispatch(count,1)
    ///
    /// Gracefully degrades when HGI is unavailable (headless / test mode):
    /// logs planned dispatch and returns without issuing GPU work.
    ///
    /// `output_bar` is the rprim's primvar BAR where compute results are written.
    /// In C++, this comes from the rprim's shared data via `SetOutputRange()`.
    pub fn execute(
        &self,
        output_bar: Option<&ManagedBarSharedPtr>,
        resource_registry: &HdStResourceRegistry,
    ) {
        // --- Build uniform buffer (push constants) per C++ reference §5.2 ---
        //
        // Layout (all int32):
        //   [0]             = outputBar.elementOffset  (0 for contiguous allocation)
        //   per output pv:  [offset/compSz, stride/compSz]
        //   per input bar:  [(byteOffset+bufOffset)/compSz, componentCount]
        //   [last]          = dispatchCount
        let mut uniforms: Vec<i32> = Vec::new();

        // Output element offset — from outputBar GPU layout
        let out_elem_offset = output_bar
            .and_then(|bar| bar.lock().ok())
            .map(|bar| bar.offset as i32)
            .unwrap_or(0);
        uniforms.push(out_elem_offset);

        // Per-output primvar: (byteOffset / componentSize, stride / componentSize)
        for pv in &self.comp_primvars {
            let comp_sz = component_size_of(pv.value_type);
            let elem_sz = element_byte_size(pv.value_type);
            let stride_comps = if comp_sz > 0 {
                (elem_sz / comp_sz) as i32
            } else {
                1
            };
            uniforms.push(0); // byte_offset / comp_size (contiguous allocation)
            uniforms.push(stride_comps);
        }

        // Per-input BAR: (flatOffset / compSz, componentCount)
        // Read real offset + component count from BAR GPU layout
        for input in self.resource.get_inputs() {
            if let Ok(bar) = input.lock() {
                let offset = bar.offset as i32;
                let comp_count = bar.element_size as i32 / 4; // bytes -> f32 components
                uniforms.push(offset);
                uniforms.push(comp_count.max(1));
            } else {
                uniforms.push(0);
                uniforms.push(3); // fallback: vec3
            }
        }

        // Final entry: dispatch count
        uniforms.push(self.dispatch_count);

        // Convert i32 uniforms to raw bytes for HGI push constants
        let uniform_bytes: Vec<u8> = uniforms.iter().flat_map(|v| v.to_le_bytes()).collect();

        // Trigger lazy resolution via registry (creates HGI pipeline when GPU available)
        let resolved = self.resource.resolve_with_registry(resource_registry);
        if !resolved {
            log::warn!(
                "ExtCompGpuComputation::execute: resolve failed for {}",
                self.id
            );
            return;
        }

        let program_id = self.resource.get_program();

        // --- Check HGI availability ---
        if !resource_registry.has_hgi() {
            log::debug!(
                "ExtCompGpuComputation::execute [no HGI]: {} dispatch={} ubo={} bytes program={:#x}",
                self.id,
                self.dispatch_count,
                uniform_bytes.len(),
                program_id,
            );
            return;
        }

        // --- Real GPU path ---
        // Get the HGI compute pipeline created during resolve
        let Some(pipeline) = self.resource.get_pipeline() else {
            log::warn!(
                "ExtCompGpuComputation::execute: no HGI pipeline for {} (resolve may have failed)",
                self.id,
            );
            return;
        };

        // Create HGI resource bindings from input + output BAR buffer handles
        let resource_bindings = match self.create_resource_bindings(output_bar, resource_registry) {
            Some(rb) => rb,
            None => {
                log::warn!(
                    "ExtCompGpuComputation::execute: failed to create resource bindings for {}",
                    self.id,
                );
                return;
            }
        };

        // Encode compute work via registry's global compute encoder
        resource_registry.encode_compute_dispatch(
            &pipeline,
            &resource_bindings,
            &uniform_bytes,
            self.dispatch_count as u32,
            &format!("ExtComputation:{}", self.id),
        );

        log::debug!(
            "ExtCompGpuComputation::execute: {} dispatch={} ubo={} bytes program={:#x}",
            self.id,
            self.dispatch_count,
            uniform_bytes.len(),
            program_id,
        );
    }

    /// Create HGI resource bindings for the computation's input/output buffers.
    ///
    /// Port of C++ ExtCompGpuComputation::Execute() binding setup:
    /// - Output BAR resources bound as writable SSBO (binding 0..N-1 per output primvar)
    /// - Input BAR resources bound as read-only SSBO (binding N..N+M-1)
    ///
    /// Binding layout matches the ResourceBinder convention: outputs first, inputs second.
    fn create_resource_bindings(
        &self,
        output_bar: Option<&ManagedBarSharedPtr>,
        registry: &HdStResourceRegistry,
    ) -> Option<usd_hgi::resource_bindings::HgiResourceBindingsHandle> {
        let hgi = registry.get_hgi()?;

        let mut buffer_bindings = Vec::new();
        let mut binding_idx = 0u32;

        // Output BAR: writable storage buffer (one binding per output primvar)
        // In C++, each output primvar's buffer resource is bound individually.
        // The output BAR is the rprim's primvar allocation that receives compute results.
        if let Some(out_bar) = output_bar {
            if let Ok(bar) = out_bar.lock() {
                let buf_handle = bar.buffer.get_handle().clone();
                // Bind once for the whole output BAR (covers all output primvars)
                buffer_bindings.push(HgiBufferBindDesc {
                    buffers: vec![buf_handle],
                    offsets: vec![0],
                    sizes: vec![0], // entire buffer
                    resource_type: HgiBindResourceType::StorageBuffer,
                    binding_index: binding_idx,
                    stage_usage: HgiShaderStage::COMPUTE,
                    writable: true,
                });
                binding_idx += 1;
            }
        } else {
            log::debug!(
                "ExtCompGpuComputation::create_resource_bindings: no output BAR for {}",
                self.id,
            );
        }

        // Input BARs: read-only storage buffers
        for input in self.resource.get_inputs() {
            if let Ok(bar) = input.lock() {
                let buf_handle = bar.buffer.get_handle().clone();
                buffer_bindings.push(HgiBufferBindDesc {
                    buffers: vec![buf_handle],
                    offsets: vec![0],
                    sizes: vec![0], // entire buffer
                    resource_type: HgiBindResourceType::StorageBuffer,
                    binding_index: binding_idx,
                    stage_usage: HgiShaderStage::COMPUTE,
                    writable: false,
                });
                binding_idx += 1;
            }
        }

        let rb_desc = HgiResourceBindingsDesc {
            debug_name: format!("ExtComp_RB:{}", self.id),
            buffer_bindings,
            texture_bindings: Vec::new(),
        };

        Some(hgi.with_write(|h| h.create_resource_bindings(&rb_desc)))
    }

    /// Get the number of GPU kernel invocations.
    pub fn get_dispatch_count(&self) -> i32 {
        self.dispatch_count
    }

    /// Get the number of output elements.
    pub fn get_num_output_elements(&self) -> i32 {
        self.element_count
    }

    /// Get the GPU resource holder.
    pub fn get_resource(&self) -> &ExtCompGpuComputationResourceSharedPtr {
        &self.resource
    }

    /// Get the computation prim path.
    pub fn get_id(&self) -> &SdfPath {
        &self.id
    }

    /// Get the primvar descriptors.
    pub fn get_comp_primvars(&self) -> &[ExtComputationPrimvarDescriptor] {
        &self.comp_primvars
    }
}

// ---------------------------------------------------------------------------
// Helpers for uniform layout computation
// ---------------------------------------------------------------------------

/// Byte size of one component for an HdType (e.g. 4 for FloatVec3 → sizeof(float)).
/// Delegates to `HdType::component_type().size_in_bytes()` for all 30 variants.
fn component_size_of(ty: HdType) -> usize {
    ty.component_type().size_in_bytes()
}

/// Total byte size of one element of an HdType.
/// Delegates to `HdType::size_in_bytes()` for all 30 variants.
fn element_byte_size(ty: HdType) -> usize {
    ty.size_in_bytes()
}

/// Collect input BARs for a GPU computation from sprim registry.
///
/// Port of C++ `_CreateGpuComputation` input gathering (extCompGpuComputation.cpp:304-372):
/// 1. Source computation's own input range (scene inputs SSBO)
/// 2. Upstream computation input ranges via GetComputationInputs() with dedup
fn collect_input_bars(
    comp_id: &SdfPath,
    delegate: &dyn HdSceneDelegate,
    registry: &HdStResourceRegistry,
) -> Vec<ManagedBarSharedPtr> {
    let mut bars: Vec<ManagedBarSharedPtr> = Vec::new();

    // 1. Source computation's own scene input BAR.
    if let Some(bar) = registry.get_ext_comp_input_bar(comp_id) {
        bars.push(bar);
    }

    // 2. Upstream computation input ranges (dedup by Arc pointer).
    let upstream_descs = delegate.get_ext_computation_input_descriptors(comp_id);
    for desc in &upstream_descs {
        if let Some(upstream_bar) = registry.get_ext_comp_input_bar(&desc.source_computation_id) {
            // Dedup: skip if already collected (same Arc).
            let dominated = bars
                .iter()
                .any(|existing| Arc::ptr_eq(existing, &upstream_bar));
            if !dominated {
                bars.push(upstream_bar);
            }
        }
    }

    if bars.is_empty() {
        log::debug!(
            "collect_input_bars: no input BARs for {} (sprims not synced yet?)",
            comp_id,
        );
    } else {
        log::debug!(
            "collect_input_bars: {} input BAR(s) for {}",
            bars.len(),
            comp_id,
        );
    }

    bars
}

// ---------------------------------------------------------------------------
// Orchestrator: get_ext_computation_primvars_computations()
// ---------------------------------------------------------------------------

/// Entry point for mesh sync to schedule ExtComputation primvar work.
///
/// Mirrors C++ `HdSt_GetExtComputationPrimvarsComputations()` (reference §6).
///
/// # Algorithm
/// 1. Collect all `HdExtComputationPrimvarDescriptor`s for this prim from the delegate
///    across all interpolation modes.
/// 2. Group descriptors by `source_computation_id`.
/// 3. For each group:
///    - Query kernel source from delegate.
///    - Non-empty kernel → **GPU path**: one `ExtCompGpuComputation` per source (shared
///      across all its dirty primvars) + one `ExtCompGpuPrimvarBufferSource` per dirty pv.
///    - Empty kernel → **CPU path**: placeholder `BufferSource` per dirty pv so the
///      BAR slot is reserved for CPU-computed data.
/// 4. All GPU computations go into `ComputeQueue::Queue0`.
///
/// # Parameters
/// - `prim_id` — rprim path for `is_primvar_dirty` checks
/// - `delegate` — scene delegate (queried for descriptors + kernel source)
/// - `dirty_bits` — current dirty flags of the prim
/// - `sources` — CPU upload queue (receives CPU-path placeholder sources)
/// - `reserve_only` — GPU reserve queue (receives GPU-path null buffer sources)
/// - `computations` — GPU computation queue (receives `(computation, queue)` pairs)
pub fn get_ext_computation_primvars_computations(
    prim_id: &SdfPath,
    delegate: &dyn HdSceneDelegate,
    dirty_bits: HdDirtyBits,
    resource_registry: &HdStResourceRegistry,
    sources: &mut Vec<BufferSourceSharedPtr>,
    reserve_only: &mut Vec<ExtCompGpuPrimvarBufferSourceSharedPtr>,
    computations: &mut Vec<(ExtCompGpuComputationSharedPtr, ComputeQueue)>,
) {
    // Early exit — nothing to do if primvars are not dirty.
    if dirty_bits & HdRprimDirtyBits::DIRTY_PRIMVAR == 0 {
        return;
    }

    // Collect all ext-computation primvar descriptors across interpolation modes.
    let all_interps = [
        HdInterpolation::Vertex,
        HdInterpolation::Varying,
        HdInterpolation::FaceVarying,
        HdInterpolation::Constant,
        HdInterpolation::Uniform,
        HdInterpolation::Instance,
    ];

    let all_comp_primvars: Vec<HdExtComputationPrimvarDescriptor> = all_interps
        .iter()
        .flat_map(|&interp| delegate.get_ext_computation_primvar_descriptors(prim_id, interp))
        .collect();

    if all_comp_primvars.is_empty() {
        return;
    }

    // --- Phase 1: group by source_computation_id ---
    let mut by_computation: HashMap<SdfPath, Vec<&HdExtComputationPrimvarDescriptor>> =
        HashMap::new();
    for desc in &all_comp_primvars {
        by_computation
            .entry(desc.source_computation_id.clone())
            .or_default()
            .push(desc);
    }

    // --- Phase 2: for each computation group, choose GPU or CPU path ---
    for (comp_id, group) in &by_computation {
        // Query GPU kernel source — non-empty means GPU path.
        let kernel_source = delegate.get_ext_computation_kernel(comp_id);

        if !kernel_source.is_empty() {
            // --- GPU path ---
            // One ExtCompGpuComputation per source computation, shared across primvars.
            let mut gpu_computation: Option<ExtCompGpuComputationSharedPtr> = None;

            for desc in group {
                if !HdRprimDirtyBits::is_primvar_dirty(dirty_bits, prim_id, &desc.name) {
                    continue;
                }

                // Create the GPU computation lazily (once per source computation).
                if gpu_computation.is_none() {
                    let local_primvars: Vec<ExtComputationPrimvarDescriptor> = group
                        .iter()
                        .map(|d| ExtComputationPrimvarDescriptor::from_hd(d))
                        .collect();

                    // Element/dispatch count: use `value_type.count` from the first
                    // descriptor as a proxy. In the full implementation this comes from
                    // the source sprim's GetElementCount() via the render index.
                    let element_count = desc.value_type.count.max(1) as i32;
                    let dispatch_count = element_count;

                    // Collect input BARs from HdStExtComputation sprims.
                    // Port of C++ _CreateGpuComputation (extCompGpuComputation.cpp:304-372):
                    //   1. Source computation's own input range (scene inputs SSBO)
                    //   2. Upstream computation input ranges (dedup by Arc pointer)
                    let input_bars = collect_input_bars(comp_id, delegate, resource_registry);

                    let comp = ExtCompGpuComputation::create_gpu_computation(
                        comp_id.clone(),
                        kernel_source.clone(),
                        local_primvars,
                        input_bars,
                        dispatch_count,
                        element_count,
                    );

                    computations.push((comp.clone(), ComputeQueue::Queue0));
                    gpu_computation = Some(comp);
                }

                // Reserve-only source: allocates the output BAR slot.
                // No CPU data is uploaded — the GPU kernel fills it.
                let elem_count = desc.value_type.count.max(1);
                let hd_type = desc.value_type.type_;
                if hd_type == HdType::Invalid {
                    log::warn!(
                        "get_ext_computation_primvars_computations: \
                         unsupported value_type for primvar '{}' on '{}' — skipping",
                        desc.name,
                        comp_id,
                    );
                    continue;
                }
                reserve_only.push(Arc::new(ExtCompGpuPrimvarBufferSource::new(
                    desc.name.clone(),
                    hd_type,
                    elem_count,
                    comp_id.clone(),
                )));
            }
        } else {
            // --- CPU path ---
            // Placeholder BufferSource per dirty primvar so the BAR slot is reserved.
            // The actual CPU computation resolves via ext_comp_cpu_computation and fills
            // data through resource_registry.add_sources() later in the commit phase.
            for desc in group {
                if !HdRprimDirtyBits::is_primvar_dirty(dirty_bits, prim_id, &desc.name) {
                    continue;
                }

                let elem_count = desc.value_type.count.max(1);
                let elem_size = desc.value_type.type_.size_in_bytes();

                // Empty data — CPU computation will overwrite via add_sources.
                sources.push(Arc::new(crate::resource_registry::BufferSource::new(
                    desc.name.clone(),
                    Vec::new(),
                    elem_count,
                    elem_size,
                )));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Debug
// ---------------------------------------------------------------------------

impl std::fmt::Debug for ExtCompGpuComputation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtCompGpuComputation")
            .field("id", &self.id)
            .field("dispatch_count", &self.dispatch_count)
            .field("element_count", &self.element_count)
            .field("primvars", &self.comp_primvars.len())
            .finish()
    }
}

/// Shared pointer alias.
pub type ExtCompGpuComputationSharedPtr = Arc<ExtCompGpuComputation>;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer_resource::HdStBufferResource;
    use crate::resource_registry::ManagedBar;
    use std::sync::Mutex;
    use usd_hd::types::HdTupleType;

    /// Build a minimal mock ManagedBarSharedPtr (no real GPU backing).
    fn make_mock_bar(id: u64, name: &str) -> ManagedBarSharedPtr {
        let buf = Arc::new(HdStBufferResource::new(
            Token::new(name),
            HdTupleType::default(),
            0,
            0,
        ));
        Arc::new(Mutex::new(ManagedBar {
            id,
            buffer: buf,
            role: Token::new(name),
            offset: 0,
            num_elements: 64,
            element_size: 4,
            version: 0,
            needs_realloc: false,
        }))
    }

    fn make_computation() -> Arc<ExtCompGpuComputation> {
        let primvars = vec![ExtComputationPrimvarDescriptor {
            name: Token::new("points"),
            source_computation_output_name: Token::new("outputPoints"),
            source_computation_id: SdfPath::from_string("/comp").unwrap(),
            value_type: HdType::FloatVec3,
        }];

        ExtCompGpuComputation::create_gpu_computation(
            SdfPath::from_string("/comp").unwrap(),
            "void main() {}".to_string(),
            primvars,
            vec![make_mock_bar(1, "inputPositions")],
            100, // dispatch
            100, // elements
        )
    }

    #[test]
    fn test_creation() {
        let comp = make_computation();
        assert_eq!(comp.get_dispatch_count(), 100);
        assert_eq!(comp.get_num_output_elements(), 100);
        assert_eq!(comp.get_comp_primvars().len(), 1);
    }

    #[test]
    fn test_buffer_specs_empty() {
        let comp = make_computation();
        // GPU ext comp doesn't add buffer specs (BAR allocated by rprim)
        assert!(comp.get_buffer_specs().is_empty());
    }

    #[test]
    fn test_execute_no_hgi() {
        let comp = make_computation();
        // Should log and return gracefully without panicking (no HGI)
        let registry = HdStResourceRegistry::new();
        comp.execute(None, &registry);
    }

    #[test]
    fn test_execute_with_output_bar() {
        let comp = make_computation();
        let registry = HdStResourceRegistry::new();
        let out_bar = make_mock_bar(42, "outputPoints");
        // Should log and return gracefully (no HGI), but exercise the output_bar path
        comp.execute(Some(&out_bar), &registry);
    }

    #[test]
    fn test_resource_access() {
        let comp = make_computation();
        let resource = comp.get_resource();
        assert_eq!(resource.get_inputs().len(), 1);
        assert_eq!(resource.get_output_buffer_specs().len(), 1);
    }

    #[test]
    fn test_id() {
        let comp = make_computation();
        assert_eq!(comp.get_id(), &SdfPath::from_string("/comp").unwrap());
    }

    #[test]
    fn test_component_size() {
        assert_eq!(component_size_of(HdType::FloatVec3), 4);
        assert_eq!(component_size_of(HdType::DoubleVec3), 8);
        assert_eq!(component_size_of(HdType::Invalid), 0);
    }

    #[test]
    fn test_element_byte_size() {
        assert_eq!(element_byte_size(HdType::FloatVec3), 12);
        assert_eq!(element_byte_size(HdType::DoubleVec3), 24);
    }

    #[test]
    fn test_orchestrator_no_dirty() {
        // When DIRTY_PRIMVAR is not set, orchestrator is a no-op.
        struct NullDelegate;
        impl HdSceneDelegate for NullDelegate {
            fn get_dirty_bits(&self, _: &SdfPath) -> HdDirtyBits {
                0
            }
            fn mark_clean(&mut self, _: &SdfPath, _: HdDirtyBits) {}
            fn get_delegate_id(&self) -> SdfPath {
                SdfPath::default()
            }
        }

        let delegate = NullDelegate;
        let mut sources = Vec::new();
        let mut reserve_only = Vec::new();
        let mut computations = Vec::new();

        let registry = HdStResourceRegistry::new();
        get_ext_computation_primvars_computations(
            &SdfPath::from_string("/mesh").unwrap(),
            &delegate,
            0, // no dirty bits
            &registry,
            &mut sources,
            &mut reserve_only,
            &mut computations,
        );

        assert!(sources.is_empty());
        assert!(reserve_only.is_empty());
        assert!(computations.is_empty());
    }

    #[test]
    fn test_orchestrator_empty_descriptors() {
        // When delegate returns no ext-comp primvars, orchestrator is a no-op.
        struct NullDelegate;
        impl HdSceneDelegate for NullDelegate {
            fn get_dirty_bits(&self, _: &SdfPath) -> HdDirtyBits {
                0
            }
            fn mark_clean(&mut self, _: &SdfPath, _: HdDirtyBits) {}
            fn get_delegate_id(&self) -> SdfPath {
                SdfPath::default()
            }
            // get_ext_computation_primvar_descriptors uses trait default → returns empty
        }

        let delegate = NullDelegate;
        let registry = HdStResourceRegistry::new();
        let mut sources = Vec::new();
        let mut reserve_only = Vec::new();
        let mut computations = Vec::new();

        get_ext_computation_primvars_computations(
            &SdfPath::from_string("/mesh").unwrap(),
            &delegate,
            HdRprimDirtyBits::DIRTY_PRIMVAR,
            &registry,
            &mut sources,
            &mut reserve_only,
            &mut computations,
        );

        assert!(sources.is_empty());
        assert!(reserve_only.is_empty());
        assert!(computations.is_empty());
    }

    // ------------------------------------------------------------------
    // Level 2.5: collect_input_bars wiring tests
    // ------------------------------------------------------------------

    #[test]
    fn test_collect_input_bars_empty_registry() {
        // No sprims registered — should return empty.
        struct NullDelegate;
        impl HdSceneDelegate for NullDelegate {
            fn get_dirty_bits(&self, _: &SdfPath) -> HdDirtyBits {
                0
            }
            fn mark_clean(&mut self, _: &SdfPath, _: HdDirtyBits) {}
            fn get_delegate_id(&self) -> SdfPath {
                SdfPath::default()
            }
        }
        let registry = HdStResourceRegistry::new();
        let bars = collect_input_bars(
            &SdfPath::from_string("/comp").unwrap(),
            &NullDelegate,
            &registry,
        );
        assert!(bars.is_empty());
    }

    #[test]
    fn test_collect_input_bars_own_range() {
        // Register source comp's BAR — should appear as first input.
        struct NullDelegate;
        impl HdSceneDelegate for NullDelegate {
            fn get_dirty_bits(&self, _: &SdfPath) -> HdDirtyBits {
                0
            }
            fn mark_clean(&mut self, _: &SdfPath, _: HdDirtyBits) {}
            fn get_delegate_id(&self) -> SdfPath {
                SdfPath::default()
            }
        }
        let registry = HdStResourceRegistry::new();
        let comp_path = SdfPath::from_string("/comp").unwrap();
        let bar = make_mock_bar(1, "sceneInput");
        registry.register_ext_comp_input_bar(&comp_path, bar.clone());

        let bars = collect_input_bars(&comp_path, &NullDelegate, &registry);
        assert_eq!(bars.len(), 1);
        assert!(Arc::ptr_eq(&bars[0], &bar));
    }

    #[test]
    fn test_collect_input_bars_with_upstream() {
        use usd_hd::scene_delegate::{
            HdExtComputationInputDescriptor, HdExtComputationInputDescriptorVector,
        };

        let comp_path = SdfPath::from_string("/comp").unwrap();
        let upstream_path = SdfPath::from_string("/upstream").unwrap();

        // Delegate returns one upstream dependency.
        struct UpstreamDelegate {
            upstream: SdfPath,
        }
        impl HdSceneDelegate for UpstreamDelegate {
            fn get_dirty_bits(&self, _: &SdfPath) -> HdDirtyBits {
                0
            }
            fn mark_clean(&mut self, _: &SdfPath, _: HdDirtyBits) {}
            fn get_delegate_id(&self) -> SdfPath {
                SdfPath::default()
            }
            fn get_ext_computation_input_descriptors(
                &self,
                _id: &SdfPath,
            ) -> HdExtComputationInputDescriptorVector {
                vec![HdExtComputationInputDescriptor {
                    name: Token::new("upstreamOutput"),
                    source_computation_id: self.upstream.clone(),
                    source_computation_output_name: Token::new("result"),
                }]
            }
        }

        let registry = HdStResourceRegistry::new();
        let own_bar = make_mock_bar(1, "ownInput");
        let upstream_bar = make_mock_bar(2, "upstreamInput");
        registry.register_ext_comp_input_bar(&comp_path, own_bar.clone());
        registry.register_ext_comp_input_bar(&upstream_path, upstream_bar.clone());

        let delegate = UpstreamDelegate {
            upstream: upstream_path,
        };
        let bars = collect_input_bars(&comp_path, &delegate, &registry);

        assert_eq!(bars.len(), 2);
        assert!(Arc::ptr_eq(&bars[0], &own_bar));
        assert!(Arc::ptr_eq(&bars[1], &upstream_bar));
    }

    #[test]
    fn test_collect_input_bars_dedup() {
        use usd_hd::scene_delegate::{
            HdExtComputationInputDescriptor, HdExtComputationInputDescriptorVector,
        };

        let comp_path = SdfPath::from_string("/comp").unwrap();

        // Delegate returns upstream pointing to SAME comp (self-reference).
        struct SelfRefDelegate {
            self_path: SdfPath,
        }
        impl HdSceneDelegate for SelfRefDelegate {
            fn get_dirty_bits(&self, _: &SdfPath) -> HdDirtyBits {
                0
            }
            fn mark_clean(&mut self, _: &SdfPath, _: HdDirtyBits) {}
            fn get_delegate_id(&self) -> SdfPath {
                SdfPath::default()
            }
            fn get_ext_computation_input_descriptors(
                &self,
                _id: &SdfPath,
            ) -> HdExtComputationInputDescriptorVector {
                vec![HdExtComputationInputDescriptor {
                    name: Token::new("selfInput"),
                    source_computation_id: self.self_path.clone(),
                    source_computation_output_name: Token::new("out"),
                }]
            }
        }

        let registry = HdStResourceRegistry::new();
        let bar = make_mock_bar(1, "input");
        registry.register_ext_comp_input_bar(&comp_path, bar.clone());

        let delegate = SelfRefDelegate {
            self_path: comp_path.clone(),
        };
        let bars = collect_input_bars(&comp_path, &delegate, &registry);

        // Dedup: same Arc should NOT appear twice.
        assert_eq!(bars.len(), 1);
        assert!(Arc::ptr_eq(&bars[0], &bar));
    }

    #[test]
    fn test_register_remove_input_bar() {
        let registry = HdStResourceRegistry::new();
        let path = SdfPath::from_string("/comp").unwrap();
        let bar = make_mock_bar(1, "input");

        assert!(registry.get_ext_comp_input_bar(&path).is_none());
        registry.register_ext_comp_input_bar(&path, bar.clone());
        assert!(registry.get_ext_comp_input_bar(&path).is_some());
        registry.remove_ext_comp_input_bar(&path);
        assert!(registry.get_ext_comp_input_bar(&path).is_none());
    }

    // ------------------------------------------------------------------
    // Level 3: raw wgpu compute dispatch smoke-test
    // ------------------------------------------------------------------

    /// Validates that a wgpu compute shader dispatches correctly at the GPU level.
    ///
    /// Uses raw wgpu — not the full HGI/ExtComputation stack — so this test
    /// runs even without a full HgiWgpu context.  Skipped gracefully when no
    /// hardware GPU adapter is available (typical in headless CI).
    ///
    /// Shader: output[i] = input[i] + 1.0  for i in 0..4
    #[cfg(feature = "gpu-culling")]
    #[test]
    fn test_gpu_compute_dispatch_add_one() {
        use wgpu::util::DeviceExt;

        // Try to acquire a GPU adapter; skip the test if none is present.
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::None,
            compatible_surface: None,
            force_fallback_adapter: false,
        }));

        // wgpu 27: request_adapter returns Result, not Option.
        let adapter = match adapter {
            Ok(a) => a,
            Err(e) => {
                // No GPU in this environment — skip without failing.
                eprintln!("[test_gpu_compute_dispatch] no adapter found ({e}), skipping");
                return;
            }
        };

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("test_device"),
            ..Default::default()
        }))
        .expect("request_device failed");

        // Input data: [1.0, 2.0, 3.0, 4.0]
        let input_data: [f32; 4] = [1.0, 2.0, 3.0, 4.0];
        let input_bytes = bytemuck_cast(&input_data);

        let input_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("input"),
            contents: input_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        // Output buffer: same size, read-back enabled.
        let output_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("output"),
            size: (std::mem::size_of::<f32>() * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (std::mem::size_of::<f32>() * 4) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Compute shader: output[i] = input[i] + 1.0
        let shader_src = r#"
            @group(0) @binding(0) var<storage, read>       input_data  : array<f32>;
            @group(0) @binding(1) var<storage, read_write> output_data : array<f32>;

            @compute @workgroup_size(64)
            fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
                let idx = gid.x;
                if (idx < 4u) {
                    output_data[idx] = input_data[idx] + 1.0;
                }
            }
        "#;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("add_one"),
            source: wgpu::ShaderSource::Wgsl(shader_src.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pl"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("add_one_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bg"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: input_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: output_buf.as_entire_binding(),
                },
            ],
        });

        // Encode and submit compute + copy.
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("enc") });
        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("cpass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            cpass.dispatch_workgroups(1, 1, 1); // 1 workgroup of 64 threads covers 4 elements
        }
        encoder.copy_buffer_to_buffer(
            &output_buf,
            0,
            &readback_buf,
            0,
            (std::mem::size_of::<f32>() * 4) as u64,
        );
        queue.submit(std::iter::once(encoder.finish()));

        // Map the readback buffer and verify results.
        let slice = readback_buf.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        rx.recv().unwrap().expect("map_async failed");

        let data = slice.get_mapped_range();
        let result: &[f32] = bytemuck_cast_slice(&data);
        assert_eq!(
            result,
            &[2.0f32, 3.0, 4.0, 5.0],
            "output must be input + 1.0"
        );
    }

    /// Cast a typed slice to its raw byte representation without pulling in bytemuck.
    #[cfg(feature = "gpu-culling")]
    fn bytemuck_cast(data: &[f32; 4]) -> &[u8] {
        // SAFETY: f32 has no padding, alignment is compatible, size is exact.
        unsafe {
            std::slice::from_raw_parts(data.as_ptr() as *const u8, std::mem::size_of_val(data))
        }
    }

    /// Cast a raw byte slice to &[f32] for result comparison.
    #[cfg(feature = "gpu-culling")]
    fn bytemuck_cast_slice(data: &[u8]) -> &[f32] {
        assert_eq!(data.len() % std::mem::size_of::<f32>(), 0);
        // SAFETY: bytes are aligned and sized correctly for f32.
        unsafe {
            std::slice::from_raw_parts(
                data.as_ptr() as *const f32,
                data.len() / std::mem::size_of::<f32>(),
            )
        }
    }
}
