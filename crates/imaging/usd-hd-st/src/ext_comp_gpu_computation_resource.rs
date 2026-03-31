#![allow(dead_code)]

//! GPU resource management for external computations.
//!
//! Holds the compiled compute program, resource bindings, and input buffer
//! ranges required to execute an ExtComputation on the GPU.
//!
//! Matches C++ `HdStExtCompGpuComputationResource`.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

use usd_hgi::HgiComputePipelineDesc;
use usd_hgi::compute_pipeline::{HgiComputePipelineHandle, HgiComputeShaderConstantsDesc};
use usd_hgi::enums::HgiShaderStage;
use usd_hgi::shader_function::HgiShaderFunctionDesc;
use usd_hgi::shader_program::HgiShaderProgramDesc;

use crate::ext_comp_compute_shader::ExtCompComputeShaderSharedPtr;
use crate::flat_normals::HdBufferSpec;
use crate::resource_binder::ResourceBinder;
use crate::resource_registry::{HdStResourceRegistry, ManagedBarSharedPtr};

// ---------------------------------------------------------------------------
// Computation resource
// ---------------------------------------------------------------------------

/// GPU resources for an external computation.
///
/// Holds everything needed to compile, bind, and execute a compute shader
/// for an ExtComputation:
/// - Output buffer specs (what the computation produces)
/// - Compiled compute shader / kernel
/// - Input BARs (ManagedBarSharedPtr slices into shared GPU buffers)
/// - Resource binder (layout mapping)
///
/// Resolve is lazy: `get_program()` triggers shader compilation on first call.
/// When HGI is absent (headless/test mode), only a deterministic hash is
/// computed — no GPU objects are created.
/// Lazy-resolved mutable state behind Mutex for Arc-safety.
struct ResolvedState {
    /// Resolved program handle: a deterministic hash over kernel + output specs + input BARs.
    /// 0 = not yet resolved.
    compute_program: u64,
    /// Resource binder matching the compute program layout
    resource_binder: ResourceBinder,
    /// Whether resolve() has been called successfully
    resolved: bool,
    /// Real HGI compute pipeline (Some when resolved with HGI)
    pipeline: Option<HgiComputePipelineHandle>,
}

pub struct ExtCompGpuComputationResource {
    /// Expected output buffer layout
    output_buffer_specs: Vec<HdBufferSpec>,
    /// The compute shader wrapping the ExtComputation kernel
    kernel: ExtCompComputeShaderSharedPtr,
    /// Input buffer ranges (GPU memory slices provided by the registry)
    inputs: Vec<ManagedBarSharedPtr>,
    /// Hash of the shader source for deduplication (kernel source only,
    /// per C++ reference — NOT the comp_id)
    shader_source_hash: u64,
    /// Lazy-resolved state (behind Mutex for Arc<Self> access)
    state: Mutex<ResolvedState>,
}

impl ExtCompGpuComputationResource {
    /// Create a new computation resource.
    ///
    /// - `output_buffer_specs`: buffer specs for the computation outputs
    /// - `kernel`: the compute shader code
    /// - `inputs`: ManagedBARs providing input data from the resource registry
    pub fn new(
        output_buffer_specs: Vec<HdBufferSpec>,
        kernel: ExtCompComputeShaderSharedPtr,
        inputs: Vec<ManagedBarSharedPtr>,
    ) -> Self {
        let shader_source_hash = kernel.compute_hash();

        Self {
            output_buffer_specs,
            kernel,
            inputs,
            shader_source_hash,
            state: Mutex::new(ResolvedState {
                compute_program: 0,
                resource_binder: ResourceBinder::new(),
                resolved: false,
                pipeline: None,
            }),
        }
    }

    /// Get the input buffer ranges.
    pub fn get_inputs(&self) -> &[ManagedBarSharedPtr] {
        &self.inputs
    }

    /// Get the compiled compute program handle.
    ///
    /// Triggers lazy resolution (without HGI) if not yet compiled.
    /// Returns the deterministic hash of kernel source + output specs.
    /// Takes `&self` (interior mutability via Mutex) for Arc<Self> access.
    pub fn get_program(&self) -> u64 {
        let mut st = self.state.lock().expect("resolve lock");
        if !st.resolved {
            Self::resolve_inner(
                &mut st,
                self.shader_source_hash,
                &self.output_buffer_specs,
                &self.inputs,
            );
        }
        st.compute_program
    }

    /// Resolve with an explicit registry reference.
    ///
    /// When HGI is available, creates a real compute pipeline:
    ///   1. Compile kernel source into HgiShaderFunction (compute stage)
    ///   2. Link into HgiShaderProgram
    ///   3. Create HgiComputePipeline with push constant layout
    /// When HGI is absent, falls back to hash-only mock resolution.
    ///
    /// Idempotent: subsequent calls return immediately.
    pub fn resolve_with_registry(&self, registry: &HdStResourceRegistry) -> bool {
        let mut st = self.state.lock().expect("resolve lock");
        if st.resolved {
            return true;
        }

        // Always compute the hash first (needed for both paths)
        Self::resolve_inner(
            &mut st,
            self.shader_source_hash,
            &self.output_buffer_specs,
            &self.inputs,
        );

        // If HGI is available, create real GPU pipeline objects
        if let Some(hgi) = registry.get_hgi() {
            let kernel_source = self.kernel.get_source(&usd_tf::Token::empty());
            if kernel_source.is_empty() {
                log::warn!(
                    "ExtCompGpuComputationResource: empty kernel source, skipping HGI pipeline"
                );
                return st.resolved;
            }

            // Compute uniform buffer size: matches layout built in execute()
            // [outputOffset, per-output (offset, stride), per-input (offset, compCount), dispatchCount]
            let ubo_entries = 1 + self.output_buffer_specs.len() * 2 + self.inputs.len() * 2 + 1;
            let ubo_byte_size = (ubo_entries * std::mem::size_of::<i32>()) as u32;

            let pipeline = hgi.with_write(|h| {
                // 1. Create shader function (compute stage)
                let fn_desc = HgiShaderFunctionDesc {
                    debug_name: format!("ExtComp_{:#x}", self.shader_source_hash),
                    shader_stage: HgiShaderStage::COMPUTE,
                    shader_code: kernel_source.to_string(),
                    entry_point: "main".to_string(),
                    ..Default::default()
                };
                let shader_fn = h.create_shader_function(&fn_desc);

                // 2. Create shader program
                let prog_desc = HgiShaderProgramDesc::new()
                    .with_debug_name(format!("ExtCompProgram_{:#x}", self.shader_source_hash))
                    .with_shader_function(shader_fn);
                let program = h.create_shader_program(&prog_desc);

                // 3. Create compute pipeline
                let pipe_desc = HgiComputePipelineDesc::new()
                    .with_debug_name(format!("ExtCompPipeline_{:#x}", self.shader_source_hash))
                    .with_shader_program(program)
                    .with_shader_constants(HgiComputeShaderConstantsDesc::new(ubo_byte_size));
                h.create_compute_pipeline(&pipe_desc)
            });
            log::debug!(
                "ExtCompGpuComputationResource: created HGI pipeline {:#x}",
                st.compute_program,
            );
            st.pipeline = Some(pipeline);
        }

        st.resolved
    }

    /// Get the resource binder (cloned, since internal state is behind Mutex).
    ///
    /// Triggers lazy resolution if not yet compiled.
    pub fn get_resource_binder(&self) -> ResourceBinder {
        let mut st = self.state.lock().expect("resolve lock");
        if !st.resolved {
            Self::resolve_inner(
                &mut st,
                self.shader_source_hash,
                &self.output_buffer_specs,
                &self.inputs,
            );
        }
        st.resource_binder.clone()
    }

    /// Get output buffer specs.
    pub fn get_output_buffer_specs(&self) -> &[HdBufferSpec] {
        &self.output_buffer_specs
    }

    /// Get the kernel shader.
    pub fn get_kernel(&self) -> &ExtCompComputeShaderSharedPtr {
        &self.kernel
    }

    /// Whether the resource has been resolved (program compiled).
    pub fn is_resolved(&self) -> bool {
        self.state.lock().expect("resolve lock").resolved
    }

    /// Get the shader source hash (kernel source hash, stable across calls).
    ///
    /// Used by ExtCompGpuComputation::execute() to key the pipeline cache
    /// (analogous to C++ which uses the raw program pointer as key).
    pub fn shader_source_hash(&self) -> u64 {
        self.shader_source_hash
    }

    /// Get the HGI compute pipeline handle, if resolved with HGI.
    ///
    /// Returns `None` in headless mode or if resolve hasn't been called.
    pub fn get_pipeline(&self) -> Option<HgiComputePipelineHandle> {
        self.state.lock().expect("resolve lock").pipeline.clone()
    }
    // ------------------------------------------------------------------
    // Internal resolution
    // ------------------------------------------------------------------

    /// Resolve: compile the compute program and set up resource bindings.
    ///
    /// Computes a combined hash over:
    ///   - kernel source hash (from `compute_hash()`, kernel source only)
    ///   - output buffer spec names + data types
    ///   - input BAR ids (for layout binding differentiation)
    ///
    /// Static method operating on `ResolvedState` to work with Mutex.
    fn resolve_inner(
        st: &mut ResolvedState,
        shader_source_hash: u64,
        output_buffer_specs: &[HdBufferSpec],
        inputs: &[ManagedBarSharedPtr],
    ) -> bool {
        if st.resolved {
            return true;
        }

        // Build combined hash: kernel source + output specs (name+type) + input BAR ids.
        let mut hasher = DefaultHasher::new();
        shader_source_hash.hash(&mut hasher);
        for spec in output_buffer_specs {
            spec.name.as_str().hash(&mut hasher);
            std::mem::discriminant(&spec.data_type).hash(&mut hasher);
        }
        // Include input BAR ids so different input sets don't hash-collide.
        for bar in inputs {
            if let Ok(locked) = bar.lock() {
                locked.id.hash(&mut hasher);
            }
        }
        st.compute_program = hasher.finish();

        st.resolved = true;

        log::debug!(
            "ExtCompGpuComputationResource::resolve: program={:#x}, {} inputs, {} outputs",
            st.compute_program,
            inputs.len(),
            output_buffer_specs.len(),
        );

        true
    }
}

impl std::fmt::Debug for ExtCompGpuComputationResource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let st = self.state.lock().expect("resolve lock");
        f.debug_struct("ExtCompGpuComputationResource")
            .field("resolved", &st.resolved)
            .field("program", &st.compute_program)
            .field("inputs", &self.inputs.len())
            .field("outputs", &self.output_buffer_specs.len())
            .finish()
    }
}

/// Shared pointer alias.
pub type ExtCompGpuComputationResourceSharedPtr = Arc<ExtCompGpuComputationResource>;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer_resource::HdStBufferResource;
    use crate::ext_comp_compute_shader::ExtCompComputeShader;
    use crate::resource_registry::ManagedBar;
    use std::sync::Mutex;
    use usd_hd::types::HdTupleType;
    use usd_hd::types::HdType;
    use usd_sdf::Path as SdfPath;
    use usd_tf::Token;

    /// Create a mock ManagedBarSharedPtr for tests (no real GPU backing).
    fn make_mock_bar(id: u64, name: &str) -> ManagedBarSharedPtr {
        // Build a buffer resource with default (null) handle — headless mode.
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

    fn make_resource() -> ExtCompGpuComputationResource {
        let kernel = Arc::new(ExtCompComputeShader::new(
            SdfPath::from_string("/comp").unwrap(),
            "void main() {}".to_string(),
        ));
        let input = make_mock_bar(1, "input");
        let output_specs = vec![HdBufferSpec {
            name: Token::new("result"),
            data_type: HdType::FloatVec3,
        }];

        ExtCompGpuComputationResource::new(output_specs, kernel, vec![input])
    }

    #[test]
    fn test_creation() {
        let resource = make_resource();
        assert!(!resource.is_resolved());
        assert_eq!(resource.get_inputs().len(), 1);
        assert_eq!(resource.get_output_buffer_specs().len(), 1);
    }

    #[test]
    fn test_lazy_resolve() {
        let resource = make_resource();
        assert!(!resource.is_resolved());

        let program = resource.get_program();
        assert!(resource.is_resolved());
        assert_ne!(program, 0);
    }

    #[test]
    fn test_resource_binder() {
        let resource = make_resource();
        let _binder = resource.get_resource_binder();
        assert!(resource.is_resolved());
    }

    #[test]
    fn test_deterministic_hash() {
        let r1 = make_resource();
        let r2 = make_resource();
        // Same kernel + same output specs + same input BAR ids => same program hash
        assert_eq!(r1.get_program(), r2.get_program());
    }

    #[test]
    fn test_different_inputs_different_hash() {
        let kernel = Arc::new(ExtCompComputeShader::new(
            SdfPath::from_string("/comp").unwrap(),
            "void main() {}".to_string(),
        ));
        let specs = vec![HdBufferSpec {
            name: Token::new("result"),
            data_type: HdType::FloatVec3,
        }];

        let r1 = ExtCompGpuComputationResource::new(
            specs.clone(),
            kernel.clone(),
            vec![make_mock_bar(1, "a")],
        );
        let r2 = ExtCompGpuComputationResource::new(
            specs,
            kernel,
            // Different BAR id => should produce a different combined hash
            vec![make_mock_bar(999, "a")],
        );
        assert_ne!(
            r1.get_program(),
            r2.get_program(),
            "different input BAR ids must yield different program hash"
        );
    }

    #[test]
    fn test_resolve_with_registry_headless() {
        let registry = HdStResourceRegistry::new();
        let resource = make_resource();
        assert!(!resource.is_resolved());
        let ok = resource.resolve_with_registry(&registry);
        assert!(ok);
        assert!(resource.is_resolved());
        assert_ne!(resource.get_program(), 0);
    }
}
