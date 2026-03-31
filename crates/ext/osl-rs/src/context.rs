//! ShadingContext + PerThreadInfo — per-execution and per-thread state.
//!
//! Port of `shadingcontext.h` / `oslexec_pvt.h` / `shading_state_uniform.h`.
//! The ShadingContext holds all per-point execution state and the
//! PerThreadInfo manages thread-local resources.
//!
//! # Why no ClosureArena?
//!
//! The C++ OSL allocates closures from a per-thread bump arena
//! (`ShadingContext::closure_arena`). In this Rust port, closures are
//! `Arc<ClosureNode>` — heap-allocated, reference-counted values that drop
//! automatically. The arena is no longer needed, removing a major source
//! of `unsafe` code. See [`crate::closure`] module docs for the full
//! rationale.
//!
//! The `ci` field (closure output) is now `Option<ClosureRef>` instead of
//! `*const ClosureColor`, making it impossible to dereference a dangling
//! or null pointer.

use crate::Float;
use crate::closure::ClosureRef;
use crate::color::ColorSystem;
use crate::dict::DictStore;
use crate::math::{Color3, Vec3};
use crate::message::MessageStore;
use crate::renderer::{AttributeData, RendererServices, TextureHandle, TraceOpt};
use crate::shaderglobals::ShaderGlobals;
use crate::shadingsys::{ErrorHandler, ShaderGroup};
use crate::typedesc::TypeDesc;
use crate::ustring::{UString, UStringHash};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// ShadingStateUniform — matches C++ `pvt::ShadingStateUniform`
// ---------------------------------------------------------------------------

/// Uniform (shared) shading state, matching `pvt::ShadingStateUniform`
/// from `shading_state_uniform.h`.
///
/// Contains the color system, common-space synonym, and error settings.
/// Designed to be serializable to GPU device buffers.
pub struct ShadingStateUniform {
    /// The active color system (primaries + blackbody table).
    pub color_system: ColorSystem,
    /// Synonym for "common" space (e.g., "world").
    pub commonspace_synonym: UString,
    /// Whether to error on unknown coordinate systems.
    pub unknown_coordsys_error: bool,
    /// Max warnings per thread before suppression.
    pub max_warnings_per_thread: i32,
}

impl Default for ShadingStateUniform {
    fn default() -> Self {
        Self {
            color_system: ColorSystem::rec709(),
            commonspace_synonym: UString::new("world"),
            unknown_coordsys_error: true,
            max_warnings_per_thread: 100,
        }
    }
}

/// Per-thread execution data.
///
/// Each thread has its own `PerThreadInfo` to avoid locking during shader
/// execution. This matches the C++ `pvt::PerThreadInfo`.
///
/// The C++ version also held a `ClosureArena` here. Since closures are now
/// `Arc<ClosureNode>`, thread-local arena management is no longer needed.
pub struct PerThreadInfo {
    /// Message store for this thread (used by `setmessage`/`getmessage`).
    pub messages: MessageStore,
    /// Thread index (for diagnostics and stats).
    pub thread_index: usize,
}

impl PerThreadInfo {
    pub fn new(thread_index: usize) -> Self {
        Self {
            messages: MessageStore::new(),
            thread_index,
        }
    }

    /// Reset thread-local state between shading points.
    pub fn reset(&mut self) {
        self.messages.clear();
    }
}

/// Execution context for a single shading operation.
///
/// Matches C++ `ShadingContext` from `shadingcontext.h`. Holds per-point
/// state: the shader globals, the output closure/color, heap memory for
/// locals, and bookkeeping for layer execution order.
///
/// # Closure output
///
/// The `ci` field stores the resulting closure tree. It is
/// `Option<ClosureRef>` — `None` means no closure output (equivalent to
/// a null pointer in C++). This is fully safe: no raw pointers, no
/// dangling references.
pub struct ShadingContext {
    /// The shader globals for this point.
    pub globals: ShaderGlobals,
    /// Per-thread info (owned by the context).
    pub thread_info: PerThreadInfo,
    /// The resulting closure tree (output).
    ///
    /// `None` = no closure output (non-closure shaders, or shader hasn't
    /// assigned to Ci yet). Replaces the old `*const ClosureColor`.
    pub ci: Option<ClosureRef>,
    /// The output color (for non-closure shaders).
    pub output_color: Color3,
    /// Whether this is the execution of a displacement shader.
    pub is_displacement: bool,
    /// Execution depth (for nested shader calls).
    pub depth: u32,
    /// Max allowed execution depth.
    pub max_depth: u32,
    /// Max warnings per thread before suppression (matches C++ m_max_warnings).
    pub max_warnings: i32,
    /// Whether execution has been aborted.
    pub aborted: bool,
    /// Heap memory for symbol data (per-point temporaries and locals).
    /// Matches C++ `m_heap`.
    pub heap: Vec<u8>,
    /// Current offset into the heap for allocation.
    /// Matches C++ `m_closure_pool` (but no longer used for closures).
    pub closure_pool_offset: usize,
    /// Group data area for the current shader group.
    /// Matches C++ `m_group_data`.
    pub group_data: Vec<u8>,
    /// Array of flags indicating which layers have been executed.
    /// Matches C++ `m_executed`.
    pub executed_layers: Vec<bool>,
    /// Renderer output names requested for this execution.
    pub renderer_outputs: Vec<UString>,
    /// Current group being executed (for debug messages).
    pub current_group_name: UString,
    /// Number of texture calls (for stats/limits).
    pub texture_calls: u32,
    /// Number of trace calls (for stats/limits).
    pub trace_calls: u32,
    /// Number of closure allocations (for stats).
    pub closure_calls: u32,
    /// Accumulated runtime error messages.
    pub error_messages: Vec<String>,
    /// Accumulated runtime warning messages.
    pub warning_messages: Vec<String>,
    /// Renderer services reference (set during execution).
    pub renderer: Option<Arc<dyn RendererServices>>,
    /// Dictionary store for dict_find/dict_value/dict_next.
    pub dict_store: DictStore,
    /// Current shader group being executed.
    pub group: Option<Arc<ShaderGroup>>,
    /// Number of layers actually executed (stats).
    pub layers_executed: u64,
    /// Batch size for batched/SIMD execution (1 = scalar).
    pub batch_size: usize,
    /// Texture options for batched lookup (wrap, filter, MIP).
    /// Matches C++ TextureOpt, used when available by texture backend.
    texture_options: BatchedTextureOptions,
}

impl ShadingContext {
    /// Default heap size (64KB, matching C++ default).
    pub const DEFAULT_HEAP_SIZE: usize = 64 * 1024;

    pub fn new(thread_index: usize) -> Self {
        Self {
            globals: ShaderGlobals::default(),
            thread_info: PerThreadInfo::new(thread_index),
            ci: None,
            output_color: Color3::ZERO,
            is_displacement: false,
            depth: 0,
            max_depth: 16,
            max_warnings: 100,
            aborted: false,
            heap: vec![0u8; Self::DEFAULT_HEAP_SIZE],
            closure_pool_offset: 0,
            group_data: Vec::new(),
            executed_layers: Vec::new(),
            renderer_outputs: Vec::new(),
            current_group_name: UString::empty(),
            texture_calls: 0,
            trace_calls: 0,
            closure_calls: 0,
            error_messages: Vec::new(),
            warning_messages: Vec::new(),
            renderer: None,
            dict_store: DictStore::new(),
            group: None,
            layers_executed: 0,
            batch_size: 1,
            texture_options: BatchedTextureOptions::default(),
        }
    }

    /// Allocate `nbytes` from the heap, returning the offset.
    /// Matches C++ `alloc_heap`.
    pub fn alloc_heap(&mut self, nbytes: usize) -> usize {
        let offset = self.closure_pool_offset;
        self.closure_pool_offset += nbytes;
        if self.closure_pool_offset > self.heap.len() {
            self.heap.resize(self.closure_pool_offset, 0);
        }
        offset
    }

    /// Get a mutable slice into the heap at the given offset.
    pub fn heap_slice_mut(&mut self, offset: usize, size: usize) -> &mut [u8] {
        &mut self.heap[offset..offset + size]
    }

    /// Prepare to execute a group with N layers.
    /// Matches C++ `ShadingContext::execute_init`.
    pub fn prepare_execution(&mut self, num_layers: usize) {
        self.executed_layers.clear();
        self.executed_layers.resize(num_layers, false);
        self.closure_pool_offset = 0;
        self.texture_calls = 0;
        self.trace_calls = 0;
        self.closure_calls = 0;
        self.ci = None;
        self.output_color = Color3::ZERO;
        self.depth = 0;
        self.aborted = false;
    }

    /// Mark a layer as executed.
    pub fn mark_layer_executed(&mut self, layer: usize) {
        if layer < self.executed_layers.len() {
            self.executed_layers[layer] = true;
        }
    }

    /// Check if a layer has been executed.
    pub fn is_layer_executed(&self, layer: usize) -> bool {
        layer < self.executed_layers.len() && self.executed_layers[layer]
    }

    /// Get the shader globals.
    pub fn shader_globals(&self) -> &ShaderGlobals {
        &self.globals
    }

    /// Get mutable shader globals.
    pub fn shader_globals_mut(&mut self) -> &mut ShaderGlobals {
        &mut self.globals
    }

    /// Set the output closure.
    ///
    /// Replaces whatever closure tree was previously stored. The old tree
    /// is dropped automatically when its last `Arc` reference goes away.
    pub fn set_closure(&mut self, ci: Option<ClosureRef>) {
        self.ci = ci;
    }

    /// Get a reference to the output closure, if any.
    pub fn get_closure(&self) -> Option<&ClosureRef> {
        self.ci.as_ref()
    }

    /// Check if we've exceeded the max execution depth.
    pub fn check_depth(&self) -> bool {
        self.depth < self.max_depth
    }

    /// Execute the init block for parameter defaults.
    /// Matches C++ `ShadingContext::execute_init`.
    pub fn execute_init(&mut self, num_layers: usize, renderer: Arc<dyn RendererServices>) {
        self.prepare_execution(num_layers);
        self.renderer = Some(renderer);
        self.error_messages.clear();
        self.warning_messages.clear();
    }

    /// Execute a single shader layer by index.
    /// Matches C++ `ShadingContext::execute_layer`.
    pub fn execute_layer(&mut self, layer_index: usize) -> bool {
        if self.aborted {
            return false;
        }
        if self.is_layer_executed(layer_index) {
            return true;
        }
        self.mark_layer_executed(layer_index);
        true
    }

    /// Raw access to symbol heap data at byte offset.
    /// Matches C++ `ShadingContext::symbol_data`.
    pub fn symbol_data(&self, offset: usize, size: usize) -> &[u8] {
        let end = (offset + size).min(self.heap.len());
        &self.heap[offset..end]
    }

    /// Record a runtime error message.
    pub fn record_error(&mut self, msg: &str) {
        self.error_messages.push(msg.to_string());
    }

    /// Record a runtime warning message.
    /// Matches C++ behavior: suppresses warnings after max_warnings reached.
    pub fn record_warning(&mut self, msg: &str) {
        if self.max_warnings > 0 {
            self.warning_messages.push(msg.to_string());
            self.max_warnings -= 1;
        }
    }

    /// Get a reference to the renderer services.
    /// Returns `None` if no renderer is bound (execution not started).
    pub fn renderer_services(&self) -> Option<&dyn RendererServices> {
        self.renderer.as_deref()
    }

    /// Get batched texture lookup options.
    /// Matches C++ `ShadingContext::batched_texture_options()` / `TextureOpt`.
    /// Returns per-context options if set, otherwise defaults (bilinear, default wrap).
    /// Use `set_batched_texture_options()` to configure before execution.
    pub fn batched_texture_options(&self) -> BatchedTextureOptions {
        self.texture_options.clone()
    }

    /// Set batched texture lookup options (wrap, filter, MIP mode).
    /// Matches C++ TextureOpt configuration.
    pub fn set_batched_texture_options(&mut self, opt: BatchedTextureOptions) {
        self.texture_options = opt;
    }

    /// Set a message in the per-point message store.
    pub fn setmessage(&mut self, name: UString, val: crate::message::MessageValue) {
        self.thread_info.messages.setmessage(name, val);
    }

    /// Get a message from the per-point message store.
    pub fn getmessage(&self, source: &str, name: UString) -> Option<&crate::message::MessageValue> {
        self.thread_info.messages.getmessage(source, name)
    }

    // -- Texture dispatch (delegates to RendererServices) -------------------

    /// 2D texture lookup, dispatched to the bound renderer.
    /// Matches C++ `ShadingContext::osl_texture`.
    pub fn texture(
        &mut self,
        filename: UStringHash,
        handle: TextureHandle,
        s: Float,
        t: Float,
        dsdx: Float,
        dtdx: Float,
        dsdy: Float,
        dtdy: Float,
        nchannels: i32,
        result: &mut [Float],
        dresultds: Option<&mut [Float]>,
        dresultdt: Option<&mut [Float]>,
    ) -> Result<(), String> {
        self.texture_calls += 1;
        let renderer = self
            .renderer
            .as_ref()
            .ok_or_else(|| "no renderer bound".to_string())?;
        let opt = self.batched_texture_options().to_texture_opt();
        renderer.texture(
            filename,
            handle,
            &self.globals,
            &opt,
            s,
            t,
            dsdx,
            dtdx,
            dsdy,
            dtdy,
            nchannels,
            result,
            dresultds,
            dresultdt,
        )
    }

    /// 3D texture lookup, dispatched to the bound renderer.
    /// Matches C++ `ShadingContext::osl_texture3d`.
    pub fn texture3d(
        &mut self,
        filename: UStringHash,
        handle: TextureHandle,
        p: &Vec3,
        dpdx: &Vec3,
        dpdy: &Vec3,
        dpdz: &Vec3,
        nchannels: i32,
        result: &mut [Float],
        dresultds: Option<&mut [Float]>,
        dresultdt: Option<&mut [Float]>,
        dresultdr: Option<&mut [Float]>,
    ) -> Result<(), String> {
        self.texture_calls += 1;
        let renderer = self
            .renderer
            .as_ref()
            .ok_or_else(|| "no renderer bound".to_string())?;
        let opt = self.batched_texture_options().to_texture_opt();
        renderer.texture3d(
            filename,
            handle,
            &self.globals,
            &opt,
            p,
            dpdx,
            dpdy,
            dpdz,
            nchannels,
            result,
            dresultds,
            dresultdt,
            dresultdr,
        )
    }

    /// Environment map lookup, dispatched to the bound renderer.
    /// Matches C++ `ShadingContext::osl_environment`.
    pub fn environment(
        &mut self,
        filename: UStringHash,
        handle: TextureHandle,
        r: &Vec3,
        drdx: &Vec3,
        drdy: &Vec3,
        nchannels: i32,
        result: &mut [Float],
        dresultds: Option<&mut [Float]>,
        dresultdt: Option<&mut [Float]>,
    ) -> Result<(), String> {
        self.texture_calls += 1;
        let renderer = self
            .renderer
            .as_ref()
            .ok_or_else(|| "no renderer bound".to_string())?;
        let opt = self.batched_texture_options().to_texture_opt();
        renderer.environment(
            filename,
            handle,
            &self.globals,
            &opt,
            r,
            drdx,
            drdy,
            nchannels,
            result,
            dresultds,
            dresultdt,
        )
    }

    // -- Attribute dispatch -------------------------------------------------

    /// Query a named attribute from the renderer.
    /// Matches C++ `ShadingContext::osl_get_attribute`.
    pub fn getattribute(
        &self,
        derivatives: bool,
        object: UStringHash,
        type_desc: TypeDesc,
        name: UStringHash,
    ) -> Option<AttributeData> {
        self.renderer
            .as_ref()?
            .get_attribute(&self.globals, derivatives, object, type_desc, name)
    }

    /// Query a named array attribute from the renderer at the given index.
    /// Matches C++ `ShadingContext::osl_get_attribute` with array_lookup=1.
    pub fn getattribute_array(
        &self,
        derivatives: bool,
        object: UStringHash,
        type_desc: TypeDesc,
        name: UStringHash,
        index: i32,
    ) -> Option<AttributeData> {
        self.renderer.as_ref()?.get_array_attribute(
            &self.globals,
            derivatives,
            object,
            type_desc,
            name,
            index,
        )
    }

    // -- Trace dispatch -----------------------------------------------------

    /// Trace a ray via the bound renderer.
    /// Matches C++ `ShadingContext::osl_trace`.
    pub fn trace(
        &mut self,
        options: &mut TraceOpt,
        p: &Vec3,
        dpdx: &Vec3,
        dpdy: &Vec3,
        r: &Vec3,
        drdx: &Vec3,
        drdy: &Vec3,
    ) -> bool {
        self.trace_calls += 1;
        match self.renderer.as_ref() {
            Some(renderer) => renderer.trace(options, &self.globals, p, dpdx, dpdy, r, drdx, drdy),
            None => false,
        }
    }

    // -- Stats tracking -----------------------------------------------------

    /// Increment the layers-executed counter.
    /// Matches C++ `osl_incr_layers_executed`.
    pub fn osl_incr_layers_executed(&mut self) {
        self.layers_executed += 1;
    }

    /// Finalize execution: flush buffered errors and record stats.
    /// Matches C++ `ShadingContext::execute_cleanup`.
    ///
    /// When `err` is `Some`, buffered errors/warnings are forwarded to the handler
    /// and cleared. When `None`, errors remain in `error_messages`/`warning_messages`
    /// for inspection (e.g. in tests).
    pub fn execute_cleanup(&mut self, err: Option<&dyn ErrorHandler>) -> bool {
        if self.group.is_none() {
            // C++ allows repeated cleanup (no-op), don't treat as error
            return true;
        }
        self.process_errors(err);
        self.group = None;
        true
    }

    /// Process buffered error/warning messages.
    /// Matches C++ `ShadingContext::process_errors`.
    ///
    /// Drains `error_messages` and `warning_messages`, forwards each to the
    /// appropriate handler method (error/warning), then clears the buffers.
    /// When `err` is `None`, does nothing (messages stay for inspection).
    pub fn process_errors(&mut self, err: Option<&dyn ErrorHandler>) {
        if let Some(e) = err {
            for msg in self.error_messages.drain(..) {
                e.error(&msg);
            }
            for msg in self.warning_messages.drain(..) {
                e.warning(&msg);
            }
        }
    }

    // -- Dictionary resources -----------------------------------------------

    /// Release all dictionary allocations for this context.
    /// Matches C++ `ShadingContext::free_dict_resources`.
    pub fn free_dict_resources(&mut self) {
        self.dict_store = DictStore::new();
    }

    // -- Accessors -----------------------------------------------------------

    /// Get the current shader group, if any.
    pub fn group(&self) -> Option<&ShaderGroup> {
        self.group.as_deref()
    }

    /// Set the current shader group.
    pub fn set_group(&mut self, group: Option<Arc<ShaderGroup>>) {
        self.group = group;
    }

    /// Get per-thread info.
    pub fn thread_info(&self) -> &PerThreadInfo {
        &self.thread_info
    }

    /// Get batch size (1 for scalar execution).
    pub fn batch_size(&self) -> usize {
        self.batch_size
    }

    // -- Typed symbol value access ------------------------------------------

    /// Read an f32 from the symbol heap at the given byte offset.
    pub fn symbol_value_f32(&self, offset: usize) -> Option<Float> {
        let bytes = self.symbol_data(offset, 4);
        if bytes.len() == 4 {
            Some(Float::from_ne_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3],
            ]))
        } else {
            None
        }
    }

    /// Read an i32 from the symbol heap at the given byte offset.
    pub fn symbol_value_i32(&self, offset: usize) -> Option<i32> {
        let bytes = self.symbol_data(offset, 4);
        if bytes.len() == 4 {
            Some(i32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        } else {
            None
        }
    }

    /// Read a Vec3 from the symbol heap at the given byte offset.
    pub fn symbol_value_vec3(&self, offset: usize) -> Option<Vec3> {
        let bytes = self.symbol_data(offset, 12);
        if bytes.len() == 12 {
            let x = Float::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            let y = Float::from_ne_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
            let z = Float::from_ne_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
            Some(Vec3::new(x, y, z))
        } else {
            None
        }
    }

    // -- Inter-shader messaging (C++ naming) --------------------------------

    /// Send a message (C++ `osl_setmessage` naming convention).
    pub fn message_send(&mut self, name: UString, val: crate::message::MessageValue) {
        self.thread_info.messages.setmessage(name, val);
    }

    /// Get a message (C++ `osl_getmessage` naming convention).
    pub fn message_get(
        &self,
        source: &str,
        name: UString,
    ) -> Option<&crate::message::MessageValue> {
        self.thread_info.messages.getmessage(source, name)
    }

    /// Reset state between executions.
    ///
    /// Drops the closure tree (if any), resets counters, clears messages.
    pub fn reset(&mut self) {
        self.thread_info.reset();
        self.ci = None;
        self.output_color = Color3::ZERO;
        self.depth = 0;
        self.max_warnings = 100;
        self.aborted = false;
        self.closure_pool_offset = 0;
        self.executed_layers.clear();
        self.texture_calls = 0;
        self.trace_calls = 0;
        self.closure_calls = 0;
        self.layers_executed = 0;
        self.error_messages.clear();
        self.warning_messages.clear();
        self.renderer = None;
        self.group = None;
        self.free_dict_resources();
    }
}

/// Batched texture lookup options.
/// Matches C++ `BatchedTextureOptions` / OIIO `TextureOpt` + per-call optional args.
#[derive(Debug, Clone)]
pub struct BatchedTextureOptions {
    /// Interpolation mode: 0=closest, 1=bilinear, 2=bicubic, 3=smart bicubic.
    pub interp_mode: i32,
    /// Wrap mode for S direction (0=default, 1=black, 2=clamp, 3=periodic, etc.).
    pub swrap: i32,
    /// Wrap mode for T direction.
    pub twrap: i32,
    /// Wrap mode for R direction (3D textures).
    pub rwrap: i32,
    /// MIP mode: 0=default, 1=none, 2=one-level, 3=trilinear, 4=aniso.
    pub mip_mode: i32,
    /// First channel to read (0-based).
    pub firstchannel: i32,
    /// Subimage index.
    pub subimage: i32,
    /// Fill value for missing/beyond bounds texels.
    pub fill: Float,
    /// Filter width S/T (used by texture(), environment()).
    pub swidth: Float,
    pub twidth: Float,
    /// Filter width R (3D textures).
    pub rwidth: Float,
}

impl Default for BatchedTextureOptions {
    fn default() -> Self {
        Self {
            interp_mode: 1, // bilinear
            swrap: 0,       // default
            twrap: 0,       // default
            rwrap: 0,       // default (3D)
            mip_mode: 0,    // default
            firstchannel: 0,
            subimage: 0,
            fill: 0.0,
            swidth: 1.0,
            twidth: 1.0,
            rwidth: 1.0,
        }
    }
}

impl BatchedTextureOptions {
    /// Build from parsed TextureOpt. Used when merging per-call optional args.
    fn from_texture_opt(opt: &crate::texture::TextureOpt) -> Self {
        use crate::texture::{TextureInterp, TextureWrap};
        let wrap_to_i32 = |w: TextureWrap| -> i32 {
            match w {
                TextureWrap::Default => 0,
                TextureWrap::Black => 1,
                TextureWrap::Clamp => 2,
                TextureWrap::Periodic => 3,
                TextureWrap::Mirror => 4,
                TextureWrap::PeriodicPow2 => 5,
                TextureWrap::PeriodicSharedBorder => 6,
            }
        };
        let interp_to_i32 = |i: TextureInterp| -> i32 {
            match i {
                TextureInterp::SmartBicubic => 0,
                TextureInterp::Bilinear => 1,
                TextureInterp::Bicubic => 2,
                TextureInterp::Closest => 3,
            }
        };
        Self {
            interp_mode: interp_to_i32(opt.interpmode),
            swrap: wrap_to_i32(opt.swrap),
            twrap: wrap_to_i32(opt.twrap),
            rwrap: wrap_to_i32(opt.rwrap),
            mip_mode: opt.mipmode,
            firstchannel: opt.firstchannel,
            subimage: opt.subimage,
            fill: opt.fill,
            swidth: opt.swidth,
            twidth: opt.twidth,
            rwidth: opt.rwidth,
        }
    }

    /// Merge per-call overrides into self. Non-default fields in `overrides` overwrite.
    pub fn merge_texture_opt_overrides(&mut self, overrides: &crate::texture::TextureOpt) {
        use crate::texture::{TextureInterp, TextureWrap};
        let o = Self::from_texture_opt(overrides);
        if overrides.swrap != TextureWrap::Default {
            self.swrap = o.swrap;
        }
        if overrides.twrap != TextureWrap::Default {
            self.twrap = o.twrap;
        }
        if overrides.rwrap != TextureWrap::Default {
            self.rwrap = o.rwrap;
        }
        if overrides.interpmode != TextureInterp::SmartBicubic {
            self.interp_mode = o.interp_mode;
        }
        if overrides.mipmode != 0 {
            self.mip_mode = o.mip_mode;
        }
        if overrides.firstchannel != 0 {
            self.firstchannel = o.firstchannel;
        }
        if overrides.subimage != 0 {
            self.subimage = o.subimage;
        }
        if overrides.fill != 0.0 {
            self.fill = o.fill;
        }
        if overrides.swidth != 1.0 || overrides.twidth != 1.0 {
            self.swidth = o.swidth;
            self.twidth = o.twidth;
        }
        if overrides.rwidth != 1.0 {
            self.rwidth = o.rwidth;
        }
    }

    /// Convert to TextureOpt for use with RendererServices.
    /// Matches C++ wide_optexture: uniform_opt.swrap -> TextureOpt::Wrap.
    pub fn to_texture_opt(&self) -> crate::texture::TextureOpt {
        use crate::texture::{TextureInterp, TextureOpt, TextureWrap};
        let wrap_from_i32 = |i: i32| -> TextureWrap {
            match i {
                1 => TextureWrap::Black,
                2 => TextureWrap::Clamp,
                3 => TextureWrap::Periodic,
                4 => TextureWrap::Mirror,
                5 => TextureWrap::PeriodicPow2,
                6 => TextureWrap::PeriodicSharedBorder,
                _ => TextureWrap::Default,
            }
        };
        let interp_from_i32 = |i: i32| -> TextureInterp {
            match i {
                0 => TextureInterp::SmartBicubic,
                1 => TextureInterp::Bilinear,
                2 => TextureInterp::Bicubic,
                3 => TextureInterp::Closest,
                _ => TextureInterp::SmartBicubic,
            }
        };
        TextureOpt {
            firstchannel: self.firstchannel,
            nchannels: 3,
            swrap: wrap_from_i32(self.swrap),
            twrap: wrap_from_i32(self.twrap),
            rwrap: wrap_from_i32(self.rwrap),
            mipmode: self.mip_mode,
            interpmode: interp_from_i32(self.interp_mode),
            swidth: self.swidth,
            twidth: self.twidth,
            rwidth: self.rwidth,
            fill: self.fill,
            subimage: self.subimage,
            ..TextureOpt::default()
        }
    }
}

/// Pool of ShadingContexts for multi-threaded execution.
///
/// Pre-allocates one `ShadingContext` per thread to avoid repeated
/// allocation. Matches the C++ `ShadingContext` pool pattern.
pub struct ContextPool {
    pool: Vec<ShadingContext>,
    #[allow(dead_code)]
    next_thread: usize,
}

impl ContextPool {
    pub fn new(num_threads: usize) -> Self {
        let pool = (0..num_threads).map(|i| ShadingContext::new(i)).collect();
        Self {
            pool,
            next_thread: 0,
        }
    }

    /// Get a context for execution, resetting it first.
    pub fn get(&mut self, thread_index: usize) -> &mut ShadingContext {
        if thread_index < self.pool.len() {
            self.pool[thread_index].reset();
            &mut self.pool[thread_index]
        } else {
            // Grow pool if needed.
            while self.pool.len() <= thread_index {
                let idx = self.pool.len();
                self.pool.push(ShadingContext::new(idx));
            }
            self.pool[thread_index].reset();
            &mut self.pool[thread_index]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::MessageValue;
    use crate::shadingsys::ErrorHandler;
    use crate::ustring::UString;
    use std::sync::Mutex;

    /// Collecting error handler for tests.
    struct CollectingHandler(Mutex<Vec<String>>, Mutex<Vec<String>>);

    impl ErrorHandler for CollectingHandler {
        fn error(&self, msg: &str) {
            self.0.lock().unwrap().push(msg.to_string());
        }
        fn warning(&self, msg: &str) {
            self.1.lock().unwrap().push(msg.to_string());
        }
        fn info(&self, msg: &str) {
            self.1.lock().unwrap().push(format!("[info] {}", msg));
        }
        fn severe(&self, msg: &str) {
            self.0.lock().unwrap().push(format!("[severe] {}", msg));
        }
    }

    #[test]
    fn test_process_errors_forwards_to_handler() {
        let mut ctx = ShadingContext::new(0);
        ctx.record_error("err1");
        ctx.record_warning("warn1");
        let handler = CollectingHandler(Mutex::new(Vec::new()), Mutex::new(Vec::new()));
        ctx.process_errors(Some(&handler));
        assert!(ctx.error_messages.is_empty());
        assert!(ctx.warning_messages.is_empty());
        assert_eq!(handler.0.lock().unwrap().as_slice(), &["err1"]);
        assert_eq!(handler.1.lock().unwrap().as_slice(), &["warn1"]);
    }

    #[test]
    fn test_process_errors_none_keeps_messages() {
        let mut ctx = ShadingContext::new(0);
        ctx.record_error("err1");
        ctx.process_errors(None);
        assert_eq!(&ctx.error_messages, &["err1"]);
    }

    #[test]
    fn test_context_creation() {
        let ctx = ShadingContext::new(0);
        assert_eq!(ctx.depth, 0);
        assert!(!ctx.aborted);
        assert!(ctx.get_closure().is_none());
    }

    #[test]
    fn test_context_messages() {
        let mut ctx = ShadingContext::new(0);
        let name = UString::new("test_val");
        ctx.setmessage(name, MessageValue::Float(0.5));
        match ctx.getmessage("", name) {
            Some(MessageValue::Float(v)) => assert!((v - 0.5).abs() < 1e-6),
            _ => panic!("Expected float message"),
        }
    }

    #[test]
    fn test_context_reset() {
        let mut ctx = ShadingContext::new(0);
        let name = UString::new("test");
        ctx.setmessage(name, MessageValue::Int(42));
        ctx.depth = 5;
        ctx.reset();
        assert_eq!(ctx.depth, 0);
        assert!(ctx.getmessage("", name).is_none());
    }

    #[test]
    fn test_context_pool() {
        let mut pool = ContextPool::new(4);
        let ctx = pool.get(2);
        assert_eq!(ctx.depth, 0);
    }

    #[test]
    fn test_batched_texture_options() {
        let mut ctx = ShadingContext::new(0);
        let default = ctx.batched_texture_options();
        assert_eq!(default.interp_mode, 1);
        assert_eq!(default.swrap, 0);
        assert_eq!(default.rwrap, 0);
        let mut custom = BatchedTextureOptions::default();
        custom.swrap = 2; // clamp
        custom.twrap = 3; // periodic
        custom.rwrap = 4; // mirror (3D)
        ctx.set_batched_texture_options(custom.clone());
        let got = ctx.batched_texture_options();
        assert_eq!(got.swrap, 2);
        assert_eq!(got.twrap, 3);
        assert_eq!(got.rwrap, 4);
    }

    #[test]
    fn test_per_thread_info() {
        let mut pti = PerThreadInfo::new(0);
        let name = UString::new("v");
        pti.messages.setmessage(name, MessageValue::Int(1));
        assert!(pti.messages.has_message(name));
        pti.reset();
        assert!(!pti.messages.has_message(name));
    }

    #[test]
    fn test_texture_no_renderer() {
        let mut ctx = ShadingContext::new(0);
        let result = ctx.texture(
            UStringHash::EMPTY,
            std::ptr::null_mut(),
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            3,
            &mut [0.0; 3],
            None,
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_texture_with_renderer() {
        use crate::renderer::BasicRenderer;
        let mut ctx = ShadingContext::new(0);
        ctx.renderer = Some(Arc::new(BasicRenderer::new()));
        let mut result = [0.0f32; 3];
        let ok = ctx.texture(
            UStringHash::EMPTY,
            std::ptr::null_mut(),
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            3,
            &mut result,
            None,
            None,
        );
        assert!(ok.is_ok());
        assert_eq!(ctx.texture_calls, 1);
    }

    #[test]
    fn test_texture3d_dispatch() {
        use crate::renderer::BasicRenderer;
        let mut ctx = ShadingContext::new(0);
        ctx.renderer = Some(Arc::new(BasicRenderer::new()));
        let p = Vec3::new(0.5, 0.5, 0.5);
        let zero = Vec3::ZERO;
        let mut result = [0.0f32; 3];
        let ok = ctx.texture3d(
            UStringHash::EMPTY,
            std::ptr::null_mut(),
            &p,
            &zero,
            &zero,
            &zero,
            3,
            &mut result,
            None,
            None,
            None,
        );
        assert!(ok.is_ok());
        assert_eq!(ctx.texture_calls, 1);
    }

    #[test]
    fn test_environment_dispatch() {
        use crate::renderer::BasicRenderer;
        let mut ctx = ShadingContext::new(0);
        ctx.renderer = Some(Arc::new(BasicRenderer::new()));
        let r = Vec3::new(0.0, 1.0, 0.0);
        let zero = Vec3::ZERO;
        let mut result = [0.0f32; 3];
        let ok = ctx.environment(
            UStringHash::EMPTY,
            std::ptr::null_mut(),
            &r,
            &zero,
            &zero,
            3,
            &mut result,
            None,
            None,
        );
        assert!(ok.is_ok());
        // Sky color for +Y direction should be close to (0.5, 0.7, 1.0)
        assert!(result[0] > 0.4);
    }

    #[test]
    fn test_getattribute_dispatch() {
        use crate::renderer::{AttributeData, BasicRenderer};
        use crate::typedesc::TypeDesc;
        let mut renderer = BasicRenderer::new();
        renderer.set_attribute("test_attr", AttributeData::Int(42));
        let mut ctx = ShadingContext::new(0);
        ctx.renderer = Some(Arc::new(renderer));
        let name = UString::new("test_attr");
        let result = ctx.getattribute(
            false,
            UStringHash::EMPTY,
            TypeDesc::INT,
            UStringHash::from_hash(name.hash()),
        );
        assert!(result.is_some());
    }

    #[test]
    fn test_trace_dispatch() {
        use crate::renderer::NullRenderer;
        let mut ctx = ShadingContext::new(0);
        ctx.renderer = Some(Arc::new(NullRenderer));
        let p = Vec3::ZERO;
        let d = Vec3::new(0.0, 0.0, 1.0);
        let zero = Vec3::ZERO;
        let mut opt = TraceOpt::default();
        let hit = ctx.trace(&mut opt, &p, &zero, &zero, &d, &zero, &zero);
        assert!(!hit);
        assert_eq!(ctx.trace_calls, 1);
    }

    #[test]
    fn test_incr_layers_executed() {
        let mut ctx = ShadingContext::new(0);
        assert_eq!(ctx.layers_executed, 0);
        ctx.osl_incr_layers_executed();
        ctx.osl_incr_layers_executed();
        assert_eq!(ctx.layers_executed, 2);
    }

    #[test]
    fn test_free_dict_resources() {
        let mut ctx = ShadingContext::new(0);
        // Insert a dict entry then free
        ctx.dict_store.dict_find_str("{\"key\": \"val\"}", "key");
        ctx.free_dict_resources();
        // After free, store should be reset (no cached documents)
        let handle = ctx
            .dict_store
            .dict_find_str("{\"key\": \"val\"}", "missing");
        assert_eq!(handle, crate::dict::DICT_INVALID);
    }

    #[test]
    fn test_group_accessor() {
        use crate::shadingsys::ShaderGroup;
        let mut ctx = ShadingContext::new(0);
        assert!(ctx.group().is_none());
        ctx.set_group(Some(Arc::new(ShaderGroup::new("test_group"))));
        assert_eq!(ctx.group().unwrap().name, UString::new("test_group"));
    }

    #[test]
    fn test_batch_size() {
        let ctx = ShadingContext::new(0);
        assert_eq!(ctx.batch_size(), 1);
    }

    #[test]
    fn test_symbol_value_f32() {
        let mut ctx = ShadingContext::new(0);
        let val: f32 = 3.14;
        let bytes = val.to_ne_bytes();
        ctx.heap[0..4].copy_from_slice(&bytes);
        assert!((ctx.symbol_value_f32(0).unwrap() - 3.14).abs() < 1e-6);
    }

    #[test]
    fn test_symbol_value_i32() {
        let mut ctx = ShadingContext::new(0);
        let val: i32 = -42;
        let bytes = val.to_ne_bytes();
        ctx.heap[8..12].copy_from_slice(&bytes);
        assert_eq!(ctx.symbol_value_i32(8).unwrap(), -42);
    }

    #[test]
    fn test_symbol_value_vec3() {
        let mut ctx = ShadingContext::new(0);
        let v = Vec3::new(1.0, 2.0, 3.0);
        ctx.heap[0..4].copy_from_slice(&v.x.to_ne_bytes());
        ctx.heap[4..8].copy_from_slice(&v.y.to_ne_bytes());
        ctx.heap[8..12].copy_from_slice(&v.z.to_ne_bytes());
        let result = ctx.symbol_value_vec3(0).unwrap();
        assert!((result.x - 1.0).abs() < 1e-6);
        assert!((result.y - 2.0).abs() < 1e-6);
        assert!((result.z - 3.0).abs() < 1e-6);
    }

    #[test]
    fn test_message_send_get() {
        let mut ctx = ShadingContext::new(0);
        let name = UString::new("msg_test");
        ctx.message_send(name, MessageValue::Float(1.5));
        match ctx.message_get("", name) {
            Some(MessageValue::Float(v)) => assert!((v - 1.5).abs() < 1e-6),
            _ => panic!("Expected float message"),
        }
    }

    #[test]
    fn test_reset_clears_new_fields() {
        use crate::shadingsys::ShaderGroup;
        let mut ctx = ShadingContext::new(0);
        ctx.layers_executed = 10;
        ctx.set_group(Some(Arc::new(ShaderGroup::new("g"))));
        ctx.reset();
        assert_eq!(ctx.layers_executed, 0);
        assert!(ctx.group().is_none());
    }
}
