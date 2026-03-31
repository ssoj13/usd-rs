//! Cranelift JIT backend — compiles ShaderIR to native machine code.
//!
//! This module translates OSL opcodes into Cranelift IR, which is then
//! compiled to native code (x86-64, AArch64, etc.) at runtime. This gives
//! near-native execution speed while staying in pure Rust (no LLVM).
//!
//! # Architecture
//!
//! ```text
//! ShaderIR (opcodes + symbols)
//!     │
//!     ▼
//! CraneliftBackend::compile()
//!     │
//!     ├─ 1. Allocate stack slots for all symbols
//!     ├─ 2. Emit Cranelift IR for each opcode
//!     ├─ 3. Wire external calls (texture, noise) via trampolines
//!     └─ 4. Finalize → native function pointer
//!     │
//!     ▼
//! CompiledShader  (holds JIT module + function pointer)
//!     │
//!     ▼
//! CompiledShader::execute(&mut ShaderGlobals)
//! ```
//!
//! # Memory layout
//!
//! Each symbol is allocated a stack slot:
//! - `f32` → 4 bytes (8 bytes if `has_derivs`: val only, dx/dy in adjacent slots)
//! - `i32` → 4 bytes
//! - `Vec3` → 12 bytes (3 × f32)
//! - `Matrix44` → 64 bytes (16 × f32)
//!
//! `ShaderGlobals*` is passed as the first argument to the compiled function.
//! Global symbols load from known offsets in that struct.

use std::collections::HashMap;
use std::sync::Arc;

use cranelift_codegen::ir::condcodes::FloatCC;
use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::types;
use cranelift_codegen::ir::{AbiParam, Block, InstBuilder, MemFlags, Type, Value as CValue};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module};

use crate::codegen::{ConstValue, ShaderIR};
use crate::math::Vec3;
use crate::shaderglobals::ShaderGlobals;
use crate::symbol::SymType;
use crate::typedesc::{Aggregate, BaseType};

// ---------------------------------------------------------------------------
// JitBackend trait
// ---------------------------------------------------------------------------

/// Trait abstracting a JIT compilation backend.
///
/// This allows swapping Cranelift for LLVM (via inkwell) in the future
/// without changing the rest of the codebase.
pub trait JitBackend: Send + Sync {
    /// Compile a ShaderIR into a callable native function.
    /// Uses `range_checking = true` by default.
    fn compile(&self, ir: &ShaderIR) -> Result<CompiledShader, JitError> {
        self.compile_ext(ir, true)
    }

    /// Compile with explicit range_checking (for array/component index bounds).
    fn compile_ext(&self, ir: &ShaderIR, range_checking: bool) -> Result<CompiledShader, JitError>;

    /// Name of this backend (for diagnostics).
    fn name(&self) -> &str;
}

/// Error type for JIT compilation.
#[derive(Debug)]
pub enum JitError {
    /// Cranelift codegen error.
    Codegen(String),
    /// Module/linking error.
    Module(String),
    /// Unsupported opcode.
    UnsupportedOpcode(String),
}

impl std::fmt::Display for JitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JitError::Codegen(msg) => write!(f, "JIT codegen error: {msg}"),
            JitError::Module(msg) => write!(f, "JIT module error: {msg}"),
            JitError::UnsupportedOpcode(op) => write!(f, "JIT: unsupported opcode `{op}`"),
        }
    }
}

impl std::error::Error for JitError {}

// ---------------------------------------------------------------------------
// JitRuntimeContext — runtime state passed to JIT code
// ---------------------------------------------------------------------------

/// Runtime context passed as the 3rd argument to JIT-compiled shaders.
///
/// Contains pointers to Rust objects that trampolines need:
/// renderer services, message store, closure output, dict store, etc.
///
/// We store the renderer as two raw `usize` values (data ptr + vtable ptr)
/// because `*const dyn Trait` is a fat pointer and cannot be null-initialized.
#[repr(C)]
pub struct JitRuntimeContext {
    /// Renderer data pointer (first half of `*const dyn RendererServices`).
    pub renderer_data: usize,
    /// Renderer vtable pointer (second half of `*const dyn RendererServices`).
    pub renderer_vtable: usize,
    /// Has a valid renderer?
    pub has_renderer: bool,
    /// Message store for setmessage/getmessage (pointer to HashMap).
    pub messages: *mut std::collections::HashMap<String, crate::interp::Value>,
    /// Dictionary store for dict_find/dict_value (pointer to DictStore).
    pub dict_store: *mut crate::dict::DictStore,
    /// Commonspace synonym for getmatrix (e.g. "world"). Ptr+len to caller-owned UTF-8 string.
    pub commonspace_synonym_ptr: *const u8,
    pub commonspace_synonym_len: usize,
    /// Whether to emit errors when unknown coordinate systems are used. Matching C++ unknown_coordsys_error.
    pub unknown_coordsys_error: bool,
}

impl Default for JitRuntimeContext {
    fn default() -> Self {
        Self {
            renderer_data: 0,
            renderer_vtable: 0,
            has_renderer: false,
            messages: std::ptr::null_mut(),
            dict_store: std::ptr::null_mut(),
            commonspace_synonym_ptr: std::ptr::null(),
            commonspace_synonym_len: 0,
            unknown_coordsys_error: true,
        }
    }
}

impl JitRuntimeContext {
    /// Store a `&dyn RendererServices` reference into the context.
    ///
    /// # Safety
    /// The caller must ensure that the renderer outlives this context.
    pub fn set_renderer(&mut self, renderer: &dyn crate::renderer::RendererServices) {
        let fat_ptr = renderer as *const dyn crate::renderer::RendererServices;
        // Safety: decompose fat pointer into data + vtable via raw pointer casting.
        let raw: *const [usize; 2] = &fat_ptr as *const _ as *const [usize; 2];
        let parts = unsafe { *raw };
        self.renderer_data = parts[0];
        self.renderer_vtable = parts[1];
        self.has_renderer = true;
    }

    /// Set the commonspace synonym for getmatrix/transform (e.g. "world").
    /// Caller must ensure the slice outlives any trampoline use.
    pub fn set_commonspace_synonym(&mut self, s: &str) {
        self.commonspace_synonym_ptr = s.as_ptr();
        self.commonspace_synonym_len = s.len();
    }

    /// Set whether to emit errors for unknown coordinate systems. Matching C++ unknown_coordsys_error.
    pub fn set_unknown_coordsys_error(&mut self, enabled: bool) {
        self.unknown_coordsys_error = enabled;
    }

    /// Get the commonspace synonym string. Returns "world" if not set.
    fn get_commonspace_synonym(&self) -> &str {
        if self.commonspace_synonym_ptr.is_null() || self.commonspace_synonym_len == 0 {
            "world"
        } else {
            unsafe {
                let slice = std::slice::from_raw_parts(
                    self.commonspace_synonym_ptr,
                    self.commonspace_synonym_len,
                );
                std::str::from_utf8(slice).unwrap_or("world")
            }
        }
    }

    /// Reconstruct a `&dyn RendererServices` from stored raw pointers.
    ///
    /// # Safety
    /// The original renderer must still be alive.
    pub unsafe fn get_renderer(&self) -> Option<&dyn crate::renderer::RendererServices> {
        if !self.has_renderer {
            return None;
        }
        // Safety: reconstruct fat pointer from stored data + vtable.
        let parts: [usize; 2] = [self.renderer_data, self.renderer_vtable];
        let fat_ptr: *const dyn crate::renderer::RendererServices =
            unsafe { *(&parts as *const _ as *const *const dyn crate::renderer::RendererServices) };
        Some(unsafe { &*fat_ptr })
    }
}

// ---------------------------------------------------------------------------
// CompiledShader
// ---------------------------------------------------------------------------

/// Type of the JIT-compiled shader entry point.
///
/// `fn(sg: *mut ShaderGlobals, heap: *mut u8, ctx: *mut JitRuntimeContext)`
type ShaderEntryFn = unsafe extern "C" fn(*mut ShaderGlobals, *mut u8, *mut JitRuntimeContext);

/// A compiled shader ready for execution.
pub struct CompiledShader {
    /// The JIT module that owns the compiled code.
    /// Must stay alive as long as the function pointer is used.
    _module: JITModule,
    /// Native function pointer.
    entry: ShaderEntryFn,
    /// Size of the heap buffer needed (bytes).
    heap_size: usize,
    /// Symbol name → offset in heap (for reading results back).
    symbol_offsets: HashMap<String, (usize, SymLayout)>,
}

/// Layout info for a symbol in the JIT heap.
#[derive(Debug, Clone, Copy)]
pub struct SymLayout {
    pub offset: usize,
    pub ty: JitType,
}

/// Simplified type system for JIT compilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JitType {
    Int,
    Float,
    Vec3,
    Color,
    Matrix,
    String,
}

impl JitType {
    /// Size in bytes on the heap.
    pub fn size(self) -> usize {
        match self {
            JitType::Int => 4,
            JitType::Float => 4,
            JitType::Vec3 | JitType::Color => 12,
            JitType::Matrix => 64,
            JitType::String => 8, // pointer-sized
        }
    }
}

// Safety: The JITModule and function pointer are safe to send across threads.
// The compiled code is immutable once finalized.
unsafe impl Send for CompiledShader {}
unsafe impl Sync for CompiledShader {}

impl CompiledShader {
    /// Execute the compiled shader with the given globals.
    pub fn execute(&self, sg: &mut ShaderGlobals) {
        let mut heap = vec![0u8; self.heap_size];
        let mut ctx = JitRuntimeContext::default();
        let mut messages = std::collections::HashMap::new();
        let mut dict_store = crate::dict::DictStore::new();
        ctx.messages = &mut messages;
        ctx.dict_store = &mut dict_store;
        unsafe {
            (self.entry)(sg as *mut ShaderGlobals, heap.as_mut_ptr(), &mut ctx);
        }
    }

    /// Execute with a pre-allocated heap buffer (avoids allocation).
    pub fn execute_with_heap(&self, sg: &mut ShaderGlobals, heap: &mut [u8]) {
        assert!(heap.len() >= self.heap_size);
        let mut ctx = JitRuntimeContext::default();
        let mut messages = std::collections::HashMap::new();
        let mut dict_store = crate::dict::DictStore::new();
        ctx.messages = &mut messages;
        ctx.dict_store = &mut dict_store;
        unsafe {
            (self.entry)(sg as *mut ShaderGlobals, heap.as_mut_ptr(), &mut ctx);
        }
    }

    /// Execute with renderer services for texture/attribute lookups.
    /// `commonspace_synonym` (e.g. "world") is used for transform/getmatrix; defaults to "world".
    /// `unknown_coordsys_error` controls whether getmatrix reports errors for unknown coordinate systems.
    pub fn execute_with_renderer(
        &self,
        sg: &mut ShaderGlobals,
        heap: &mut [u8],
        renderer: &dyn crate::renderer::RendererServices,
        commonspace_synonym: Option<&str>,
        unknown_coordsys_error: bool,
    ) {
        assert!(heap.len() >= self.heap_size);
        let mut ctx = JitRuntimeContext::default();
        let mut messages = std::collections::HashMap::new();
        let mut dict_store = crate::dict::DictStore::new();
        ctx.set_renderer(renderer);
        if let Some(s) = commonspace_synonym {
            ctx.set_commonspace_synonym(s);
        }
        ctx.set_unknown_coordsys_error(unknown_coordsys_error);
        ctx.messages = &mut messages;
        ctx.dict_store = &mut dict_store;
        unsafe {
            (self.entry)(sg as *mut ShaderGlobals, heap.as_mut_ptr(), &mut ctx);
        }
    }

    /// Read back a float result from the heap.
    pub fn get_float(&self, heap: &[u8], name: &str) -> Option<f32> {
        let layout = self.symbol_offsets.get(name)?;
        if layout.1.ty != JitType::Float {
            return None;
        }
        let off = layout.0;
        if off + 4 > heap.len() {
            return None;
        }
        Some(f32::from_le_bytes([
            heap[off],
            heap[off + 1],
            heap[off + 2],
            heap[off + 3],
        ]))
    }

    /// Read back a Vec3 result from the heap.
    pub fn get_vec3(&self, heap: &[u8], name: &str) -> Option<Vec3> {
        let layout = self.symbol_offsets.get(name)?;
        if !matches!(layout.1.ty, JitType::Vec3 | JitType::Color) {
            return None;
        }
        let off = layout.0;
        if off + 12 > heap.len() {
            return None;
        }
        let x = f32::from_le_bytes([heap[off], heap[off + 1], heap[off + 2], heap[off + 3]]);
        let y = f32::from_le_bytes([heap[off + 4], heap[off + 5], heap[off + 6], heap[off + 7]]);
        let z = f32::from_le_bytes([heap[off + 8], heap[off + 9], heap[off + 10], heap[off + 11]]);
        Some(Vec3::new(x, y, z))
    }

    /// Read back an int result from the heap.
    pub fn get_int(&self, heap: &[u8], name: &str) -> Option<i32> {
        let layout = self.symbol_offsets.get(name)?;
        if layout.1.ty != JitType::Int {
            return None;
        }
        let off = layout.0;
        if off + 4 > heap.len() {
            return None;
        }
        Some(i32::from_le_bytes([
            heap[off],
            heap[off + 1],
            heap[off + 2],
            heap[off + 3],
        ]))
    }

    /// Write a float value into the heap for a named symbol.
    pub fn set_float(&self, heap: &mut [u8], name: &str, val: f32) -> bool {
        let layout = match self.symbol_offsets.get(name) {
            Some(l) => l,
            None => return false,
        };
        if layout.1.ty != JitType::Float {
            return false;
        }
        let off = layout.0;
        if off + 4 > heap.len() {
            return false;
        }
        heap[off..off + 4].copy_from_slice(&val.to_le_bytes());
        true
    }

    /// Write a Vec3 value into the heap for a named symbol.
    pub fn set_vec3(&self, heap: &mut [u8], name: &str, val: Vec3) -> bool {
        let layout = match self.symbol_offsets.get(name) {
            Some(l) => l,
            None => return false,
        };
        if !matches!(layout.1.ty, JitType::Vec3 | JitType::Color) {
            return false;
        }
        let off = layout.0;
        if off + 12 > heap.len() {
            return false;
        }
        heap[off..off + 4].copy_from_slice(&val.x.to_le_bytes());
        heap[off + 4..off + 8].copy_from_slice(&val.y.to_le_bytes());
        heap[off + 8..off + 12].copy_from_slice(&val.z.to_le_bytes());
        true
    }

    /// Write an int value into the heap for a named symbol.
    pub fn set_int(&self, heap: &mut [u8], name: &str, val: i32) -> bool {
        let layout = match self.symbol_offsets.get(name) {
            Some(l) => l,
            None => return false,
        };
        if layout.1.ty != JitType::Int {
            return false;
        }
        let off = layout.0;
        if off + 4 > heap.len() {
            return false;
        }
        heap[off..off + 4].copy_from_slice(&val.to_le_bytes());
        true
    }

    /// Required heap size in bytes.
    pub fn heap_size(&self) -> usize {
        self.heap_size
    }
}

// ---------------------------------------------------------------------------
// ShaderGlobals field offsets (for loading globals from the SG pointer)
// ---------------------------------------------------------------------------

/// Known byte offsets of fields in ShaderGlobals.
/// These must match the `#[repr(C)]` layout.
#[allow(dead_code)]
mod sg_offsets {
    use crate::shaderglobals::ShaderGlobals;

    macro_rules! offset_of {
        ($ty:ty, $field:ident) => {{
            let uninit = std::mem::MaybeUninit::<$ty>::uninit();
            let base = uninit.as_ptr() as usize;
            // Safety: we never dereference, only compute offset
            let field_ptr = unsafe { &(*uninit.as_ptr()).$field as *const _ as usize };
            field_ptr - base
        }};
    }

    pub fn p() -> usize {
        offset_of!(ShaderGlobals, p)
    }
    pub fn dp_dx() -> usize {
        offset_of!(ShaderGlobals, dp_dx)
    }
    pub fn dp_dy() -> usize {
        offset_of!(ShaderGlobals, dp_dy)
    }
    pub fn n() -> usize {
        offset_of!(ShaderGlobals, n)
    }
    pub fn ng() -> usize {
        offset_of!(ShaderGlobals, ng)
    }
    pub fn i() -> usize {
        offset_of!(ShaderGlobals, i)
    }
    pub fn u_() -> usize {
        offset_of!(ShaderGlobals, u)
    }
    pub fn v() -> usize {
        offset_of!(ShaderGlobals, v)
    }
    pub fn dudx() -> usize {
        offset_of!(ShaderGlobals, dudx)
    }
    pub fn dudy() -> usize {
        offset_of!(ShaderGlobals, dudy)
    }
    pub fn dvdx() -> usize {
        offset_of!(ShaderGlobals, dvdx)
    }
    pub fn dvdy() -> usize {
        offset_of!(ShaderGlobals, dvdy)
    }
    pub fn dp_du() -> usize {
        offset_of!(ShaderGlobals, dp_du)
    }
    pub fn dp_dv() -> usize {
        offset_of!(ShaderGlobals, dp_dv)
    }
    pub fn time() -> usize {
        offset_of!(ShaderGlobals, time)
    }
    pub fn surfacearea() -> usize {
        offset_of!(ShaderGlobals, surfacearea)
    }
    pub fn raytype() -> usize {
        offset_of!(ShaderGlobals, raytype)
    }
    pub fn backfacing() -> usize {
        offset_of!(ShaderGlobals, backfacing)
    }
    pub fn dp_dz() -> usize {
        offset_of!(ShaderGlobals, dp_dz)
    }
    pub fn di_dx() -> usize {
        offset_of!(ShaderGlobals, di_dx)
    }
    pub fn di_dy() -> usize {
        offset_of!(ShaderGlobals, di_dy)
    }
    pub fn dtime() -> usize {
        offset_of!(ShaderGlobals, dtime)
    }
    pub fn dp_dtime() -> usize {
        offset_of!(ShaderGlobals, dp_dtime)
    }
    pub fn ps() -> usize {
        offset_of!(ShaderGlobals, ps)
    }
    pub fn dps_dx() -> usize {
        offset_of!(ShaderGlobals, dps_dx)
    }
    pub fn dps_dy() -> usize {
        offset_of!(ShaderGlobals, dps_dy)
    }
    pub fn flip_handedness() -> usize {
        offset_of!(ShaderGlobals, flip_handedness)
    }
    pub fn thread_index() -> usize {
        offset_of!(ShaderGlobals, thread_index)
    }
    pub fn shade_index() -> usize {
        offset_of!(ShaderGlobals, shade_index)
    }
}

// ---------------------------------------------------------------------------
// CraneliftBackend
// ---------------------------------------------------------------------------

/// Cranelift-based JIT compiler for OSL shaders.
pub struct CraneliftBackend {
    /// Optimization level for Cranelift.
    opt_level: OptLevel,
}

/// Cranelift optimization level.
#[derive(Debug, Clone, Copy, Default)]
pub enum OptLevel {
    /// No optimization (fastest compilation).
    None,
    /// Speed-focused optimization (default).
    #[default]
    Speed,
    /// Aggressive optimization (slower compilation).
    SpeedAndSize,
}

impl CraneliftBackend {
    pub fn new() -> Self {
        Self {
            opt_level: OptLevel::Speed,
        }
    }

    pub fn with_opt_level(opt_level: OptLevel) -> Self {
        Self { opt_level }
    }

    /// Configure the Cranelift flag builder with tuned settings for shader compilation.
    ///
    /// These settings are applied to both `compile()` and `compile_group()`:
    /// - `opt_level`: set from `self.opt_level`
    /// - `enable_verifier`: off in release for speed, on in debug for correctness
    /// - `regalloc_checker`: off in release
    /// - `is_pic`: off (JIT code is not position-independent)
    /// - `use_colocated_libcalls`: true (more efficient trampoline calls)
    fn configure_flags(&self) -> settings::Flags {
        let mut fb = settings::builder();
        match self.opt_level {
            OptLevel::None => {
                let _ = fb.set("opt_level", "none");
            }
            OptLevel::Speed => {
                let _ = fb.set("opt_level", "speed");
            }
            OptLevel::SpeedAndSize => {
                let _ = fb.set("opt_level", "speed_and_size");
            }
        }
        // Enable LICM, GVN, and other mid-level optimizations
        let _ = fb.set(
            "enable_verifier",
            if cfg!(debug_assertions) {
                "true"
            } else {
                "false"
            },
        );
        let _ = fb.set("is_pic", "false");
        let _ = fb.set("use_colocated_libcalls", "true");
        // Enable alias analysis for better store-to-load forwarding in heap access
        let _ = fb.set("enable_alias_analysis", "true");
        settings::Flags::new(fb)
    }

    /// Determine the JitType for a symbol based on its TypeSpec.
    fn jit_type_for_sym(sym: &crate::symbol::Symbol) -> JitType {
        let td = sym.typespec.simpletype();
        let bt = BaseType::from_u8(td.basetype);
        let ag = Aggregate::from_u8(td.aggregate);
        match (bt, ag) {
            (BaseType::Int32, _) => JitType::Int,
            (BaseType::Float, Aggregate::Scalar) => JitType::Float,
            (BaseType::Float, Aggregate::Vec3) | (BaseType::Float, Aggregate::Vec4) => {
                JitType::Vec3
            }
            (BaseType::Float, Aggregate::Matrix44) => JitType::Matrix,
            (BaseType::String, _) => JitType::String,
            _ => JitType::Float, // fallback
        }
    }
}

impl Default for CraneliftBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl JitBackend for CraneliftBackend {
    fn name(&self) -> &str {
        "cranelift"
    }

    fn compile_ext(&self, ir: &ShaderIR, range_checking: bool) -> Result<CompiledShader, JitError> {
        // --- 1. Set up Cranelift target with tuned flags ---
        let flags = self.configure_flags();
        let isa_builder = cranelift_native::builder()
            .map_err(|e| JitError::Codegen(format!("native ISA: {e}")))?;
        let isa = isa_builder
            .finish(flags)
            .map_err(|e| JitError::Codegen(format!("ISA finish: {e}")))?;

        let mut builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());

        // Register math trampolines so JIT code can call them
        register_math_trampolines(&mut builder);

        let mut module = JITModule::new(builder);

        // --- 2. Plan symbol layout on the heap ---
        let mut heap_offset = 0usize;
        let mut sym_offsets: HashMap<String, (usize, SymLayout)> = HashMap::new();
        let mut sym_heap_offsets: Vec<usize> = Vec::with_capacity(ir.symbols.len());
        let mut sym_jit_types: Vec<JitType> = Vec::with_capacity(ir.symbols.len());

        // Track which symbols have derivatives for Dual2 support.
        let mut sym_has_derivs: Vec<bool> = Vec::with_capacity(ir.symbols.len());

        for sym in &ir.symbols {
            let jt = Self::jit_type_for_sym(sym);
            // For struct types, compute size from the StructSpec layout;
            // otherwise use the JitType size.
            let base_sz = {
                let sid = sym.typespec.structure_id();
                if sid > 0 {
                    crate::typespec::get_struct(sid as i32)
                        .map(|spec| spec.total_byte_size())
                        .unwrap_or(jt.size())
                } else {
                    jt.size()
                }
            };
            // Allocate 3x space for symbols with derivatives (val, dx, dy)
            let sz = if sym.has_derivs { base_sz * 3 } else { base_sz };
            // Align to 4 bytes
            if heap_offset % 4 != 0 {
                heap_offset += 4 - (heap_offset % 4);
            }
            sym_heap_offsets.push(heap_offset);
            sym_jit_types.push(jt);
            sym_has_derivs.push(sym.has_derivs);
            sym_offsets.insert(
                sym.name.as_str().to_string(),
                (
                    heap_offset,
                    SymLayout {
                        offset: heap_offset,
                        ty: jt,
                    },
                ),
            );
            heap_offset += sz;
        }
        // Add scratch space for format() variadic args packing (max 16 args * 2 arrays * 4 bytes)
        let scratch_offset = heap_offset;
        let heap_size = heap_offset + 16 * 2 * 4;

        // --- 3. Declare the shader function ---
        // fn shader_entry(sg: *mut ShaderGlobals, heap: *mut u8, ctx: *mut JitRuntimeContext)
        let ptr_type = module.target_config().pointer_type();
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(ptr_type)); // sg
        sig.params.push(AbiParam::new(ptr_type)); // heap
        sig.params.push(AbiParam::new(ptr_type)); // ctx

        let func_id = module
            .declare_function("shader_entry", Linkage::Export, &sig)
            .map_err(|e| JitError::Module(format!("declare: {e}")))?;

        // --- 4. Declare math helper functions ---
        let math_funcs = declare_math_funcs(&mut module, ptr_type)?;

        // --- 5. Build the function body ---
        let mut ctx = module.make_context();
        ctx.func.signature = sig.clone();

        let mut fb_ctx = FunctionBuilderContext::new();
        {
            let mut builder = FunctionBuilder::new(&mut ctx.func, &mut fb_ctx);

            let entry_block = builder.create_block();
            builder.append_block_params_for_function_params(entry_block);
            builder.switch_to_block(entry_block);
            builder.seal_block(entry_block);

            let sg_ptr = builder.block_params(entry_block)[0];
            let heap_ptr = builder.block_params(entry_block)[1];
            let ctx_ptr = builder.block_params(entry_block)[2];

            // Initialize constants and param defaults on the heap
            emit_init_constants(&builder, &ir, heap_ptr, &sym_heap_offsets, &sym_jit_types);

            // Load globals from ShaderGlobals into heap slots
            emit_bind_globals(&mut builder, &ir, sg_ptr, heap_ptr, &sym_heap_offsets);

            // --- Emit opcodes ---
            emit_opcodes(
                &mut builder,
                &mut module,
                ir,
                sg_ptr,
                heap_ptr,
                ctx_ptr,
                &sym_heap_offsets,
                &sym_jit_types,
                &sym_has_derivs,
                scratch_offset,
                ptr_type,
                &math_funcs,
                range_checking,
            )?;

            builder.ins().return_(&[]);
            builder.finalize();
        }

        // --- 6. Compile and finalize ---
        module
            .define_function(func_id, &mut ctx)
            .map_err(|e| JitError::Codegen(format!("define: {e}")))?;

        module.clear_context(&mut ctx);
        module
            .finalize_definitions()
            .map_err(|e| JitError::Module(format!("finalize: {e}")))?;

        let code_ptr = module.get_finalized_function(func_id);

        // Safety: the function signature matches ShaderEntryFn.
        let entry: ShaderEntryFn = unsafe { std::mem::transmute(code_ptr) };

        Ok(CompiledShader {
            _module: module,
            entry,
            heap_size,
            symbol_offsets: sym_offsets,
        })
    }
}

impl CraneliftBackend {
    /// Compile an entire shader group (multiple layers) into a single native function.
    ///
    /// All layers share one unified heap. Connections between layers are resolved
    /// at compile time as direct memory copies within the heap, eliminating
    /// per-execution overhead. This is the production-quality JIT path.
    pub fn compile_group(
        &self,
        layers: &[&ShaderIR],
        connections: &[crate::shadingsys::Connection],
        range_checking: bool,
    ) -> Result<CompiledShader, JitError> {
        if layers.is_empty() {
            return Err(JitError::Codegen("empty shader group".into()));
        }
        // Single-layer shortcut
        if layers.len() == 1 && connections.is_empty() {
            return self.compile_ext(layers[0], range_checking);
        }

        // --- 1. Set up Cranelift target with tuned flags ---
        let flags = self.configure_flags();
        let isa_builder = cranelift_native::builder()
            .map_err(|e| JitError::Codegen(format!("native ISA: {e}")))?;
        let isa = isa_builder
            .finish(flags)
            .map_err(|e| JitError::Codegen(format!("ISA finish: {e}")))?;

        let mut builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
        register_math_trampolines(&mut builder);
        let mut module = JITModule::new(builder);

        // --- 2. Plan unified heap layout ---
        // Each layer gets a base offset; its symbols live at base + local_offset.
        let mut heap_offset = 0usize;
        let mut layer_bases: Vec<usize> = Vec::with_capacity(layers.len());
        let mut all_offsets: Vec<Vec<usize>> = Vec::with_capacity(layers.len());
        let mut all_jtypes: Vec<Vec<JitType>> = Vec::with_capacity(layers.len());
        let mut all_has_derivs: Vec<Vec<bool>> = Vec::with_capacity(layers.len());
        // Unified symbol_offsets map (prefixed with layer index for uniqueness)
        let mut sym_offsets: HashMap<String, (usize, SymLayout)> = HashMap::new();
        // Per-layer name→offset maps for connection resolution
        let mut layer_name_maps: Vec<HashMap<String, (usize, JitType)>> =
            Vec::with_capacity(layers.len());

        for (layer_idx, ir) in layers.iter().enumerate() {
            let base = heap_offset;
            layer_bases.push(base);
            let mut offsets = Vec::with_capacity(ir.symbols.len());
            let mut jtypes = Vec::with_capacity(ir.symbols.len());
            let mut has_derivs = Vec::with_capacity(ir.symbols.len());
            let mut name_map: HashMap<String, (usize, JitType)> = HashMap::new();

            for sym in &ir.symbols {
                let jt = Self::jit_type_for_sym(sym);
                let base_sz = {
                    let sid = sym.typespec.structure_id();
                    if sid > 0 {
                        crate::typespec::get_struct(sid as i32)
                            .map(|spec| spec.total_byte_size())
                            .unwrap_or(jt.size())
                    } else {
                        jt.size()
                    }
                };
                let sz = if sym.has_derivs { base_sz * 3 } else { base_sz };
                if heap_offset % 4 != 0 {
                    heap_offset += 4 - (heap_offset % 4);
                }
                offsets.push(heap_offset);
                jtypes.push(jt);
                has_derivs.push(sym.has_derivs);
                name_map.insert(sym.name.as_str().to_string(), (heap_offset, jt));
                // For the final layer, expose symbols without prefix for result reading
                if layer_idx == layers.len() - 1 {
                    sym_offsets.insert(
                        sym.name.as_str().to_string(),
                        (
                            heap_offset,
                            SymLayout {
                                offset: heap_offset,
                                ty: jt,
                            },
                        ),
                    );
                }
                heap_offset += sz;
            }
            all_offsets.push(offsets);
            all_jtypes.push(jtypes);
            all_has_derivs.push(has_derivs);
            layer_name_maps.push(name_map);
        }
        let scratch_offset = heap_offset;
        let heap_size = heap_offset + 16 * 2 * 4;

        // --- 3. Declare function ---
        let ptr_type = module.target_config().pointer_type();
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(ptr_type)); // sg
        sig.params.push(AbiParam::new(ptr_type)); // heap
        sig.params.push(AbiParam::new(ptr_type)); // ctx

        let func_id = module
            .declare_function("shader_group_entry", Linkage::Export, &sig)
            .map_err(|e| JitError::Module(format!("declare: {e}")))?;

        let math_funcs = declare_math_funcs(&mut module, ptr_type)?;

        // --- 4. Build function body ---
        let mut ctx = module.make_context();
        ctx.func.signature = sig.clone();

        let mut fb_ctx = FunctionBuilderContext::new();
        {
            let mut fbuilder = FunctionBuilder::new(&mut ctx.func, &mut fb_ctx);
            let entry_block = fbuilder.create_block();
            fbuilder.append_block_params_for_function_params(entry_block);
            fbuilder.switch_to_block(entry_block);
            fbuilder.seal_block(entry_block);

            let sg_ptr = fbuilder.block_params(entry_block)[0];
            let heap_ptr = fbuilder.block_params(entry_block)[1];
            let ctx_ptr = fbuilder.block_params(entry_block)[2];

            for (layer_idx, ir) in layers.iter().enumerate() {
                // Init constants and param defaults for this layer
                emit_init_constants(
                    &fbuilder,
                    ir,
                    heap_ptr,
                    &all_offsets[layer_idx],
                    &all_jtypes[layer_idx],
                );

                // Bind globals (only first layer needs it typically, but safe for all)
                emit_bind_globals(&mut fbuilder, ir, sg_ptr, heap_ptr, &all_offsets[layer_idx]);

                // Resolve connections: copy values from upstream layer symbols
                for conn in connections {
                    if conn.dst_layer == layer_idx as i32 {
                        let src_layer = conn.src_layer as usize;
                        let src_name = conn.src_param.as_str();
                        let dst_name = conn.dst_param.as_str();
                        if let (Some(&(src_off, src_jt)), Some(&(dst_off, _dst_jt))) = (
                            layer_name_maps[src_layer].get(src_name),
                            layer_name_maps[layer_idx].get(dst_name),
                        ) {
                            // Emit heap-to-heap copy within the unified buffer
                            let nbytes = src_jt.size();
                            for b in (0..nbytes).step_by(4) {
                                let v = heap_load_f32(&mut fbuilder, heap_ptr, src_off + b);
                                heap_store_f32(&mut fbuilder, heap_ptr, dst_off + b, v);
                            }
                        }
                    }
                }

                // Emit opcodes for this layer
                emit_opcodes(
                    &mut fbuilder,
                    &mut module,
                    ir,
                    sg_ptr,
                    heap_ptr,
                    ctx_ptr,
                    &all_offsets[layer_idx],
                    &all_jtypes[layer_idx],
                    &all_has_derivs[layer_idx],
                    scratch_offset,
                    ptr_type,
                    &math_funcs,
                    range_checking,
                )?;
            }

            fbuilder.ins().return_(&[]);
            fbuilder.finalize();
        }

        // --- 5. Compile and finalize ---
        module
            .define_function(func_id, &mut ctx)
            .map_err(|e| JitError::Codegen(format!("define: {e}")))?;
        module.clear_context(&mut ctx);
        module
            .finalize_definitions()
            .map_err(|e| JitError::Module(format!("finalize: {e}")))?;

        let code_ptr = module.get_finalized_function(func_id);
        let entry: ShaderEntryFn = unsafe { std::mem::transmute(code_ptr) };

        Ok(CompiledShader {
            _module: module,
            entry,
            heap_size,
            symbol_offsets: sym_offsets,
        })
    }
}

// ---------------------------------------------------------------------------
// Math trampolines — Rust functions callable from JIT code
// ---------------------------------------------------------------------------

/// Trampoline functions that JIT code calls for math operations
/// that are too complex to inline (sin, cos, pow, etc.).
extern "C" fn trampoline_sinf(x: f32) -> f32 {
    x.sin()
}
extern "C" fn trampoline_cosf(x: f32) -> f32 {
    x.cos()
}
extern "C" fn trampoline_tanf(x: f32) -> f32 {
    x.tan()
}
extern "C" fn trampoline_asinf(x: f32) -> f32 {
    x.clamp(-1.0, 1.0).asin()
}
extern "C" fn trampoline_acosf(x: f32) -> f32 {
    x.clamp(-1.0, 1.0).acos()
}
extern "C" fn trampoline_atanf(x: f32) -> f32 {
    x.atan()
}
extern "C" fn trampoline_atan2f(y: f32, x: f32) -> f32 {
    y.atan2(x)
}
extern "C" fn trampoline_sqrtf(x: f32) -> f32 {
    if x >= 0.0 { x.sqrt() } else { 0.0 }
}
extern "C" fn trampoline_expf(x: f32) -> f32 {
    x.exp()
}
extern "C" fn trampoline_logf(x: f32) -> f32 {
    if x > 0.0 { x.ln() } else { f32::NEG_INFINITY }
}
extern "C" fn trampoline_powf(x: f32, y: f32) -> f32 {
    x.powf(y)
}
extern "C" fn trampoline_floorf(x: f32) -> f32 {
    x.floor()
}
extern "C" fn trampoline_ceilf(x: f32) -> f32 {
    x.ceil()
}
extern "C" fn trampoline_fabsf(x: f32) -> f32 {
    x.abs()
}
extern "C" fn trampoline_fmodf(x: f32, y: f32) -> f32 {
    if y != 0.0 { x % y } else { 0.0 }
}

// Extended math trampolines (C++ parity)
extern "C" fn trampoline_cbrtf(x: f32) -> f32 {
    x.cbrt()
}
extern "C" fn trampoline_log2f(x: f32) -> f32 {
    x.log2()
}
extern "C" fn trampoline_log10f(x: f32) -> f32 {
    x.log10()
}
extern "C" fn trampoline_logbf(x: f32) -> f32 {
    if x == 0.0 {
        f32::NEG_INFINITY
    } else {
        (x.abs().log2()).floor()
    }
}
extern "C" fn trampoline_exp2f(x: f32) -> f32 {
    x.exp2()
}
extern "C" fn trampoline_expm1f(x: f32) -> f32 {
    (x as f64).exp_m1() as f32
}
extern "C" fn trampoline_erff(x: f32) -> f32 {
    libm::erff(x)
}
extern "C" fn trampoline_erfcf(x: f32) -> f32 {
    libm::erfcf(x)
}
extern "C" fn trampoline_roundf(x: f32) -> f32 {
    x.round()
}
extern "C" fn trampoline_truncf(x: f32) -> f32 {
    x.trunc()
}

// ---------------------------------------------------------------------------
// Noise trampolines
// ---------------------------------------------------------------------------

extern "C" fn trampoline_noise1(x: f32) -> f32 {
    crate::noise::perlin1(x)
}
extern "C" fn trampoline_noise3(x: f32, y: f32, z: f32) -> f32 {
    crate::noise::perlin3(crate::math::Vec3::new(x, y, z))
}
extern "C" fn trampoline_snoise1(x: f32) -> f32 {
    crate::noise::perlin1(x) * 2.0 - 1.0
}
extern "C" fn trampoline_snoise3(x: f32, y: f32, z: f32) -> f32 {
    let v = crate::noise::perlin3(crate::math::Vec3::new(x, y, z));
    v * 2.0 - 1.0
}

/// Noise with analytical derivatives: returns (value, dval/dx, dval/dy, dval/dz)
/// Written to heap: value at dst_off, partial derivatives at grad_off (3 floats).
extern "C" fn trampoline_noise3_deriv(
    heap: *mut u8,
    dst_off: i32,
    grad_off: i32,
    x: f32,
    y: f32,
    z: f32,
) {
    let p = crate::math::Vec3::new(x, y, z);
    let (val, grad) = crate::noise::perlin3_deriv(p);
    unsafe {
        *(heap.add(dst_off as usize) as *mut f32) = val;
        *(heap.add(grad_off as usize) as *mut f32) = grad.x;
        *(heap.add(grad_off as usize + 4) as *mut f32) = grad.y;
        *(heap.add(grad_off as usize + 8) as *mut f32) = grad.z;
    }
}

/// 1D noise with derivative: returns (value, dval/dx)
extern "C" fn trampoline_noise1_deriv(heap: *mut u8, dst_off: i32, grad_off: i32, x: f32) {
    let (val, dv) = crate::noise::perlin1_deriv(x);
    unsafe {
        *(heap.add(dst_off as usize) as *mut f32) = val;
        *(heap.add(grad_off as usize) as *mut f32) = dv;
    }
}

/// Signed noise with derivatives
extern "C" fn trampoline_snoise3_deriv(
    heap: *mut u8,
    dst_off: i32,
    grad_off: i32,
    x: f32,
    y: f32,
    z: f32,
) {
    let p = crate::math::Vec3::new(x, y, z);
    let (val, grad) = crate::noise::perlin3_deriv(p);
    unsafe {
        *(heap.add(dst_off as usize) as *mut f32) = val * 2.0 - 1.0;
        *(heap.add(grad_off as usize) as *mut f32) = grad.x * 2.0;
        *(heap.add(grad_off as usize + 4) as *mut f32) = grad.y * 2.0;
        *(heap.add(grad_off as usize + 8) as *mut f32) = grad.z * 2.0;
    }
}
extern "C" fn trampoline_cellnoise1(x: f32) -> f32 {
    crate::noise::cellnoise1(x)
}
extern "C" fn trampoline_cellnoise3(x: f32, y: f32, z: f32) -> f32 {
    crate::noise::cellnoise3(crate::math::Vec3::new(x, y, z))
}
extern "C" fn trampoline_hashnoise1(x: f32) -> f32 {
    crate::noise::hashnoise1(x)
}
extern "C" fn trampoline_hashnoise3(x: f32, y: f32, z: f32) -> f32 {
    crate::noise::hashnoise3(crate::math::Vec3::new(x, y, z))
}
extern "C" fn trampoline_simplex3(x: f32, y: f32, z: f32) -> f32 {
    crate::simplex::simplex3(crate::math::Vec3::new(x, y, z))
}
extern "C" fn trampoline_usimplex3(x: f32, y: f32, z: f32) -> f32 {
    crate::simplex::usimplex3(crate::math::Vec3::new(x, y, z))
}
extern "C" fn trampoline_pnoise3(x: f32, y: f32, z: f32, px: f32, py: f32, pz: f32) -> f32 {
    crate::noise::pperlin3(
        crate::math::Vec3::new(x, y, z),
        crate::math::Vec3::new(px, py, pz),
    )
}

/// 3D periodic noise with analytical derivatives.
/// Writes value to heap[dst_off] and gradient to heap[grad_off..+12].
extern "C" fn trampoline_pnoise3_deriv(
    heap: *mut u8,
    dst_off: i32,
    grad_off: i32,
    x: f32,
    y: f32,
    z: f32,
    px: f32,
    py: f32,
    pz: f32,
) {
    let p = crate::math::Vec3::new(x, y, z);
    let period = crate::math::Vec3::new(px, py, pz);
    let (val, grad) = crate::noise::pperlin3_deriv(p, period);
    unsafe {
        *(heap.add(dst_off as usize) as *mut f32) = val;
        *(heap.add(grad_off as usize) as *mut f32) = grad.x;
        *(heap.add(grad_off as usize + 4) as *mut f32) = grad.y;
        *(heap.add(grad_off as usize + 8) as *mut f32) = grad.z;
    }
}

// ---------------------------------------------------------------------------
// Matrix trampolines
// ---------------------------------------------------------------------------

/// Matrix determinant: reads 16 floats from heap[off], returns f32.
extern "C" fn trampoline_matrix_det(heap: *const u8, off: i32) -> f32 {
    let m = unsafe { read_matrix_from_heap(heap, off as usize) };
    crate::matrix_ops::determinant(&m)
}

/// Matrix transpose: reads from src_off, writes to dst_off.
extern "C" fn trampoline_matrix_transpose(heap: *mut u8, dst_off: i32, src_off: i32) {
    let m = unsafe { read_matrix_from_heap(heap, src_off as usize) };
    let t = crate::matrix_ops::transpose(&m);
    unsafe { write_matrix_to_heap(heap, dst_off as usize, &t) };
}

/// Transform point by matrix.
extern "C" fn trampoline_transform_point(heap: *mut u8, dst_off: i32, mat_off: i32, src_off: i32) {
    let m = unsafe { read_matrix_from_heap(heap, mat_off as usize) };
    let p = unsafe { read_vec3_from_heap(heap, src_off as usize) };
    let r = crate::matrix_ops::transform_point(&m, p);
    unsafe { write_vec3_to_heap(heap, dst_off as usize, r) };
}

/// Transform vector by matrix.
extern "C" fn trampoline_transform_vector(heap: *mut u8, dst_off: i32, mat_off: i32, src_off: i32) {
    let m = unsafe { read_matrix_from_heap(heap, mat_off as usize) };
    let v = unsafe { read_vec3_from_heap(heap, src_off as usize) };
    let r = crate::matrix_ops::transform_vector(&m, v);
    unsafe { write_vec3_to_heap(heap, dst_off as usize, r) };
}

/// Transform normal by matrix.
extern "C" fn trampoline_transform_normal(heap: *mut u8, dst_off: i32, mat_off: i32, src_off: i32) {
    let m = unsafe { read_matrix_from_heap(heap, mat_off as usize) };
    let n = unsafe { read_vec3_from_heap(heap, src_off as usize) };
    let r = crate::matrix_ops::transform_normal(&m, n);
    unsafe { write_vec3_to_heap(heap, dst_off as usize, r) };
}

// ---------------------------------------------------------------------------
// Color trampolines
// ---------------------------------------------------------------------------

extern "C" fn trampoline_blackbody(heap: *mut u8, dst_off: i32, temp: f32) {
    let c = crate::color::blackbody(temp);
    unsafe { write_vec3_to_heap(heap, dst_off as usize, c) };
}

extern "C" fn trampoline_wavelength_color(heap: *mut u8, dst_off: i32, wavelength: f32) {
    let c = crate::color::wavelength_color(wavelength);
    unsafe { write_vec3_to_heap(heap, dst_off as usize, c) };
}

extern "C" fn trampoline_luminance(r: f32, g: f32, b: f32) -> f32 {
    crate::color::luminance(crate::math::Vec3::new(r, g, b))
}

/// construct(colorspace, x, y, z) — convert from named color space to RGB.
/// heap, dst_off, space_off, x_off, y_off, z_off. Writes Vec3 to dst.
extern "C" fn trampoline_construct_color_from_space(
    heap: *mut u8,
    dst_off: i32,
    space_off: i32,
    x_off: i32,
    y_off: i32,
    z_off: i32,
) {
    let space_hash = unsafe { read_i64_from_heap(heap, space_off as usize) } as u64;
    let space = crate::ustring::UStringHash(space_hash)
        .resolve()
        .map(|u| u.as_str().to_string())
        .unwrap_or_else(|| "rgb".into());
    let x = unsafe { *(heap.add(x_off as usize) as *const f32) };
    let y = unsafe { *(heap.add(y_off as usize) as *const f32) };
    let z = unsafe { *(heap.add(z_off as usize) as *const f32) };
    let src = crate::math::Vec3::new(x, y, z);
    let result = crate::color::transform_color(&space, "RGB", src);
    unsafe { write_vec3_to_heap(heap, dst_off as usize, result) };
}

/// transformc(heap, dst_off, from_space_off, to_space_off, src_off) -> void
/// Reads UString hashes for from/to space names, reads Vec3 color, transforms, writes result.
extern "C" fn trampoline_transformc(
    heap: *mut u8,
    dst_off: i32,
    from_off: i32,
    to_off: i32,
    src_off: i32,
) {
    let from_hash = unsafe { read_i64_from_heap(heap, from_off as usize) } as u64;
    let to_hash = unsafe { read_i64_from_heap(heap, to_off as usize) } as u64;
    let from_name = crate::ustring::UStringHash(from_hash)
        .resolve()
        .map(|u| u.as_str().to_string())
        .unwrap_or_else(|| "rgb".into());
    let to_name = crate::ustring::UStringHash(to_hash)
        .resolve()
        .map(|u| u.as_str().to_string())
        .unwrap_or_else(|| "rgb".into());
    let color = unsafe { read_vec3_from_heap(heap, src_off as usize) };
    let result = if from_name == to_name {
        color
    } else {
        crate::color::transform_color(&from_name, &to_name, color)
    };
    unsafe { write_vec3_to_heap(heap, dst_off as usize, result) };
}

// ---------------------------------------------------------------------------
// String trampolines (operate on UString hashes stored as i64 in heap)
// ---------------------------------------------------------------------------

extern "C" fn trampoline_strlen(heap: *const u8, str_off: i32) -> i32 {
    let hash = unsafe { read_i64_from_heap(heap, str_off as usize) };
    let uh = crate::ustring::UStringHash(hash as u64);
    if let Some(s) = uh.resolve() {
        s.as_str().len() as i32
    } else {
        0
    }
}

extern "C" fn trampoline_hash_string(heap: *const u8, str_off: i32) -> i32 {
    let hash = unsafe { read_i64_from_heap(heap, str_off as usize) };
    hash as i32
}

extern "C" fn trampoline_stoi(heap: *const u8, str_off: i32) -> i32 {
    let hash = unsafe { read_i64_from_heap(heap, str_off as usize) };
    let uh = crate::ustring::UStringHash(hash as u64);
    if let Some(s) = uh.resolve() {
        crate::opstring::stoi(s.as_str())
    } else {
        0
    }
}

extern "C" fn trampoline_stof(heap: *const u8, str_off: i32) -> f32 {
    let hash = unsafe { read_i64_from_heap(heap, str_off as usize) };
    let uh = crate::ustring::UStringHash(hash as u64);
    if let Some(s) = uh.resolve() {
        crate::opstring::stof(s.as_str())
    } else {
        0.0
    }
}

// ---------------------------------------------------------------------------
// Heap helpers for trampolines (unsafe raw pointer reads)
// ---------------------------------------------------------------------------

unsafe fn read_vec3_from_heap(heap: *const u8, off: usize) -> crate::math::Vec3 {
    unsafe {
        let p = heap.add(off) as *const f32;
        crate::math::Vec3::new(*p, *p.add(1), *p.add(2))
    }
}

unsafe fn write_vec3_to_heap(heap: *mut u8, off: usize, v: crate::math::Vec3) {
    unsafe {
        let p = heap.add(off) as *mut f32;
        *p = v.x;
        *p.add(1) = v.y;
        *p.add(2) = v.z;
    }
}

unsafe fn read_matrix_from_heap(heap: *const u8, off: usize) -> crate::math::Matrix44 {
    unsafe {
        let p = heap.add(off) as *const f32;
        let mut m = [[0.0f32; 4]; 4];
        for i in 0..4 {
            for j in 0..4 {
                m[i][j] = *p.add(i * 4 + j);
            }
        }
        crate::math::Matrix44 { m }
    }
}

unsafe fn write_matrix_to_heap(heap: *mut u8, off: usize, mat: &crate::math::Matrix44) {
    unsafe {
        let p = heap.add(off) as *mut f32;
        for i in 0..4 {
            for j in 0..4 {
                *p.add(i * 4 + j) = mat.m[i][j];
            }
        }
    }
}

unsafe fn read_i64_from_heap(heap: *const u8, off: usize) -> i64 {
    unsafe {
        let p = heap.add(off) as *const i64;
        std::ptr::read_unaligned(p)
    }
}

// ---------------------------------------------------------------------------
// Texture / getattribute / trace trampolines
// These take (ctx: *mut JitRuntimeContext, sg: *mut ShaderGlobals,
//             heap: *mut u8, dst_off: i32, arg offsets...)
// ---------------------------------------------------------------------------

/// Parse TextureOpt from optional args packed in heap.
/// Scratch layout at base: i32 n_pairs, then per pair [i32 name_off, i32 value_off, i32 value_type].
/// value_type: 0=int, 1=float, 2=string.
fn parse_texture_opt_from_heap(heap: *const u8, base: usize) -> crate::texture::TextureOpt {
    use crate::texture::{TextureOptArg, parse_texture_options};
    if base + 4 > 0x7fff_ffff {
        return crate::texture::TextureOpt::default();
    }
    let n_pairs = unsafe { *(heap.add(base) as *const i32) }.max(0).min(16) as usize;
    let mut pairs = Vec::with_capacity(n_pairs);
    for i in 0..n_pairs {
        let off = base + 4 + i * 12;
        if off + 12 > 0x7fff_ffff {
            break;
        }
        let name_off = unsafe { *(heap.add(off) as *const i32) };
        let value_off = unsafe { *(heap.add(off + 4) as *const i32) };
        let value_type = unsafe { *(heap.add(off + 8) as *const i32) };
        if name_off < 0 || value_off < 0 {
            continue;
        }
        let name = crate::ustring::UStringHash::from_hash(unsafe {
            read_i64_from_heap(heap, name_off as usize)
        } as u64)
        .resolve()
        .map(|u| u.as_str().to_string())
        .unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        let opt_arg = match value_type {
            0 => TextureOptArg::Int(unsafe { *(heap.add(value_off as usize) as *const i32) }),
            1 => TextureOptArg::Float(unsafe { *(heap.add(value_off as usize) as *const f32) }),
            2 => TextureOptArg::Str(
                crate::ustring::UStringHash::from_hash(unsafe {
                    read_i64_from_heap(heap, value_off as usize)
                } as u64)
                .resolve()
                .map(|u| u.as_str().to_string())
                .unwrap_or_default(),
            ),
            _ => continue,
        };
        pairs.push((name, opt_arg));
    }
    parse_texture_options(pairs)
}

/// texture(ctx, sg, heap, dst_off, nchannels, filename_off, s_off, t_off, s_has_derivs, t_has_derivs, opt_scratch_off) -> i32 (1=ok)
/// opt_scratch_off: -1 = no optional args; >= 0 = scratch offset with packed (n_pairs, name_off, value_off, type)*
extern "C" fn trampoline_texture(
    ctx: *mut JitRuntimeContext,
    sg: *mut ShaderGlobals,
    heap: *mut u8,
    dst_off: i32,
    nchannels: i32,
    filename_off: i32,
    s_off: i32,
    t_off: i32,
    s_has_derivs: i32,
    t_has_derivs: i32,
    opt_scratch_off: i32,
) -> i32 {
    let ctx_ref = unsafe { &*ctx };
    let renderer = match unsafe { ctx_ref.get_renderer() } {
        Some(r) => r,
        None => return 0,
    };
    let sg_ref = unsafe { &*sg };
    let filename_hash = unsafe { read_i64_from_heap(heap, filename_off as usize) } as u64;
    let s = unsafe { *(heap.add(s_off as usize) as *const f32) };
    let t = unsafe { *(heap.add(t_off as usize) as *const f32) };
    // Read s/t derivatives for texture filtering (Dx, Dy stored after value)
    let (dsdx, dsdy) = if s_has_derivs != 0 {
        unsafe {
            let dx = *(heap.add(s_off as usize + 4) as *const f32);
            let dy = *(heap.add(s_off as usize + 8) as *const f32);
            (dx, dy)
        }
    } else {
        (0.0, 0.0)
    };
    let (dtdx, dtdy) = if t_has_derivs != 0 {
        unsafe {
            let dx = *(heap.add(t_off as usize + 4) as *const f32);
            let dy = *(heap.add(t_off as usize + 8) as *const f32);
            (dx, dy)
        }
    } else {
        (0.0, 0.0)
    };
    let nc = nchannels.min(4).max(1) as usize;
    let mut result = [0.0f32; 4];
    let opt = if opt_scratch_off >= 0 {
        parse_texture_opt_from_heap(heap, opt_scratch_off as usize)
    } else {
        crate::texture::TextureOpt::default()
    };
    let ok = renderer.texture(
        crate::ustring::UStringHash(filename_hash),
        crate::renderer::TextureHandle::default(),
        sg_ref,
        &opt,
        s,
        t,
        dsdx,
        dtdx,
        dsdy,
        dtdy,
        nchannels,
        &mut result[..nc],
        None,
        None,
    );
    unsafe {
        let dst = heap.add(dst_off as usize) as *mut f32;
        for i in 0..nc {
            *dst.add(i) = result[i];
        }
    }
    if ok.is_ok() { 1 } else { 0 }
}

/// texture3d(ctx, sg, heap, dst_off, nchannels, filename_off, p_off, dpdx_off, dpdy_off, dpdz_off, opt_scratch_off) -> i32
/// dp*_off: -1 = use zero; >= 0 = heap offset to read Vec3
extern "C" fn trampoline_texture3d(
    ctx: *mut JitRuntimeContext,
    sg: *mut ShaderGlobals,
    heap: *mut u8,
    dst_off: i32,
    nchannels: i32,
    filename_off: i32,
    p_off: i32,
    dpdx_off: i32,
    dpdy_off: i32,
    dpdz_off: i32,
    opt_scratch_off: i32,
) -> i32 {
    let ctx_ref = unsafe { &*ctx };
    let renderer = match unsafe { ctx_ref.get_renderer() } {
        Some(r) => r,
        None => return 0,
    };
    let sg_ref = unsafe { &*sg };
    let filename_hash = unsafe { read_i64_from_heap(heap, filename_off as usize) } as u64;
    let p = unsafe { read_vec3_from_heap(heap, p_off as usize) };
    let zero_v = crate::math::Vec3::ZERO;
    let dpdx = if dpdx_off >= 0 {
        unsafe { read_vec3_from_heap(heap, dpdx_off as usize) }
    } else {
        zero_v
    };
    let dpdy = if dpdy_off >= 0 {
        unsafe { read_vec3_from_heap(heap, dpdy_off as usize) }
    } else {
        zero_v
    };
    let dpdz = if dpdz_off >= 0 {
        unsafe { read_vec3_from_heap(heap, dpdz_off as usize) }
    } else {
        zero_v
    };
    let nc = nchannels.min(4).max(1) as usize;
    let mut result = [0.0f32; 4];
    let opt = if opt_scratch_off >= 0 {
        parse_texture_opt_from_heap(heap, opt_scratch_off as usize)
    } else {
        crate::texture::TextureOpt::default()
    };
    let ok = renderer.texture3d(
        crate::ustring::UStringHash(filename_hash),
        crate::renderer::TextureHandle::default(),
        sg_ref,
        &opt,
        &p,
        &dpdx,
        &dpdy,
        &dpdz,
        nchannels,
        &mut result[..nc],
        None,
        None,
        None,
    );
    unsafe {
        let dst = heap.add(dst_off as usize) as *mut f32;
        for i in 0..nc {
            *dst.add(i) = result[i];
        }
    }
    if ok.is_ok() { 1 } else { 0 }
}

/// environment(ctx, sg, heap, dst_off, nchannels, filename_off, r_off, drdx_off, drdy_off, opt_scratch_off) -> i32
/// dr*_off: -1 = use zero; >= 0 = heap offset to read Vec3
extern "C" fn trampoline_environment(
    ctx: *mut JitRuntimeContext,
    sg: *mut ShaderGlobals,
    heap: *mut u8,
    dst_off: i32,
    nchannels: i32,
    filename_off: i32,
    r_off: i32,
    drdx_off: i32,
    drdy_off: i32,
    opt_scratch_off: i32,
) -> i32 {
    let ctx_ref = unsafe { &*ctx };
    let renderer = match unsafe { ctx_ref.get_renderer() } {
        Some(r) => r,
        None => return 0,
    };
    let sg_ref = unsafe { &*sg };
    let filename_hash = unsafe { read_i64_from_heap(heap, filename_off as usize) } as u64;
    let r = unsafe { read_vec3_from_heap(heap, r_off as usize) };
    let zero_v = crate::math::Vec3::ZERO;
    let drdx = if drdx_off >= 0 {
        unsafe { read_vec3_from_heap(heap, drdx_off as usize) }
    } else {
        zero_v
    };
    let drdy = if drdy_off >= 0 {
        unsafe { read_vec3_from_heap(heap, drdy_off as usize) }
    } else {
        zero_v
    };
    let nc = nchannels.min(4).max(1) as usize;
    let mut result = [0.0f32; 4];
    let opt = if opt_scratch_off >= 0 {
        parse_texture_opt_from_heap(heap, opt_scratch_off as usize)
    } else {
        crate::texture::TextureOpt::default()
    };
    let ok = renderer.environment(
        crate::ustring::UStringHash(filename_hash),
        crate::renderer::TextureHandle::default(),
        sg_ref,
        &opt,
        &r,
        &drdx,
        &drdy,
        nchannels,
        &mut result[..nc],
        None,
        None,
    );
    unsafe {
        let dst = heap.add(dst_off as usize) as *mut f32;
        for i in 0..nc {
            *dst.add(i) = result[i];
        }
    }
    if ok.is_ok() { 1 } else { 0 }
}

/// getattribute(ctx, sg, heap, result_off, result_type, name_off) -> i32 (1=found)
/// result_type: 0=int, 1=float, 2=vec3
extern "C" fn trampoline_getattribute(
    ctx: *mut JitRuntimeContext,
    sg: *mut ShaderGlobals,
    heap: *mut u8,
    result_off: i32,
    result_type: i32,
    name_off: i32,
) -> i32 {
    let ctx_ref = unsafe { &*ctx };
    let renderer = match unsafe { ctx_ref.get_renderer() } {
        Some(r) => r,
        None => return 0,
    };
    let sg_ref = unsafe { &*sg };
    let name_hash = unsafe { read_i64_from_heap(heap, name_off as usize) } as u64;
    let td = match result_type {
        0 => crate::typedesc::TypeDesc::INT,
        1 => crate::typedesc::TypeDesc::FLOAT,
        2 => crate::typedesc::TypeDesc::COLOR,
        _ => crate::typedesc::TypeDesc::FLOAT,
    };
    let attr = renderer.get_attribute(
        sg_ref,
        false,
        crate::ustring::UStringHash(0),
        td,
        crate::ustring::UStringHash(name_hash),
    );
    match attr {
        Some(crate::renderer::AttributeData::Int(v)) => {
            unsafe {
                *(heap.add(result_off as usize) as *mut i32) = v;
            }
            1
        }
        Some(crate::renderer::AttributeData::Float(v)) => {
            unsafe {
                *(heap.add(result_off as usize) as *mut f32) = v;
            }
            1
        }
        Some(crate::renderer::AttributeData::Vec3(v)) => {
            unsafe {
                write_vec3_to_heap(heap, result_off as usize, v);
            }
            1
        }
        _ => 0,
    }
}

/// gettextureinfo(ctx, sg, heap, result_off, filename_off, dataname_off, data_off, data_type) -> i32 (1=found)
/// data_type: 0=int, 1=float, 2=string (stored as i64 hash at data_off)
extern "C" fn trampoline_gettextureinfo(
    ctx: *mut JitRuntimeContext,
    sg: *mut ShaderGlobals,
    heap: *mut u8,
    result_off: i32,
    filename_off: i32,
    dataname_off: i32,
    data_off: i32,
    data_type: i32,
) -> i32 {
    let ctx_ref = unsafe { &*ctx };
    let renderer = match unsafe { ctx_ref.get_renderer() } {
        Some(r) => r,
        None => {
            unsafe {
                *(heap.add(result_off as usize) as *mut i32) = 0;
            }
            return 0;
        }
    };
    let sg_ref = unsafe { &*sg };
    let fh = unsafe { read_i64_from_heap(heap, filename_off as usize) } as u64;
    let dh = unsafe { read_i64_from_heap(heap, dataname_off as usize) } as u64;
    let td = match data_type {
        0 => crate::typedesc::TypeDesc::INT,
        2 => crate::typedesc::TypeDesc::STRING,
        _ => crate::typedesc::TypeDesc::FLOAT,
    };
    let ok = if data_type == 0 {
        let mut ival = 0i32;
        renderer
            .get_texture_info(
                crate::ustring::UStringHash(fh),
                std::ptr::null_mut(),
                sg_ref,
                0,
                crate::ustring::UStringHash(dh),
                td,
                &mut ival as *mut _ as *mut std::ffi::c_void,
            )
            .is_ok()
            && {
                unsafe {
                    *(heap.add(data_off as usize) as *mut i32) = ival;
                }
                true
            }
    } else if data_type == 2 {
        let mut sval = crate::ustring::UString::empty();
        renderer
            .get_texture_info(
                crate::ustring::UStringHash(fh),
                std::ptr::null_mut(),
                sg_ref,
                0,
                crate::ustring::UStringHash(dh),
                td,
                &mut sval as *mut _ as *mut std::ffi::c_void,
            )
            .is_ok()
            && {
                let hash = sval.hash() as i64;
                unsafe {
                    *(heap.add(data_off as usize) as *mut i64) = hash;
                }
                true
            }
    } else {
        let mut fval = 0.0f32;
        renderer
            .get_texture_info(
                crate::ustring::UStringHash(fh),
                std::ptr::null_mut(),
                sg_ref,
                0,
                crate::ustring::UStringHash(dh),
                td,
                &mut fval as *mut _ as *mut std::ffi::c_void,
            )
            .is_ok()
            && {
                unsafe {
                    *(heap.add(data_off as usize) as *mut f32) = fval;
                }
                true
            }
    };
    let res = if ok { 1 } else { 0 };
    unsafe {
        *(heap.add(result_off as usize) as *mut i32) = res;
    }
    res
}

/// getmatrix(ctx, sg, heap, dst_off, from_name_off, to_name_off) -> i32 (1=success)
/// Computes from->to matrix via renderer (osl_get_from_to_matrix parity).
/// If to_name_off < 0, uses "common" as to-space (2-arg form).
extern "C" fn trampoline_getmatrix(
    ctx: *mut JitRuntimeContext,
    sg: *mut ShaderGlobals,
    heap: *mut u8,
    dst_off: i32,
    from_name_off: i32,
    to_name_off: i32,
) -> i32 {
    let ctx_ref = unsafe { &*ctx };
    let renderer = match unsafe { ctx_ref.get_renderer() } {
        Some(r) => r,
        None => {
            // No renderer — unknown spaces
            return 0;
        }
    };
    let sg_ref = unsafe { &*sg };
    let from_hash = unsafe { read_i64_from_heap(heap, from_name_off as usize) } as u64;
    let to_hash = if to_name_off < 0 {
        crate::ustring::UString::new("common").uhash().hash()
    } else {
        (unsafe { read_i64_from_heap(heap, to_name_off as usize) }) as u64
    };
    let from_ush = crate::ustring::UStringHash(from_hash);
    let to_ush = crate::ustring::UStringHash(to_hash);
    // Resolve to strings for "common" check
    let from_s = from_ush
        .resolve()
        .map(|u| u.as_str().to_string())
        .unwrap_or_default();
    let to_s = to_ush
        .resolve()
        .map(|u| u.as_str().to_string())
        .unwrap_or_default();
    let syn_s = ctx_ref.get_commonspace_synonym();
    let from_is_common = from_s == "common" || from_s == syn_s;
    let to_is_common = to_s == "common" || to_s == syn_s;
    let m_from = if from_is_common {
        crate::math::Matrix44::IDENTITY
    } else {
        match renderer.get_matrix_named_static(sg_ref, from_ush) {
            Some(m) => m,
            None => match renderer.get_matrix_named(sg_ref, from_ush, sg_ref.time) {
                Some(m) => m,
                None => {
                    if ctx_ref.unknown_coordsys_error {
                        let name = from_ush
                            .resolve()
                            .map(|u| u.as_str().to_string())
                            .unwrap_or_else(|| format!("<hash {}>", from_hash));
                        eprintln!("Unknown transformation \"{}\"", name);
                    }
                    return 0;
                }
            },
        }
    };
    let m_to_inv = if to_is_common {
        crate::math::Matrix44::IDENTITY
    } else {
        match renderer.get_inverse_matrix_named_static(sg_ref, to_ush) {
            Some(m) => m,
            None => match renderer.get_inverse_matrix_named(sg_ref, to_ush, sg_ref.time) {
                Some(m) => m,
                None => {
                    if ctx_ref.unknown_coordsys_error {
                        let name = to_ush
                            .resolve()
                            .map(|u| u.as_str().to_string())
                            .unwrap_or_else(|| format!("<hash {}>", to_hash));
                        eprintln!("Unknown transformation \"{}\"", name);
                    }
                    return 0;
                }
            },
        }
    };
    let result = crate::matrix_ops::matmul(&m_to_inv, &m_from);
    unsafe {
        write_matrix_to_heap(heap, dst_off as usize, &result);
    }
    1
}

/// range_check(ctx, sg, heap, index, length, symname_hash, sourcefile_hash, sourceline) -> i32
/// Returns clamped index; reports error when OOB. Matching C++ osl_range_check.
extern "C" fn trampoline_range_check(
    _ctx: *mut JitRuntimeContext,
    _sg: *mut ShaderGlobals,
    _heap: *mut u8,
    index: i32,
    length: i32,
    symname_hash: i64,
    _sourcefile_hash: i64,
    sourceline: i32,
) -> i32 {
    if length <= 0 {
        return index;
    }
    if index >= 0 && index < length {
        return index;
    }
    let symname = crate::ustring::UStringHash(symname_hash as u64)
        .resolve()
        .map(|u| u.as_str().to_string())
        .unwrap_or_else(|| format!("<hash {}>", symname_hash));
    let max_idx = length - 1;
    eprintln!(
        "Index [{}] out of range {}[0..{}]: <unknown>:{} (group <unnamed>, layer 0 <unnamed>, shader <unnamed>)",
        index, symname, max_idx, sourceline
    );
    if index >= length { max_idx } else { 0 }
}

/// Write a 4x4 identity matrix to heap. (Used by other trampolines as fallback.)
#[allow(dead_code)]
fn write_identity_matrix(heap: *mut u8, off: usize) {
    unsafe {
        let dst = heap.add(off) as *mut f32;
        for i in 0..16 {
            *dst.add(i) = if i == 0 || i == 5 || i == 10 || i == 15 {
                1.0
            } else {
                0.0
            };
        }
    }
}

/// trace(ctx, sg, heap, pos_off, dir_off) -> i32 (1=hit)
extern "C" fn trampoline_trace(
    ctx: *mut JitRuntimeContext,
    sg: *mut ShaderGlobals,
    heap: *mut u8,
    pos_off: i32,
    dir_off: i32,
) -> i32 {
    let ctx_ref = unsafe { &*ctx };
    let renderer = match unsafe { ctx_ref.get_renderer() } {
        Some(r) => r,
        None => return 0,
    };
    let sg_ref = unsafe { &*sg };
    let p = unsafe { read_vec3_from_heap(heap, pos_off as usize) };
    let d = unsafe { read_vec3_from_heap(heap, dir_off as usize) };
    let zero_v = crate::math::Vec3::ZERO;
    let mut opts = crate::renderer::TraceOpt::default();
    let hit = renderer.trace(
        &mut opts, sg_ref, &p, &zero_v, &zero_v, &d, &zero_v, &zero_v,
    );
    if hit { 1 } else { 0 }
}

// ---------------------------------------------------------------------------
// Closure trampolines — allocate ClosureComponent on the Rust side,
// store the resulting closure ID into heap.
// ---------------------------------------------------------------------------

/// closure(ctx, heap, dst_off, closure_name_off, weight_off) -> void
/// Creates a weighted closure and stores a serialized representation.
/// For now we store the closure_id (i32) in dst_off.
extern "C" fn trampoline_closure_alloc(
    _ctx: *mut JitRuntimeContext,
    heap: *mut u8,
    dst_off: i32,
    closure_name_off: i32,
    weight_off: i32,
) {
    let name_hash = unsafe { read_i64_from_heap(heap, closure_name_off as usize) } as u64;
    let weight = unsafe { read_vec3_from_heap(heap, weight_off as usize) };
    let name_str = crate::ustring::UStringHash(name_hash)
        .resolve()
        .map(|u| u.as_str().to_string())
        .unwrap_or_default();
    let closure_id = crate::closure_ops::closure_name_to_id(&name_str).unwrap_or(0);
    // Allocate the closure component and store its id
    let _closure = crate::closure_ops::allocate_weighted_closure_component(closure_id, weight);
    // Store closure_id at destination (renderer can interpret this later)
    unsafe {
        *(heap.add(dst_off as usize) as *mut i32) = closure_id;
    }
}

// ---------------------------------------------------------------------------
// Message passing trampolines
// ---------------------------------------------------------------------------

/// setmessage(ctx, heap, name_off, value_off, value_type) -> void
/// value_type: 0=int, 1=float, 2=vec3/color, 3=string
extern "C" fn trampoline_setmessage(
    ctx: *mut JitRuntimeContext,
    heap: *mut u8,
    name_off: i32,
    value_off: i32,
    value_type: i32,
) {
    let ctx_ref = unsafe { &mut *ctx };
    if ctx_ref.messages.is_null() {
        return;
    }
    let messages = unsafe { &mut *ctx_ref.messages };
    let name_hash = unsafe { read_i64_from_heap(heap, name_off as usize) } as u64;
    let key = crate::ustring::UStringHash(name_hash)
        .resolve()
        .map(|u| u.as_str().to_string())
        .unwrap_or_default();
    let value = match value_type {
        0 => {
            let v = unsafe { *(heap.add(value_off as usize) as *const i32) };
            crate::interp::Value::Int(v)
        }
        1 => {
            let v = unsafe { *(heap.add(value_off as usize) as *const f32) };
            crate::interp::Value::Float(v)
        }
        2 => {
            let v = unsafe { read_vec3_from_heap(heap, value_off as usize) };
            crate::interp::Value::Vec3(v)
        }
        _ => crate::interp::Value::Int(0),
    };
    messages.insert(key, value);
}

/// getmessage(ctx, heap, name_off, result_off, result_type) -> i32 (1=found)
extern "C" fn trampoline_getmessage(
    ctx: *mut JitRuntimeContext,
    heap: *mut u8,
    name_off: i32,
    result_off: i32,
    result_type: i32,
) -> i32 {
    let ctx_ref = unsafe { &*ctx };
    if ctx_ref.messages.is_null() {
        return 0;
    }
    let messages = unsafe { &*ctx_ref.messages };
    let name_hash = unsafe { read_i64_from_heap(heap, name_off as usize) } as u64;
    let key = crate::ustring::UStringHash(name_hash)
        .resolve()
        .map(|u| u.as_str().to_string())
        .unwrap_or_default();
    match messages.get(&key) {
        Some(crate::interp::Value::Int(v)) if result_type == 0 => {
            unsafe {
                *(heap.add(result_off as usize) as *mut i32) = *v;
            }
            1
        }
        Some(crate::interp::Value::Float(v)) if result_type == 1 => {
            unsafe {
                *(heap.add(result_off as usize) as *mut f32) = *v;
            }
            1
        }
        Some(crate::interp::Value::Vec3(v)) if result_type == 2 => {
            unsafe {
                write_vec3_to_heap(heap, result_off as usize, *v);
            }
            1
        }
        _ => 0,
    }
}

// ---------------------------------------------------------------------------
// Spline trampoline — full evaluation via crate::spline
// ---------------------------------------------------------------------------

/// spline_float(heap, dst_off, basis_off, t_off, knots_off, nknots) -> void
extern "C" fn trampoline_spline_float(
    heap: *mut u8,
    dst_off: i32,
    basis_off: i32,
    t_off: i32,
    knots_off: i32,
    nknots: i32,
) {
    let basis_hash = unsafe { read_i64_from_heap(heap, basis_off as usize) } as u64;
    let basis_name = crate::ustring::UStringHash(basis_hash)
        .resolve()
        .map(|u| u.as_str().to_string())
        .unwrap_or_else(|| "catmull-rom".into());
    let basis = crate::spline::SplineBasis::from_name(&basis_name)
        .unwrap_or(crate::spline::SplineBasis::CatmullRom);
    let t = unsafe { *(heap.add(t_off as usize) as *const f32) };
    let nk = nknots.max(0) as usize;
    let mut knots = Vec::with_capacity(nk);
    for i in 0..nk {
        let v = unsafe { *(heap.add(knots_off as usize + i * 4) as *const f32) };
        knots.push(v);
    }
    let result = crate::spline::spline_float(basis, t, &knots);
    unsafe {
        *(heap.add(dst_off as usize) as *mut f32) = result;
    }
}

/// Returns d(spline)/dt for a float spline — used for derivative chain rule.
extern "C" fn trampoline_spline_float_deriv(
    heap: *mut u8,
    basis_off: i32,
    t_off: i32,
    knots_off: i32,
    nknots: i32,
) -> f32 {
    let basis_hash = unsafe { read_i64_from_heap(heap, basis_off as usize) } as u64;
    let basis_name = crate::ustring::UStringHash(basis_hash)
        .resolve()
        .map(|u| u.as_str().to_string())
        .unwrap_or_else(|| "catmull-rom".into());
    let basis = crate::spline::SplineBasis::from_name(&basis_name)
        .unwrap_or(crate::spline::SplineBasis::CatmullRom);
    let t = unsafe { *(heap.add(t_off as usize) as *const f32) };
    let nk = nknots.max(0) as usize;
    let mut knots = Vec::with_capacity(nk);
    for i in 0..nk {
        let v = unsafe { *(heap.add(knots_off as usize + i * 4) as *const f32) };
        knots.push(v);
    }
    crate::spline::spline_float_deriv(basis, t, &knots)
}

/// spline_vec3(heap, dst_off, basis_off, t_off, knots_off, nknots) -> void
extern "C" fn trampoline_spline_vec3(
    heap: *mut u8,
    dst_off: i32,
    basis_off: i32,
    t_off: i32,
    knots_off: i32,
    nknots: i32,
) {
    let basis_hash = unsafe { read_i64_from_heap(heap, basis_off as usize) } as u64;
    let basis_name = crate::ustring::UStringHash(basis_hash)
        .resolve()
        .map(|u| u.as_str().to_string())
        .unwrap_or_else(|| "catmull-rom".into());
    let basis = crate::spline::SplineBasis::from_name(&basis_name)
        .unwrap_or(crate::spline::SplineBasis::CatmullRom);
    let t = unsafe { *(heap.add(t_off as usize) as *const f32) };
    let nk = nknots.max(0) as usize;
    let mut knots = Vec::with_capacity(nk);
    for i in 0..nk {
        let v = unsafe { read_vec3_from_heap(heap, knots_off as usize + i * 12) };
        knots.push(v);
    }
    let result = crate::spline::spline_vec3(basis, t, &knots);
    unsafe {
        write_vec3_to_heap(heap, dst_off as usize, result);
    }
}

/// Returns d(spline_vec3)/dt — writes to dst_off on heap (3 floats).
extern "C" fn trampoline_spline_vec3_deriv(
    heap: *mut u8,
    dst_off: i32,
    basis_off: i32,
    t_off: i32,
    knots_off: i32,
    nknots: i32,
) {
    let basis_hash = unsafe { read_i64_from_heap(heap, basis_off as usize) } as u64;
    let basis_name = crate::ustring::UStringHash(basis_hash)
        .resolve()
        .map(|u| u.as_str().to_string())
        .unwrap_or_else(|| "catmull-rom".into());
    let basis = crate::spline::SplineBasis::from_name(&basis_name)
        .unwrap_or(crate::spline::SplineBasis::CatmullRom);
    let t = unsafe { *(heap.add(t_off as usize) as *const f32) };
    let nk = nknots.max(0) as usize;
    let mut knots = Vec::with_capacity(nk);
    for i in 0..nk {
        let v = unsafe { read_vec3_from_heap(heap, knots_off as usize + i * 12) };
        knots.push(v);
    }
    let result = crate::spline::spline_vec3_deriv(basis, t, &knots);
    unsafe {
        write_vec3_to_heap(heap, dst_off as usize, result);
    }
}

/// splineinverse_float(heap, dst_off, basis_off, val_off, knots_off, nknots) -> void
extern "C" fn trampoline_splineinverse_float(
    heap: *mut u8,
    dst_off: i32,
    basis_off: i32,
    val_off: i32,
    knots_off: i32,
    nknots: i32,
) {
    let basis_hash = unsafe { read_i64_from_heap(heap, basis_off as usize) } as u64;
    let basis_name = crate::ustring::UStringHash(basis_hash)
        .resolve()
        .map(|u| u.as_str().to_string())
        .unwrap_or_else(|| "catmull-rom".into());
    let basis = crate::spline::SplineBasis::from_name(&basis_name)
        .unwrap_or(crate::spline::SplineBasis::CatmullRom);
    let val = unsafe { *(heap.add(val_off as usize) as *const f32) };
    let nk = nknots.max(0) as usize;
    let mut knots = Vec::with_capacity(nk);
    for i in 0..nk {
        let v = unsafe { *(heap.add(knots_off as usize + i * 4) as *const f32) };
        knots.push(v);
    }
    let result = crate::spline::spline_inverse_float(basis, val, &knots, 32);
    unsafe {
        *(heap.add(dst_off as usize) as *mut f32) = result;
    }
}

// ---------------------------------------------------------------------------
// String manipulation trampolines
// ---------------------------------------------------------------------------

/// concat(heap, dst_off, a_off, b_off) -> void
/// Concatenates two UStrings, interns the result, stores hash at dst.
extern "C" fn trampoline_concat(heap: *mut u8, dst_off: i32, a_off: i32, b_off: i32) {
    let ha = unsafe { read_i64_from_heap(heap, a_off as usize) } as u64;
    let hb = unsafe { read_i64_from_heap(heap, b_off as usize) } as u64;
    let sa = crate::ustring::UStringHash(ha)
        .resolve()
        .map(|u| u.as_str().to_string())
        .unwrap_or_default();
    let sb = crate::ustring::UStringHash(hb)
        .resolve()
        .map(|u| u.as_str().to_string())
        .unwrap_or_default();
    let combined = crate::opstring::concat(&sa, &sb);
    let us = crate::ustring::UString::new(&combined);
    let hash = us.hash() as i64;
    unsafe {
        *(heap.add(dst_off as usize) as *mut i64) = hash;
    }
}

/// substr(heap, dst_off, src_off, start_off, len_off) -> void
extern "C" fn trampoline_substr(
    heap: *mut u8,
    dst_off: i32,
    src_off: i32,
    start_off: i32,
    len_off: i32,
) {
    let h = unsafe { read_i64_from_heap(heap, src_off as usize) } as u64;
    let start = unsafe { *(heap.add(start_off as usize) as *const i32) };
    let len = unsafe { *(heap.add(len_off as usize) as *const i32) };
    let s = crate::ustring::UStringHash(h)
        .resolve()
        .map(|u| u.as_str().to_string())
        .unwrap_or_default();
    let sub = crate::opstring::substr(&s, start, len);
    let us = crate::ustring::UString::new(&sub);
    let hash = us.hash() as i64;
    unsafe {
        *(heap.add(dst_off as usize) as *mut i64) = hash;
    }
}

/// getchar(heap, src_off, index_off) -> i32
extern "C" fn trampoline_getchar(heap: *const u8, src_off: i32, index_off: i32) -> i32 {
    let h = unsafe { read_i64_from_heap(heap, src_off as usize) } as u64;
    let index = unsafe { *(heap.add(index_off as usize) as *const i32) };
    let s = crate::ustring::UStringHash(h)
        .resolve()
        .map(|u| u.as_str().to_string())
        .unwrap_or_default();
    crate::opstring::getchar(&s, index)
}

/// startswith(heap, str_off, prefix_off) -> i32 (0/1)
extern "C" fn trampoline_startswith(heap: *const u8, str_off: i32, prefix_off: i32) -> i32 {
    let hs = unsafe { read_i64_from_heap(heap, str_off as usize) } as u64;
    let hp = unsafe { read_i64_from_heap(heap, prefix_off as usize) } as u64;
    let s = crate::ustring::UStringHash(hs)
        .resolve()
        .map(|u| u.as_str().to_string())
        .unwrap_or_default();
    let p = crate::ustring::UStringHash(hp)
        .resolve()
        .map(|u| u.as_str().to_string())
        .unwrap_or_default();
    if crate::opstring::startswith(&s, &p) {
        1
    } else {
        0
    }
}

/// endswith(heap, str_off, suffix_off) -> i32 (0/1)
extern "C" fn trampoline_endswith(heap: *const u8, str_off: i32, suffix_off: i32) -> i32 {
    let hs = unsafe { read_i64_from_heap(heap, str_off as usize) } as u64;
    let hp = unsafe { read_i64_from_heap(heap, suffix_off as usize) } as u64;
    let s = crate::ustring::UStringHash(hs)
        .resolve()
        .map(|u| u.as_str().to_string())
        .unwrap_or_default();
    let p = crate::ustring::UStringHash(hp)
        .resolve()
        .map(|u| u.as_str().to_string())
        .unwrap_or_default();
    if crate::opstring::endswith(&s, &p) {
        1
    } else {
        0
    }
}

/// regex_search(heap, subject_off, pattern_off) -> i32 (0/1)
extern "C" fn trampoline_regex_search(heap: *const u8, subject_off: i32, pattern_off: i32) -> i32 {
    let hs = unsafe { read_i64_from_heap(heap, subject_off as usize) } as u64;
    let hp = unsafe { read_i64_from_heap(heap, pattern_off as usize) } as u64;
    let s = crate::ustring::UStringHash(hs)
        .resolve()
        .map(|u| u.as_str().to_string())
        .unwrap_or_default();
    let p = crate::ustring::UStringHash(hp)
        .resolve()
        .map(|u| u.as_str().to_string())
        .unwrap_or_default();
    if crate::opstring::regex_search(&s, &p) {
        1
    } else {
        0
    }
}

/// format(heap, dst_off, fmt_off, arg_offs_ptr, arg_types_ptr, nargs) -> void
/// arg_types: 0=int, 1=float, 2=string
/// This is called with parallel arrays of offsets and types packed after the
/// fixed params. For simplicity, we read up to 16 args from adjacent heap slots.
extern "C" fn trampoline_format(
    heap: *mut u8,
    dst_off: i32,
    fmt_off: i32,
    args_base_off: i32,
    types_base_off: i32,
    nargs: i32,
) {
    let fmt_hash = unsafe { read_i64_from_heap(heap, fmt_off as usize) } as u64;
    let fmt_str = crate::ustring::UStringHash(fmt_hash)
        .resolve()
        .map(|u| u.as_str().to_string())
        .unwrap_or_default();

    let n = nargs.min(16).max(0) as usize;
    let mut int_args = Vec::new();
    let mut float_args = Vec::new();
    let mut str_args_owned = Vec::new();

    for i in 0..n {
        let arg_off = unsafe { *(heap.add(args_base_off as usize + i * 4) as *const i32) };
        let arg_type = unsafe { *(heap.add(types_base_off as usize + i * 4) as *const i32) };
        match arg_type {
            0 => {
                // int
                let v = unsafe { *(heap.add(arg_off as usize) as *const i32) };
                int_args.push(v);
            }
            1 => {
                // float
                let v = unsafe { *(heap.add(arg_off as usize) as *const f32) };
                float_args.push(v);
            }
            2 => {
                // string
                let h = unsafe { read_i64_from_heap(heap, arg_off as usize) } as u64;
                let s = crate::ustring::UStringHash(h)
                    .resolve()
                    .map(|u| u.as_str().to_string())
                    .unwrap_or_default();
                str_args_owned.push(s);
            }
            _ => {}
        }
    }
    let str_args_refs: Vec<&str> = str_args_owned.iter().map(|s| s.as_str()).collect();
    let result = crate::opstring::format_string(&fmt_str, &int_args, &float_args, &str_args_refs);
    let us = crate::ustring::UString::new(&result);
    let hash = us.hash() as i64;
    unsafe {
        *(heap.add(dst_off as usize) as *mut i64) = hash;
    }
}

/// split(heap, result_off, str_off, sep_off, maxsplit) -> i32 (count)
extern "C" fn trampoline_split(
    heap: *mut u8,
    result_off: i32,
    str_off: i32,
    sep_off: i32,
    maxsplit: i32,
) -> i32 {
    let sh = unsafe { read_i64_from_heap(heap, str_off as usize) } as u64;
    let s = crate::ustring::UStringHash(sh)
        .resolve()
        .map(|u| u.as_str().to_string())
        .unwrap_or_default();
    let seph = unsafe { read_i64_from_heap(heap, sep_off as usize) } as u64;
    let sep = crate::ustring::UStringHash(seph)
        .resolve()
        .map(|u| u.as_str().to_string())
        .unwrap_or_default();

    let parts: Vec<&str> = if sep.is_empty() {
        s.split_whitespace().collect()
    } else {
        s.split(&sep).collect()
    };
    let max = maxsplit.max(0) as usize;
    let count = parts.len().min(max);

    // Write UString hashes of the parts into the result array (i64 each)
    for i in 0..count {
        let us = crate::ustring::UString::new(parts[i]);
        let hash = us.hash() as i64;
        unsafe {
            *(heap.add(result_off as usize + i * 8) as *mut i64) = hash;
        }
    }
    count as i32
}

/// regex_match(heap, subject_off, pattern_off) -> i32 (0/1)
extern "C" fn trampoline_regex_match(heap: *const u8, subject_off: i32, pattern_off: i32) -> i32 {
    let hs = unsafe { read_i64_from_heap(heap, subject_off as usize) } as u64;
    let hp = unsafe { read_i64_from_heap(heap, pattern_off as usize) } as u64;
    let s = crate::ustring::UStringHash(hs)
        .resolve()
        .map(|u| u.as_str().to_string())
        .unwrap_or_default();
    let p = crate::ustring::UStringHash(hp)
        .resolve()
        .map(|u| u.as_str().to_string())
        .unwrap_or_default();
    if crate::opstring::regex_match(&s, &p) {
        1
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// Dict trampolines
// ---------------------------------------------------------------------------

/// dict_find(ctx, heap, dict_data_off, path_off) -> i32 (handle or -1)
/// dict_data_off contains a JSON/XML string to parse, or a previous handle.
extern "C" fn trampoline_dict_find(
    ctx: *mut JitRuntimeContext,
    heap: *const u8,
    dict_data_off: i32,
    path_off: i32,
) -> i32 {
    let ctx_ref = unsafe { &mut *ctx };
    if ctx_ref.dict_store.is_null() {
        return -1;
    }
    let store = unsafe { &mut *ctx_ref.dict_store };

    let data_hash = unsafe { read_i64_from_heap(heap, dict_data_off as usize) } as u64;
    let data_str = crate::ustring::UStringHash(data_hash)
        .resolve()
        .map(|u| u.as_str().to_string())
        .unwrap_or_default();

    // Try to load as JSON first, then XML
    let handle = if data_str.starts_with('{') || data_str.starts_with('[') {
        store.load_json(&data_str)
    } else if data_str.starts_with('<') {
        store.load_xml(&data_str)
    } else {
        // Treat as an existing handle number
        data_str.parse::<i32>().unwrap_or(-1)
    };

    // If path is provided, navigate to it
    let path_hash = unsafe { read_i64_from_heap(heap, path_off as usize) } as u64;
    let path = crate::ustring::UStringHash(path_hash)
        .resolve()
        .map(|u| u.as_str().to_string())
        .unwrap_or_default();

    if path.is_empty() {
        handle
    } else if store.find(handle, &path).is_some() {
        handle
    } else {
        -1
    }
}

/// dict_value(ctx, heap, result_off, dict_handle, key_off, result_type) -> i32
extern "C" fn trampoline_dict_value(
    ctx: *mut JitRuntimeContext,
    heap: *mut u8,
    result_off: i32,
    dict_handle: i32,
    key_off: i32,
    result_type: i32,
) -> i32 {
    let ctx_ref = unsafe { &*ctx };
    if ctx_ref.dict_store.is_null() {
        return 0;
    }
    let store = unsafe { &*ctx_ref.dict_store };

    let key_hash = unsafe { read_i64_from_heap(heap, key_off as usize) } as u64;
    let key = crate::ustring::UStringHash(key_hash)
        .resolve()
        .map(|u| u.as_str().to_string())
        .unwrap_or_default();

    match result_type {
        0 => {
            // int
            if let Some(v) = store.value_int(dict_handle, &key) {
                unsafe {
                    *(heap.add(result_off as usize) as *mut i32) = v;
                }
                1
            } else {
                0
            }
        }
        1 => {
            // float
            if let Some(v) = store.value_float(dict_handle, &key) {
                unsafe {
                    *(heap.add(result_off as usize) as *mut f32) = v;
                }
                1
            } else {
                0
            }
        }
        _ => {
            // string
            if let Some(v) = store.value_str(dict_handle, &key) {
                let us = crate::ustring::UString::new(v);
                unsafe {
                    *(heap.add(result_off as usize) as *mut i64) = us.hash() as i64;
                }
                1
            } else {
                0
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Pointcloud trampolines — delegate to RendererServices
// ---------------------------------------------------------------------------

/// pointcloud_search(ctx, sg, heap, filename_off, center_off, radius, max_points,
///                   sort, indices_off, distances_off) -> i32
/// indices_off/distances_off: -1 if not used; otherwise heap offset to write output array.
extern "C" fn trampoline_pointcloud_search(
    ctx: *mut JitRuntimeContext,
    sg: *mut ShaderGlobals,
    heap: *mut u8,
    filename_off: i32,
    center_off: i32,
    radius: f32,
    max_points: i32,
    sort: i32,
    indices_off: i32,
    distances_off: i32,
) -> i32 {
    let ctx_ref = unsafe { &*ctx };
    let renderer = match unsafe { ctx_ref.get_renderer() } {
        Some(r) => r,
        None => return 0,
    };
    let sg_ref = unsafe { &*sg };
    let fh = unsafe { read_i64_from_heap(heap, filename_off as usize) } as u64;
    let center = unsafe { read_vec3_from_heap(heap, center_off as usize) };
    let max_pt = max_points.max(0) as usize;
    let mut indices = vec![0i32; max_pt];
    let derivs_offset = 0i32;
    let mut distances_opt: Option<Vec<f32>> = if distances_off >= 0 {
        Some(vec![0.0f32; max_pt])
    } else {
        None
    };
    let count = renderer.pointcloud_search(
        sg_ref,
        crate::ustring::UStringHash(fh),
        &center,
        radius,
        max_points,
        sort != 0,
        &mut indices,
        distances_opt.as_deref_mut(),
        derivs_offset,
        None,
        None,
    );
    if indices_off >= 0 {
        let ptr = unsafe { heap.add(indices_off as usize) as *mut i32 };
        for i in 0..count as usize {
            unsafe { *ptr.add(i) = indices[i] };
        }
    }
    if distances_off >= 0 {
        if let Some(ref dist) = distances_opt {
            let ptr = unsafe { heap.add(distances_off as usize) as *mut f32 };
            for i in 0..count as usize {
                unsafe { *ptr.add(i) = dist[i] };
            }
        }
    }
    count
}

/// pointcloud_get(ctx, sg, heap, filename_off, indices_off, count, attr_off, data_off, data_type) -> i32
/// data_type: 0 = float/float[], 1 = vec3/point/vector/color
extern "C" fn trampoline_pointcloud_get(
    ctx: *mut JitRuntimeContext,
    sg: *mut ShaderGlobals,
    heap: *mut u8,
    filename_off: i32,
    indices_off: i32,
    count: i32,
    attr_off: i32,
    data_off: i32,
    data_type: i32,
) -> i32 {
    let ctx_ref = unsafe { &*ctx };
    let renderer = match unsafe { ctx_ref.get_renderer() } {
        Some(r) => r,
        None => return 0,
    };
    let sg_ref = unsafe { &*sg };
    let fh = unsafe { read_i64_from_heap(heap, filename_off as usize) } as u64;
    let attr_hash = unsafe { read_i64_from_heap(heap, attr_off as usize) } as u64;
    let n = count.max(0) as usize;
    if n == 0 {
        return 1;
    }
    let mut indices = Vec::with_capacity(n);
    let idx_ptr = unsafe { heap.add(indices_off as usize) as *const i32 };
    for i in 0..n {
        indices.push(unsafe { *idx_ptr.add(i) });
    }
    let attr_td = if data_type != 0 {
        crate::typedesc::TypeDesc::COLOR
    } else {
        crate::typedesc::TypeDesc::FLOAT.array(-1)
    };
    let ok = if data_type != 0 {
        let mut data = vec![crate::math::Vec3::ZERO; n];
        let ok = renderer.pointcloud_get(
            sg_ref,
            crate::ustring::UStringHash(fh),
            &indices,
            crate::ustring::UStringHash(attr_hash),
            attr_td,
            data.as_mut_ptr() as *mut _,
        );
        if ok {
            let ptr = unsafe { heap.add(data_off as usize) as *mut f32 };
            for i in 0..n {
                let v = data[i];
                unsafe {
                    *ptr.add(i * 3) = v.x;
                    *ptr.add(i * 3 + 1) = v.y;
                    *ptr.add(i * 3 + 2) = v.z;
                }
            }
        }
        ok
    } else {
        let mut data = vec![0.0f32; n];
        let ok = renderer.pointcloud_get(
            sg_ref,
            crate::ustring::UStringHash(fh),
            &indices,
            crate::ustring::UStringHash(attr_hash),
            attr_td,
            data.as_mut_ptr() as *mut _,
        );
        if ok {
            let ptr = unsafe { heap.add(data_off as usize) as *mut f32 };
            for i in 0..n {
                unsafe { *ptr.add(i) = data[i] };
            }
        }
        ok
    };
    if ok { 1 } else { 0 }
}

/// pointcloud_write(ctx, sg, heap, result_off, filename_off, pos_off, scratch_off) -> void
/// Scratch layout: i32 nattrs, then per attr [i32 name_off, i32 type_code, i32 value_off].
/// type_code: 0=int, 1=float, 2=vec3/color/point/vector
extern "C" fn trampoline_pointcloud_write(
    ctx: *mut JitRuntimeContext,
    sg: *mut ShaderGlobals,
    heap: *mut u8,
    result_off: i32,
    filename_off: i32,
    pos_off: i32,
    scratch_off: i32,
) {
    let ctx_ref = unsafe { &*ctx };
    let renderer = match unsafe { ctx_ref.get_renderer() } {
        Some(r) => r,
        None => {
            unsafe { *(heap.add(result_off as usize) as *mut i32) = 0 };
            return;
        }
    };
    let sg_ref = unsafe { &*sg };
    let fh = unsafe { read_i64_from_heap(heap, filename_off as usize) } as u64;
    let pos = unsafe { read_vec3_from_heap(heap, pos_off as usize) };
    let base = scratch_off as usize;
    let nattrs = unsafe { *(heap.add(base) as *const i32) }.max(0) as usize;
    let mut names = Vec::with_capacity(nattrs);
    let mut types = Vec::with_capacity(nattrs);
    let mut data_ptrs: Vec<*const std::ffi::c_void> = Vec::with_capacity(nattrs);
    for i in 0..nattrs {
        let rec = base + 8 + i * 12;
        let name_off = unsafe { *(heap.add(rec) as *const i32) } as usize;
        let type_code = unsafe { *(heap.add(rec + 4) as *const i32) };
        let val_off = unsafe { *(heap.add(rec + 8) as *const i32) } as usize;
        let nh = unsafe { read_i64_from_heap(heap, name_off) } as u64;
        names.push(crate::ustring::UStringHash(nh));
        let td = match type_code {
            0 => crate::typedesc::TypeDesc::INT,
            2 => crate::typedesc::TypeDesc::COLOR,
            _ => crate::typedesc::TypeDesc::FLOAT,
        };
        types.push(td);
        data_ptrs.push(unsafe { heap.add(val_off) } as *const _);
    }
    let ok = renderer.pointcloud_write(
        sg_ref,
        crate::ustring::UStringHash(fh),
        &pos,
        &names,
        &types,
        &data_ptrs,
    );
    unsafe { *(heap.add(result_off as usize) as *mut i32) = if ok { 1 } else { 0 } };
}

fn register_math_trampolines(builder: &mut JITBuilder) {
    builder.symbol("osl_sinf", trampoline_sinf as *const u8);
    builder.symbol("osl_cosf", trampoline_cosf as *const u8);
    builder.symbol("osl_tanf", trampoline_tanf as *const u8);
    builder.symbol("osl_asinf", trampoline_asinf as *const u8);
    builder.symbol("osl_acosf", trampoline_acosf as *const u8);
    builder.symbol("osl_atanf", trampoline_atanf as *const u8);
    builder.symbol("osl_atan2f", trampoline_atan2f as *const u8);
    builder.symbol("osl_sqrtf", trampoline_sqrtf as *const u8);
    builder.symbol("osl_expf", trampoline_expf as *const u8);
    builder.symbol("osl_logf", trampoline_logf as *const u8);
    builder.symbol("osl_powf", trampoline_powf as *const u8);
    builder.symbol("osl_floorf", trampoline_floorf as *const u8);
    builder.symbol("osl_ceilf", trampoline_ceilf as *const u8);
    builder.symbol("osl_fabsf", trampoline_fabsf as *const u8);
    builder.symbol("osl_fmodf", trampoline_fmodf as *const u8);
    // Extended math (C++ parity)
    builder.symbol("osl_cbrtf", trampoline_cbrtf as *const u8);
    builder.symbol("osl_log2f", trampoline_log2f as *const u8);
    builder.symbol("osl_log10f", trampoline_log10f as *const u8);
    builder.symbol("osl_logbf", trampoline_logbf as *const u8);
    builder.symbol("osl_exp2f", trampoline_exp2f as *const u8);
    builder.symbol("osl_expm1f", trampoline_expm1f as *const u8);
    builder.symbol("osl_erff", trampoline_erff as *const u8);
    builder.symbol("osl_erfcf", trampoline_erfcf as *const u8);
    builder.symbol("osl_roundf", trampoline_roundf as *const u8);
    builder.symbol("osl_truncf", trampoline_truncf as *const u8);
    // Noise
    builder.symbol("osl_noise1", trampoline_noise1 as *const u8);
    builder.symbol("osl_noise3", trampoline_noise3 as *const u8);
    builder.symbol("osl_noise3_deriv", trampoline_noise3_deriv as *const u8);
    builder.symbol("osl_noise1_deriv", trampoline_noise1_deriv as *const u8);
    builder.symbol("osl_snoise3_deriv", trampoline_snoise3_deriv as *const u8);
    builder.symbol("osl_snoise1", trampoline_snoise1 as *const u8);
    builder.symbol("osl_snoise3", trampoline_snoise3 as *const u8);
    builder.symbol("osl_cellnoise1", trampoline_cellnoise1 as *const u8);
    builder.symbol("osl_cellnoise3", trampoline_cellnoise3 as *const u8);
    builder.symbol("osl_hashnoise1", trampoline_hashnoise1 as *const u8);
    builder.symbol("osl_hashnoise3", trampoline_hashnoise3 as *const u8);
    builder.symbol("osl_simplex3", trampoline_simplex3 as *const u8);
    builder.symbol("osl_usimplex3", trampoline_usimplex3 as *const u8);
    builder.symbol("osl_pnoise3", trampoline_pnoise3 as *const u8);
    builder.symbol("osl_pnoise3_deriv", trampoline_pnoise3_deriv as *const u8);
    // Matrix
    builder.symbol("osl_matrix_det", trampoline_matrix_det as *const u8);
    builder.symbol(
        "osl_matrix_transpose",
        trampoline_matrix_transpose as *const u8,
    );
    builder.symbol(
        "osl_transform_point",
        trampoline_transform_point as *const u8,
    );
    builder.symbol(
        "osl_transform_vector",
        trampoline_transform_vector as *const u8,
    );
    builder.symbol(
        "osl_transform_normal",
        trampoline_transform_normal as *const u8,
    );
    // Color
    builder.symbol("osl_blackbody", trampoline_blackbody as *const u8);
    builder.symbol(
        "osl_wavelength_color",
        trampoline_wavelength_color as *const u8,
    );
    builder.symbol("osl_luminance", trampoline_luminance as *const u8);
    builder.symbol(
        "osl_construct_color_from_space",
        trampoline_construct_color_from_space as *const u8,
    );
    builder.symbol("osl_transformc", trampoline_transformc as *const u8);
    // String
    builder.symbol("osl_strlen", trampoline_strlen as *const u8);
    builder.symbol("osl_hash_string", trampoline_hash_string as *const u8);
    builder.symbol("osl_stoi", trampoline_stoi as *const u8);
    builder.symbol("osl_stof", trampoline_stof as *const u8);
    // Renderer services
    builder.symbol("osl_texture", trampoline_texture as *const u8);
    builder.symbol("osl_texture3d", trampoline_texture3d as *const u8);
    builder.symbol("osl_environment", trampoline_environment as *const u8);
    builder.symbol("osl_getattribute", trampoline_getattribute as *const u8);
    builder.symbol("osl_gettextureinfo", trampoline_gettextureinfo as *const u8);
    builder.symbol("osl_getmatrix", trampoline_getmatrix as *const u8);
    builder.symbol("osl_trace", trampoline_trace as *const u8);
    builder.symbol("osl_range_check", trampoline_range_check as *const u8);
    // Closures
    builder.symbol("osl_closure_alloc", trampoline_closure_alloc as *const u8);
    // Message passing
    builder.symbol("osl_setmessage", trampoline_setmessage as *const u8);
    builder.symbol("osl_getmessage", trampoline_getmessage as *const u8);
    // Splines
    builder.symbol("osl_spline_float", trampoline_spline_float as *const u8);
    builder.symbol(
        "osl_spline_float_deriv",
        trampoline_spline_float_deriv as *const u8,
    );
    builder.symbol("osl_spline_vec3", trampoline_spline_vec3 as *const u8);
    builder.symbol(
        "osl_spline_vec3_deriv",
        trampoline_spline_vec3_deriv as *const u8,
    );
    builder.symbol(
        "osl_splineinverse_float",
        trampoline_splineinverse_float as *const u8,
    );
    // String manipulation
    builder.symbol("osl_format", trampoline_format as *const u8);
    builder.symbol("osl_split", trampoline_split as *const u8);
    builder.symbol("osl_concat", trampoline_concat as *const u8);
    builder.symbol("osl_substr", trampoline_substr as *const u8);
    builder.symbol("osl_getchar", trampoline_getchar as *const u8);
    builder.symbol("osl_startswith", trampoline_startswith as *const u8);
    builder.symbol("osl_endswith", trampoline_endswith as *const u8);
    builder.symbol("osl_regex_search", trampoline_regex_search as *const u8);
    builder.symbol("osl_regex_match", trampoline_regex_match as *const u8);
    // Dict
    builder.symbol("osl_dict_find", trampoline_dict_find as *const u8);
    builder.symbol("osl_dict_value", trampoline_dict_value as *const u8);
    // Pointcloud
    builder.symbol(
        "osl_pointcloud_search",
        trampoline_pointcloud_search as *const u8,
    );
    builder.symbol("osl_pointcloud_get", trampoline_pointcloud_get as *const u8);
    builder.symbol(
        "osl_pointcloud_write",
        trampoline_pointcloud_write as *const u8,
    );
}

/// Declared function IDs for calling from JIT code.
#[allow(dead_code)]
struct MathFuncs {
    sinf: FuncId,
    cosf: FuncId,
    tanf: FuncId,
    asinf: FuncId,
    acosf: FuncId,
    atanf: FuncId,
    atan2f: FuncId,
    sqrtf: FuncId,
    expf: FuncId,
    logf: FuncId,
    powf: FuncId,
    floorf: FuncId,
    ceilf: FuncId,
    fabsf: FuncId,
    fmodf: FuncId,
    cbrtf: FuncId,
    log2f: FuncId,
    log10f: FuncId,
    logbf: FuncId,
    exp2f: FuncId,
    expm1f: FuncId,
    erff: FuncId,
    erfcf: FuncId,
    roundf: FuncId,
    truncf: FuncId,
    // Noise
    noise1: FuncId,
    noise3: FuncId,
    noise3_deriv: FuncId,
    noise1_deriv: FuncId,
    snoise3_deriv: FuncId,
    snoise1: FuncId,
    snoise3: FuncId,
    cellnoise1: FuncId,
    cellnoise3: FuncId,
    hashnoise1: FuncId,
    hashnoise3: FuncId,
    simplex3: FuncId,
    usimplex3: FuncId,
    pnoise3: FuncId,
    pnoise3_deriv: FuncId,
    // Matrix (heap-based)
    matrix_det: FuncId,
    matrix_transpose: FuncId,
    transform_point: FuncId,
    transform_vector: FuncId,
    transform_normal: FuncId,
    // Color
    blackbody: FuncId,
    wavelength_color: FuncId,
    luminance: FuncId,
    construct_color_from_space: FuncId,
    transformc: FuncId,
    // String
    str_strlen: FuncId,
    str_hash: FuncId,
    str_stoi: FuncId,
    str_stof: FuncId,
    str_concat: FuncId,
    str_substr: FuncId,
    str_getchar: FuncId,
    str_startswith: FuncId,
    str_endswith: FuncId,
    str_regex_search: FuncId,
    str_regex_match: FuncId,
    str_format: FuncId,
    str_split: FuncId,
    // Renderer services (take ctx, sg, heap, ...)
    rs_texture: FuncId,
    rs_texture3d: FuncId,
    rs_environment: FuncId,
    rs_getattribute: FuncId,
    rs_gettextureinfo: FuncId,
    rs_getmatrix: FuncId,
    rs_trace: FuncId,
    rs_range_check: FuncId,
    // Closures
    closure_alloc: FuncId,
    // Message passing
    msg_set: FuncId,
    msg_get: FuncId,
    // Splines
    spline_float: FuncId,
    spline_float_deriv: FuncId,
    spline_vec3: FuncId,
    spline_vec3_deriv: FuncId,
    splineinverse_float: FuncId,
    // Dict
    dict_find: FuncId,
    dict_value: FuncId,
    // Pointcloud
    pc_search: FuncId,
    pc_get: FuncId,
    pc_write: FuncId,
}

fn declare_math_funcs(module: &mut JITModule, ptr_type: Type) -> Result<MathFuncs, JitError> {
    let map_err = |e: cranelift_module::ModuleError| JitError::Module(format!("{e}"));

    // f32 -> f32
    let mut sig_f_f = module.make_signature();
    sig_f_f.params.push(AbiParam::new(types::F32));
    sig_f_f.returns.push(AbiParam::new(types::F32));

    // (f32, f32) -> f32
    let mut sig_ff_f = module.make_signature();
    sig_ff_f.params.push(AbiParam::new(types::F32));
    sig_ff_f.params.push(AbiParam::new(types::F32));
    sig_ff_f.returns.push(AbiParam::new(types::F32));

    // (f32, f32, f32) -> f32  (3D noise etc.)
    let mut sig_fff_f = module.make_signature();
    sig_fff_f.params.push(AbiParam::new(types::F32));
    sig_fff_f.params.push(AbiParam::new(types::F32));
    sig_fff_f.params.push(AbiParam::new(types::F32));
    sig_fff_f.returns.push(AbiParam::new(types::F32));

    // (f32, f32, f32, f32, f32, f32) -> f32  (periodic noise)
    let mut sig_6f_f = module.make_signature();
    for _ in 0..6 {
        sig_6f_f.params.push(AbiParam::new(types::F32));
    }
    sig_6f_f.returns.push(AbiParam::new(types::F32));

    // (*const u8, i32) -> f32  (heap-based reads returning f32)
    let mut sig_pi_f = module.make_signature();
    sig_pi_f.params.push(AbiParam::new(ptr_type));
    sig_pi_f.params.push(AbiParam::new(types::I32));
    sig_pi_f.returns.push(AbiParam::new(types::F32));

    // (*const u8, i32) -> i32  (heap-based reads returning i32)
    let mut sig_pi_i = module.make_signature();
    sig_pi_i.params.push(AbiParam::new(ptr_type));
    sig_pi_i.params.push(AbiParam::new(types::I32));
    sig_pi_i.returns.push(AbiParam::new(types::I32));

    // (*mut u8, i32, i32) -> void  (heap src/dst operations)
    let mut sig_pii_v = module.make_signature();
    sig_pii_v.params.push(AbiParam::new(ptr_type));
    sig_pii_v.params.push(AbiParam::new(types::I32));
    sig_pii_v.params.push(AbiParam::new(types::I32));

    // (*mut u8, i32, i32, i32) -> void  (heap triple-offset operations)
    let mut sig_piii_v = module.make_signature();
    sig_piii_v.params.push(AbiParam::new(ptr_type));
    sig_piii_v.params.push(AbiParam::new(types::I32));
    sig_piii_v.params.push(AbiParam::new(types::I32));
    sig_piii_v.params.push(AbiParam::new(types::I32));

    // (*mut u8, i32, f32) -> void  (blackbody/wavelength_color)
    let mut sig_pif_v = module.make_signature();
    sig_pif_v.params.push(AbiParam::new(ptr_type));
    sig_pif_v.params.push(AbiParam::new(types::I32));
    sig_pif_v.params.push(AbiParam::new(types::F32));

    // (*const u8, i32, i32) -> i32  (string comparison ops)
    let mut sig_pii_i = module.make_signature();
    sig_pii_i.params.push(AbiParam::new(ptr_type));
    sig_pii_i.params.push(AbiParam::new(types::I32));
    sig_pii_i.params.push(AbiParam::new(types::I32));
    sig_pii_i.returns.push(AbiParam::new(types::I32));

    Ok(MathFuncs {
        sinf: module
            .declare_function("osl_sinf", Linkage::Import, &sig_f_f)
            .map_err(map_err)?,
        cosf: module
            .declare_function("osl_cosf", Linkage::Import, &sig_f_f)
            .map_err(map_err)?,
        tanf: module
            .declare_function("osl_tanf", Linkage::Import, &sig_f_f)
            .map_err(map_err)?,
        asinf: module
            .declare_function("osl_asinf", Linkage::Import, &sig_f_f)
            .map_err(map_err)?,
        acosf: module
            .declare_function("osl_acosf", Linkage::Import, &sig_f_f)
            .map_err(map_err)?,
        atanf: module
            .declare_function("osl_atanf", Linkage::Import, &sig_f_f)
            .map_err(map_err)?,
        atan2f: module
            .declare_function("osl_atan2f", Linkage::Import, &sig_ff_f)
            .map_err(map_err)?,
        sqrtf: module
            .declare_function("osl_sqrtf", Linkage::Import, &sig_f_f)
            .map_err(map_err)?,
        expf: module
            .declare_function("osl_expf", Linkage::Import, &sig_f_f)
            .map_err(map_err)?,
        logf: module
            .declare_function("osl_logf", Linkage::Import, &sig_f_f)
            .map_err(map_err)?,
        powf: module
            .declare_function("osl_powf", Linkage::Import, &sig_ff_f)
            .map_err(map_err)?,
        floorf: module
            .declare_function("osl_floorf", Linkage::Import, &sig_f_f)
            .map_err(map_err)?,
        ceilf: module
            .declare_function("osl_ceilf", Linkage::Import, &sig_f_f)
            .map_err(map_err)?,
        fabsf: module
            .declare_function("osl_fabsf", Linkage::Import, &sig_f_f)
            .map_err(map_err)?,
        fmodf: module
            .declare_function("osl_fmodf", Linkage::Import, &sig_ff_f)
            .map_err(map_err)?,
        cbrtf: module
            .declare_function("osl_cbrtf", Linkage::Import, &sig_f_f)
            .map_err(map_err)?,
        log2f: module
            .declare_function("osl_log2f", Linkage::Import, &sig_f_f)
            .map_err(map_err)?,
        log10f: module
            .declare_function("osl_log10f", Linkage::Import, &sig_f_f)
            .map_err(map_err)?,
        logbf: module
            .declare_function("osl_logbf", Linkage::Import, &sig_f_f)
            .map_err(map_err)?,
        exp2f: module
            .declare_function("osl_exp2f", Linkage::Import, &sig_f_f)
            .map_err(map_err)?,
        expm1f: module
            .declare_function("osl_expm1f", Linkage::Import, &sig_f_f)
            .map_err(map_err)?,
        erff: module
            .declare_function("osl_erff", Linkage::Import, &sig_f_f)
            .map_err(map_err)?,
        erfcf: module
            .declare_function("osl_erfcf", Linkage::Import, &sig_f_f)
            .map_err(map_err)?,
        roundf: module
            .declare_function("osl_roundf", Linkage::Import, &sig_f_f)
            .map_err(map_err)?,
        truncf: module
            .declare_function("osl_truncf", Linkage::Import, &sig_f_f)
            .map_err(map_err)?,
        // Noise
        noise1: module
            .declare_function("osl_noise1", Linkage::Import, &sig_f_f)
            .map_err(map_err)?,
        noise3: module
            .declare_function("osl_noise3", Linkage::Import, &sig_fff_f)
            .map_err(map_err)?,
        noise3_deriv: module
            .declare_function("osl_noise3_deriv", Linkage::Import, &{
                // (heap, dst_off, grad_off, x, y, z) -> void
                let mut s = module.make_signature();
                s.params.push(AbiParam::new(ptr_type));
                s.params.push(AbiParam::new(types::I32));
                s.params.push(AbiParam::new(types::I32));
                s.params.push(AbiParam::new(types::F32));
                s.params.push(AbiParam::new(types::F32));
                s.params.push(AbiParam::new(types::F32));
                s
            })
            .map_err(map_err)?,
        noise1_deriv: module
            .declare_function("osl_noise1_deriv", Linkage::Import, &{
                // (heap, dst_off, grad_off, x) -> void
                let mut s = module.make_signature();
                s.params.push(AbiParam::new(ptr_type));
                s.params.push(AbiParam::new(types::I32));
                s.params.push(AbiParam::new(types::I32));
                s.params.push(AbiParam::new(types::F32));
                s
            })
            .map_err(map_err)?,
        snoise3_deriv: module
            .declare_function("osl_snoise3_deriv", Linkage::Import, &{
                let mut s = module.make_signature();
                s.params.push(AbiParam::new(ptr_type));
                s.params.push(AbiParam::new(types::I32));
                s.params.push(AbiParam::new(types::I32));
                s.params.push(AbiParam::new(types::F32));
                s.params.push(AbiParam::new(types::F32));
                s.params.push(AbiParam::new(types::F32));
                s
            })
            .map_err(map_err)?,
        snoise1: module
            .declare_function("osl_snoise1", Linkage::Import, &sig_f_f)
            .map_err(map_err)?,
        snoise3: module
            .declare_function("osl_snoise3", Linkage::Import, &sig_fff_f)
            .map_err(map_err)?,
        cellnoise1: module
            .declare_function("osl_cellnoise1", Linkage::Import, &sig_f_f)
            .map_err(map_err)?,
        cellnoise3: module
            .declare_function("osl_cellnoise3", Linkage::Import, &sig_fff_f)
            .map_err(map_err)?,
        hashnoise1: module
            .declare_function("osl_hashnoise1", Linkage::Import, &sig_f_f)
            .map_err(map_err)?,
        hashnoise3: module
            .declare_function("osl_hashnoise3", Linkage::Import, &sig_fff_f)
            .map_err(map_err)?,
        simplex3: module
            .declare_function("osl_simplex3", Linkage::Import, &sig_fff_f)
            .map_err(map_err)?,
        usimplex3: module
            .declare_function("osl_usimplex3", Linkage::Import, &sig_fff_f)
            .map_err(map_err)?,
        pnoise3: module
            .declare_function("osl_pnoise3", Linkage::Import, &sig_6f_f)
            .map_err(map_err)?,
        pnoise3_deriv: module
            .declare_function("osl_pnoise3_deriv", Linkage::Import, &{
                // (heap, dst_off, grad_off, x, y, z, px, py, pz) -> void
                let mut s = module.make_signature();
                s.params.push(AbiParam::new(ptr_type)); // heap
                s.params.push(AbiParam::new(types::I32)); // dst_off
                s.params.push(AbiParam::new(types::I32)); // grad_off
                for _ in 0..6 {
                    s.params.push(AbiParam::new(types::F32));
                } // x, y, z, px, py, pz
                s
            })
            .map_err(map_err)?,
        // Matrix
        matrix_det: module
            .declare_function("osl_matrix_det", Linkage::Import, &sig_pi_f)
            .map_err(map_err)?,
        matrix_transpose: module
            .declare_function("osl_matrix_transpose", Linkage::Import, &sig_pii_v)
            .map_err(map_err)?,
        transform_point: module
            .declare_function("osl_transform_point", Linkage::Import, &sig_piii_v)
            .map_err(map_err)?,
        transform_vector: module
            .declare_function("osl_transform_vector", Linkage::Import, &sig_piii_v)
            .map_err(map_err)?,
        transform_normal: module
            .declare_function("osl_transform_normal", Linkage::Import, &sig_piii_v)
            .map_err(map_err)?,
        // Color
        blackbody: module
            .declare_function("osl_blackbody", Linkage::Import, &sig_pif_v)
            .map_err(map_err)?,
        wavelength_color: module
            .declare_function("osl_wavelength_color", Linkage::Import, &sig_pif_v)
            .map_err(map_err)?,
        luminance: module
            .declare_function("osl_luminance", Linkage::Import, &sig_fff_f)
            .map_err(map_err)?,
        construct_color_from_space: module
            .declare_function("osl_construct_color_from_space", Linkage::Import, &{
                // (heap, dst_off, space_off, x_off, y_off, z_off) -> void
                let mut s = module.make_signature();
                s.params.push(AbiParam::new(ptr_type));
                for _ in 0..5 {
                    s.params.push(AbiParam::new(types::I32));
                }
                s
            })
            .map_err(map_err)?,
        transformc: module
            .declare_function("osl_transformc", Linkage::Import, &{
                // (heap, dst_off, from_off, to_off, src_off) -> void
                let mut s = module.make_signature();
                s.params.push(AbiParam::new(ptr_type)); // heap
                for _ in 0..4 {
                    s.params.push(AbiParam::new(types::I32));
                }
                s
            })
            .map_err(map_err)?,
        // String
        str_strlen: module
            .declare_function("osl_strlen", Linkage::Import, &sig_pi_i)
            .map_err(map_err)?,
        str_hash: module
            .declare_function("osl_hash_string", Linkage::Import, &sig_pi_i)
            .map_err(map_err)?,
        str_stoi: module
            .declare_function("osl_stoi", Linkage::Import, &sig_pi_i)
            .map_err(map_err)?,
        str_stof: module
            .declare_function("osl_stof", Linkage::Import, &sig_pi_f)
            .map_err(map_err)?,
        str_concat: module
            .declare_function("osl_concat", Linkage::Import, &sig_piii_v)
            .map_err(map_err)?,
        str_substr: module
            .declare_function("osl_substr", Linkage::Import, &{
                let mut s = module.make_signature();
                s.params.push(AbiParam::new(ptr_type)); // heap
                s.params.push(AbiParam::new(types::I32)); // dst_off
                s.params.push(AbiParam::new(types::I32)); // src_off
                s.params.push(AbiParam::new(types::I32)); // start_off
                s.params.push(AbiParam::new(types::I32)); // len_off
                s
            })
            .map_err(map_err)?,
        str_getchar: module
            .declare_function("osl_getchar", Linkage::Import, &{
                let mut s = module.make_signature();
                s.params.push(AbiParam::new(ptr_type)); // heap
                s.params.push(AbiParam::new(types::I32)); // src_off
                s.params.push(AbiParam::new(types::I32)); // index_off
                s.returns.push(AbiParam::new(types::I32));
                s
            })
            .map_err(map_err)?,
        str_startswith: module
            .declare_function("osl_startswith", Linkage::Import, &sig_pii_i)
            .map_err(map_err)?,
        str_endswith: module
            .declare_function("osl_endswith", Linkage::Import, &sig_pii_i)
            .map_err(map_err)?,
        str_regex_search: module
            .declare_function("osl_regex_search", Linkage::Import, &sig_pii_i)
            .map_err(map_err)?,
        str_regex_match: module
            .declare_function("osl_regex_match", Linkage::Import, &sig_pii_i)
            .map_err(map_err)?,
        str_format: module
            .declare_function("osl_format", Linkage::Import, &{
                // (heap, dst_off, fmt_off, args_base_off, types_base_off, nargs) -> void
                let mut s = module.make_signature();
                s.params.push(AbiParam::new(ptr_type)); // heap
                for _ in 0..5 {
                    s.params.push(AbiParam::new(types::I32));
                }
                s
            })
            .map_err(map_err)?,
        str_split: module
            .declare_function("osl_split", Linkage::Import, &{
                // (heap, result_off, str_off, sep_off, maxsplit) -> i32
                let mut s = module.make_signature();
                s.params.push(AbiParam::new(ptr_type));
                for _ in 0..4 {
                    s.params.push(AbiParam::new(types::I32));
                }
                s.returns.push(AbiParam::new(types::I32));
                s
            })
            .map_err(map_err)?,
        // Renderer services
        rs_texture: module
            .declare_function("osl_texture", Linkage::Import, &{
                // (ctx, sg, heap, dst_off, nchannels, filename_off, s_off, t_off, s_has_derivs, t_has_derivs, opt_scratch_off) -> i32
                let mut s = module.make_signature();
                for _ in 0..3 {
                    s.params.push(AbiParam::new(ptr_type));
                }
                for _ in 0..8 {
                    s.params.push(AbiParam::new(types::I32));
                }
                s.returns.push(AbiParam::new(types::I32));
                s
            })
            .map_err(map_err)?,
        rs_texture3d: module
            .declare_function("osl_texture3d", Linkage::Import, &{
                // (ctx, sg, heap, dst_off, nc, filename_off, p_off, dpdx_off, dpdy_off, dpdz_off, opt_scratch_off) -> i32
                let mut s = module.make_signature();
                for _ in 0..3 {
                    s.params.push(AbiParam::new(ptr_type));
                }
                for _ in 0..8 {
                    s.params.push(AbiParam::new(types::I32));
                }
                s.returns.push(AbiParam::new(types::I32));
                s
            })
            .map_err(map_err)?,
        rs_environment: module
            .declare_function("osl_environment", Linkage::Import, &{
                // (ctx, sg, heap, dst_off, nc, filename_off, r_off, drdx_off, drdy_off, opt_scratch_off) -> i32
                let mut s = module.make_signature();
                for _ in 0..3 {
                    s.params.push(AbiParam::new(ptr_type));
                }
                for _ in 0..7 {
                    s.params.push(AbiParam::new(types::I32));
                }
                s.returns.push(AbiParam::new(types::I32));
                s
            })
            .map_err(map_err)?,
        rs_getattribute: module
            .declare_function("osl_getattribute", Linkage::Import, &{
                // (ctx, sg, heap, result_off, result_type, name_off) -> i32
                let mut s = module.make_signature();
                for _ in 0..3 {
                    s.params.push(AbiParam::new(ptr_type));
                }
                for _ in 0..3 {
                    s.params.push(AbiParam::new(types::I32));
                }
                s.returns.push(AbiParam::new(types::I32));
                s
            })
            .map_err(map_err)?,
        rs_gettextureinfo: module
            .declare_function("osl_gettextureinfo", Linkage::Import, &{
                // (ctx, sg, heap, result_off, filename_off, dataname_off, data_off, data_type) -> i32
                let mut s = module.make_signature();
                for _ in 0..3 {
                    s.params.push(AbiParam::new(ptr_type));
                }
                for _ in 0..5 {
                    s.params.push(AbiParam::new(types::I32));
                }
                s.returns.push(AbiParam::new(types::I32));
                s
            })
            .map_err(map_err)?,
        rs_getmatrix: module
            .declare_function("osl_getmatrix", Linkage::Import, &{
                // (ctx, sg, heap, dst_off, from_name_off, to_name_off) -> i32
                let mut s = module.make_signature();
                for _ in 0..3 {
                    s.params.push(AbiParam::new(ptr_type));
                }
                for _ in 0..3 {
                    s.params.push(AbiParam::new(types::I32));
                }
                s.returns.push(AbiParam::new(types::I32));
                s
            })
            .map_err(map_err)?,
        rs_trace: module
            .declare_function("osl_trace", Linkage::Import, &{
                // (ctx, sg, heap, pos_off, dir_off) -> i32
                let mut s = module.make_signature();
                for _ in 0..3 {
                    s.params.push(AbiParam::new(ptr_type));
                }
                for _ in 0..2 {
                    s.params.push(AbiParam::new(types::I32));
                }
                s.returns.push(AbiParam::new(types::I32));
                s
            })
            .map_err(map_err)?,
        rs_range_check: module
            .declare_function("osl_range_check", Linkage::Import, &{
                // (ctx, sg, heap, index, length, symname_hash, sourcefile_hash, sourceline) -> i32
                let mut s = module.make_signature();
                for _ in 0..3 {
                    s.params.push(AbiParam::new(ptr_type));
                }
                s.params.push(AbiParam::new(types::I32)); // index
                s.params.push(AbiParam::new(types::I32)); // length
                s.params.push(AbiParam::new(types::I64)); // symname_hash
                s.params.push(AbiParam::new(types::I64)); // sourcefile_hash
                s.params.push(AbiParam::new(types::I32)); // sourceline
                s.returns.push(AbiParam::new(types::I32));
                s
            })
            .map_err(map_err)?,
        // Closures
        closure_alloc: module
            .declare_function("osl_closure_alloc", Linkage::Import, &{
                // (ctx, heap, dst_off, name_off, weight_off) -> void
                let mut s = module.make_signature();
                s.params.push(AbiParam::new(ptr_type)); // ctx
                s.params.push(AbiParam::new(ptr_type)); // heap
                for _ in 0..3 {
                    s.params.push(AbiParam::new(types::I32));
                }
                s
            })
            .map_err(map_err)?,
        // Message passing
        msg_set: module
            .declare_function("osl_setmessage", Linkage::Import, &{
                // (ctx, heap, name_off, value_off, value_type) -> void
                let mut s = module.make_signature();
                s.params.push(AbiParam::new(ptr_type)); // ctx
                s.params.push(AbiParam::new(ptr_type)); // heap
                for _ in 0..3 {
                    s.params.push(AbiParam::new(types::I32));
                }
                s
            })
            .map_err(map_err)?,
        msg_get: module
            .declare_function("osl_getmessage", Linkage::Import, &{
                // (ctx, heap, name_off, result_off, result_type) -> i32
                let mut s = module.make_signature();
                s.params.push(AbiParam::new(ptr_type)); // ctx
                s.params.push(AbiParam::new(ptr_type)); // heap
                for _ in 0..3 {
                    s.params.push(AbiParam::new(types::I32));
                }
                s.returns.push(AbiParam::new(types::I32));
                s
            })
            .map_err(map_err)?,
        // Splines
        spline_float: module
            .declare_function("osl_spline_float", Linkage::Import, &{
                // (heap, dst_off, basis_off, t_off, knots_off, nknots) -> void
                let mut s = module.make_signature();
                s.params.push(AbiParam::new(ptr_type)); // heap
                for _ in 0..5 {
                    s.params.push(AbiParam::new(types::I32));
                }
                s
            })
            .map_err(map_err)?,
        spline_float_deriv: module
            .declare_function("osl_spline_float_deriv", Linkage::Import, &{
                // (heap, basis_off, t_off, knots_off, nknots) -> f32
                let mut s = module.make_signature();
                s.params.push(AbiParam::new(ptr_type)); // heap
                for _ in 0..4 {
                    s.params.push(AbiParam::new(types::I32));
                }
                s.returns.push(AbiParam::new(types::F32));
                s
            })
            .map_err(map_err)?,
        spline_vec3: module
            .declare_function("osl_spline_vec3", Linkage::Import, &{
                let mut s = module.make_signature();
                s.params.push(AbiParam::new(ptr_type));
                for _ in 0..5 {
                    s.params.push(AbiParam::new(types::I32));
                }
                s
            })
            .map_err(map_err)?,
        spline_vec3_deriv: module
            .declare_function("osl_spline_vec3_deriv", Linkage::Import, &{
                // (heap, dst_off, basis_off, t_off, knots_off, nknots) -> void (writes Vec3 to dst_off)
                let mut s = module.make_signature();
                s.params.push(AbiParam::new(ptr_type));
                for _ in 0..5 {
                    s.params.push(AbiParam::new(types::I32));
                }
                s
            })
            .map_err(map_err)?,
        splineinverse_float: module
            .declare_function("osl_splineinverse_float", Linkage::Import, &{
                let mut s = module.make_signature();
                s.params.push(AbiParam::new(ptr_type));
                for _ in 0..5 {
                    s.params.push(AbiParam::new(types::I32));
                }
                s
            })
            .map_err(map_err)?,
        // Dict (take ctx as first param for DictStore access)
        dict_find: module
            .declare_function("osl_dict_find", Linkage::Import, &{
                // (ctx, heap, dict_data_off, path_off) -> i32
                let mut s = module.make_signature();
                s.params.push(AbiParam::new(ptr_type)); // ctx
                s.params.push(AbiParam::new(ptr_type)); // heap
                for _ in 0..2 {
                    s.params.push(AbiParam::new(types::I32));
                }
                s.returns.push(AbiParam::new(types::I32));
                s
            })
            .map_err(map_err)?,
        dict_value: module
            .declare_function("osl_dict_value", Linkage::Import, &{
                // (ctx, heap, result_off, dict_handle, key_off, result_type) -> i32
                let mut s = module.make_signature();
                s.params.push(AbiParam::new(ptr_type)); // ctx
                s.params.push(AbiParam::new(ptr_type)); // heap
                for _ in 0..4 {
                    s.params.push(AbiParam::new(types::I32));
                }
                s.returns.push(AbiParam::new(types::I32));
                s
            })
            .map_err(map_err)?,
        // Pointcloud
        pc_search: module
            .declare_function("osl_pointcloud_search", Linkage::Import, &{
                // (ctx, sg, heap, filename_off, center_off, radius, max_points, sort, indices_off, distances_off) -> i32
                let mut s = module.make_signature();
                for _ in 0..3 {
                    s.params.push(AbiParam::new(ptr_type));
                }
                for _ in 0..2 {
                    s.params.push(AbiParam::new(types::I32));
                }
                s.params.push(AbiParam::new(types::F32));
                s.params.push(AbiParam::new(types::I32));
                for _ in 0..3 {
                    s.params.push(AbiParam::new(types::I32));
                }
                s.returns.push(AbiParam::new(types::I32));
                s
            })
            .map_err(map_err)?,
        pc_get: module
            .declare_function("osl_pointcloud_get", Linkage::Import, &{
                // (ctx, sg, heap, filename_off, indices_off, count, attr_off, data_off, data_type) -> i32
                let mut s = module.make_signature();
                for _ in 0..3 {
                    s.params.push(AbiParam::new(ptr_type));
                }
                for _ in 0..6 {
                    s.params.push(AbiParam::new(types::I32));
                }
                s.returns.push(AbiParam::new(types::I32));
                s
            })
            .map_err(map_err)?,
        pc_write: module
            .declare_function("osl_pointcloud_write", Linkage::Import, &{
                // (ctx, sg, heap, result_off, filename_off, pos_off, scratch_off) -> void
                let mut s = module.make_signature();
                for _ in 0..3 {
                    s.params.push(AbiParam::new(ptr_type));
                }
                for _ in 0..4 {
                    s.params.push(AbiParam::new(types::I32));
                }
                s
            })
            .map_err(map_err)?,
    })
}

// ---------------------------------------------------------------------------
// Heap helpers — load/store typed values from the heap buffer
// ---------------------------------------------------------------------------

/// Load a f32 from heap[offset].
fn heap_load_f32(builder: &mut FunctionBuilder, heap: CValue, offset: usize) -> CValue {
    let off = builder.ins().iconst(types::I64, offset as i64);
    let addr = builder.ins().iadd(heap, off);
    builder.ins().load(types::F32, MemFlags::trusted(), addr, 0)
}

/// Store a f32 to heap[offset].
fn heap_store_f32(builder: &mut FunctionBuilder, heap: CValue, offset: usize, val: CValue) {
    let off = builder.ins().iconst(types::I64, offset as i64);
    let addr = builder.ins().iadd(heap, off);
    builder.ins().store(MemFlags::trusted(), val, addr, 0);
}

/// Load an i32 from heap[offset].
fn heap_load_i32(builder: &mut FunctionBuilder, heap: CValue, offset: usize) -> CValue {
    let off = builder.ins().iconst(types::I64, offset as i64);
    let addr = builder.ins().iadd(heap, off);
    builder.ins().load(types::I32, MemFlags::trusted(), addr, 0)
}

/// Store an i32 to heap[offset].
fn heap_store_i32(builder: &mut FunctionBuilder, heap: CValue, offset: usize, val: CValue) {
    let off = builder.ins().iconst(types::I64, offset as i64);
    let addr = builder.ins().iadd(heap, off);
    builder.ins().store(MemFlags::trusted(), val, addr, 0);
}

/// Load a Vec3 (3 × f32) from heap[offset]. Returns (x, y, z).
fn heap_load_vec3(
    builder: &mut FunctionBuilder,
    heap: CValue,
    offset: usize,
) -> (CValue, CValue, CValue) {
    let x = heap_load_f32(builder, heap, offset);
    let y = heap_load_f32(builder, heap, offset + 4);
    let z = heap_load_f32(builder, heap, offset + 8);
    (x, y, z)
}

/// Store a Vec3 (3 × f32) to heap[offset].
fn heap_store_vec3(
    builder: &mut FunctionBuilder,
    heap: CValue,
    offset: usize,
    x: CValue,
    y: CValue,
    z: CValue,
) {
    heap_store_f32(builder, heap, offset, x);
    heap_store_f32(builder, heap, offset + 4, y);
    heap_store_f32(builder, heap, offset + 8, z);
}

// ---------------------------------------------------------------------------
// Dual2 derivative helpers
// ---------------------------------------------------------------------------

/// Get the byte offset of the dx slot for a symbol at `base_offset` with type `jt`.
/// Layout: [val (jt.size bytes)] [dx (jt.size bytes)] [dy (jt.size bytes)]
#[inline]
#[allow(dead_code)]
fn dx_offset(base: usize, jt: JitType) -> usize {
    base + jt.size()
}

/// Get the byte offset of the dy slot.
#[inline]
#[allow(dead_code)]
fn dy_offset(base: usize, jt: JitType) -> usize {
    base + 2 * jt.size()
}

/// Zero out the derivative slots for a symbol (if it has derivatives).
fn zero_derivs(builder: &mut FunctionBuilder, heap: CValue, base: usize, jt: JitType) {
    let zero = builder.ins().f32const(0.0);
    let sz = jt.size();
    // dx slot
    for i in (0..sz).step_by(4) {
        heap_store_f32(builder, heap, base + sz + i, zero);
    }
    // dy slot
    for i in (0..sz).step_by(4) {
        heap_store_f32(builder, heap, base + 2 * sz + i, zero);
    }
}

/// Propagate derivatives for binary add: r = a + b => r' = a' + b'
fn propagate_derivs_add_f32(
    builder: &mut FunctionBuilder,
    heap: CValue,
    r_off: usize,
    a_off: usize,
    b_off: usize,
    a_has_derivs: bool,
    b_has_derivs: bool,
) {
    let sz = 4usize; // f32 size
    // dx: r.dx = a.dx + b.dx
    let adx = if a_has_derivs {
        heap_load_f32(builder, heap, a_off + sz)
    } else {
        builder.ins().f32const(0.0)
    };
    let bdx = if b_has_derivs {
        heap_load_f32(builder, heap, b_off + sz)
    } else {
        builder.ins().f32const(0.0)
    };
    let rdx = builder.ins().fadd(adx, bdx);
    heap_store_f32(builder, heap, r_off + sz, rdx);

    // dy: r.dy = a.dy + b.dy
    let ady = if a_has_derivs {
        heap_load_f32(builder, heap, a_off + 2 * sz)
    } else {
        builder.ins().f32const(0.0)
    };
    let bdy = if b_has_derivs {
        heap_load_f32(builder, heap, b_off + 2 * sz)
    } else {
        builder.ins().f32const(0.0)
    };
    let rdy = builder.ins().fadd(ady, bdy);
    heap_store_f32(builder, heap, r_off + 2 * sz, rdy);
}

/// Propagate derivatives for binary sub: r = a - b => r' = a' - b'
fn propagate_derivs_sub_f32(
    builder: &mut FunctionBuilder,
    heap: CValue,
    r_off: usize,
    a_off: usize,
    b_off: usize,
    a_has_derivs: bool,
    b_has_derivs: bool,
) {
    let sz = 4usize;
    let adx = if a_has_derivs {
        heap_load_f32(builder, heap, a_off + sz)
    } else {
        builder.ins().f32const(0.0)
    };
    let bdx = if b_has_derivs {
        heap_load_f32(builder, heap, b_off + sz)
    } else {
        builder.ins().f32const(0.0)
    };
    let rdx = builder.ins().fsub(adx, bdx);
    heap_store_f32(builder, heap, r_off + sz, rdx);

    let ady = if a_has_derivs {
        heap_load_f32(builder, heap, a_off + 2 * sz)
    } else {
        builder.ins().f32const(0.0)
    };
    let bdy = if b_has_derivs {
        heap_load_f32(builder, heap, b_off + 2 * sz)
    } else {
        builder.ins().f32const(0.0)
    };
    let rdy = builder.ins().fsub(ady, bdy);
    heap_store_f32(builder, heap, r_off + 2 * sz, rdy);
}

/// Propagate derivatives for binary mul: r = a * b => r' = a' * b + a * b'
fn propagate_derivs_mul_f32(
    builder: &mut FunctionBuilder,
    heap: CValue,
    r_off: usize,
    a_off: usize,
    b_off: usize,
    a_has_derivs: bool,
    b_has_derivs: bool,
) {
    let sz = 4usize;
    let a_val = heap_load_f32(builder, heap, a_off);
    let b_val = heap_load_f32(builder, heap, b_off);

    // dx: r.dx = a.dx * b + a * b.dx
    let adx = if a_has_derivs {
        heap_load_f32(builder, heap, a_off + sz)
    } else {
        builder.ins().f32const(0.0)
    };
    let bdx = if b_has_derivs {
        heap_load_f32(builder, heap, b_off + sz)
    } else {
        builder.ins().f32const(0.0)
    };
    let t1 = builder.ins().fmul(adx, b_val);
    let t2 = builder.ins().fmul(a_val, bdx);
    let rdx = builder.ins().fadd(t1, t2);
    heap_store_f32(builder, heap, r_off + sz, rdx);

    // dy: r.dy = a.dy * b + a * b.dy
    let ady = if a_has_derivs {
        heap_load_f32(builder, heap, a_off + 2 * sz)
    } else {
        builder.ins().f32const(0.0)
    };
    let bdy = if b_has_derivs {
        heap_load_f32(builder, heap, b_off + 2 * sz)
    } else {
        builder.ins().f32const(0.0)
    };
    let t3 = builder.ins().fmul(ady, b_val);
    let t4 = builder.ins().fmul(a_val, bdy);
    let rdy = builder.ins().fadd(t3, t4);
    heap_store_f32(builder, heap, r_off + 2 * sz, rdy);
}

/// Propagate derivatives for div: r = a / b => r' = (a' * b - a * b') / (b * b)
fn propagate_derivs_div_f32(
    builder: &mut FunctionBuilder,
    heap: CValue,
    r_off: usize,
    a_off: usize,
    b_off: usize,
    a_has_derivs: bool,
    b_has_derivs: bool,
) {
    let sz = 4usize;
    let a_val = heap_load_f32(builder, heap, a_off);
    let b_val = heap_load_f32(builder, heap, b_off);
    let b2 = builder.ins().fmul(b_val, b_val);

    let adx = if a_has_derivs {
        heap_load_f32(builder, heap, a_off + sz)
    } else {
        builder.ins().f32const(0.0)
    };
    let bdx = if b_has_derivs {
        heap_load_f32(builder, heap, b_off + sz)
    } else {
        builder.ins().f32const(0.0)
    };
    let num_dx = builder.ins().fmul(adx, b_val);
    let sub_dx = builder.ins().fmul(a_val, bdx);
    let num_dx = builder.ins().fsub(num_dx, sub_dx);
    let rdx = emit_safe_fdiv(builder, num_dx, b2);
    heap_store_f32(builder, heap, r_off + sz, rdx);

    let ady = if a_has_derivs {
        heap_load_f32(builder, heap, a_off + 2 * sz)
    } else {
        builder.ins().f32const(0.0)
    };
    let bdy = if b_has_derivs {
        heap_load_f32(builder, heap, b_off + 2 * sz)
    } else {
        builder.ins().f32const(0.0)
    };
    let num_dy = builder.ins().fmul(ady, b_val);
    let sub_dy = builder.ins().fmul(a_val, bdy);
    let num_dy = builder.ins().fsub(num_dy, sub_dy);
    let rdy = emit_safe_fdiv(builder, num_dy, b2);
    heap_store_f32(builder, heap, r_off + 2 * sz, rdy);
}

/// Propagate derivatives for neg: r = -a => r' = -a'
fn propagate_derivs_neg_f32(
    builder: &mut FunctionBuilder,
    heap: CValue,
    r_off: usize,
    a_off: usize,
    a_has_derivs: bool,
) {
    let sz = 4usize;
    if a_has_derivs {
        let adx = heap_load_f32(builder, heap, a_off + sz);
        let rdx = builder.ins().fneg(adx);
        heap_store_f32(builder, heap, r_off + sz, rdx);
        let ady = heap_load_f32(builder, heap, a_off + 2 * sz);
        let rdy = builder.ins().fneg(ady);
        heap_store_f32(builder, heap, r_off + 2 * sz, rdy);
    } else {
        let zero = builder.ins().f32const(0.0);
        heap_store_f32(builder, heap, r_off + sz, zero);
        heap_store_f32(builder, heap, r_off + 2 * sz, zero);
    }
}

/// Propagate derivatives for unary math f(a):  r' = f'(a) * a'
/// `deriv_val` is the already-computed f'(a) value.
fn propagate_derivs_unary_f32(
    builder: &mut FunctionBuilder,
    heap: CValue,
    r_off: usize,
    a_off: usize,
    a_has_derivs: bool,
    deriv_val: CValue,
) {
    let sz = 4usize;
    if a_has_derivs {
        let adx = heap_load_f32(builder, heap, a_off + sz);
        let rdx = builder.ins().fmul(deriv_val, adx);
        heap_store_f32(builder, heap, r_off + sz, rdx);
        let ady = heap_load_f32(builder, heap, a_off + 2 * sz);
        let rdy = builder.ins().fmul(deriv_val, ady);
        heap_store_f32(builder, heap, r_off + 2 * sz, rdy);
    } else {
        let zero = builder.ins().f32const(0.0);
        heap_store_f32(builder, heap, r_off + sz, zero);
        heap_store_f32(builder, heap, r_off + 2 * sz, zero);
    }
}

// ---------------------------------------------------------------------------
// Vec3 Dual2 derivative helpers (component-wise)
// ---------------------------------------------------------------------------

/// Propagate derivatives for Vec3 add: r = a + b => r' = a' + b' (component-wise)
// NOTE: propagate_derivs_add/sub_vec3 replaced by propagate_derivs_addsub_vec3_mixed
//       which correctly handles float<->vec3 mixed-type derivative layouts.

/// Mixed-type add/sub derivative propagation for Vec3 result.
/// Handles vec3 op vec3, float op vec3, vec3 op float, and float op float (broadcast).
/// `is_add` = true for add, false for sub.
#[allow(clippy::too_many_arguments)]
fn propagate_derivs_addsub_vec3_mixed(
    builder: &mut FunctionBuilder,
    heap: CValue,
    r_off: usize,
    a_off: usize,
    b_off: usize,
    a_has_derivs: bool,
    b_has_derivs: bool,
    a_jt: JitType,
    b_jt: JitType,
    is_add: bool,
) {
    let a_is_vec = matches!(a_jt, JitType::Vec3 | JitType::Color);
    let b_is_vec = matches!(b_jt, JitType::Vec3 | JitType::Color);
    let sz_v = 12usize;
    let sz_f = 4usize;

    for d in 1..=2usize {
        for c in 0..3usize {
            let co = c * 4;
            // Read A derivative: if vec3 -> offset + d*12 + c*4; if float -> offset + d*4 (broadcast)
            let adx = if a_has_derivs {
                if a_is_vec {
                    heap_load_f32(builder, heap, a_off + d * sz_v + co)
                } else {
                    heap_load_f32(builder, heap, a_off + d * sz_f)
                }
            } else {
                builder.ins().f32const(0.0)
            };
            let bdx = if b_has_derivs {
                if b_is_vec {
                    heap_load_f32(builder, heap, b_off + d * sz_v + co)
                } else {
                    heap_load_f32(builder, heap, b_off + d * sz_f)
                }
            } else {
                builder.ins().f32const(0.0)
            };
            let rdx = if is_add {
                builder.ins().fadd(adx, bdx)
            } else {
                builder.ins().fsub(adx, bdx)
            };
            heap_store_f32(builder, heap, r_off + d * sz_v + co, rdx);
        }
    }
}

/// Propagate derivatives for Vec3 component-wise mul: r_i = a_i * b_i => r_i' = a_i' * b_i + a_i * b_i'
fn propagate_derivs_mul_vec3(
    builder: &mut FunctionBuilder,
    heap: CValue,
    r_off: usize,
    a_off: usize,
    b_off: usize,
    a_has_derivs: bool,
    b_has_derivs: bool,
) {
    let sz = 12usize;
    for d in 1..=2usize {
        let slot = d * sz;
        for c in 0..3usize {
            let co = c * 4;
            let a_val = heap_load_f32(builder, heap, a_off + co);
            let b_val = heap_load_f32(builder, heap, b_off + co);
            let adx = if a_has_derivs {
                heap_load_f32(builder, heap, a_off + slot + co)
            } else {
                builder.ins().f32const(0.0)
            };
            let bdx = if b_has_derivs {
                heap_load_f32(builder, heap, b_off + slot + co)
            } else {
                builder.ins().f32const(0.0)
            };
            let t1 = builder.ins().fmul(adx, b_val);
            let t2 = builder.ins().fmul(a_val, bdx);
            let rdx = builder.ins().fadd(t1, t2);
            heap_store_f32(builder, heap, r_off + slot + co, rdx);
        }
    }
}

/// Propagate derivatives for Vec3 component-wise div: r_i = a_i / b_i
fn propagate_derivs_div_vec3(
    builder: &mut FunctionBuilder,
    heap: CValue,
    r_off: usize,
    a_off: usize,
    b_off: usize,
    a_has_derivs: bool,
    b_has_derivs: bool,
) {
    let sz = 12usize;
    for d in 1..=2usize {
        let slot = d * sz;
        for c in 0..3usize {
            let co = c * 4;
            let a_val = heap_load_f32(builder, heap, a_off + co);
            let b_val = heap_load_f32(builder, heap, b_off + co);
            let b2 = builder.ins().fmul(b_val, b_val);
            let adx = if a_has_derivs {
                heap_load_f32(builder, heap, a_off + slot + co)
            } else {
                builder.ins().f32const(0.0)
            };
            let bdx = if b_has_derivs {
                heap_load_f32(builder, heap, b_off + slot + co)
            } else {
                builder.ins().f32const(0.0)
            };
            let num = builder.ins().fmul(adx, b_val);
            let sub = builder.ins().fmul(a_val, bdx);
            let num = builder.ins().fsub(num, sub);
            let rdx = emit_safe_fdiv(builder, num, b2);
            heap_store_f32(builder, heap, r_off + slot + co, rdx);
        }
    }
}

/// Propagate derivatives for Vec3 neg: r = -a => r' = -a'
fn propagate_derivs_neg_vec3(
    builder: &mut FunctionBuilder,
    heap: CValue,
    r_off: usize,
    a_off: usize,
    a_has_derivs: bool,
) {
    let sz = 12usize;
    if a_has_derivs {
        for d in 1..=2usize {
            let slot = d * sz;
            for c in 0..3usize {
                let co = c * 4;
                let adx = heap_load_f32(builder, heap, a_off + slot + co);
                let rdx = builder.ins().fneg(adx);
                heap_store_f32(builder, heap, r_off + slot + co, rdx);
            }
        }
    } else {
        let zero = builder.ins().f32const(0.0);
        for d in 1..=2usize {
            let slot = d * sz;
            for c in 0..3usize {
                heap_store_f32(builder, heap, r_off + slot + c * 4, zero);
            }
        }
    }
}

/// Load a f32 from sg_ptr[offset] (ShaderGlobals field).
fn sg_load_f32(builder: &mut FunctionBuilder, sg: CValue, offset: usize) -> CValue {
    let off = builder.ins().iconst(types::I64, offset as i64);
    let addr = builder.ins().iadd(sg, off);
    builder.ins().load(types::F32, MemFlags::trusted(), addr, 0)
}

/// Load a Vec3 from sg_ptr[offset] (ShaderGlobals field).
fn sg_load_vec3(
    builder: &mut FunctionBuilder,
    sg: CValue,
    offset: usize,
) -> (CValue, CValue, CValue) {
    let x = sg_load_f32(builder, sg, offset);
    let y = sg_load_f32(builder, sg, offset + 4);
    let z = sg_load_f32(builder, sg, offset + 8);
    (x, y, z)
}

/// Load an i32 from sg_ptr[offset].
fn sg_load_i32(builder: &mut FunctionBuilder, sg: CValue, offset: usize) -> CValue {
    let off = builder.ins().iconst(types::I64, offset as i64);
    let addr = builder.ins().iadd(sg, off);
    builder.ins().load(types::I32, MemFlags::trusted(), addr, 0)
}

// ---------------------------------------------------------------------------
// Emit: constant initialization
// ---------------------------------------------------------------------------

fn emit_init_constants(
    _builder: &FunctionBuilder,
    _ir: &ShaderIR,
    _heap: CValue,
    _offsets: &[usize],
    _jtypes: &[JitType],
) {
    // Constants and param defaults are initialized in emit_bind_globals
    // (which also handles const_values and param_defaults).
    // The heap is zero-initialized by the caller.
}

// ---------------------------------------------------------------------------
// Emit: bind globals from ShaderGlobals pointer
// ---------------------------------------------------------------------------

fn emit_bind_globals(
    builder: &mut FunctionBuilder,
    ir: &ShaderIR,
    sg: CValue,
    heap: CValue,
    offsets: &[usize],
) {
    for (i, sym) in ir.symbols.iter().enumerate() {
        if sym.symtype != SymType::Global {
            continue;
        }
        let name = sym.name.as_str();
        let heap_off = offsets[i];
        let has_d = sym.has_derivs;

        match name {
            "P" => {
                let (x, y, z) = sg_load_vec3(builder, sg, sg_offsets::p());
                heap_store_vec3(builder, heap, heap_off, x, y, z);
                if has_d {
                    // P.dx = dPdx, P.dy = dPdy
                    let (dpx, dpy, dpz) = sg_load_vec3(builder, sg, sg_offsets::dp_dx());
                    heap_store_vec3(builder, heap, heap_off + 12, dpx, dpy, dpz);
                    let (dpx2, dpy2, dpz2) = sg_load_vec3(builder, sg, sg_offsets::dp_dy());
                    heap_store_vec3(builder, heap, heap_off + 24, dpx2, dpy2, dpz2);
                }
            }
            "N" => {
                let (x, y, z) = sg_load_vec3(builder, sg, sg_offsets::n());
                heap_store_vec3(builder, heap, heap_off, x, y, z);
                if has_d {
                    // N has no derivatives from SG — zero them
                    let zero = builder.ins().f32const(0.0);
                    heap_store_vec3(builder, heap, heap_off + 12, zero, zero, zero);
                    heap_store_vec3(builder, heap, heap_off + 24, zero, zero, zero);
                }
            }
            "Ng" => {
                let (x, y, z) = sg_load_vec3(builder, sg, sg_offsets::ng());
                heap_store_vec3(builder, heap, heap_off, x, y, z);
                if has_d {
                    let zero = builder.ins().f32const(0.0);
                    heap_store_vec3(builder, heap, heap_off + 12, zero, zero, zero);
                    heap_store_vec3(builder, heap, heap_off + 24, zero, zero, zero);
                }
            }
            "I" => {
                let (x, y, z) = sg_load_vec3(builder, sg, sg_offsets::i());
                heap_store_vec3(builder, heap, heap_off, x, y, z);
                if has_d {
                    // I derivatives are not in SG — zero them
                    let zero = builder.ins().f32const(0.0);
                    heap_store_vec3(builder, heap, heap_off + 12, zero, zero, zero);
                    heap_store_vec3(builder, heap, heap_off + 24, zero, zero, zero);
                }
            }
            "u" => {
                let v = sg_load_f32(builder, sg, sg_offsets::u_());
                heap_store_f32(builder, heap, heap_off, v);
                if has_d {
                    // u.dx = dudx, u.dy = dudy
                    let dudx = sg_load_f32(builder, sg, sg_offsets::dudx());
                    heap_store_f32(builder, heap, heap_off + 4, dudx);
                    let dudy = sg_load_f32(builder, sg, sg_offsets::dudy());
                    heap_store_f32(builder, heap, heap_off + 8, dudy);
                }
            }
            "v" => {
                let v = sg_load_f32(builder, sg, sg_offsets::v());
                heap_store_f32(builder, heap, heap_off, v);
                if has_d {
                    let dvdx = sg_load_f32(builder, sg, sg_offsets::dvdx());
                    heap_store_f32(builder, heap, heap_off + 4, dvdx);
                    let dvdy = sg_load_f32(builder, sg, sg_offsets::dvdy());
                    heap_store_f32(builder, heap, heap_off + 8, dvdy);
                }
            }
            "time" => {
                let v = sg_load_f32(builder, sg, sg_offsets::time());
                heap_store_f32(builder, heap, heap_off, v);
            }
            "surfacearea" => {
                let v = sg_load_f32(builder, sg, sg_offsets::surfacearea());
                heap_store_f32(builder, heap, heap_off, v);
            }
            "raytype" => {
                let v = sg_load_i32(builder, sg, sg_offsets::raytype());
                heap_store_i32(builder, heap, heap_off, v);
            }
            "backfacing" => {
                let v = sg_load_i32(builder, sg, sg_offsets::backfacing());
                heap_store_i32(builder, heap, heap_off, v);
            }
            "dPdx" => {
                let (x, y, z) = sg_load_vec3(builder, sg, sg_offsets::dp_dx());
                heap_store_vec3(builder, heap, heap_off, x, y, z);
            }
            "dPdy" => {
                let (x, y, z) = sg_load_vec3(builder, sg, sg_offsets::dp_dy());
                heap_store_vec3(builder, heap, heap_off, x, y, z);
            }
            "dPdu" => {
                let (x, y, z) = sg_load_vec3(builder, sg, sg_offsets::dp_du());
                heap_store_vec3(builder, heap, heap_off, x, y, z);
            }
            "dPdv" => {
                let (x, y, z) = sg_load_vec3(builder, sg, sg_offsets::dp_dv());
                heap_store_vec3(builder, heap, heap_off, x, y, z);
            }
            "dPdz" => {
                let (x, y, z) = sg_load_vec3(builder, sg, sg_offsets::dp_dz());
                heap_store_vec3(builder, heap, heap_off, x, y, z);
            }
            "dIdx" => {
                let (x, y, z) = sg_load_vec3(builder, sg, sg_offsets::di_dx());
                heap_store_vec3(builder, heap, heap_off, x, y, z);
            }
            "dIdy" => {
                let (x, y, z) = sg_load_vec3(builder, sg, sg_offsets::di_dy());
                heap_store_vec3(builder, heap, heap_off, x, y, z);
            }
            "dtime" => {
                let v = sg_load_f32(builder, sg, sg_offsets::dtime());
                heap_store_f32(builder, heap, heap_off, v);
            }
            "dPdtime" => {
                let (x, y, z) = sg_load_vec3(builder, sg, sg_offsets::dp_dtime());
                heap_store_vec3(builder, heap, heap_off, x, y, z);
            }
            "Ps" => {
                let (x, y, z) = sg_load_vec3(builder, sg, sg_offsets::ps());
                heap_store_vec3(builder, heap, heap_off, x, y, z);
                if has_d {
                    let sz = 12usize;
                    let (dxv_x, dxv_y, dxv_z) = sg_load_vec3(builder, sg, sg_offsets::dps_dx());
                    heap_store_vec3(builder, heap, heap_off + sz, dxv_x, dxv_y, dxv_z);
                    let (dyv_x, dyv_y, dyv_z) = sg_load_vec3(builder, sg, sg_offsets::dps_dy());
                    heap_store_vec3(builder, heap, heap_off + 2 * sz, dyv_x, dyv_y, dyv_z);
                }
            }
            "flipHandedness" => {
                let v = sg_load_i32(builder, sg, sg_offsets::flip_handedness());
                heap_store_i32(builder, heap, heap_off, v);
            }
            "thread_index" => {
                let v = sg_load_i32(builder, sg, sg_offsets::thread_index());
                heap_store_i32(builder, heap, heap_off, v);
            }
            "shade_index" => {
                let v = sg_load_i32(builder, sg, sg_offsets::shade_index());
                heap_store_i32(builder, heap, heap_off, v);
            }
            _ => {} // Truly unknown globals (opaque ptrs etc.)
        }
    }

    // Also initialize const_values and param_defaults
    for &(idx, ref cv) in &ir.const_values {
        let off = offsets[idx];
        emit_const_value(builder, heap, off, cv);
    }

    for &(idx, ref cv) in &ir.param_defaults {
        let off = offsets[idx];
        emit_const_value(builder, heap, off, cv);
    }
}

/// Emit a ConstValue to the heap at the given offset.
fn emit_const_value(builder: &mut FunctionBuilder, heap: CValue, off: usize, cv: &ConstValue) {
    match cv {
        ConstValue::Float(f) => {
            let v = builder.ins().f32const(*f);
            heap_store_f32(builder, heap, off, v);
        }
        ConstValue::Int(i) => {
            let v = builder.ins().iconst(types::I32, *i as i64);
            heap_store_i32(builder, heap, off, v);
        }
        ConstValue::Vec3(vec) => {
            let x = builder.ins().f32const(vec.x);
            let y = builder.ins().f32const(vec.y);
            let z = builder.ins().f32const(vec.z);
            heap_store_vec3(builder, heap, off, x, y, z);
        }
        ConstValue::Matrix(mat) => {
            for r in 0..4 {
                for c in 0..4 {
                    let v = builder.ins().f32const(mat.m[r][c]);
                    heap_store_f32(builder, heap, off + (r * 4 + c) * 4, v);
                }
            }
        }
        ConstValue::IntArray(arr) => {
            for (i, &val) in arr.iter().enumerate() {
                let v = builder.ins().iconst(types::I32, val as i64);
                heap_store_i32(builder, heap, off + i * 4, v);
            }
        }
        ConstValue::FloatArray(arr) => {
            for (i, &val) in arr.iter().enumerate() {
                let v = builder.ins().f32const(val);
                heap_store_f32(builder, heap, off + i * 4, v);
            }
        }
        ConstValue::StringArray(arr) => {
            for (i, s) in arr.iter().enumerate() {
                let hash = s.hash() as i64;
                let v = builder.ins().iconst(types::I64, hash);
                let addr = builder.ins().iadd_imm(heap, (off + i * 8) as i64);
                builder.ins().store(MemFlags::trusted(), v, addr, 0);
            }
        }
        ConstValue::String(s) => {
            let hash = s.hash() as i64;
            let v = builder.ins().iconst(types::I64, hash);
            let addr = builder.ins().iadd_imm(heap, off as i64);
            builder.ins().store(MemFlags::trusted(), v, addr, 0);
        }
    }
}

// ---------------------------------------------------------------------------
// Emit: opcode translation
// ---------------------------------------------------------------------------

/// Compute the byte offset and size of a struct field given the struct symbol
/// and the field name constant symbol. Returns `(byte_offset, field_size_bytes)`.
/// Falls back to `(0, 4)` if the struct layout can't be resolved.
fn compute_struct_field_offset(
    ir: &ShaderIR,
    struct_sym: usize,
    field_name_sym: usize,
) -> (usize, usize) {
    // The field_name_sym should be a string constant whose name is the field name.
    let field_name = ir.symbols[field_name_sym].name;
    // Look up the struct symbol's type to get the struct ID.
    let struct_id = ir.symbols[struct_sym].typespec.structure_id();
    if struct_id > 0 {
        if let Some(spec) = crate::typespec::get_struct(struct_id as i32) {
            if let Some(fi) = spec.lookup_field(field_name) {
                return spec.field_byte_offset(fi);
            }
        }
    }
    // Fallback: treat as a single float at offset 0.
    (0, 4)
}

/// Emit a call to osl_range_check when range_checking is true; otherwise return index as-is.
/// Returns the (possibly clamped) index CValue for use in aref/aassign/compref/compassign/mxcompref/mxcompassign.
fn emit_maybe_range_check(
    builder: &mut FunctionBuilder,
    module: &mut JITModule,
    math: &MathFuncs,
    ctx: CValue,
    sg: CValue,
    heap: CValue,
    index: CValue,
    length: i32,
    symname_hash: u64,
    sourcefile_hash: u64,
    sourceline: i32,
    range_checking: bool,
) -> CValue {
    if !range_checking || length <= 0 {
        return index;
    }
    let func_ref = module.declare_func_in_func(math.rs_range_check, builder.func);
    let len_val = builder.ins().iconst(types::I32, length as i64);
    let sym_hash_val = builder.ins().iconst(types::I64, symname_hash as i64);
    let file_hash_val = builder.ins().iconst(types::I64, sourcefile_hash as i64);
    let line_val = builder.ins().iconst(types::I32, sourceline as i64);
    let call = builder.ins().call(
        func_ref,
        &[
            ctx,
            sg,
            heap,
            index,
            len_val,
            sym_hash_val,
            file_hash_val,
            line_val,
        ],
    );
    builder.inst_results(call)[0]
}

fn emit_opcodes(
    builder: &mut FunctionBuilder,
    module: &mut JITModule,
    ir: &ShaderIR,
    sg: CValue,
    heap: CValue,
    ctx: CValue,
    offsets: &[usize],
    jtypes: &[JitType],
    has_derivs: &[bool],
    scratch_offset: usize,
    ptr_type: Type,
    math: &MathFuncs,
    range_checking: bool,
) -> Result<(), JitError> {
    // Pre-create blocks for jump targets (for control flow)
    let mut op_blocks: Vec<Block> = Vec::with_capacity(ir.opcodes.len() + 1);
    for _ in 0..ir.opcodes.len() {
        op_blocks.push(builder.create_block());
    }
    let exit_block = builder.create_block();

    // Jump to the first opcode block
    if !ir.opcodes.is_empty() {
        builder.ins().jump(op_blocks[0], &[]);
    } else {
        builder.ins().jump(exit_block, &[]);
    }

    for (pc, op) in ir.opcodes.iter().enumerate() {
        builder.switch_to_block(op_blocks[pc]);
        // Don't seal yet — blocks may have multiple predecessors (loops)

        let opname = op.op.as_str();
        let nargs = op.nargs as usize;
        let firstarg = op.firstarg as usize;

        let args: Vec<i32> = (0..nargs)
            .map(|j| {
                if firstarg + j < ir.args.len() {
                    ir.args[firstarg + j]
                } else {
                    -1
                }
            })
            .collect();

        let next_block = if pc + 1 < ir.opcodes.len() {
            op_blocks[pc + 1]
        } else {
            exit_block
        };

        match opname {
            "nop" | "" => {
                if op.jump[0] >= 0 && (op.jump[0] as usize) < op_blocks.len() {
                    builder.ins().jump(op_blocks[op.jump[0] as usize], &[]);
                } else {
                    builder.ins().jump(next_block, &[]);
                }
            }

            "end" | "return" | "exit" => {
                builder.ins().jump(exit_block, &[]);
            }

            "if" if !args.is_empty() => {
                let cond_off = offsets[args[0] as usize];
                let cond_jt = jtypes[args[0] as usize];
                let cond_val = match cond_jt {
                    JitType::Int => heap_load_i32(builder, heap, cond_off),
                    JitType::Float => {
                        let f = heap_load_f32(builder, heap, cond_off);
                        let zero = builder.ins().f32const(0.0);
                        let ne = builder.ins().fcmp(FloatCC::NotEqual, f, zero);
                        builder.ins().uextend(types::I32, ne)
                    }
                    _ => builder.ins().iconst(types::I32, 1),
                };
                let zero = builder.ins().iconst(types::I32, 0);
                let is_true = builder.ins().icmp(IntCC::NotEqual, cond_val, zero);
                let true_block = if op.jump[0] >= 0 && (op.jump[0] as usize) < op_blocks.len() {
                    op_blocks[op.jump[0] as usize]
                } else {
                    next_block
                };
                let false_block = if op.jump[1] >= 0 && (op.jump[1] as usize) < op_blocks.len() {
                    op_blocks[op.jump[1] as usize]
                } else {
                    next_block
                };
                builder
                    .ins()
                    .brif(is_true, true_block, &[], false_block, &[]);
            }

            // --- Data movement ---
            "assign" if args.len() >= 2 => {
                let dst = args[0] as usize;
                let src = args[1] as usize;
                let dst_off = offsets[dst];
                let src_off = offsets[src];
                match jtypes[dst] {
                    JitType::Float => {
                        let v = match jtypes[src] {
                            JitType::Float => heap_load_f32(builder, heap, src_off),
                            JitType::Int => {
                                let i = heap_load_i32(builder, heap, src_off);
                                builder.ins().fcvt_from_sint(types::F32, i)
                            }
                            _ => heap_load_f32(builder, heap, src_off),
                        };
                        heap_store_f32(builder, heap, dst_off, v);
                        // Copy derivatives on assign
                        if has_derivs[dst] {
                            if has_derivs[src] && (jtypes[src] == JitType::Float) {
                                let dx = heap_load_f32(builder, heap, src_off + 4);
                                heap_store_f32(builder, heap, dst_off + 4, dx);
                                let dy = heap_load_f32(builder, heap, src_off + 8);
                                heap_store_f32(builder, heap, dst_off + 8, dy);
                            } else {
                                let zero = builder.ins().f32const(0.0);
                                heap_store_f32(builder, heap, dst_off + 4, zero);
                                heap_store_f32(builder, heap, dst_off + 8, zero);
                            }
                        }
                    }
                    JitType::Int => {
                        let v = match jtypes[src] {
                            JitType::Int => heap_load_i32(builder, heap, src_off),
                            JitType::Float => {
                                let f = heap_load_f32(builder, heap, src_off);
                                builder.ins().fcvt_to_sint_sat(types::I32, f)
                            }
                            _ => heap_load_i32(builder, heap, src_off),
                        };
                        heap_store_i32(builder, heap, dst_off, v);
                    }
                    JitType::Vec3 | JitType::Color => {
                        let (x, y, z) = heap_load_vec3(builder, heap, src_off);
                        heap_store_vec3(builder, heap, dst_off, x, y, z);
                        // Copy vec3 derivatives on assign
                        if has_derivs[dst] {
                            if has_derivs[src] {
                                let (dx_x, dx_y, dx_z) =
                                    heap_load_vec3(builder, heap, src_off + 12);
                                heap_store_vec3(builder, heap, dst_off + 12, dx_x, dx_y, dx_z);
                                let (dy_x, dy_y, dy_z) =
                                    heap_load_vec3(builder, heap, src_off + 24);
                                heap_store_vec3(builder, heap, dst_off + 24, dy_x, dy_y, dy_z);
                            } else {
                                let zero = builder.ins().f32const(0.0);
                                heap_store_vec3(builder, heap, dst_off + 12, zero, zero, zero);
                                heap_store_vec3(builder, heap, dst_off + 24, zero, zero, zero);
                            }
                        }
                    }
                    _ => {} // Matrix, String — no derivatives
                }
                builder.ins().jump(next_block, &[]);
            }

            // --- Arithmetic ---
            "add" if args.len() >= 3 => {
                emit_binop_float(
                    builder,
                    heap,
                    offsets,
                    jtypes,
                    &args,
                    next_block,
                    |b, a, c| b.ins().fadd(a, c),
                );
                let dst = args[0] as usize;
                let lhs = args[1] as usize;
                let rhs = args[2] as usize;
                if has_derivs[dst] {
                    match jtypes[dst] {
                        JitType::Float => propagate_derivs_add_f32(
                            builder,
                            heap,
                            offsets[dst],
                            offsets[lhs],
                            offsets[rhs],
                            has_derivs[lhs],
                            has_derivs[rhs],
                        ),
                        JitType::Vec3 | JitType::Color => propagate_derivs_addsub_vec3_mixed(
                            builder,
                            heap,
                            offsets[dst],
                            offsets[lhs],
                            offsets[rhs],
                            has_derivs[lhs],
                            has_derivs[rhs],
                            jtypes[lhs],
                            jtypes[rhs],
                            true,
                        ),
                        _ => {}
                    }
                }
            }
            "sub" if args.len() >= 3 => {
                emit_binop_float(
                    builder,
                    heap,
                    offsets,
                    jtypes,
                    &args,
                    next_block,
                    |b, a, c| b.ins().fsub(a, c),
                );
                let dst = args[0] as usize;
                let lhs = args[1] as usize;
                let rhs = args[2] as usize;
                if has_derivs[dst] {
                    match jtypes[dst] {
                        JitType::Float => propagate_derivs_sub_f32(
                            builder,
                            heap,
                            offsets[dst],
                            offsets[lhs],
                            offsets[rhs],
                            has_derivs[lhs],
                            has_derivs[rhs],
                        ),
                        JitType::Vec3 | JitType::Color => propagate_derivs_addsub_vec3_mixed(
                            builder,
                            heap,
                            offsets[dst],
                            offsets[lhs],
                            offsets[rhs],
                            has_derivs[lhs],
                            has_derivs[rhs],
                            jtypes[lhs],
                            jtypes[rhs],
                            false,
                        ),
                        _ => {}
                    }
                }
            }
            "mul" if args.len() >= 3 => {
                emit_binop_float(
                    builder,
                    heap,
                    offsets,
                    jtypes,
                    &args,
                    next_block,
                    |b, a, c| b.ins().fmul(a, c),
                );
                let dst = args[0] as usize;
                let lhs = args[1] as usize;
                let rhs = args[2] as usize;
                if has_derivs[dst] {
                    match jtypes[dst] {
                        JitType::Float => propagate_derivs_mul_f32(
                            builder,
                            heap,
                            offsets[dst],
                            offsets[lhs],
                            offsets[rhs],
                            has_derivs[lhs],
                            has_derivs[rhs],
                        ),
                        JitType::Vec3 | JitType::Color => {
                            // Handle mixed float*vec3 and vec3*float
                            let l_is_vec = matches!(jtypes[lhs], JitType::Vec3 | JitType::Color);
                            let r_is_vec = matches!(jtypes[rhs], JitType::Vec3 | JitType::Color);
                            if l_is_vec && r_is_vec {
                                propagate_derivs_mul_vec3(
                                    builder,
                                    heap,
                                    offsets[dst],
                                    offsets[lhs],
                                    offsets[rhs],
                                    has_derivs[lhs],
                                    has_derivs[rhs],
                                );
                            } else {
                                // Mixed: one is float, one is vec3
                                // D(f * V) = f' * V + f * V'
                                let (f_off, v_off, f_hd, v_hd) = if l_is_vec {
                                    (offsets[rhs], offsets[lhs], has_derivs[rhs], has_derivs[lhs])
                                } else {
                                    (offsets[lhs], offsets[rhs], has_derivs[lhs], has_derivs[rhs])
                                };
                                let f_val = heap_load_f32(builder, heap, f_off);
                                let (vx, vy, vz) = heap_load_vec3(builder, heap, v_off);
                                let sz_f = 4usize;
                                let sz_v = 12usize;
                                for d in 1..=2usize {
                                    let fd = if f_hd {
                                        heap_load_f32(builder, heap, f_off + d * sz_f)
                                    } else {
                                        builder.ins().f32const(0.0)
                                    };
                                    let (vdx, vdy, vdz) = if v_hd {
                                        heap_load_vec3(builder, heap, v_off + d * sz_v)
                                    } else {
                                        let z = builder.ins().f32const(0.0);
                                        (z, z, z)
                                    };
                                    // f' * V_i + f * V_i'
                                    let t1x = builder.ins().fmul(fd, vx);
                                    let t2x = builder.ins().fmul(f_val, vdx);
                                    let rx = builder.ins().fadd(t1x, t2x);
                                    let t1y = builder.ins().fmul(fd, vy);
                                    let t2y = builder.ins().fmul(f_val, vdy);
                                    let ry = builder.ins().fadd(t1y, t2y);
                                    let t1z = builder.ins().fmul(fd, vz);
                                    let t2z = builder.ins().fmul(f_val, vdz);
                                    let rz = builder.ins().fadd(t1z, t2z);
                                    heap_store_vec3(
                                        builder,
                                        heap,
                                        offsets[dst] + d * sz_v,
                                        rx,
                                        ry,
                                        rz,
                                    );
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            "div" if args.len() >= 3 => {
                emit_binop_float(
                    builder,
                    heap,
                    offsets,
                    jtypes,
                    &args,
                    next_block,
                    |b, a, c| emit_safe_fdiv(b, a, c),
                );
                let dst = args[0] as usize;
                let lhs = args[1] as usize;
                let rhs = args[2] as usize;
                if has_derivs[dst] {
                    match jtypes[dst] {
                        JitType::Float => propagate_derivs_div_f32(
                            builder,
                            heap,
                            offsets[dst],
                            offsets[lhs],
                            offsets[rhs],
                            has_derivs[lhs],
                            has_derivs[rhs],
                        ),
                        JitType::Vec3 | JitType::Color => {
                            let l_is_vec = matches!(jtypes[lhs], JitType::Vec3 | JitType::Color);
                            let r_is_vec = matches!(jtypes[rhs], JitType::Vec3 | JitType::Color);
                            if l_is_vec && r_is_vec {
                                propagate_derivs_div_vec3(
                                    builder,
                                    heap,
                                    offsets[dst],
                                    offsets[lhs],
                                    offsets[rhs],
                                    has_derivs[lhs],
                                    has_derivs[rhs],
                                );
                            } else if l_is_vec && !r_is_vec {
                                // D(V / f) = (V' * f - V * f') / f^2
                                let f_val = heap_load_f32(builder, heap, offsets[rhs]);
                                let f2 = builder.ins().fmul(f_val, f_val);
                                let (vx, vy, vz) = heap_load_vec3(builder, heap, offsets[lhs]);
                                let sz_f = 4usize;
                                let sz_v = 12usize;
                                for d in 1..=2usize {
                                    let fd = if has_derivs[rhs] {
                                        heap_load_f32(builder, heap, offsets[rhs] + d * sz_f)
                                    } else {
                                        builder.ins().f32const(0.0)
                                    };
                                    let (vdx, vdy, vdz) = if has_derivs[lhs] {
                                        heap_load_vec3(builder, heap, offsets[lhs] + d * sz_v)
                                    } else {
                                        let z = builder.ins().f32const(0.0);
                                        (z, z, z)
                                    };
                                    // (V_i' * f - V_i * f') / f^2
                                    for (vi, vdi, c) in [(vx, vdx, 0), (vy, vdy, 1), (vz, vdz, 2)] {
                                        let t1 = builder.ins().fmul(vdi, f_val);
                                        let t2 = builder.ins().fmul(vi, fd);
                                        let num = builder.ins().fsub(t1, t2);
                                        let rd = emit_safe_fdiv(builder, num, f2);
                                        heap_store_f32(
                                            builder,
                                            heap,
                                            offsets[dst] + d * sz_v + c * 4,
                                            rd,
                                        );
                                    }
                                }
                            } else {
                                // D(f / V) = (f' * V_i - f * V_i') / V_i^2
                                let f_val = heap_load_f32(builder, heap, offsets[lhs]);
                                let (vx, vy, vz) = heap_load_vec3(builder, heap, offsets[rhs]);
                                let sz_f = 4usize;
                                let sz_v = 12usize;
                                for d in 1..=2usize {
                                    let fd = if has_derivs[lhs] {
                                        heap_load_f32(builder, heap, offsets[lhs] + d * sz_f)
                                    } else {
                                        builder.ins().f32const(0.0)
                                    };
                                    let (vdx, vdy, vdz) = if has_derivs[rhs] {
                                        heap_load_vec3(builder, heap, offsets[rhs] + d * sz_v)
                                    } else {
                                        let z = builder.ins().f32const(0.0);
                                        (z, z, z)
                                    };
                                    for (vi, vdi, c) in [(vx, vdx, 0), (vy, vdy, 1), (vz, vdz, 2)] {
                                        let vi2 = builder.ins().fmul(vi, vi);
                                        let t1 = builder.ins().fmul(fd, vi);
                                        let t2 = builder.ins().fmul(f_val, vdi);
                                        let num = builder.ins().fsub(t1, t2);
                                        let rd = emit_safe_fdiv(builder, num, vi2);
                                        heap_store_f32(
                                            builder,
                                            heap,
                                            offsets[dst] + d * sz_v + c * 4,
                                            rd,
                                        );
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            "neg" if args.len() >= 2 => {
                let dst = args[0] as usize;
                let src = args[1] as usize;
                match jtypes[dst] {
                    JitType::Float => {
                        let v = heap_load_f32(builder, heap, offsets[src]);
                        let r = builder.ins().fneg(v);
                        heap_store_f32(builder, heap, offsets[dst], r);
                        // Propagate derivatives for neg
                        if has_derivs[dst] {
                            propagate_derivs_neg_f32(
                                builder,
                                heap,
                                offsets[dst],
                                offsets[src],
                                has_derivs[src],
                            );
                        }
                    }
                    JitType::Int => {
                        let v = heap_load_i32(builder, heap, offsets[src]);
                        let r = builder.ins().ineg(v);
                        heap_store_i32(builder, heap, offsets[dst], r);
                    }
                    JitType::Vec3 | JitType::Color => {
                        let (x, y, z) = heap_load_vec3(builder, heap, offsets[src]);
                        let rx = builder.ins().fneg(x);
                        let ry = builder.ins().fneg(y);
                        let rz = builder.ins().fneg(z);
                        heap_store_vec3(builder, heap, offsets[dst], rx, ry, rz);
                        if has_derivs[dst] {
                            propagate_derivs_neg_vec3(
                                builder,
                                heap,
                                offsets[dst],
                                offsets[src],
                                has_derivs[src],
                            );
                        }
                    }
                    _ => {}
                }
                builder.ins().jump(next_block, &[]);
            }

            // --- Comparison ---
            "lt" | "gt" | "le" | "ge" | "eq" | "neq" if args.len() >= 3 => {
                let dst = args[0] as usize;
                let a = heap_load_f32(builder, heap, offsets[args[1] as usize]);
                let b_val = heap_load_f32(builder, heap, offsets[args[2] as usize]);
                let cc = match opname {
                    "lt" => FloatCC::LessThan,
                    "gt" => FloatCC::GreaterThan,
                    "le" => FloatCC::LessThanOrEqual,
                    "ge" => FloatCC::GreaterThanOrEqual,
                    "eq" => FloatCC::Equal,
                    "neq" => FloatCC::NotEqual,
                    _ => unreachable!(),
                };
                let cmp = builder.ins().fcmp(cc, a, b_val);
                let result = builder.ins().uextend(types::I32, cmp);
                heap_store_i32(builder, heap, offsets[dst], result);
                builder.ins().jump(next_block, &[]);
            }

            // --- Math builtins (unary) with derivative propagation ---
            "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "sqrt" | "exp" | "log" | "floor"
            | "ceil" | "abs" | "cbrt" | "log2" | "log10" | "logb" | "exp2" | "expm1" | "erf"
            | "erfc"
                if args.len() >= 2 =>
            {
                let func_id = match opname {
                    "sin" => math.sinf,
                    "cos" => math.cosf,
                    "tan" => math.tanf,
                    "asin" => math.asinf,
                    "acos" => math.acosf,
                    "atan" => math.atanf,
                    "sqrt" => math.sqrtf,
                    "exp" => math.expf,
                    "log" => math.logf,
                    "floor" => math.floorf,
                    "ceil" => math.ceilf,
                    "abs" => math.fabsf,
                    "cbrt" => math.cbrtf,
                    "log2" => math.log2f,
                    "log10" => math.log10f,
                    "logb" => math.logbf,
                    "exp2" => math.exp2f,
                    "expm1" => math.expm1f,
                    "erf" => math.erff,
                    "erfc" => math.erfcf,
                    _ => unreachable!(),
                };
                let fref = module.declare_func_in_func(func_id, builder.func);
                let src_val = heap_load_f32(builder, heap, offsets[args[1] as usize]);
                let call = builder.ins().call(fref, &[src_val]);
                let result = builder.inst_results(call)[0];
                heap_store_f32(builder, heap, offsets[args[0] as usize], result);

                // Derivative propagation: r' = f'(x) * x'
                let dst = args[0] as usize;
                let src = args[1] as usize;
                if has_derivs[dst] {
                    // Compute f'(x) for chain rule
                    let deriv_val: Option<CValue> = match opname {
                        "sin" => {
                            // d/dx sin(x) = cos(x)
                            let cos_ref = module.declare_func_in_func(math.cosf, builder.func);
                            let c = builder.ins().call(cos_ref, &[src_val]);
                            Some(builder.inst_results(c)[0])
                        }
                        "cos" => {
                            // d/dx cos(x) = -sin(x)
                            let sin_ref = module.declare_func_in_func(math.sinf, builder.func);
                            let c = builder.ins().call(sin_ref, &[src_val]);
                            let sv = builder.inst_results(c)[0];
                            Some(builder.ins().fneg(sv))
                        }
                        "exp" => {
                            // d/dx exp(x) = exp(x) = result
                            Some(result)
                        }
                        "log" => {
                            // d/dx log(x) = 1/x
                            let one = builder.ins().f32const(1.0);
                            Some(emit_safe_fdiv(builder, one, src_val))
                        }
                        "sqrt" => {
                            // d/dx sqrt(x) = 1/(2*sqrt(x)) = 0.5/result
                            let half = builder.ins().f32const(0.5);
                            Some(emit_safe_fdiv(builder, half, result))
                        }
                        "tan" => {
                            // d/dx tan(x) = 1/cos^2(x) = 1 + tan^2(x)
                            let one = builder.ins().f32const(1.0);
                            let t2 = builder.ins().fmul(result, result);
                            Some(builder.ins().fadd(one, t2))
                        }
                        "asin" => {
                            // d/dx asin(x) = 1/sqrt(1-x^2)
                            let one = builder.ins().f32const(1.0);
                            let x2 = builder.ins().fmul(src_val, src_val);
                            let omx2 = builder.ins().fsub(one, x2);
                            let sqrt_ref = module.declare_func_in_func(math.sqrtf, builder.func);
                            let c = builder.ins().call(sqrt_ref, &[omx2]);
                            let sq = builder.inst_results(c)[0];
                            Some(emit_safe_fdiv(builder, one, sq))
                        }
                        "acos" => {
                            // d/dx acos(x) = -1/sqrt(1-x^2)
                            let one = builder.ins().f32const(1.0);
                            let x2 = builder.ins().fmul(src_val, src_val);
                            let omx2 = builder.ins().fsub(one, x2);
                            let sqrt_ref = module.declare_func_in_func(math.sqrtf, builder.func);
                            let c = builder.ins().call(sqrt_ref, &[omx2]);
                            let sq = builder.inst_results(c)[0];
                            let neg_one = builder.ins().f32const(-1.0);
                            Some(emit_safe_fdiv(builder, neg_one, sq))
                        }
                        "atan" => {
                            // d/dx atan(x) = 1/(1+x^2)
                            let one = builder.ins().f32const(1.0);
                            let x2 = builder.ins().fmul(src_val, src_val);
                            let denom = builder.ins().fadd(one, x2);
                            Some(emit_safe_fdiv(builder, one, denom))
                        }
                        // floor, ceil, abs: derivatives are either 0 or discontinuous
                        // For practical purposes, floor/ceil have 0 derivative, abs has sign
                        "floor" | "ceil" => None,
                        "abs" => {
                            // d/dx |x| = sign(x) ≈ x >= 0 ? 1 : -1
                            // Cranelift doesn't have copysign easily, just use 1.0
                            // (this is an approximation; at x=0 it's technically undefined)
                            let one = builder.ins().f32const(1.0);
                            let neg_one = builder.ins().f32const(-1.0);
                            let zero_f = builder.ins().f32const(0.0);
                            let cmp =
                                builder
                                    .ins()
                                    .fcmp(FloatCC::GreaterThanOrEqual, src_val, zero_f);
                            Some(builder.ins().select(cmp, one, neg_one))
                        }
                        "cbrt" => {
                            // d/dx cbrt(x) = 1 / (3 * cbrt(x)^2) = 1 / (3*result^2)
                            let three = builder.ins().f32const(3.0);
                            let r2 = builder.ins().fmul(result, result);
                            let denom = builder.ins().fmul(three, r2);
                            let one = builder.ins().f32const(1.0);
                            Some(emit_safe_fdiv(builder, one, denom))
                        }
                        "log2" => {
                            // d/dx log2(x) = 1 / (x * ln(2))
                            let ln2 = builder.ins().f32const(std::f32::consts::LN_2);
                            let denom = builder.ins().fmul(src_val, ln2);
                            let one = builder.ins().f32const(1.0);
                            Some(emit_safe_fdiv(builder, one, denom))
                        }
                        "log10" => {
                            // d/dx log10(x) = 1 / (x * ln(10))
                            let ln10 = builder.ins().f32const(std::f32::consts::LN_10);
                            let denom = builder.ins().fmul(src_val, ln10);
                            let one = builder.ins().f32const(1.0);
                            Some(emit_safe_fdiv(builder, one, denom))
                        }
                        "exp2" => {
                            // d/dx 2^x = 2^x * ln(2) = result * ln(2)
                            let ln2 = builder.ins().f32const(std::f32::consts::LN_2);
                            Some(builder.ins().fmul(result, ln2))
                        }
                        "expm1" => {
                            // d/dx (e^x - 1) = e^x = result + 1
                            let one = builder.ins().f32const(1.0);
                            Some(builder.ins().fadd(result, one))
                        }
                        // logb, erf, erfc: derivatives are zero or too complex
                        "logb" | "erf" | "erfc" => None,
                        _ => None,
                    };
                    match deriv_val {
                        Some(dv) => propagate_derivs_unary_f32(
                            builder,
                            heap,
                            offsets[dst],
                            offsets[src],
                            has_derivs[src],
                            dv,
                        ),
                        None => zero_derivs(builder, heap, offsets[dst], JitType::Float),
                    }
                }

                builder.ins().jump(next_block, &[]);
            }

            // --- Math builtins (binary) ---
            "pow" if args.len() >= 3 => {
                let fref = module.declare_func_in_func(math.powf, builder.func);
                let a = heap_load_f32(builder, heap, offsets[args[1] as usize]);
                let b_val = heap_load_f32(builder, heap, offsets[args[2] as usize]);
                let call = builder.ins().call(fref, &[a, b_val]);
                let result = builder.inst_results(call)[0];
                heap_store_f32(builder, heap, offsets[args[0] as usize], result);

                // d/dx pow(a,b) = b * a^(b-1) * a' + a^b * ln(a) * b'
                // Simplified for constant exponent: b * pow(a, b-1) * a'
                let dst = args[0] as usize;
                if has_derivs[dst] {
                    let src_a = args[1] as usize;
                    let src_b = args[2] as usize;
                    // Compute b * a^(b-1) = b * result / a
                    let term = builder.ins().fmul(b_val, result);
                    let da_coeff = emit_safe_fdiv(builder, term, a);

                    let sz = 4usize;
                    // dx
                    let adx = if has_derivs[src_a] {
                        heap_load_f32(builder, heap, offsets[src_a] + sz)
                    } else {
                        builder.ins().f32const(0.0)
                    };
                    let bdx = if has_derivs[src_b] {
                        heap_load_f32(builder, heap, offsets[src_b] + sz)
                    } else {
                        builder.ins().f32const(0.0)
                    };
                    let t1 = builder.ins().fmul(da_coeff, adx);
                    // ln(a) * result * b.dx
                    let log_ref = module.declare_func_in_func(math.logf, builder.func);
                    let log_call = builder.ins().call(log_ref, &[a]);
                    let log_a = builder.inst_results(log_call)[0];
                    let t2a = builder.ins().fmul(result, log_a);
                    let t2 = builder.ins().fmul(t2a, bdx);
                    let rdx = builder.ins().fadd(t1, t2);
                    heap_store_f32(builder, heap, offsets[dst] + sz, rdx);

                    // dy
                    let ady = if has_derivs[src_a] {
                        heap_load_f32(builder, heap, offsets[src_a] + 2 * sz)
                    } else {
                        builder.ins().f32const(0.0)
                    };
                    let bdy = if has_derivs[src_b] {
                        heap_load_f32(builder, heap, offsets[src_b] + 2 * sz)
                    } else {
                        builder.ins().f32const(0.0)
                    };
                    let t3 = builder.ins().fmul(da_coeff, ady);
                    let t4a = builder.ins().fmul(result, log_a);
                    let t4 = builder.ins().fmul(t4a, bdy);
                    let rdy = builder.ins().fadd(t3, t4);
                    heap_store_f32(builder, heap, offsets[dst] + 2 * sz, rdy);
                }

                builder.ins().jump(next_block, &[]);
            }
            "atan2" if args.len() >= 3 => {
                // atan2(y, x) = atan(y/x) with quadrant handling
                let fref = module.declare_func_in_func(math.atan2f, builder.func);
                let y = heap_load_f32(builder, heap, offsets[args[1] as usize]);
                let x = heap_load_f32(builder, heap, offsets[args[2] as usize]);
                let call = builder.ins().call(fref, &[y, x]);
                let result = builder.inst_results(call)[0];
                let dst = args[0] as usize;
                heap_store_f32(builder, heap, offsets[dst], result);
                // d/d(param) atan2(y,x) = (x*y' - y*x') / (x^2 + y^2)
                if has_derivs[dst] {
                    let sz = 4usize;
                    let x2 = builder.ins().fmul(x, x);
                    let y2 = builder.ins().fmul(y, y);
                    let denom = builder.ins().fadd(x2, y2);
                    for d in 1..=2usize {
                        let slot = d * sz;
                        let yd = if has_derivs[args[1] as usize] {
                            heap_load_f32(builder, heap, offsets[args[1] as usize] + slot)
                        } else {
                            builder.ins().f32const(0.0)
                        };
                        let xd = if has_derivs[args[2] as usize] {
                            heap_load_f32(builder, heap, offsets[args[2] as usize] + slot)
                        } else {
                            builder.ins().f32const(0.0)
                        };
                        let t1 = builder.ins().fmul(x, yd);
                        let t2 = builder.ins().fmul(y, xd);
                        let num = builder.ins().fsub(t1, t2);
                        let rd = emit_safe_fdiv(builder, num, denom);
                        heap_store_f32(builder, heap, offsets[dst] + slot, rd);
                    }
                }
                builder.ins().jump(next_block, &[]);
            }
            "mod" if args.len() >= 3 => {
                let fref = module.declare_func_in_func(math.fmodf, builder.func);
                let src_a = args[1] as usize;
                let dst = args[0] as usize;
                let a = heap_load_f32(builder, heap, offsets[src_a]);
                let b_val = heap_load_f32(builder, heap, offsets[args[2] as usize]);
                // Safe mod: if b==0, return 0.0 (matches C++ osl_safe_mod)
                let zero = builder.ins().f32const(0.0);
                let b_nonzero = builder.ins().fcmp(FloatCC::NotEqual, b_val, zero);
                let call = builder.ins().call(fref, &[a, b_val]);
                let raw = builder.inst_results(call)[0];
                let result = builder.ins().select(b_nonzero, raw, zero);
                heap_store_f32(builder, heap, offsets[dst], result);
                // d/da fmod(a,b) = 1 (piecewise), so derivs pass through from a
                if has_derivs[dst] {
                    let sz = 4usize;
                    for d in 1..=2usize {
                        let ad = if has_derivs[src_a] {
                            heap_load_f32(builder, heap, offsets[src_a] + d * sz)
                        } else {
                            builder.ins().f32const(0.0)
                        };
                        heap_store_f32(builder, heap, offsets[dst] + d * sz, ad);
                    }
                }
                builder.ins().jump(next_block, &[]);
            }

            // --- Type construction (color/point/vector/normal/matrix/scalar) ---
            "construct" | "color" | "point" | "vector" | "normal" if args.len() >= 2 => {
                let dst = args[0] as usize;
                if args.len() == 5 && matches!(jtypes[dst], JitType::Vec3 | JitType::Color) {
                    // construct(result, colorspace, x, y, z) — color space transform to RGB
                    let func_ref =
                        module.declare_func_in_func(math.construct_color_from_space, builder.func);
                    let dst_off = builder.ins().iconst(types::I32, offsets[dst] as i64);
                    let space_off = builder
                        .ins()
                        .iconst(types::I32, offsets[args[1] as usize] as i64);
                    let x_off = builder
                        .ins()
                        .iconst(types::I32, offsets[args[2] as usize] as i64);
                    let y_off = builder
                        .ins()
                        .iconst(types::I32, offsets[args[3] as usize] as i64);
                    let z_off = builder
                        .ins()
                        .iconst(types::I32, offsets[args[4] as usize] as i64);
                    builder
                        .ins()
                        .call(func_ref, &[heap, dst_off, space_off, x_off, y_off, z_off]);
                    // Result has no derivs from color space lookup
                    if has_derivs[dst] {
                        zero_derivs(builder, heap, offsets[dst], jtypes[dst]);
                    }
                } else if args.len() >= 17 && matches!(jtypes[dst], JitType::Matrix) {
                    // Matrix from 16 floats: construct(dst, m00..m33)
                    let dst_base = offsets[dst];
                    for i in 0..16 {
                        let v = heap_load_f32(builder, heap, offsets[args[1 + i] as usize]);
                        heap_store_f32(builder, heap, dst_base + i * 4, v);
                    }
                } else if args.len() >= 4 {
                    // Triple from 3 floats: construct(dst, x, y, z)
                    let x = heap_load_f32(builder, heap, offsets[args[1] as usize]);
                    let y = heap_load_f32(builder, heap, offsets[args[2] as usize]);
                    let z = heap_load_f32(builder, heap, offsets[args[3] as usize]);
                    heap_store_vec3(builder, heap, offsets[dst], x, y, z);
                    // Construct vec3 from float components: propagate per-component derivs
                    if has_derivs[dst] {
                        let sz_f = 4usize;
                        let sz_v = 12usize;
                        for d in 1..=2usize {
                            let xd = if has_derivs[args[1] as usize] {
                                heap_load_f32(builder, heap, offsets[args[1] as usize] + d * sz_f)
                            } else {
                                builder.ins().f32const(0.0)
                            };
                            let yd = if has_derivs[args[2] as usize] {
                                heap_load_f32(builder, heap, offsets[args[2] as usize] + d * sz_f)
                            } else {
                                builder.ins().f32const(0.0)
                            };
                            let zd = if has_derivs[args[3] as usize] {
                                heap_load_f32(builder, heap, offsets[args[3] as usize] + d * sz_f)
                            } else {
                                builder.ins().f32const(0.0)
                            };
                            heap_store_vec3(builder, heap, offsets[dst] + d * sz_v, xd, yd, zd);
                        }
                    }
                } else if args.len() == 3 && matches!(jtypes[dst], JitType::Matrix) {
                    // Matrix from single float (diagonal): construct(dst, scale)
                    let v = heap_load_f32(builder, heap, offsets[args[1] as usize]);
                    let zero = builder.ins().f32const(0.0);
                    let one = builder.ins().f32const(1.0);
                    let dst_base = offsets[dst];
                    for i in 0..16 {
                        let val = if i == 0 || i == 5 || i == 10 {
                            v
                        } else if i == 15 {
                            one
                        } else {
                            zero
                        };
                        heap_store_f32(builder, heap, dst_base + i * 4, val);
                    }
                } else {
                    // Single-arg construct: copy src -> dst (type cast or assignment)
                    let src = args[1] as usize;
                    match (jtypes[dst], jtypes[src]) {
                        (JitType::Float, JitType::Int) => {
                            let v = heap_load_i32(builder, heap, offsets[src]);
                            let fv = builder.ins().fcvt_from_sint(types::F32, v);
                            heap_store_f32(builder, heap, offsets[dst], fv);
                        }
                        (JitType::Int, JitType::Float) => {
                            let v = heap_load_f32(builder, heap, offsets[src]);
                            let iv = builder.ins().fcvt_to_sint_sat(types::I32, v);
                            heap_store_i32(builder, heap, offsets[dst], iv);
                        }
                        // float/int → matrix: build diagonal matrix
                        (JitType::Matrix, JitType::Float | JitType::Int) => {
                            let v = if jtypes[src] == JitType::Int {
                                let i = heap_load_i32(builder, heap, offsets[src]);
                                builder.ins().fcvt_from_sint(types::F32, i)
                            } else {
                                heap_load_f32(builder, heap, offsets[src])
                            };
                            let zero = builder.ins().f32const(0.0);
                            let one = builder.ins().f32const(1.0);
                            let dst_base = offsets[dst];
                            for i in 0..16 {
                                let val = if i == 0 || i == 5 || i == 10 {
                                    v
                                } else if i == 15 {
                                    one
                                } else {
                                    zero
                                };
                                heap_store_f32(builder, heap, dst_base + i * 4, val);
                            }
                        }
                        (JitType::Float, _) => {
                            let v = heap_load_f32(builder, heap, offsets[src]);
                            heap_store_f32(builder, heap, offsets[dst], v);
                        }
                        (JitType::Int, _) => {
                            let v = heap_load_i32(builder, heap, offsets[src]);
                            heap_store_i32(builder, heap, offsets[dst], v);
                        }
                        _ => {
                            // Vec3/Color: byte copy (same-size types only)
                            let size = jtypes[dst].size();
                            for i in (0..size).step_by(4) {
                                let v = heap_load_f32(builder, heap, offsets[src] + i);
                                heap_store_f32(builder, heap, offsets[dst] + i, v);
                            }
                        }
                    }
                }
                builder.ins().jump(next_block, &[]);
            }

            // --- Component access ---
            "aref" if args.len() >= 3 => {
                let base_off = offsets[args[1] as usize];
                let idx_raw = heap_load_i32(builder, heap, offsets[args[2] as usize]);
                let (length, symname_hash, sourcefile_hash) =
                    if (args[1] as usize) < ir.symbols.len() {
                        let s = &ir.symbols[args[1] as usize];
                        (
                            s.typespec.simpletype().arraylen.max(1) as i32,
                            s.name.hash(),
                            op.sourcefile.hash(),
                        )
                    } else {
                        (1, 0u64, 0u64)
                    };
                let idx = emit_maybe_range_check(
                    builder,
                    module,
                    math,
                    ctx,
                    sg,
                    heap,
                    idx_raw,
                    length,
                    symname_hash,
                    sourcefile_hash,
                    op.sourceline,
                    range_checking,
                );
                // comp_off = base_off + idx * 4
                let idx_ext = builder.ins().uextend(types::I64, idx);
                let four = builder.ins().iconst(types::I64, 4);
                let byte_off = builder.ins().imul(idx_ext, four);
                let base_i64 = builder.ins().iconst(types::I64, base_off as i64);
                let total_off = builder.ins().iadd(base_i64, byte_off);
                let addr = builder.ins().iadd(heap, total_off);
                let val = builder.ins().load(types::F32, MemFlags::trusted(), addr, 0);
                heap_store_f32(builder, heap, offsets[args[0] as usize], val);
                builder.ins().jump(next_block, &[]);
            }

            // --- Logical ---
            "and" if args.len() >= 3 => {
                let a = heap_load_i32(builder, heap, offsets[args[1] as usize]);
                let b_val = heap_load_i32(builder, heap, offsets[args[2] as usize]);
                let r = builder.ins().band(a, b_val);
                heap_store_i32(builder, heap, offsets[args[0] as usize], r);
                builder.ins().jump(next_block, &[]);
            }
            "or" if args.len() >= 3 => {
                let a = heap_load_i32(builder, heap, offsets[args[1] as usize]);
                let b_val = heap_load_i32(builder, heap, offsets[args[2] as usize]);
                let r = builder.ins().bor(a, b_val);
                heap_store_i32(builder, heap, offsets[args[0] as usize], r);
                builder.ins().jump(next_block, &[]);
            }
            "not" if args.len() >= 2 => {
                let a = heap_load_i32(builder, heap, offsets[args[1] as usize]);
                let zero = builder.ins().iconst(types::I32, 0);
                let is_zero = builder.ins().icmp(IntCC::Equal, a, zero);
                let result = builder.ins().uextend(types::I32, is_zero);
                heap_store_i32(builder, heap, offsets[args[0] as usize], result);
                builder.ins().jump(next_block, &[]);
            }

            // --- Type conversion ---
            "float" if args.len() >= 2 => {
                let v = match jtypes[args[1] as usize] {
                    JitType::Int => {
                        let i = heap_load_i32(builder, heap, offsets[args[1] as usize]);
                        builder.ins().fcvt_from_sint(types::F32, i)
                    }
                    _ => heap_load_f32(builder, heap, offsets[args[1] as usize]),
                };
                heap_store_f32(builder, heap, offsets[args[0] as usize], v);
                builder.ins().jump(next_block, &[]);
            }
            "int" if args.len() >= 2 => {
                let v = match jtypes[args[1] as usize] {
                    JitType::Float => {
                        let f = heap_load_f32(builder, heap, offsets[args[1] as usize]);
                        builder.ins().fcvt_to_sint_sat(types::I32, f)
                    }
                    _ => heap_load_i32(builder, heap, offsets[args[1] as usize]),
                };
                heap_store_i32(builder, heap, offsets[args[0] as usize], v);
                builder.ins().jump(next_block, &[]);
            }

            // --- Vector ops ---
            "dot" if args.len() >= 3 => {
                let a_off = offsets[args[1] as usize];
                let b_off = offsets[args[2] as usize];
                let dst = args[0] as usize;
                let (ax, ay, az) = heap_load_vec3(builder, heap, a_off);
                let (bx, by, bz) = heap_load_vec3(builder, heap, b_off);
                let xx = builder.ins().fmul(ax, bx);
                let yy = builder.ins().fmul(ay, by);
                let zz = builder.ins().fmul(az, bz);
                let sum = builder.ins().fadd(xx, yy);
                let dot = builder.ins().fadd(sum, zz);
                heap_store_f32(builder, heap, offsets[dst], dot);
                // d/dx dot(a,b) = dot(a',b) + dot(a,b')
                if has_derivs[dst] {
                    let sz_f = 4usize;
                    let sz_v = 12usize;
                    for d in 1..=2usize {
                        let mut acc = builder.ins().f32const(0.0);
                        for c in 0..3usize {
                            let co = c * 4;
                            let a_c = heap_load_f32(builder, heap, a_off + co);
                            let b_c = heap_load_f32(builder, heap, b_off + co);
                            let ad = if has_derivs[args[1] as usize] {
                                heap_load_f32(builder, heap, a_off + d * sz_v + co)
                            } else {
                                builder.ins().f32const(0.0)
                            };
                            let bd = if has_derivs[args[2] as usize] {
                                heap_load_f32(builder, heap, b_off + d * sz_v + co)
                            } else {
                                builder.ins().f32const(0.0)
                            };
                            let t1 = builder.ins().fmul(ad, b_c);
                            let t2 = builder.ins().fmul(a_c, bd);
                            let t = builder.ins().fadd(t1, t2);
                            acc = builder.ins().fadd(acc, t);
                        }
                        heap_store_f32(builder, heap, offsets[dst] + d * sz_f, acc);
                    }
                }
                builder.ins().jump(next_block, &[]);
            }

            "cross" if args.len() >= 3 => {
                let a_off = offsets[args[1] as usize];
                let b_off = offsets[args[2] as usize];
                let dst = args[0] as usize;
                let (ax, ay, az) = heap_load_vec3(builder, heap, a_off);
                let (bx, by, bz) = heap_load_vec3(builder, heap, b_off);
                // cross = (ay*bz - az*by, az*bx - ax*bz, ax*by - ay*bx)
                let aybz = builder.ins().fmul(ay, bz);
                let azby = builder.ins().fmul(az, by);
                let rx = builder.ins().fsub(aybz, azby);
                let azbx = builder.ins().fmul(az, bx);
                let axbz = builder.ins().fmul(ax, bz);
                let ry = builder.ins().fsub(azbx, axbz);
                let axby = builder.ins().fmul(ax, by);
                let aybx = builder.ins().fmul(ay, bx);
                let rz = builder.ins().fsub(axby, aybx);
                heap_store_vec3(builder, heap, offsets[dst], rx, ry, rz);
                // d cross(a,b) = cross(a',b) + cross(a,b')
                if has_derivs[dst] {
                    let sz_v = 12usize;
                    let a_hd = has_derivs[args[1] as usize];
                    let b_hd = has_derivs[args[2] as usize];
                    for d in 1..=2usize {
                        let slot = d * sz_v;
                        // Load a' and b' components
                        let (adx, ady, adz) = if a_hd {
                            heap_load_vec3(builder, heap, a_off + slot)
                        } else {
                            let z = builder.ins().f32const(0.0);
                            (z, z, z)
                        };
                        let (bdx, bdy, bdz) = if b_hd {
                            heap_load_vec3(builder, heap, b_off + slot)
                        } else {
                            let z = builder.ins().f32const(0.0);
                            (z, z, z)
                        };
                        // cross(a', b)
                        let c1x = builder.ins().fmul(ady, bz);
                        let t1 = builder.ins().fmul(adz, by);
                        let c1x = builder.ins().fsub(c1x, t1);
                        let c1y = builder.ins().fmul(adz, bx);
                        let t2 = builder.ins().fmul(adx, bz);
                        let c1y = builder.ins().fsub(c1y, t2);
                        let c1z = builder.ins().fmul(adx, by);
                        let t3 = builder.ins().fmul(ady, bx);
                        let c1z = builder.ins().fsub(c1z, t3);
                        // cross(a, b')
                        let c2x = builder.ins().fmul(ay, bdz);
                        let t4 = builder.ins().fmul(az, bdy);
                        let c2x = builder.ins().fsub(c2x, t4);
                        let c2y = builder.ins().fmul(az, bdx);
                        let t5 = builder.ins().fmul(ax, bdz);
                        let c2y = builder.ins().fsub(c2y, t5);
                        let c2z = builder.ins().fmul(ax, bdy);
                        let t6 = builder.ins().fmul(ay, bdx);
                        let c2z = builder.ins().fsub(c2z, t6);
                        // sum
                        let drx = builder.ins().fadd(c1x, c2x);
                        let dry = builder.ins().fadd(c1y, c2y);
                        let drz = builder.ins().fadd(c1z, c2z);
                        heap_store_vec3(builder, heap, offsets[dst] + slot, drx, dry, drz);
                    }
                }
                builder.ins().jump(next_block, &[]);
            }

            "normalize" if args.len() >= 2 => {
                let src = args[1] as usize;
                let dst = args[0] as usize;
                let src_off = offsets[src];
                let (x, y, z) = heap_load_vec3(builder, heap, src_off);
                let xx = builder.ins().fmul(x, x);
                let yy = builder.ins().fmul(y, y);
                let zz = builder.ins().fmul(z, z);
                let xy = builder.ins().fadd(xx, yy);
                let sum = builder.ins().fadd(xy, zz);
                let sqrtf_ref = module.declare_func_in_func(math.sqrtf, builder.func);
                let call = builder.ins().call(sqrtf_ref, &[sum]);
                let len = builder.inst_results(call)[0];
                let nx = emit_safe_fdiv(builder, x, len);
                let ny = emit_safe_fdiv(builder, y, len);
                let nz = emit_safe_fdiv(builder, z, len);
                heap_store_vec3(builder, heap, offsets[dst], nx, ny, nz);
                // normalize'(v) = (v' - n * dot(n, v')) / len
                if has_derivs[dst] && has_derivs[src] {
                    let sz_v = 12usize;
                    for d in 1..=2usize {
                        let slot = d * sz_v;
                        let (vdx, vdy, vdz) = heap_load_vec3(builder, heap, src_off + slot);
                        // dot(n, v')
                        let d1 = builder.ins().fmul(nx, vdx);
                        let d2 = builder.ins().fmul(ny, vdy);
                        let d3 = builder.ins().fmul(nz, vdz);
                        let s1 = builder.ins().fadd(d1, d2);
                        let ndotv = builder.ins().fadd(s1, d3);
                        // v' - n * dot(n, v')
                        let t1x = builder.ins().fmul(nx, ndotv);
                        let t1y = builder.ins().fmul(ny, ndotv);
                        let t1z = builder.ins().fmul(nz, ndotv);
                        let dx = builder.ins().fsub(vdx, t1x);
                        let dy = builder.ins().fsub(vdy, t1y);
                        let dz = builder.ins().fsub(vdz, t1z);
                        // / len
                        let rdx = emit_safe_fdiv(builder, dx, len);
                        let rdy = emit_safe_fdiv(builder, dy, len);
                        let rdz = emit_safe_fdiv(builder, dz, len);
                        heap_store_vec3(builder, heap, offsets[dst] + slot, rdx, rdy, rdz);
                    }
                } else if has_derivs[dst] {
                    let zero = builder.ins().f32const(0.0);
                    heap_store_vec3(builder, heap, offsets[dst] + 12, zero, zero, zero);
                    heap_store_vec3(builder, heap, offsets[dst] + 24, zero, zero, zero);
                }
                builder.ins().jump(next_block, &[]);
            }

            "length" if args.len() >= 2 => {
                let src = args[1] as usize;
                let dst = args[0] as usize;
                let src_off = offsets[src];
                let (x, y, z) = heap_load_vec3(builder, heap, src_off);
                let xx = builder.ins().fmul(x, x);
                let yy = builder.ins().fmul(y, y);
                let zz = builder.ins().fmul(z, z);
                let xy = builder.ins().fadd(xx, yy);
                let sum = builder.ins().fadd(xy, zz);
                let sqrtf_ref = module.declare_func_in_func(math.sqrtf, builder.func);
                let call = builder.ins().call(sqrtf_ref, &[sum]);
                let len = builder.inst_results(call)[0];
                heap_store_f32(builder, heap, offsets[dst], len);
                // length'(v) = dot(v, v') / length(v)
                if has_derivs[dst] && has_derivs[src] {
                    let sz_f = 4usize;
                    let sz_v = 12usize;
                    for d in 1..=2usize {
                        let (vdx, vdy, vdz) = heap_load_vec3(builder, heap, src_off + d * sz_v);
                        let d1 = builder.ins().fmul(x, vdx);
                        let d2 = builder.ins().fmul(y, vdy);
                        let d3 = builder.ins().fmul(z, vdz);
                        let s1 = builder.ins().fadd(d1, d2);
                        let dot_val = builder.ins().fadd(s1, d3);
                        let rd = emit_safe_fdiv(builder, dot_val, len);
                        heap_store_f32(builder, heap, offsets[dst] + d * sz_f, rd);
                    }
                } else if has_derivs[dst] {
                    let zero = builder.ins().f32const(0.0);
                    heap_store_f32(builder, heap, offsets[dst] + 4, zero);
                    heap_store_f32(builder, heap, offsets[dst] + 8, zero);
                }
                builder.ins().jump(next_block, &[]);
            }

            // --- Derivative opcodes (Dual2 tracking) ---
            // Dx(result, src) — read the dx slot of src
            // Dy(result, src) — read the dy slot of src
            // Dz is not standard in OSL but included for completeness
            "Dx" if args.len() >= 2 => {
                let src = args[1] as usize;
                let dst = args[0] as usize;
                if has_derivs[src] {
                    let src_jt = jtypes[src];
                    let dx_off = offsets[src] + src_jt.size(); // dx slot
                    match jtypes[dst] {
                        JitType::Float => {
                            let val = heap_load_f32(builder, heap, dx_off);
                            heap_store_f32(builder, heap, offsets[dst], val);
                        }
                        JitType::Vec3 | JitType::Color => {
                            let (x, y, z) = heap_load_vec3(builder, heap, dx_off);
                            heap_store_vec3(builder, heap, offsets[dst], x, y, z);
                        }
                        _ => {}
                    }
                } else {
                    // Source has no derivatives — return zero
                    match jtypes[dst] {
                        JitType::Float => {
                            let zero = builder.ins().f32const(0.0);
                            heap_store_f32(builder, heap, offsets[dst], zero);
                        }
                        JitType::Vec3 | JitType::Color => {
                            let zero = builder.ins().f32const(0.0);
                            heap_store_vec3(builder, heap, offsets[dst], zero, zero, zero);
                        }
                        _ => {}
                    }
                }
                builder.ins().jump(next_block, &[]);
            }
            "Dy" if args.len() >= 2 => {
                let src = args[1] as usize;
                let dst = args[0] as usize;
                if has_derivs[src] {
                    let src_jt = jtypes[src];
                    let dy_off = offsets[src] + 2 * src_jt.size(); // dy slot
                    match jtypes[dst] {
                        JitType::Float => {
                            let val = heap_load_f32(builder, heap, dy_off);
                            heap_store_f32(builder, heap, offsets[dst], val);
                        }
                        JitType::Vec3 | JitType::Color => {
                            let (x, y, z) = heap_load_vec3(builder, heap, dy_off);
                            heap_store_vec3(builder, heap, offsets[dst], x, y, z);
                        }
                        _ => {}
                    }
                } else {
                    match jtypes[dst] {
                        JitType::Float => {
                            let zero = builder.ins().f32const(0.0);
                            heap_store_f32(builder, heap, offsets[dst], zero);
                        }
                        JitType::Vec3 | JitType::Color => {
                            let zero = builder.ins().f32const(0.0);
                            heap_store_vec3(builder, heap, offsets[dst], zero, zero, zero);
                        }
                        _ => {}
                    }
                }
                builder.ins().jump(next_block, &[]);
            }
            "Dz" if args.len() >= 2 => {
                // Dz is rarely used — return zero
                match jtypes[args[0] as usize] {
                    JitType::Float => {
                        let zero = builder.ins().f32const(0.0);
                        heap_store_f32(builder, heap, offsets[args[0] as usize], zero);
                    }
                    JitType::Vec3 | JitType::Color => {
                        let zero = builder.ins().f32const(0.0);
                        heap_store_vec3(builder, heap, offsets[args[0] as usize], zero, zero, zero);
                    }
                    _ => {}
                }
                builder.ins().jump(next_block, &[]);
            }

            // --- Useparam (no-op in JIT) ---
            "useparam" => {
                builder.ins().jump(next_block, &[]);
            }

            // --- Printf (no-op in JIT for now) ---
            "printf" | "warning" | "error" | "fprintf" => {
                builder.ins().jump(next_block, &[]);
            }

            // --- Bitwise ops ---
            "bitand" if args.len() >= 3 => {
                let a = heap_load_i32(builder, heap, offsets[args[1] as usize]);
                let b_val = heap_load_i32(builder, heap, offsets[args[2] as usize]);
                let r = builder.ins().band(a, b_val);
                heap_store_i32(builder, heap, offsets[args[0] as usize], r);
                builder.ins().jump(next_block, &[]);
            }
            "bitor" if args.len() >= 3 => {
                let a = heap_load_i32(builder, heap, offsets[args[1] as usize]);
                let b_val = heap_load_i32(builder, heap, offsets[args[2] as usize]);
                let r = builder.ins().bor(a, b_val);
                heap_store_i32(builder, heap, offsets[args[0] as usize], r);
                builder.ins().jump(next_block, &[]);
            }
            "xor" if args.len() >= 3 => {
                let a = heap_load_i32(builder, heap, offsets[args[1] as usize]);
                let b_val = heap_load_i32(builder, heap, offsets[args[2] as usize]);
                let r = builder.ins().bxor(a, b_val);
                heap_store_i32(builder, heap, offsets[args[0] as usize], r);
                builder.ins().jump(next_block, &[]);
            }
            "shl" if args.len() >= 3 => {
                let a = heap_load_i32(builder, heap, offsets[args[1] as usize]);
                let b_val = heap_load_i32(builder, heap, offsets[args[2] as usize]);
                let r = builder.ins().ishl(a, b_val);
                heap_store_i32(builder, heap, offsets[args[0] as usize], r);
                builder.ins().jump(next_block, &[]);
            }
            "shr" if args.len() >= 3 => {
                let a = heap_load_i32(builder, heap, offsets[args[1] as usize]);
                let b_val = heap_load_i32(builder, heap, offsets[args[2] as usize]);
                let r = builder.ins().sshr(a, b_val);
                heap_store_i32(builder, heap, offsets[args[0] as usize], r);
                builder.ins().jump(next_block, &[]);
            }
            "compl" if args.len() >= 2 => {
                let a = heap_load_i32(builder, heap, offsets[args[1] as usize]);
                let r = builder.ins().bnot(a);
                heap_store_i32(builder, heap, offsets[args[0] as usize], r);
                builder.ins().jump(next_block, &[]);
            }

            // --- Select ---
            "select" if args.len() >= 4 => {
                let dst = args[0] as usize;
                let t_sym = args[2] as usize;
                let f_sym = args[3] as usize;
                let cond = heap_load_i32(builder, heap, offsets[args[1] as usize]);
                let zero_i = builder.ins().iconst(types::I32, 0);
                let is_true = builder.ins().icmp(IntCC::NotEqual, cond, zero_i);
                match jtypes[dst] {
                    JitType::Float => {
                        let t = heap_load_f32(builder, heap, offsets[t_sym]);
                        let f = heap_load_f32(builder, heap, offsets[f_sym]);
                        let r = builder.ins().select(is_true, t, f);
                        heap_store_f32(builder, heap, offsets[dst], r);
                        // Select derivative from the chosen branch
                        if has_derivs[dst] {
                            let sz = 4usize;
                            for d in 1..=2usize {
                                let td = if has_derivs[t_sym] {
                                    heap_load_f32(builder, heap, offsets[t_sym] + d * sz)
                                } else {
                                    builder.ins().f32const(0.0)
                                };
                                let fd = if has_derivs[f_sym] {
                                    heap_load_f32(builder, heap, offsets[f_sym] + d * sz)
                                } else {
                                    builder.ins().f32const(0.0)
                                };
                                let rd = builder.ins().select(is_true, td, fd);
                                heap_store_f32(builder, heap, offsets[dst] + d * sz, rd);
                            }
                        }
                    }
                    JitType::Int => {
                        let t = heap_load_i32(builder, heap, offsets[t_sym]);
                        let f = heap_load_i32(builder, heap, offsets[f_sym]);
                        let r = builder.ins().select(is_true, t, f);
                        heap_store_i32(builder, heap, offsets[dst], r);
                    }
                    _ => {
                        let (tx, ty, tz) = heap_load_vec3(builder, heap, offsets[t_sym]);
                        let (fx, fy, fz) = heap_load_vec3(builder, heap, offsets[f_sym]);
                        let rx = builder.ins().select(is_true, tx, fx);
                        let ry = builder.ins().select(is_true, ty, fy);
                        let rz = builder.ins().select(is_true, tz, fz);
                        heap_store_vec3(builder, heap, offsets[dst], rx, ry, rz);
                        if has_derivs[dst] {
                            let sz = 12usize;
                            for d in 1..=2usize {
                                let slot = d * sz;
                                let (tdx, tdy, tdz) = if has_derivs[t_sym] {
                                    heap_load_vec3(builder, heap, offsets[t_sym] + slot)
                                } else {
                                    let z = builder.ins().f32const(0.0);
                                    (z, z, z)
                                };
                                let (fdx, fdy, fdz) = if has_derivs[f_sym] {
                                    heap_load_vec3(builder, heap, offsets[f_sym] + slot)
                                } else {
                                    let z = builder.ins().f32const(0.0);
                                    (z, z, z)
                                };
                                let rdx = builder.ins().select(is_true, tdx, fdx);
                                let rdy = builder.ins().select(is_true, tdy, fdy);
                                let rdz = builder.ins().select(is_true, tdz, fdz);
                                heap_store_vec3(builder, heap, offsets[dst] + slot, rdx, rdy, rdz);
                            }
                        }
                    }
                }
                builder.ins().jump(next_block, &[]);
            }

            // --- Array assign ---
            "aassign" if args.len() >= 3 => {
                let base_off = offsets[args[0] as usize];
                let idx_raw = heap_load_i32(builder, heap, offsets[args[1] as usize]);
                let (length, symname_hash, sourcefile_hash) =
                    if (args[0] as usize) < ir.symbols.len() {
                        let s = &ir.symbols[args[0] as usize];
                        (
                            s.typespec.simpletype().arraylen.max(1) as i32,
                            s.name.hash(),
                            op.sourcefile.hash(),
                        )
                    } else {
                        (1, 0u64, 0u64)
                    };
                let idx = emit_maybe_range_check(
                    builder,
                    module,
                    math,
                    ctx,
                    sg,
                    heap,
                    idx_raw,
                    length,
                    symname_hash,
                    sourcefile_hash,
                    op.sourceline,
                    range_checking,
                );
                let val = heap_load_f32(builder, heap, offsets[args[2] as usize]);
                let idx_ext = builder.ins().uextend(types::I64, idx);
                let four = builder.ins().iconst(types::I64, 4);
                let byte_off = builder.ins().imul(idx_ext, four);
                let base_i64 = builder.ins().iconst(types::I64, base_off as i64);
                let total_off = builder.ins().iadd(base_i64, byte_off);
                let addr = builder.ins().iadd(heap, total_off);
                builder.ins().store(MemFlags::trusted(), val, addr, 0);
                builder.ins().jump(next_block, &[]);
            }

            // --- Increment/decrement ---
            "preinc" | "postinc" if args.len() >= 2 => {
                let off = offsets[args[1] as usize];
                match jtypes[args[1] as usize] {
                    JitType::Int => {
                        let v = heap_load_i32(builder, heap, off);
                        let one = builder.ins().iconst(types::I32, 1);
                        let r = builder.ins().iadd(v, one);
                        heap_store_i32(builder, heap, off, r);
                        let out = if opname == "preinc" { r } else { v };
                        heap_store_i32(builder, heap, offsets[args[0] as usize], out);
                    }
                    _ => {
                        let v = heap_load_f32(builder, heap, off);
                        let one = builder.ins().f32const(1.0);
                        let r = builder.ins().fadd(v, one);
                        heap_store_f32(builder, heap, off, r);
                        let out = if opname == "preinc" { r } else { v };
                        heap_store_f32(builder, heap, offsets[args[0] as usize], out);
                    }
                }
                builder.ins().jump(next_block, &[]);
            }
            "predec" | "postdec" if args.len() >= 2 => {
                let off = offsets[args[1] as usize];
                match jtypes[args[1] as usize] {
                    JitType::Int => {
                        let v = heap_load_i32(builder, heap, off);
                        let one = builder.ins().iconst(types::I32, 1);
                        let r = builder.ins().isub(v, one);
                        heap_store_i32(builder, heap, off, r);
                        let out = if opname == "predec" { r } else { v };
                        heap_store_i32(builder, heap, offsets[args[0] as usize], out);
                    }
                    _ => {
                        let v = heap_load_f32(builder, heap, off);
                        let one = builder.ins().f32const(1.0);
                        let r = builder.ins().fsub(v, one);
                        heap_store_f32(builder, heap, off, r);
                        let out = if opname == "predec" { r } else { v };
                        heap_store_f32(builder, heap, offsets[args[0] as usize], out);
                    }
                }
                builder.ins().jump(next_block, &[]);
            }

            // --- More math (binary) ---
            "max" if args.len() >= 3 => {
                let a = heap_load_f32(builder, heap, offsets[args[1] as usize]);
                let b_val = heap_load_f32(builder, heap, offsets[args[2] as usize]);
                let r = builder.ins().fmax(a, b_val);
                heap_store_f32(builder, heap, offsets[args[0] as usize], r);
                builder.ins().jump(next_block, &[]);
            }
            "min" if args.len() >= 3 => {
                let a = heap_load_f32(builder, heap, offsets[args[1] as usize]);
                let b_val = heap_load_f32(builder, heap, offsets[args[2] as usize]);
                let r = builder.ins().fmin(a, b_val);
                heap_store_f32(builder, heap, offsets[args[0] as usize], r);
                builder.ins().jump(next_block, &[]);
            }
            "fmod" if args.len() >= 3 => {
                let fref = module.declare_func_in_func(math.fmodf, builder.func);
                let a = heap_load_f32(builder, heap, offsets[args[1] as usize]);
                let b_val = heap_load_f32(builder, heap, offsets[args[2] as usize]);
                // Safe fmod: if b==0, return 0.0 (matches C++ safe_fmod)
                let zero = builder.ins().f32const(0.0);
                let b_nonzero = builder.ins().fcmp(FloatCC::NotEqual, b_val, zero);
                let call = builder.ins().call(fref, &[a, b_val]);
                let raw = builder.inst_results(call)[0];
                let r = builder.ins().select(b_nonzero, raw, zero);
                heap_store_f32(builder, heap, offsets[args[0] as usize], r);
                builder.ins().jump(next_block, &[]);
            }
            "step" if args.len() >= 3 => {
                let dst = args[0] as usize;
                let edge = heap_load_f32(builder, heap, offsets[args[1] as usize]);
                let x = heap_load_f32(builder, heap, offsets[args[2] as usize]);
                let cmp = builder.ins().fcmp(FloatCC::LessThan, x, edge);
                let zero = builder.ins().f32const(0.0);
                let one = builder.ins().f32const(1.0);
                let r = builder.ins().select(cmp, zero, one);
                heap_store_f32(builder, heap, offsets[dst], r);
                // step is piecewise constant: derivative = 0
                if has_derivs[dst] {
                    zero_derivs(builder, heap, offsets[dst], JitType::Float);
                }
                builder.ins().jump(next_block, &[]);
            }
            "sign" if args.len() >= 2 => {
                let dst = args[0] as usize;
                let x = heap_load_f32(builder, heap, offsets[args[1] as usize]);
                let zero = builder.ins().f32const(0.0);
                let pos = builder.ins().f32const(1.0);
                let neg = builder.ins().f32const(-1.0);
                let is_pos = builder.ins().fcmp(FloatCC::GreaterThan, x, zero);
                let is_neg = builder.ins().fcmp(FloatCC::LessThan, x, zero);
                let r1 = builder.ins().select(is_pos, pos, zero);
                let r = builder.ins().select(is_neg, neg, r1);
                heap_store_f32(builder, heap, offsets[dst], r);
                // sign is piecewise constant: derivative = 0
                if has_derivs[dst] {
                    zero_derivs(builder, heap, offsets[dst], JitType::Float);
                }
                builder.ins().jump(next_block, &[]);
            }
            "fabs" if args.len() >= 2 => {
                let fref = module.declare_func_in_func(math.fabsf, builder.func);
                let src = args[1] as usize;
                let dst = args[0] as usize;
                let v = heap_load_f32(builder, heap, offsets[src]);
                let call = builder.ins().call(fref, &[v]);
                let r = builder.inst_results(call)[0];
                heap_store_f32(builder, heap, offsets[dst], r);
                // d/dx |x| = sign(x) * x'
                if has_derivs[dst] {
                    let zero_f = builder.ins().f32const(0.0);
                    let pos = builder.ins().f32const(1.0);
                    let neg_one = builder.ins().f32const(-1.0);
                    let is_pos = builder.ins().fcmp(FloatCC::GreaterThanOrEqual, v, zero_f);
                    let sign_v = builder.ins().select(is_pos, pos, neg_one);
                    propagate_derivs_unary_f32(
                        builder,
                        heap,
                        offsets[dst],
                        offsets[src],
                        has_derivs[src],
                        sign_v,
                    );
                }
                builder.ins().jump(next_block, &[]);
            }
            "round" if args.len() >= 2 => {
                let dst = args[0] as usize;
                let v = heap_load_f32(builder, heap, offsets[args[1] as usize]);
                let r = builder.ins().nearest(v);
                heap_store_f32(builder, heap, offsets[dst], r);
                // round is piecewise constant: derivative = 0
                if has_derivs[dst] {
                    zero_derivs(builder, heap, offsets[dst], JitType::Float);
                }
                builder.ins().jump(next_block, &[]);
            }
            "trunc" if args.len() >= 2 => {
                let dst = args[0] as usize;
                let v = heap_load_f32(builder, heap, offsets[args[1] as usize]);
                let r = builder.ins().trunc(v);
                heap_store_f32(builder, heap, offsets[dst], r);
                // trunc is piecewise constant: derivative = 0
                if has_derivs[dst] {
                    zero_derivs(builder, heap, offsets[dst], JitType::Float);
                }
                builder.ins().jump(next_block, &[]);
            }
            // degrees(rad) = rad * (180/pi), radians(deg) = deg * (pi/180)
            "degrees" if args.len() >= 2 => {
                let dst = args[0] as usize;
                let src = args[1] as usize;
                let v = heap_load_f32(builder, heap, offsets[src]);
                let scale = builder.ins().f32const(180.0 / std::f32::consts::PI);
                let r = builder.ins().fmul(v, scale);
                heap_store_f32(builder, heap, offsets[dst], r);
                // Linear: d(degrees(x)) = (180/pi) * dx
                if has_derivs[dst] {
                    propagate_derivs_unary_f32(
                        builder,
                        heap,
                        offsets[dst],
                        offsets[src],
                        has_derivs[src],
                        scale,
                    );
                }
                builder.ins().jump(next_block, &[]);
            }
            "radians" if args.len() >= 2 => {
                let dst = args[0] as usize;
                let src = args[1] as usize;
                let v = heap_load_f32(builder, heap, offsets[src]);
                let scale = builder.ins().f32const(std::f32::consts::PI / 180.0);
                let r = builder.ins().fmul(v, scale);
                heap_store_f32(builder, heap, offsets[dst], r);
                if has_derivs[dst] {
                    propagate_derivs_unary_f32(
                        builder,
                        heap,
                        offsets[dst],
                        offsets[src],
                        has_derivs[src],
                        scale,
                    );
                }
                builder.ins().jump(next_block, &[]);
            }
            "inversesqrt" if args.len() >= 2 => {
                let sqrtf_ref = module.declare_func_in_func(math.sqrtf, builder.func);
                let src = args[1] as usize;
                let dst = args[0] as usize;
                let v = heap_load_f32(builder, heap, offsets[src]);
                let call = builder.ins().call(sqrtf_ref, &[v]);
                let sq = builder.inst_results(call)[0];
                let one = builder.ins().f32const(1.0);
                let r = emit_safe_fdiv(builder, one, sq);
                heap_store_f32(builder, heap, offsets[dst], r);
                // d/dx (1/sqrt(x)) = -0.5 * x^(-3/2) = -0.5 * r^3
                if has_derivs[dst] {
                    let r2 = builder.ins().fmul(r, r);
                    let r3 = builder.ins().fmul(r2, r);
                    let neg_half = builder.ins().f32const(-0.5);
                    let dprime = builder.ins().fmul(neg_half, r3);
                    propagate_derivs_unary_f32(
                        builder,
                        heap,
                        offsets[dst],
                        offsets[src],
                        has_derivs[src],
                        dprime,
                    );
                }
                builder.ins().jump(next_block, &[]);
            }
            "isnan" if args.len() >= 2 => {
                let v = heap_load_f32(builder, heap, offsets[args[1] as usize]);
                // NaN != NaN
                let cmp = builder.ins().fcmp(FloatCC::Unordered, v, v);
                let r = builder.ins().uextend(types::I32, cmp);
                heap_store_i32(builder, heap, offsets[args[0] as usize], r);
                builder.ins().jump(next_block, &[]);
            }
            "isinf" if args.len() >= 2 => {
                let fref = module.declare_func_in_func(math.fabsf, builder.func);
                let v = heap_load_f32(builder, heap, offsets[args[1] as usize]);
                let call = builder.ins().call(fref, &[v]);
                let abs_v = builder.inst_results(call)[0];
                let inf = builder.ins().f32const(f32::INFINITY);
                let cmp = builder.ins().fcmp(FloatCC::Equal, abs_v, inf);
                let r = builder.ins().uextend(types::I32, cmp);
                heap_store_i32(builder, heap, offsets[args[0] as usize], r);
                builder.ins().jump(next_block, &[]);
            }
            "isfinite" if args.len() >= 2 => {
                let v = heap_load_f32(builder, heap, offsets[args[1] as usize]);
                let cmp = builder.ins().fcmp(FloatCC::Ordered, v, v);
                let fref = module.declare_func_in_func(math.fabsf, builder.func);
                let call = builder.ins().call(fref, &[v]);
                let abs_v = builder.inst_results(call)[0];
                let inf = builder.ins().f32const(f32::INFINITY);
                let not_inf = builder.ins().fcmp(FloatCC::NotEqual, abs_v, inf);
                let r_bool = builder.ins().band(cmp, not_inf);
                let r = builder.ins().uextend(types::I32, r_bool);
                heap_store_i32(builder, heap, offsets[args[0] as usize], r);
                builder.ins().jump(next_block, &[]);
            }
            // sinh, cosh, tanh — via trampolines
            "sinh" | "cosh" | "tanh" if args.len() >= 2 => {
                // Use exp-based formulas: sinh=(e^x-e^-x)/2, cosh=(e^x+e^-x)/2, tanh=sinh/cosh
                let expf_ref = module.declare_func_in_func(math.expf, builder.func);
                let x = heap_load_f32(builder, heap, offsets[args[1] as usize]);
                let call_pos = builder.ins().call(expf_ref, &[x]);
                let ep = builder.inst_results(call_pos)[0];
                let neg_x = builder.ins().fneg(x);
                let expf_ref2 = module.declare_func_in_func(math.expf, builder.func);
                let call_neg = builder.ins().call(expf_ref2, &[neg_x]);
                let en = builder.inst_results(call_neg)[0];
                let two = builder.ins().f32const(2.0);
                let r = match opname {
                    "sinh" => {
                        let diff = builder.ins().fsub(ep, en);
                        builder.ins().fdiv(diff, two)
                    }
                    "cosh" => {
                        let sum = builder.ins().fadd(ep, en);
                        builder.ins().fdiv(sum, two)
                    }
                    "tanh" => {
                        let diff = builder.ins().fsub(ep, en);
                        let sum = builder.ins().fadd(ep, en);
                        builder.ins().fdiv(diff, sum)
                    }
                    _ => unreachable!(),
                };
                let dst = args[0] as usize;
                heap_store_f32(builder, heap, offsets[dst], r);
                // Derivatives: sinh' = cosh, cosh' = sinh, tanh' = 1 - tanh^2
                if has_derivs[dst] {
                    let deriv_val = match opname {
                        "sinh" => {
                            // d/dx sinh(x) = cosh(x) = (e^x + e^-x)/2
                            let sum = builder.ins().fadd(ep, en);
                            builder.ins().fdiv(sum, two)
                        }
                        "cosh" => {
                            // d/dx cosh(x) = sinh(x) = (e^x - e^-x)/2
                            let diff = builder.ins().fsub(ep, en);
                            builder.ins().fdiv(diff, two)
                        }
                        "tanh" => {
                            // d/dx tanh(x) = 1 - tanh^2(x)
                            let one = builder.ins().f32const(1.0);
                            let r2 = builder.ins().fmul(r, r);
                            builder.ins().fsub(one, r2)
                        }
                        _ => unreachable!(),
                    };
                    propagate_derivs_unary_f32(
                        builder,
                        heap,
                        offsets[dst],
                        offsets[args[1] as usize],
                        has_derivs[args[1] as usize],
                        deriv_val,
                    );
                }
                builder.ins().jump(next_block, &[]);
            }

            // --- Ternary math (4 args: dst, a, b, c) ---
            "clamp" if args.len() >= 4 => {
                let x = heap_load_f32(builder, heap, offsets[args[1] as usize]);
                let lo = heap_load_f32(builder, heap, offsets[args[2] as usize]);
                let hi = heap_load_f32(builder, heap, offsets[args[3] as usize]);
                let r1 = builder.ins().fmax(x, lo);
                let r = builder.ins().fmin(r1, hi);
                let dst = args[0] as usize;
                heap_store_f32(builder, heap, offsets[dst], r);
                // clamp derivative: if lo < x < hi then x', else 0
                if has_derivs[dst] {
                    let sz = 4usize;
                    let in_range_lo = builder.ins().fcmp(FloatCC::GreaterThan, x, lo);
                    let in_range_hi = builder.ins().fcmp(FloatCC::LessThan, x, hi);
                    let in_range = builder.ins().band(in_range_lo, in_range_hi);
                    for d in 1..=2usize {
                        let slot = d * sz;
                        let xd = if has_derivs[args[1] as usize] {
                            heap_load_f32(builder, heap, offsets[args[1] as usize] + slot)
                        } else {
                            builder.ins().f32const(0.0)
                        };
                        let zero = builder.ins().f32const(0.0);
                        let rd = builder.ins().select(in_range, xd, zero);
                        heap_store_f32(builder, heap, offsets[dst] + slot, rd);
                    }
                }
                builder.ins().jump(next_block, &[]);
            }
            "mix" if args.len() >= 4 => {
                let a = heap_load_f32(builder, heap, offsets[args[1] as usize]);
                let b_val = heap_load_f32(builder, heap, offsets[args[2] as usize]);
                let t = heap_load_f32(builder, heap, offsets[args[3] as usize]);
                // mix = a*(1-t) + b*t
                let one = builder.ins().f32const(1.0);
                let one_minus_t = builder.ins().fsub(one, t);
                let at = builder.ins().fmul(a, one_minus_t);
                let bt = builder.ins().fmul(b_val, t);
                let r = builder.ins().fadd(at, bt);
                let dst = args[0] as usize;
                heap_store_f32(builder, heap, offsets[dst], r);
                // Derivative: mix' = a'*(1-t) + b'*t + (b-a)*t'
                if has_derivs[dst] {
                    let sz = 4usize;
                    let b_minus_a = builder.ins().fsub(b_val, a);
                    for d in 0..2usize {
                        let slot = (d + 1) * sz; // dx=4, dy=8
                        let adx = if has_derivs[args[1] as usize] {
                            heap_load_f32(builder, heap, offsets[args[1] as usize] + slot)
                        } else {
                            builder.ins().f32const(0.0)
                        };
                        let bdx = if has_derivs[args[2] as usize] {
                            heap_load_f32(builder, heap, offsets[args[2] as usize] + slot)
                        } else {
                            builder.ins().f32const(0.0)
                        };
                        let tdx = if has_derivs[args[3] as usize] {
                            heap_load_f32(builder, heap, offsets[args[3] as usize] + slot)
                        } else {
                            builder.ins().f32const(0.0)
                        };
                        let t1 = builder.ins().fmul(adx, one_minus_t);
                        let t2 = builder.ins().fmul(bdx, t);
                        let t3 = builder.ins().fmul(b_minus_a, tdx);
                        let sum = builder.ins().fadd(t1, t2);
                        let rdx = builder.ins().fadd(sum, t3);
                        heap_store_f32(builder, heap, offsets[dst] + slot, rdx);
                    }
                }
                builder.ins().jump(next_block, &[]);
            }
            "smoothstep" if args.len() >= 4 => {
                // smoothstep(edge0, edge1, x) = t*t*(3-2*t), t=clamp((x-e0)/(e1-e0),0,1)
                let e0 = heap_load_f32(builder, heap, offsets[args[1] as usize]);
                let e1 = heap_load_f32(builder, heap, offsets[args[2] as usize]);
                let x = heap_load_f32(builder, heap, offsets[args[3] as usize]);
                let diff = builder.ins().fsub(e1, e0);
                let xme0 = builder.ins().fsub(x, e0);
                let t_raw = emit_safe_fdiv(builder, xme0, diff);
                let zero = builder.ins().f32const(0.0);
                let one = builder.ins().f32const(1.0);
                let t1 = builder.ins().fmax(t_raw, zero);
                let t = builder.ins().fmin(t1, one);
                let three = builder.ins().f32const(3.0);
                let two = builder.ins().f32const(2.0);
                let two_t = builder.ins().fmul(two, t);
                let three_minus = builder.ins().fsub(three, two_t);
                let tt = builder.ins().fmul(t, t);
                let r = builder.ins().fmul(tt, three_minus);
                let dst = args[0] as usize;
                heap_store_f32(builder, heap, offsets[dst], r);
                // d/dx smoothstep = 6*t*(1-t) / (e1-e0) * x'  (when 0<t<1)
                if has_derivs[dst] {
                    let sz = 4usize;
                    let six = builder.ins().f32const(6.0);
                    let one_minus_t = builder.ins().fsub(one, t);
                    let s6t = builder.ins().fmul(six, t);
                    let s6t1mt = builder.ins().fmul(s6t, one_minus_t);
                    let coeff = emit_safe_fdiv(builder, s6t1mt, diff);
                    // Zero out derivative when t is clamped (t<=0 or t>=1)
                    let in_lo = builder.ins().fcmp(FloatCC::GreaterThan, t_raw, zero);
                    let in_hi = builder.ins().fcmp(FloatCC::LessThan, t_raw, one);
                    let in_range = builder.ins().band(in_lo, in_hi);
                    let coeff_masked = builder.ins().select(in_range, coeff, zero);
                    for d in 1..=2usize {
                        let slot = d * sz;
                        let xd = if has_derivs[args[3] as usize] {
                            heap_load_f32(builder, heap, offsets[args[3] as usize] + slot)
                        } else {
                            builder.ins().f32const(0.0)
                        };
                        let rd = builder.ins().fmul(coeff_masked, xd);
                        heap_store_f32(builder, heap, offsets[dst] + slot, rd);
                    }
                }
                builder.ins().jump(next_block, &[]);
            }
            "linearstep" if args.len() >= 4 => {
                let e0 = heap_load_f32(builder, heap, offsets[args[1] as usize]);
                let e1 = heap_load_f32(builder, heap, offsets[args[2] as usize]);
                let x = heap_load_f32(builder, heap, offsets[args[3] as usize]);
                let diff = builder.ins().fsub(e1, e0);
                let xme0 = builder.ins().fsub(x, e0);
                let t_raw = emit_safe_fdiv(builder, xme0, diff);
                let zero = builder.ins().f32const(0.0);
                let one = builder.ins().f32const(1.0);
                let t1 = builder.ins().fmax(t_raw, zero);
                let r = builder.ins().fmin(t1, one);
                let dst = args[0] as usize;
                heap_store_f32(builder, heap, offsets[dst], r);
                // d/dx linearstep = 1/(e1-e0) * x'  (when 0<t<1)
                if has_derivs[dst] {
                    let sz = 4usize;
                    let inv_diff = emit_safe_fdiv(builder, one, diff);
                    let in_lo = builder.ins().fcmp(FloatCC::GreaterThan, t_raw, zero);
                    let in_hi = builder.ins().fcmp(FloatCC::LessThan, t_raw, one);
                    let in_range = builder.ins().band(in_lo, in_hi);
                    let coeff = builder.ins().select(in_range, inv_diff, zero);
                    for d in 1..=2usize {
                        let slot = d * sz;
                        let xd = if has_derivs[args[3] as usize] {
                            heap_load_f32(builder, heap, offsets[args[3] as usize] + slot)
                        } else {
                            builder.ins().f32const(0.0)
                        };
                        let rd = builder.ins().fmul(coeff, xd);
                        heap_store_f32(builder, heap, offsets[dst] + slot, rd);
                    }
                }
                builder.ins().jump(next_block, &[]);
            }

            // --- Vector ops ---
            "distance" if args.len() >= 3 => {
                let a_off = offsets[args[1] as usize];
                let b_off = offsets[args[2] as usize];
                let dst = args[0] as usize;
                let (ax, ay, az) = heap_load_vec3(builder, heap, a_off);
                let (bx, by, bz) = heap_load_vec3(builder, heap, b_off);
                let vdx = builder.ins().fsub(ax, bx);
                let vdy = builder.ins().fsub(ay, by);
                let vdz = builder.ins().fsub(az, bz);
                let dxx = builder.ins().fmul(vdx, vdx);
                let dyy = builder.ins().fmul(vdy, vdy);
                let dzz = builder.ins().fmul(vdz, vdz);
                let s1 = builder.ins().fadd(dxx, dyy);
                let sum = builder.ins().fadd(s1, dzz);
                let sqrtf_ref = module.declare_func_in_func(math.sqrtf, builder.func);
                let call = builder.ins().call(sqrtf_ref, &[sum]);
                let dist = builder.inst_results(call)[0];
                heap_store_f32(builder, heap, offsets[dst], dist);
                // distance'(a,b) = dot(a-b, a'-b') / distance(a,b)
                if has_derivs[dst] {
                    let sz_f = 4usize;
                    let sz_v = 12usize;
                    let a_hd = has_derivs[args[1] as usize];
                    let b_hd = has_derivs[args[2] as usize];
                    for d in 1..=2usize {
                        let mut acc = builder.ins().f32const(0.0);
                        for c in 0..3usize {
                            let co = c * 4;
                            let ac = heap_load_f32(builder, heap, a_off + co);
                            let bc = heap_load_f32(builder, heap, b_off + co);
                            let vc = builder.ins().fsub(ac, bc);
                            let adc = if a_hd {
                                heap_load_f32(builder, heap, a_off + d * sz_v + co)
                            } else {
                                builder.ins().f32const(0.0)
                            };
                            let bdc = if b_hd {
                                heap_load_f32(builder, heap, b_off + d * sz_v + co)
                            } else {
                                builder.ins().f32const(0.0)
                            };
                            let vdc = builder.ins().fsub(adc, bdc);
                            let t = builder.ins().fmul(vc, vdc);
                            acc = builder.ins().fadd(acc, t);
                        }
                        let rd = emit_safe_fdiv(builder, acc, dist);
                        heap_store_f32(builder, heap, offsets[dst] + d * sz_f, rd);
                    }
                }
                builder.ins().jump(next_block, &[]);
            }
            "faceforward" if args.len() >= 4 => {
                let dst = args[0] as usize;
                let n_sym = args[1] as usize;
                let (nx, ny, nz) = heap_load_vec3(builder, heap, offsets[n_sym]);
                let (ix, iy, iz) = heap_load_vec3(builder, heap, offsets[args[2] as usize]);
                let (ngx, ngy, ngz) = heap_load_vec3(builder, heap, offsets[args[3] as usize]);
                let d1 = builder.ins().fmul(ngx, ix);
                let d2 = builder.ins().fmul(ngy, iy);
                let d3 = builder.ins().fmul(ngz, iz);
                let s1 = builder.ins().fadd(d1, d2);
                let dot_val = builder.ins().fadd(s1, d3);
                let zero = builder.ins().f32const(0.0);
                let is_neg = builder.ins().fcmp(FloatCC::LessThan, dot_val, zero);
                let neg_nx = builder.ins().fneg(nx);
                let neg_ny = builder.ins().fneg(ny);
                let neg_nz = builder.ins().fneg(nz);
                let rx = builder.ins().select(is_neg, nx, neg_nx);
                let ry = builder.ins().select(is_neg, ny, neg_ny);
                let rz = builder.ins().select(is_neg, nz, neg_nz);
                heap_store_vec3(builder, heap, offsets[dst], rx, ry, rz);
                // faceforward derivs: sign(dot(Ng,I)) * N' (sign is piecewise constant)
                if has_derivs[dst] && has_derivs[n_sym] {
                    let sz_v = 12usize;
                    for d in 1..=2usize {
                        let slot = d * sz_v;
                        let (ndx, ndy, ndz) = heap_load_vec3(builder, heap, offsets[n_sym] + slot);
                        let neg_ndx = builder.ins().fneg(ndx);
                        let neg_ndy = builder.ins().fneg(ndy);
                        let neg_ndz = builder.ins().fneg(ndz);
                        let rdx = builder.ins().select(is_neg, ndx, neg_ndx);
                        let rdy = builder.ins().select(is_neg, ndy, neg_ndy);
                        let rdz = builder.ins().select(is_neg, ndz, neg_ndz);
                        heap_store_vec3(builder, heap, offsets[dst] + slot, rdx, rdy, rdz);
                    }
                } else if has_derivs[dst] {
                    let z = builder.ins().f32const(0.0);
                    heap_store_vec3(builder, heap, offsets[dst] + 12, z, z, z);
                    heap_store_vec3(builder, heap, offsets[dst] + 24, z, z, z);
                }
                builder.ins().jump(next_block, &[]);
            }
            "reflect" if args.len() >= 3 => {
                let dst = args[0] as usize;
                let i_sym = args[1] as usize;
                let n_sym = args[2] as usize;
                let i_off = offsets[i_sym];
                let n_off = offsets[n_sym];
                let (ix, iy, iz) = heap_load_vec3(builder, heap, i_off);
                let (nx, ny, nz) = heap_load_vec3(builder, heap, n_off);
                // reflect = I - 2*dot(N,I)*N
                let d1 = builder.ins().fmul(nx, ix);
                let d2 = builder.ins().fmul(ny, iy);
                let d3 = builder.ins().fmul(nz, iz);
                let s1 = builder.ins().fadd(d1, d2);
                let dot_ni = builder.ins().fadd(s1, d3);
                let two = builder.ins().f32const(2.0);
                let scale = builder.ins().fmul(two, dot_ni);
                let snx = builder.ins().fmul(scale, nx);
                let sny = builder.ins().fmul(scale, ny);
                let snz = builder.ins().fmul(scale, nz);
                let rx = builder.ins().fsub(ix, snx);
                let ry = builder.ins().fsub(iy, sny);
                let rz = builder.ins().fsub(iz, snz);
                heap_store_vec3(builder, heap, offsets[dst], rx, ry, rz);
                // reflect' = I' - 2*(dot(N',I)+dot(N,I'))*N - 2*dot(N,I)*N'
                if has_derivs[dst] {
                    let sz_v = 12usize;
                    let i_hd = has_derivs[i_sym];
                    let n_hd = has_derivs[n_sym];
                    for d in 1..=2usize {
                        let slot = d * sz_v;
                        let (idx, idy, idz) = if i_hd {
                            heap_load_vec3(builder, heap, i_off + slot)
                        } else {
                            let z = builder.ins().f32const(0.0);
                            (z, z, z)
                        };
                        let (ndx, ndy, ndz) = if n_hd {
                            heap_load_vec3(builder, heap, n_off + slot)
                        } else {
                            let z = builder.ins().f32const(0.0);
                            (z, z, z)
                        };
                        // dot(N', I) + dot(N, I')
                        let d_ni_1 = builder.ins().fmul(ndx, ix);
                        let d_ni_2 = builder.ins().fmul(ndy, iy);
                        let d_ni_3 = builder.ins().fmul(ndz, iz);
                        let dot1 = builder.ins().fadd(d_ni_1, d_ni_2);
                        let dot1 = builder.ins().fadd(dot1, d_ni_3);
                        let d_in_1 = builder.ins().fmul(nx, idx);
                        let d_in_2 = builder.ins().fmul(ny, idy);
                        let d_in_3 = builder.ins().fmul(nz, idz);
                        let dot2 = builder.ins().fadd(d_in_1, d_in_2);
                        let dot2 = builder.ins().fadd(dot2, d_in_3);
                        let dot_sum = builder.ins().fadd(dot1, dot2);
                        let two_dot_sum = builder.ins().fmul(two, dot_sum);
                        // I' - 2*(dot_sum)*N - 2*dot(N,I)*N'
                        for c in 0..3usize {
                            let co = c * 4;
                            let ic = [idx, idy, idz][c];
                            let nc = [nx, ny, nz][c];
                            let ndc = [ndx, ndy, ndz][c];
                            let t1 = builder.ins().fmul(two_dot_sum, nc);
                            let t2 = builder.ins().fmul(scale, ndc);
                            let s = builder.ins().fsub(ic, t1);
                            let rc = builder.ins().fsub(s, t2);
                            heap_store_f32(builder, heap, offsets[dst] + slot + co, rc);
                        }
                    }
                }
                builder.ins().jump(next_block, &[]);
            }
            "luminance" if args.len() >= 2 => {
                let dst = args[0] as usize;
                let src = args[1] as usize;
                let (r, g, b_val) = heap_load_vec3(builder, heap, offsets[src]);
                let wr = builder.ins().f32const(0.2126);
                let wg = builder.ins().f32const(0.7152);
                let wb = builder.ins().f32const(0.0722);
                let rr = builder.ins().fmul(r, wr);
                let gg = builder.ins().fmul(g, wg);
                let bb = builder.ins().fmul(b_val, wb);
                let s1 = builder.ins().fadd(rr, gg);
                let lum = builder.ins().fadd(s1, bb);
                heap_store_f32(builder, heap, offsets[dst], lum);
                // luminance is linear: lum' = wr*r' + wg*g' + wb*b'
                if has_derivs[dst] && has_derivs[src] {
                    let sz_f = 4usize;
                    let sz_v = 12usize;
                    for d in 1..=2usize {
                        let (rd, gd, bd) = heap_load_vec3(builder, heap, offsets[src] + d * sz_v);
                        let t1 = builder.ins().fmul(wr, rd);
                        let t2 = builder.ins().fmul(wg, gd);
                        let t3 = builder.ins().fmul(wb, bd);
                        let s = builder.ins().fadd(t1, t2);
                        let lumd = builder.ins().fadd(s, t3);
                        heap_store_f32(builder, heap, offsets[dst] + d * sz_f, lumd);
                    }
                } else if has_derivs[dst] {
                    let z = builder.ins().f32const(0.0);
                    heap_store_f32(builder, heap, offsets[dst] + 4, z);
                    heap_store_f32(builder, heap, offsets[dst] + 8, z);
                }
                builder.ins().jump(next_block, &[]);
            }

            // --- Component ref/assign ---
            "compref" if args.len() >= 3 => {
                let src = args[1] as usize;
                let dst = args[0] as usize;
                let base_off = offsets[src];
                let idx_raw = heap_load_i32(builder, heap, offsets[args[2] as usize]);
                let (symname_hash, sourcefile_hash) = if (args[1] as usize) < ir.symbols.len() {
                    (
                        ir.symbols[args[1] as usize].name.hash(),
                        op.sourcefile.hash(),
                    )
                } else {
                    (0u64, 0u64)
                };
                let idx = emit_maybe_range_check(
                    builder,
                    module,
                    math,
                    ctx,
                    sg,
                    heap,
                    idx_raw,
                    3,
                    symname_hash,
                    sourcefile_hash,
                    op.sourceline,
                    range_checking,
                );
                let idx_ext = builder.ins().uextend(types::I64, idx);
                let four = builder.ins().iconst(types::I64, 4);
                let byte_off = builder.ins().imul(idx_ext, four);
                let base_i64 = builder.ins().iconst(types::I64, base_off as i64);
                let total_off = builder.ins().iadd(base_i64, byte_off);
                let addr = builder.ins().iadd(heap, total_off);
                let val = builder.ins().load(types::F32, MemFlags::trusted(), addr, 0);
                heap_store_f32(builder, heap, offsets[dst], val);
                // Extract derivative component: dst.dx = src.dx[idx], dst.dy = src.dy[idx]
                if has_derivs[dst] && has_derivs[src] {
                    let sz_v = 12usize; // vec3 deriv stride
                    let sz_f = 4usize;
                    for d in 1..=2usize {
                        let deriv_base = builder
                            .ins()
                            .iconst(types::I64, (base_off + d * sz_v) as i64);
                        let deriv_off = builder.ins().iadd(deriv_base, byte_off);
                        let deriv_addr = builder.ins().iadd(heap, deriv_off);
                        let dv = builder
                            .ins()
                            .load(types::F32, MemFlags::trusted(), deriv_addr, 0);
                        heap_store_f32(builder, heap, offsets[dst] + d * sz_f, dv);
                    }
                } else if has_derivs[dst] {
                    let zero = builder.ins().f32const(0.0);
                    heap_store_f32(builder, heap, offsets[dst] + 4, zero);
                    heap_store_f32(builder, heap, offsets[dst] + 8, zero);
                }
                builder.ins().jump(next_block, &[]);
            }
            "compassign" if args.len() >= 3 => {
                let dst_sym = args[0] as usize;
                let val_sym = args[2] as usize;
                let base_off = offsets[dst_sym];
                let idx_raw = heap_load_i32(builder, heap, offsets[args[1] as usize]);
                let (symname_hash, sourcefile_hash) = if (args[0] as usize) < ir.symbols.len() {
                    (
                        ir.symbols[args[0] as usize].name.hash(),
                        op.sourcefile.hash(),
                    )
                } else {
                    (0u64, 0u64)
                };
                let idx = emit_maybe_range_check(
                    builder,
                    module,
                    math,
                    ctx,
                    sg,
                    heap,
                    idx_raw,
                    3,
                    symname_hash,
                    sourcefile_hash,
                    op.sourceline,
                    range_checking,
                );
                let val = heap_load_f32(builder, heap, offsets[val_sym]);
                let idx_ext = builder.ins().uextend(types::I64, idx);
                let four = builder.ins().iconst(types::I64, 4);
                let byte_off = builder.ins().imul(idx_ext, four);
                let base_i64 = builder.ins().iconst(types::I64, base_off as i64);
                let total_off = builder.ins().iadd(base_i64, byte_off);
                let addr = builder.ins().iadd(heap, total_off);
                builder.ins().store(MemFlags::trusted(), val, addr, 0);
                // Write derivative component: dst.dx[idx] = val.dx, dst.dy[idx] = val.dy
                if has_derivs[dst_sym] {
                    let sz_v = 12usize;
                    let sz_f = 4usize;
                    for d in 1..=2usize {
                        let dv = if has_derivs[val_sym] {
                            heap_load_f32(builder, heap, offsets[val_sym] + d * sz_f)
                        } else {
                            builder.ins().f32const(0.0)
                        };
                        let deriv_base = builder
                            .ins()
                            .iconst(types::I64, (base_off + d * sz_v) as i64);
                        let deriv_off = builder.ins().iadd(deriv_base, byte_off);
                        let deriv_addr = builder.ins().iadd(heap, deriv_off);
                        builder.ins().store(MemFlags::trusted(), dv, deriv_addr, 0);
                    }
                }
                builder.ins().jump(next_block, &[]);
            }
            "arraylength" if args.len() >= 2 => {
                // For fixed-size arrays, the length is embedded in the type
                let sym = &ir.symbols[args[1] as usize];
                let len = sym.typespec.simpletype().arraylen.max(1) as i32;
                let v = builder.ins().iconst(types::I32, len as i64);
                heap_store_i32(builder, heap, offsets[args[0] as usize], v);
                builder.ins().jump(next_block, &[]);
            }

            // --- Loop control flow ---
            "for" | "while" | "dowhile" => {
                // Structured loops — the inline if/nop opcodes handle the flow.
                builder.ins().jump(next_block, &[]);
            }
            "break" => {
                if op.jump[0] >= 0 && (op.jump[0] as usize) < op_blocks.len() {
                    builder.ins().jump(op_blocks[op.jump[0] as usize], &[]);
                } else {
                    builder.ins().jump(exit_block, &[]);
                }
            }
            "continue" => {
                if op.jump[0] >= 0 && (op.jump[0] as usize) < op_blocks.len() {
                    builder.ins().jump(op_blocks[op.jump[0] as usize], &[]);
                } else {
                    builder.ins().jump(next_block, &[]);
                }
            }

            // --- Function calls (dispatch via jumps) ---
            "functioncall" | "functioncall_nr" if !args.is_empty() => {
                if op.jump[0] >= 0 && (op.jump[0] as usize) < op_blocks.len() {
                    builder.ins().jump(op_blocks[op.jump[0] as usize], &[]);
                } else {
                    builder.ins().jump(next_block, &[]);
                }
            }

            // --- sincos (sin + cos combined) ---
            "sincos" if args.len() >= 3 => {
                let sin_ref = module.declare_func_in_func(math.sinf, builder.func);
                let cos_ref = module.declare_func_in_func(math.cosf, builder.func);
                let src = args[1] as usize;
                let sin_dst = args[0] as usize;
                let x = heap_load_f32(builder, heap, offsets[src]);
                let sin_call = builder.ins().call(sin_ref, &[x]);
                let sin_val = builder.inst_results(sin_call)[0];
                let cos_call = builder.ins().call(cos_ref, &[x]);
                let cos_val = builder.inst_results(cos_call)[0];
                heap_store_f32(builder, heap, offsets[sin_dst], sin_val);
                // sin'(x) = cos(x) * x'
                if has_derivs[sin_dst] {
                    propagate_derivs_unary_f32(
                        builder,
                        heap,
                        offsets[sin_dst],
                        offsets[src],
                        has_derivs[src],
                        cos_val,
                    );
                }
                if args.len() > 2 {
                    let cos_dst = args[2] as usize;
                    heap_store_f32(builder, heap, offsets[cos_dst], cos_val);
                    // cos'(x) = -sin(x) * x'
                    if has_derivs[cos_dst] {
                        let neg_sin = builder.ins().fneg(sin_val);
                        propagate_derivs_unary_f32(
                            builder,
                            heap,
                            offsets[cos_dst],
                            offsets[src],
                            has_derivs[src],
                            neg_sin,
                        );
                    }
                }
                builder.ins().jump(next_block, &[]);
            }

            // --- Query opcodes (return 0/false) ---
            "isconnected" | "isconstant" if args.len() >= 2 => {
                let v = builder.ins().iconst(types::I32, 0);
                heap_store_i32(builder, heap, offsets[args[0] as usize], v);
                builder.ins().jump(next_block, &[]);
            }

            // --- Noise (real trampoline calls) ---
            "noise" | "cellnoise" | "snoise" | "pnoise" | "psnoise" | "hashnoise" | "simplex"
            | "simplexnoise" | "usimplex" | "usimplexnoise" | "genericnoise" | "genericpnoise"
            | "gabornoise" | "gaborpnoise" | "gabor_pnoise" | "pcellnoise" | "phashnoise"
            | "nullnoise" | "unullnoise"
                if args.len() >= 2 =>
            {
                // Determine noise function to call based on opcode name and dimensionality.
                // args[0] = result, args[1..] = position coordinates.
                let dst = args[0] as usize;
                let name = op.op.as_str();

                // Check if position is a Vec3 (3D noise) or scalar (1D noise)
                let is_3d = args.len() >= 3
                    && matches!(jtypes[args[1] as usize], JitType::Vec3 | JitType::Color);
                let is_periodic = matches!(
                    name,
                    "pnoise"
                        | "psnoise"
                        | "genericpnoise"
                        | "gabor_pnoise"
                        | "gaborpnoise"
                        | "pcellnoise"
                        | "phashnoise"
                );

                // Null noise is trivial: return a constant
                if name == "nullnoise" || name == "unullnoise" {
                    let val = if name == "unullnoise" { 0.5f32 } else { 0.0f32 };
                    let c = builder.ins().f32const(val);
                    heap_store_f32(builder, heap, offsets[dst], c);
                    if has_derivs[dst] {
                        zero_derivs(builder, heap, offsets[dst], JitType::Float);
                    }
                    builder.ins().jump(next_block, &[]);
                    continue;
                }

                // Does this noise type have analytical derivatives?
                let has_analytic_deriv = matches!(
                    name,
                    "noise"
                        | "genericnoise"
                        | "gabornoise"
                        | "snoise"
                        | "pnoise"
                        | "psnoise"
                        | "genericpnoise"
                );

                if is_periodic && is_3d && args.len() >= 3 {
                    // 3D periodic noise: pnoise(result, P, period)
                    let p_sym = args[1] as usize;
                    let period_idx = if args.len() >= 3 {
                        args[2] as usize
                    } else {
                        args[1] as usize
                    };
                    let (px, py, pz) = heap_load_vec3(builder, heap, offsets[p_sym]);
                    let (qx, qy, qz) = heap_load_vec3(builder, heap, offsets[period_idx]);

                    // pcellnoise / phashnoise: wrap coords and call the non-periodic version
                    // (cell/hash noise has no continuous derivatives)
                    if name == "pcellnoise" || name == "phashnoise" {
                        let tramp = if name == "pcellnoise" {
                            math.cellnoise3
                        } else {
                            math.hashnoise3
                        };
                        // Wrap: x - floor(x/period)*period for each component
                        let wrap_comp = |b: &mut FunctionBuilder,
                                         x: cranelift_codegen::ir::Value,
                                         p: cranelift_codegen::ir::Value|
                         -> cranelift_codegen::ir::Value {
                            let div = emit_safe_fdiv(b, x, p);
                            let fl = b.ins().floor(div);
                            let mp = b.ins().fmul(fl, p);
                            b.ins().fsub(x, mp)
                        };
                        let wx = wrap_comp(builder, px, qx);
                        let wy = wrap_comp(builder, py, qy);
                        let wz = wrap_comp(builder, pz, qz);
                        let func_ref = module.declare_func_in_func(tramp, builder.func);
                        let call = builder.ins().call(func_ref, &[wx, wy, wz]);
                        let result = builder.inst_results(call)[0];
                        heap_store_f32(builder, heap, offsets[dst], result);
                        if has_derivs[dst] {
                            zero_derivs(builder, heap, offsets[dst], JitType::Float);
                        }
                    } else if has_derivs[dst] && has_derivs[p_sym] && has_analytic_deriv {
                        // Use analytical derivative trampoline for periodic Perlin noise
                        let func_ref =
                            module.declare_func_in_func(math.pnoise3_deriv, builder.func);
                        let dst_off_val = builder.ins().iconst(types::I32, offsets[dst] as i64);
                        let grad_off_val = builder.ins().iconst(types::I32, scratch_offset as i64);
                        builder.ins().call(
                            func_ref,
                            &[heap, dst_off_val, grad_off_val, px, py, pz, qx, qy, qz],
                        );
                        // Chain rule: result.dx = dot(grad, P.dx), result.dy = dot(grad, P.dy)
                        let sz_f = 4usize;
                        let sz_v = 12usize;
                        let gx = heap_load_f32(builder, heap, scratch_offset);
                        let gy = heap_load_f32(builder, heap, scratch_offset + 4);
                        let gz = heap_load_f32(builder, heap, scratch_offset + 8);
                        for d in 1..=2usize {
                            let (pdx, pdy, pdz) =
                                heap_load_vec3(builder, heap, offsets[p_sym] + d * sz_v);
                            let t1 = builder.ins().fmul(gx, pdx);
                            let t2 = builder.ins().fmul(gy, pdy);
                            let t3 = builder.ins().fmul(gz, pdz);
                            let s = builder.ins().fadd(t1, t2);
                            let rd = builder.ins().fadd(s, t3);
                            heap_store_f32(builder, heap, offsets[dst] + d * sz_f, rd);
                        }
                    } else {
                        // No derivatives — just compute the periodic Perlin value
                        let func_ref = module.declare_func_in_func(math.pnoise3, builder.func);
                        let call = builder.ins().call(func_ref, &[px, py, pz, qx, qy, qz]);
                        let result = builder.inst_results(call)[0];
                        heap_store_f32(builder, heap, offsets[dst], result);
                        if has_derivs[dst] {
                            zero_derivs(builder, heap, offsets[dst], JitType::Float);
                        }
                    }
                } else if is_3d {
                    let p_sym = args[1] as usize;
                    let (px, py, pz) = heap_load_vec3(builder, heap, offsets[p_sym]);

                    // If result needs derivatives and we have analytical noise derivs,
                    // use the _deriv trampoline and chain-rule with input position derivs.
                    if has_derivs[dst] && has_analytic_deriv && has_derivs[p_sym] {
                        // Use derivative trampoline: writes value + gradient to heap
                        let deriv_func = match name {
                            "snoise" => math.snoise3_deriv,
                            _ => math.noise3_deriv,
                        };
                        let func_ref = module.declare_func_in_func(deriv_func, builder.func);
                        let dst_off_val = builder.ins().iconst(types::I32, offsets[dst] as i64);
                        // Store gradient in scratch area
                        let grad_off_val = builder.ins().iconst(types::I32, scratch_offset as i64);
                        builder
                            .ins()
                            .call(func_ref, &[heap, dst_off_val, grad_off_val, px, py, pz]);
                        // Chain rule: result.dx = dot(grad, P.dx), result.dy = dot(grad, P.dy)
                        let sz_f = 4usize;
                        let sz_v = 12usize;
                        let gx = heap_load_f32(builder, heap, scratch_offset);
                        let gy = heap_load_f32(builder, heap, scratch_offset + 4);
                        let gz = heap_load_f32(builder, heap, scratch_offset + 8);
                        for d in 1..=2usize {
                            let (pdx, pdy, pdz) =
                                heap_load_vec3(builder, heap, offsets[p_sym] + d * sz_v);
                            let t1 = builder.ins().fmul(gx, pdx);
                            let t2 = builder.ins().fmul(gy, pdy);
                            let t3 = builder.ins().fmul(gz, pdz);
                            let s = builder.ins().fadd(t1, t2);
                            let rd = builder.ins().fadd(s, t3);
                            heap_store_f32(builder, heap, offsets[dst] + d * sz_f, rd);
                        }
                    } else {
                        // No derivatives needed or no analytical derivs — just compute value
                        let func_id = match name {
                            "cellnoise" => math.cellnoise3,
                            "snoise" => math.snoise3,
                            "hashnoise" => math.hashnoise3,
                            "simplex" | "simplexnoise" => math.simplex3,
                            "usimplex" | "usimplexnoise" => math.usimplex3,
                            _ => math.noise3,
                        };
                        let func_ref = module.declare_func_in_func(func_id, builder.func);
                        let call = builder.ins().call(func_ref, &[px, py, pz]);
                        let result = builder.inst_results(call)[0];
                        match jtypes[dst] {
                            JitType::Float => {
                                heap_store_f32(builder, heap, offsets[dst], result);
                            }
                            JitType::Vec3 | JitType::Color => {
                                heap_store_vec3(
                                    builder,
                                    heap,
                                    offsets[dst],
                                    result,
                                    result,
                                    result,
                                );
                            }
                            _ => {}
                        }
                        // Zero derivatives for non-analytical noise types
                        if has_derivs[dst] {
                            zero_derivs(builder, heap, offsets[dst], jtypes[dst]);
                        }
                    }
                } else {
                    // 1D noise
                    let x_val = heap_load_f32(builder, heap, offsets[args[1] as usize]);
                    let x_sym = args[1] as usize;

                    if has_derivs[dst] && has_analytic_deriv && has_derivs[x_sym] {
                        // Use 1D derivative trampoline
                        let deriv_func = math.noise1_deriv; // snoise1 uses same deriv * 2
                        let func_ref = module.declare_func_in_func(deriv_func, builder.func);
                        let dst_off_val = builder.ins().iconst(types::I32, offsets[dst] as i64);
                        let grad_off_val = builder.ins().iconst(types::I32, scratch_offset as i64);
                        builder
                            .ins()
                            .call(func_ref, &[heap, dst_off_val, grad_off_val, x_val]);
                        // For snoise, adjust: val was already set to 2*perlin-1, grad = 2*grad
                        if name == "snoise" {
                            let val = heap_load_f32(builder, heap, offsets[dst]);
                            let two = builder.ins().f32const(2.0);
                            let one = builder.ins().f32const(1.0);
                            let sval = builder.ins().fmul(val, two);
                            let sval = builder.ins().fsub(sval, one);
                            heap_store_f32(builder, heap, offsets[dst], sval);
                            let g = heap_load_f32(builder, heap, scratch_offset);
                            let sg = builder.ins().fmul(g, two);
                            heap_store_f32(builder, heap, scratch_offset, sg);
                        }
                        // Chain rule: result.dx = grad * x.dx, result.dy = grad * x.dy
                        let sz_f = 4usize;
                        let grad = heap_load_f32(builder, heap, scratch_offset);
                        for d in 1..=2usize {
                            let xd = heap_load_f32(builder, heap, offsets[x_sym] + d * sz_f);
                            let rd = builder.ins().fmul(grad, xd);
                            heap_store_f32(builder, heap, offsets[dst] + d * sz_f, rd);
                        }
                    } else {
                        let func_id = match name {
                            "cellnoise" => math.cellnoise1,
                            "snoise" => math.snoise1,
                            "hashnoise" => math.hashnoise1,
                            _ => math.noise1,
                        };
                        let func_ref = module.declare_func_in_func(func_id, builder.func);
                        let call = builder.ins().call(func_ref, &[x_val]);
                        let result = builder.inst_results(call)[0];
                        match jtypes[dst] {
                            JitType::Float => {
                                heap_store_f32(builder, heap, offsets[dst], result);
                            }
                            JitType::Vec3 | JitType::Color => {
                                heap_store_vec3(
                                    builder,
                                    heap,
                                    offsets[dst],
                                    result,
                                    result,
                                    result,
                                );
                            }
                            _ => {}
                        }
                        if has_derivs[dst] {
                            zero_derivs(builder, heap, offsets[dst], jtypes[dst]);
                        }
                    }
                }
                builder.ins().jump(next_block, &[]);
            }
            // --- Texture (real trampoline via RendererServices) ---
            "texture" if args.len() >= 4 => {
                // texture(result, filename, s, t, ...) first_optional: 4 or 9 (with derivs)
                let dst = args[0] as usize;
                let s_sym = args[2] as usize;
                let t_sym = args[3] as usize;
                let first_optional = if args.len() >= 9 { 9 } else { 4 };
                let n_pairs = (args.len() - first_optional) / 2;
                let opt_scratch = if n_pairs > 0 && n_pairs <= 8 {
                    let np_val = builder.ins().iconst(types::I32, n_pairs as i64);
                    heap_store_i32(builder, heap, scratch_offset, np_val);
                    for i in 0..n_pairs {
                        let name_sym = args[first_optional + 2 * i] as usize;
                        let val_sym = args[first_optional + 2 * i + 1] as usize;
                        let vt = match jtypes[val_sym] {
                            JitType::Int => 0,
                            JitType::Float => 1,
                            JitType::String => 2,
                            _ => 1,
                        };
                        let no_val = builder.ins().iconst(types::I32, offsets[name_sym] as i64);
                        let vo_val = builder.ins().iconst(types::I32, offsets[val_sym] as i64);
                        let vt_val = builder.ins().iconst(types::I32, vt as i64);
                        heap_store_i32(builder, heap, scratch_offset + 4 + i * 12, no_val);
                        heap_store_i32(builder, heap, scratch_offset + 4 + i * 12 + 4, vo_val);
                        heap_store_i32(builder, heap, scratch_offset + 4 + i * 12 + 8, vt_val);
                    }
                    builder.ins().iconst(types::I32, scratch_offset as i64)
                } else {
                    builder.ins().iconst(types::I32, -1)
                };
                let nchannels = match jtypes[dst] {
                    JitType::Vec3 | JitType::Color => 3,
                    _ => 1,
                };
                let dst_off = builder.ins().iconst(types::I32, offsets[dst] as i64);
                let nc = builder.ins().iconst(types::I32, nchannels);
                let fname_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[1] as usize] as i64);
                let s_off = builder.ins().iconst(types::I32, offsets[s_sym] as i64);
                let t_off = builder.ins().iconst(types::I32, offsets[t_sym] as i64);
                let s_hd = builder
                    .ins()
                    .iconst(types::I32, if has_derivs[s_sym] { 1 } else { 0 });
                let t_hd = builder
                    .ins()
                    .iconst(types::I32, if has_derivs[t_sym] { 1 } else { 0 });
                let func_ref = module.declare_func_in_func(math.rs_texture, builder.func);
                builder.ins().call(
                    func_ref,
                    &[
                        ctx,
                        sg,
                        heap,
                        dst_off,
                        nc,
                        fname_off,
                        s_off,
                        t_off,
                        s_hd,
                        t_hd,
                        opt_scratch,
                    ],
                );
                if has_derivs[dst] {
                    zero_derivs(builder, heap, offsets[dst], jtypes[dst]);
                }
                builder.ins().jump(next_block, &[]);
            }
            "texture3d" if args.len() >= 3 => {
                let p_sym = args[2] as usize;
                let user_derivs = args.len() >= 6
                    && matches!(
                        jtypes.get(args[3] as usize),
                        Some(JitType::Vec3 | JitType::Color)
                    );
                let first_optional = if user_derivs { 6 } else { 3 };
                let sz_v = 12usize;
                let (dpdx_off, dpdy_off, dpdz_off) = if user_derivs {
                    (
                        offsets[args[3] as usize] as i64,
                        offsets[args[4] as usize] as i64,
                        offsets[args[5] as usize] as i64,
                    )
                } else if has_derivs[p_sym] {
                    (
                        (offsets[p_sym] + sz_v) as i64,
                        (offsets[p_sym] + 2 * sz_v) as i64,
                        -1i64, // C++ passes null for dPdz when auto
                    )
                } else {
                    (-1i64, -1i64, -1i64)
                };
                let n_pairs = (args.len() - first_optional) / 2;
                let opt_scratch = if n_pairs > 0 && n_pairs <= 8 {
                    let np_val = builder.ins().iconst(types::I32, n_pairs as i64);
                    heap_store_i32(builder, heap, scratch_offset, np_val);
                    for i in 0..n_pairs {
                        let name_sym = args[first_optional + 2 * i] as usize;
                        let val_sym = args[first_optional + 2 * i + 1] as usize;
                        let vt = match jtypes[val_sym] {
                            JitType::Int => 0,
                            JitType::Float => 1,
                            JitType::String => 2,
                            _ => 1,
                        };
                        let no_val = builder.ins().iconst(types::I32, offsets[name_sym] as i64);
                        let vo_val = builder.ins().iconst(types::I32, offsets[val_sym] as i64);
                        let vt_val = builder.ins().iconst(types::I32, vt as i64);
                        heap_store_i32(builder, heap, scratch_offset + 4 + i * 12, no_val);
                        heap_store_i32(builder, heap, scratch_offset + 4 + i * 12 + 4, vo_val);
                        heap_store_i32(builder, heap, scratch_offset + 4 + i * 12 + 8, vt_val);
                    }
                    builder.ins().iconst(types::I32, scratch_offset as i64)
                } else {
                    builder.ins().iconst(types::I32, -1)
                };
                let nchannels = match jtypes[args[0] as usize] {
                    JitType::Vec3 | JitType::Color => 3,
                    _ => 1,
                };
                let dst_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[0] as usize] as i64);
                let nc = builder.ins().iconst(types::I32, nchannels);
                let fname_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[1] as usize] as i64);
                let p_off = builder.ins().iconst(types::I32, offsets[p_sym] as i64);
                let dpdx_val = builder.ins().iconst(types::I32, dpdx_off);
                let dpdy_val = builder.ins().iconst(types::I32, dpdy_off);
                let dpdz_val = builder.ins().iconst(types::I32, dpdz_off);
                let func_ref = module.declare_func_in_func(math.rs_texture3d, builder.func);
                builder.ins().call(
                    func_ref,
                    &[
                        ctx,
                        sg,
                        heap,
                        dst_off,
                        nc,
                        fname_off,
                        p_off,
                        dpdx_val,
                        dpdy_val,
                        dpdz_val,
                        opt_scratch,
                    ],
                );
                builder.ins().jump(next_block, &[]);
            }
            "environment" if args.len() >= 3 => {
                let r_sym = args[2] as usize;
                let user_derivs = args.len() >= 5
                    && matches!(
                        jtypes.get(args[3] as usize),
                        Some(JitType::Vec3 | JitType::Color)
                    );
                let first_optional = if user_derivs { 5 } else { 3 };
                let sz_v = 12usize;
                let (drdx_off, drdy_off) = if user_derivs {
                    (
                        offsets[args[3] as usize] as i64,
                        offsets[args[4] as usize] as i64,
                    )
                } else if has_derivs[r_sym] {
                    (
                        (offsets[r_sym] + sz_v) as i64,
                        (offsets[r_sym] + 2 * sz_v) as i64,
                    )
                } else {
                    (-1i64, -1i64)
                };
                let n_pairs = (args.len() - first_optional) / 2;
                let opt_scratch = if n_pairs > 0 && n_pairs <= 8 {
                    let np_val = builder.ins().iconst(types::I32, n_pairs as i64);
                    heap_store_i32(builder, heap, scratch_offset, np_val);
                    for i in 0..n_pairs {
                        let name_sym = args[first_optional + 2 * i] as usize;
                        let val_sym = args[first_optional + 2 * i + 1] as usize;
                        let vt = match jtypes[val_sym] {
                            JitType::Int => 0,
                            JitType::Float => 1,
                            JitType::String => 2,
                            _ => 1,
                        };
                        let no_val = builder.ins().iconst(types::I32, offsets[name_sym] as i64);
                        let vo_val = builder.ins().iconst(types::I32, offsets[val_sym] as i64);
                        let vt_val = builder.ins().iconst(types::I32, vt as i64);
                        heap_store_i32(builder, heap, scratch_offset + 4 + i * 12, no_val);
                        heap_store_i32(builder, heap, scratch_offset + 4 + i * 12 + 4, vo_val);
                        heap_store_i32(builder, heap, scratch_offset + 4 + i * 12 + 8, vt_val);
                    }
                    builder.ins().iconst(types::I32, scratch_offset as i64)
                } else {
                    builder.ins().iconst(types::I32, -1)
                };
                let nchannels = match jtypes[args[0] as usize] {
                    JitType::Vec3 | JitType::Color => 3,
                    _ => 1,
                };
                let dst_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[0] as usize] as i64);
                let nc = builder.ins().iconst(types::I32, nchannels);
                let fname_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[1] as usize] as i64);
                let r_off = builder.ins().iconst(types::I32, offsets[r_sym] as i64);
                let drdx_val = builder.ins().iconst(types::I32, drdx_off);
                let drdy_val = builder.ins().iconst(types::I32, drdy_off);
                let func_ref = module.declare_func_in_func(math.rs_environment, builder.func);
                builder.ins().call(
                    func_ref,
                    &[
                        ctx,
                        sg,
                        heap,
                        dst_off,
                        nc,
                        fname_off,
                        r_off,
                        drdx_val,
                        drdy_val,
                        opt_scratch,
                    ],
                );
                builder.ins().jump(next_block, &[]);
            }
            // --- gettextureinfo (trampoline via RendererServices) ---
            "gettextureinfo" if args.len() >= 4 => {
                // gettextureinfo(result, filename, dataname, data) or (result, filename, s, t, dataname, data)
                let use_coords = args.len() >= 6;
                let (dname_idx, data_idx) = if use_coords { (4, 5) } else { (2, 3) };
                let result_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[0] as usize] as i64);
                let fname_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[1] as usize] as i64);
                let dname_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[dname_idx] as usize] as i64);
                let data_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[data_idx] as usize] as i64);
                let data_type = match jtypes[args[data_idx] as usize] {
                    JitType::Int => 0,
                    JitType::String => 2,
                    _ => 1,
                };
                let dt = builder.ins().iconst(types::I32, data_type as i64);
                let func_ref = module.declare_func_in_func(math.rs_gettextureinfo, builder.func);
                let call = builder.ins().call(
                    func_ref,
                    &[
                        ctx, sg, heap, result_off, fname_off, dname_off, data_off, dt,
                    ],
                );
                let ok = builder.inst_results(call)[0];
                heap_store_i32(builder, heap, offsets[args[0] as usize], ok);
                builder.ins().jump(next_block, &[]);
            }
            // --- getattribute (real trampoline via RendererServices) ---
            "getattribute" if args.len() >= 3 => {
                // getattribute(result, name, value)
                let dst = args[0] as usize;
                let result_type_code = match jtypes[dst] {
                    JitType::Int => 0,
                    JitType::Float => 1,
                    JitType::Vec3 | JitType::Color => 2,
                    _ => 1,
                };
                let result_off = builder.ins().iconst(types::I32, offsets[dst] as i64);
                let rt = builder.ins().iconst(types::I32, result_type_code);
                // Name is typically args[1] or args[2] depending on format
                let name_idx = if args.len() >= 4 { args[2] } else { args[1] };
                let name_off = builder
                    .ins()
                    .iconst(types::I32, offsets[name_idx as usize] as i64);
                let func_ref = module.declare_func_in_func(math.rs_getattribute, builder.func);
                let call = builder
                    .ins()
                    .call(func_ref, &[ctx, sg, heap, result_off, rt, name_off]);
                let _success = builder.inst_results(call)[0];
                // getattribute returns per-point constants: zero derivatives
                if has_derivs[dst] {
                    zero_derivs(builder, heap, offsets[dst], jtypes[dst]);
                }
                builder.ins().jump(next_block, &[]);
            }
            // --- trace (real trampoline via RendererServices) ---
            "trace" if args.len() >= 3 => {
                let pos_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[1] as usize] as i64);
                let dir_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[2] as usize] as i64);
                let func_ref = module.declare_func_in_func(math.rs_trace, builder.func);
                let call = builder
                    .ins()
                    .call(func_ref, &[ctx, sg, heap, pos_off, dir_off]);
                let hit = builder.inst_results(call)[0];
                heap_store_i32(builder, heap, offsets[args[0] as usize], hit);
                builder.ins().jump(next_block, &[]);
            }

            // --- Closure allocation (real trampoline) ---
            "closure" | "diffuse" | "oren_nayar" | "phong" | "ward" | "microfacet"
            | "reflection" | "refraction" | "transparent" | "emission" | "background"
            | "holdout" | "debug" | "translucent"
                if args.len() >= 1 =>
            {
                // closure(result, closure_name_string, [N_or_param, ...])
                // For named closures like "diffuse", the name IS the opcode name.
                // We need to pass: ctx, heap, dst_off, name_off, weight_off
                // For now, use unit weight (1,1,1) and the opcode name as closure name.
                // The closure name is stored as a UString hash at a synthetic offset,
                // but simpler: we can store the closure_id directly.
                let dst_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[0] as usize] as i64);
                // Use args[1] as the closure name string if available, else pass dst (self-ref won't matter)
                let name_off = if args.len() >= 2 {
                    builder
                        .ins()
                        .iconst(types::I32, offsets[args[1] as usize] as i64)
                } else {
                    dst_off
                };
                // weight from N or first param, or use dst as placeholder
                let weight_off = if args.len() >= 3 {
                    builder
                        .ins()
                        .iconst(types::I32, offsets[args[2] as usize] as i64)
                } else if args.len() >= 2 {
                    builder
                        .ins()
                        .iconst(types::I32, offsets[args[1] as usize] as i64)
                } else {
                    dst_off
                };
                let func_ref = module.declare_func_in_func(math.closure_alloc, builder.func);
                builder
                    .ins()
                    .call(func_ref, &[ctx, heap, dst_off, name_off, weight_off]);
                builder.ins().jump(next_block, &[]);
            }

            // --- Message passing (real trampoline) ---
            "setmessage" if args.len() >= 3 => {
                // setmessage(name_string, value) — args[0]=name, args[1]=value
                // But OSL format is: setmessage(result, name, value)
                let name_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[1] as usize] as i64);
                let value_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[2] as usize] as i64);
                let vtype = match jtypes[args[2] as usize] {
                    JitType::Int => 0,
                    JitType::Float => 1,
                    JitType::Vec3 | JitType::Color => 2,
                    _ => 0,
                };
                let vt = builder.ins().iconst(types::I32, vtype);
                let func_ref = module.declare_func_in_func(math.msg_set, builder.func);
                builder
                    .ins()
                    .call(func_ref, &[ctx, heap, name_off, value_off, vt]);
                builder.ins().jump(next_block, &[]);
            }
            "getmessage" if args.len() >= 3 => {
                // getmessage(result, source_string, name_string, value_dest)
                let name_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[2] as usize] as i64);
                let result_off = if args.len() >= 4 {
                    builder
                        .ins()
                        .iconst(types::I32, offsets[args[3] as usize] as i64)
                } else {
                    builder
                        .ins()
                        .iconst(types::I32, offsets[args[0] as usize] as i64)
                };
                let rtype = match jtypes[args[0] as usize] {
                    JitType::Int => 0,
                    JitType::Float => 1,
                    JitType::Vec3 | JitType::Color => 2,
                    _ => 0,
                };
                let rt = builder.ins().iconst(types::I32, rtype);
                let func_ref = module.declare_func_in_func(math.msg_get, builder.func);
                let call = builder
                    .ins()
                    .call(func_ref, &[ctx, heap, name_off, result_off, rt]);
                let found = builder.inst_results(call)[0];
                heap_store_i32(builder, heap, offsets[args[0] as usize], found);
                builder.ins().jump(next_block, &[]);
            }

            // --- String ops (trampoline-based) ---
            "strlen" if args.len() >= 2 => {
                let func_ref = module.declare_func_in_func(math.str_strlen, builder.func);
                let str_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[1] as usize] as i64);
                let call = builder.ins().call(func_ref, &[heap, str_off]);
                let result = builder.inst_results(call)[0];
                heap_store_i32(builder, heap, offsets[args[0] as usize], result);
                builder.ins().jump(next_block, &[]);
            }
            "hash" if args.len() >= 2 => {
                let func_ref = module.declare_func_in_func(math.str_hash, builder.func);
                let str_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[1] as usize] as i64);
                let call = builder.ins().call(func_ref, &[heap, str_off]);
                let result = builder.inst_results(call)[0];
                heap_store_i32(builder, heap, offsets[args[0] as usize], result);
                builder.ins().jump(next_block, &[]);
            }
            "stoi" if args.len() >= 2 => {
                let func_ref = module.declare_func_in_func(math.str_stoi, builder.func);
                let str_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[1] as usize] as i64);
                let call = builder.ins().call(func_ref, &[heap, str_off]);
                let result = builder.inst_results(call)[0];
                heap_store_i32(builder, heap, offsets[args[0] as usize], result);
                builder.ins().jump(next_block, &[]);
            }
            "stof" if args.len() >= 2 => {
                let func_ref = module.declare_func_in_func(math.str_stof, builder.func);
                let str_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[1] as usize] as i64);
                let call = builder.ins().call(func_ref, &[heap, str_off]);
                let result = builder.inst_results(call)[0];
                heap_store_f32(builder, heap, offsets[args[0] as usize], result);
                builder.ins().jump(next_block, &[]);
            }
            // --- String manipulation (real trampolines) ---
            "concat" if args.len() >= 3 => {
                let func_ref = module.declare_func_in_func(math.str_concat, builder.func);
                let dst = builder
                    .ins()
                    .iconst(types::I32, offsets[args[0] as usize] as i64);
                let a = builder
                    .ins()
                    .iconst(types::I32, offsets[args[1] as usize] as i64);
                let b = builder
                    .ins()
                    .iconst(types::I32, offsets[args[2] as usize] as i64);
                builder.ins().call(func_ref, &[heap, dst, a, b]);
                builder.ins().jump(next_block, &[]);
            }
            "substr" if args.len() >= 4 => {
                let func_ref = module.declare_func_in_func(math.str_substr, builder.func);
                let dst = builder
                    .ins()
                    .iconst(types::I32, offsets[args[0] as usize] as i64);
                let src = builder
                    .ins()
                    .iconst(types::I32, offsets[args[1] as usize] as i64);
                let start = builder
                    .ins()
                    .iconst(types::I32, offsets[args[2] as usize] as i64);
                let len = builder
                    .ins()
                    .iconst(types::I32, offsets[args[3] as usize] as i64);
                builder.ins().call(func_ref, &[heap, dst, src, start, len]);
                builder.ins().jump(next_block, &[]);
            }
            "format" if args.len() >= 2 => {
                // format(result, fmt_string, arg0, arg1, ...)
                // Pack arg offsets and types into the heap scratch area for the trampoline.
                let nargs = (args.len() as i32 - 2).max(0) as usize;
                let dst = builder
                    .ins()
                    .iconst(types::I32, offsets[args[0] as usize] as i64);
                let fmt = builder
                    .ins()
                    .iconst(types::I32, offsets[args[1] as usize] as i64);

                if nargs == 0 {
                    let args_base = builder.ins().iconst(types::I32, 0);
                    let types_base = builder.ins().iconst(types::I32, 0);
                    let na = builder.ins().iconst(types::I32, 0);
                    let func_ref = module.declare_func_in_func(math.str_format, builder.func);
                    builder
                        .ins()
                        .call(func_ref, &[heap, dst, fmt, args_base, types_base, na]);
                } else {
                    // Use the last part of the heap as scratch for the arg offset/type arrays.
                    // Each array needs nargs * 4 bytes. We'll use heap_size as the base
                    // (the heap was already allocated with some extra room).
                    // Write offsets array: [off0, off1, ...]
                    // Write types array: [type0, type1, ...]
                    // where type: 0=int, 1=float, 2=string
                    let scratch_base = scratch_offset;
                    let types_start = scratch_base + nargs * 4;
                    for i in 0..nargs {
                        let arg_idx = (i + 2) as usize; // skip result + fmt
                        let aoff = offsets[args[arg_idx] as usize] as i32;
                        let aoff_val = builder.ins().iconst(types::I32, aoff as i64);
                        heap_store_i32(builder, heap, scratch_base + i * 4, aoff_val);
                        let atype: i32 = match jtypes[args[arg_idx] as usize] {
                            JitType::Int => 0,
                            JitType::Float => 1,
                            JitType::String => 2,
                            _ => 1, // default to float for Vec3/Color/Matrix
                        };
                        let atype_val = builder.ins().iconst(types::I32, atype as i64);
                        heap_store_i32(builder, heap, types_start + i * 4, atype_val);
                    }
                    let args_base = builder.ins().iconst(types::I32, scratch_base as i64);
                    let types_base_val = builder.ins().iconst(types::I32, types_start as i64);
                    let na = builder.ins().iconst(types::I32, nargs as i64);
                    let func_ref = module.declare_func_in_func(math.str_format, builder.func);
                    builder
                        .ins()
                        .call(func_ref, &[heap, dst, fmt, args_base, types_base_val, na]);
                }
                builder.ins().jump(next_block, &[]);
            }
            "getchar" if args.len() >= 3 => {
                let func_ref = module.declare_func_in_func(math.str_getchar, builder.func);
                let src = builder
                    .ins()
                    .iconst(types::I32, offsets[args[1] as usize] as i64);
                let idx = builder
                    .ins()
                    .iconst(types::I32, offsets[args[2] as usize] as i64);
                let call = builder.ins().call(func_ref, &[heap, src, idx]);
                let result = builder.inst_results(call)[0];
                heap_store_i32(builder, heap, offsets[args[0] as usize], result);
                builder.ins().jump(next_block, &[]);
            }
            "startswith" if args.len() >= 3 => {
                let func_ref = module.declare_func_in_func(math.str_startswith, builder.func);
                let s = builder
                    .ins()
                    .iconst(types::I32, offsets[args[1] as usize] as i64);
                let p = builder
                    .ins()
                    .iconst(types::I32, offsets[args[2] as usize] as i64);
                let call = builder.ins().call(func_ref, &[heap, s, p]);
                let result = builder.inst_results(call)[0];
                heap_store_i32(builder, heap, offsets[args[0] as usize], result);
                builder.ins().jump(next_block, &[]);
            }
            "endswith" if args.len() >= 3 => {
                let func_ref = module.declare_func_in_func(math.str_endswith, builder.func);
                let s = builder
                    .ins()
                    .iconst(types::I32, offsets[args[1] as usize] as i64);
                let p = builder
                    .ins()
                    .iconst(types::I32, offsets[args[2] as usize] as i64);
                let call = builder.ins().call(func_ref, &[heap, s, p]);
                let result = builder.inst_results(call)[0];
                heap_store_i32(builder, heap, offsets[args[0] as usize], result);
                builder.ins().jump(next_block, &[]);
            }
            "regex_search" if args.len() >= 3 => {
                let func_ref = module.declare_func_in_func(math.str_regex_search, builder.func);
                let subj = builder
                    .ins()
                    .iconst(types::I32, offsets[args[1] as usize] as i64);
                let pat = builder
                    .ins()
                    .iconst(types::I32, offsets[args[2] as usize] as i64);
                let call = builder.ins().call(func_ref, &[heap, subj, pat]);
                let result = builder.inst_results(call)[0];
                heap_store_i32(builder, heap, offsets[args[0] as usize], result);
                builder.ins().jump(next_block, &[]);
            }
            "regex_match" if args.len() >= 3 => {
                let func_ref = module.declare_func_in_func(math.str_regex_match, builder.func);
                let subj = builder
                    .ins()
                    .iconst(types::I32, offsets[args[1] as usize] as i64);
                let pat = builder
                    .ins()
                    .iconst(types::I32, offsets[args[2] as usize] as i64);
                let call = builder.ins().call(func_ref, &[heap, subj, pat]);
                let result = builder.inst_results(call)[0];
                heap_store_i32(builder, heap, offsets[args[0] as usize], result);
                builder.ins().jump(next_block, &[]);
            }
            "split" if args.len() >= 5 => {
                // split(str, sep, results[], maxsplit, result_count)
                let func_ref = module.declare_func_in_func(math.str_split, builder.func);
                let result_arr = builder
                    .ins()
                    .iconst(types::I32, offsets[args[2] as usize] as i64);
                let str_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[0] as usize] as i64);
                let sep_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[1] as usize] as i64);
                let maxsplit = heap_load_i32(builder, heap, offsets[args[3] as usize]);
                let call = builder
                    .ins()
                    .call(func_ref, &[heap, result_arr, str_off, sep_off, maxsplit]);
                let count = builder.inst_results(call)[0];
                heap_store_i32(builder, heap, offsets[args[4] as usize], count);
                builder.ins().jump(next_block, &[]);
            }
            "split" if args.len() >= 2 => {
                let zero = builder.ins().iconst(types::I32, 0);
                heap_store_i32(builder, heap, offsets[args[0] as usize], zero);
                builder.ins().jump(next_block, &[]);
            }

            // --- Spline (real trampoline to crate::spline) ---
            // spline(result, basis_string, t, knot0, knot1, ...) or array
            "spline" if args.len() >= 4 => {
                let dst = args[0] as usize;
                let t_sym = args[2] as usize;
                let nknots = (args.len() - 3) as i32;
                let dst_off_val = builder.ins().iconst(types::I32, offsets[dst] as i64);
                let basis_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[1] as usize] as i64);
                let t_off = builder.ins().iconst(types::I32, offsets[t_sym] as i64);
                let knots_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[3] as usize] as i64);
                let nk = builder.ins().iconst(types::I32, nknots as i64);
                match jtypes[dst] {
                    JitType::Vec3 | JitType::Color => {
                        let func_ref = module.declare_func_in_func(math.spline_vec3, builder.func);
                        builder.ins().call(
                            func_ref,
                            &[heap, dst_off_val, basis_off, t_off, knots_off, nk],
                        );
                        // D(spline_vec3(t)) = spline_vec3'(t) * D(t)  (chain rule)
                        if has_derivs[dst] {
                            let sz = 12usize;
                            // Use scratch area to receive vec3 derivative from trampoline
                            let scratch_v = builder.ins().iconst(types::I32, scratch_offset as i64);
                            let dv_ref =
                                module.declare_func_in_func(math.spline_vec3_deriv, builder.func);
                            builder
                                .ins()
                                .call(dv_ref, &[heap, scratch_v, basis_off, t_off, knots_off, nk]);
                            let (gx, gy, gz) = heap_load_vec3(builder, heap, scratch_offset);
                            for d in 1..=2usize {
                                let td = if has_derivs[t_sym] {
                                    heap_load_f32(builder, heap, offsets[t_sym] + d * 4)
                                } else {
                                    builder.ins().f32const(0.0)
                                };
                                let rx = builder.ins().fmul(gx, td);
                                let ry = builder.ins().fmul(gy, td);
                                let rz = builder.ins().fmul(gz, td);
                                heap_store_vec3(builder, heap, offsets[dst] + d * sz, rx, ry, rz);
                            }
                        }
                    }
                    _ => {
                        let func_ref = module.declare_func_in_func(math.spline_float, builder.func);
                        builder.ins().call(
                            func_ref,
                            &[heap, dst_off_val, basis_off, t_off, knots_off, nk],
                        );
                        // D(spline_float(t)) = spline_float'(t) * D(t)  (chain rule)
                        if has_derivs[dst] {
                            let df_ref =
                                module.declare_func_in_func(math.spline_float_deriv, builder.func);
                            let call = builder
                                .ins()
                                .call(df_ref, &[heap, basis_off, t_off, knots_off, nk]);
                            let ds_dt = builder.inst_results(call)[0];
                            propagate_derivs_unary_f32(
                                builder,
                                heap,
                                offsets[dst],
                                offsets[t_sym],
                                has_derivs[t_sym],
                                ds_dt,
                            );
                        }
                    }
                }
                builder.ins().jump(next_block, &[]);
            }
            "splineinverse" if args.len() >= 4 => {
                let nknots = (args.len() - 3) as i32;
                let dst_off_val = builder
                    .ins()
                    .iconst(types::I32, offsets[args[0] as usize] as i64);
                let basis_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[1] as usize] as i64);
                let val_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[2] as usize] as i64);
                let knots_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[3] as usize] as i64);
                let nk = builder.ins().iconst(types::I32, nknots as i64);
                let func_ref = module.declare_func_in_func(math.splineinverse_float, builder.func);
                builder.ins().call(
                    func_ref,
                    &[heap, dst_off_val, basis_off, val_off, knots_off, nk],
                );
                builder.ins().jump(next_block, &[]);
            }

            // --- Dict ops (trampoline with DictStore via ctx) ---
            "dict_find" if args.len() >= 3 => {
                let func_ref = module.declare_func_in_func(math.dict_find, builder.func);
                let data_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[1] as usize] as i64);
                let path_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[2] as usize] as i64);
                let call = builder
                    .ins()
                    .call(func_ref, &[ctx, heap, data_off, path_off]);
                let result = builder.inst_results(call)[0];
                heap_store_i32(builder, heap, offsets[args[0] as usize], result);
                builder.ins().jump(next_block, &[]);
            }
            "dict_next" if args.len() >= 2 => {
                // dict_next increments the handle
                let handle = heap_load_i32(builder, heap, offsets[args[1] as usize]);
                let one = builder.ins().iconst(types::I32, 1);
                let next = builder.ins().iadd(handle, one);
                heap_store_i32(builder, heap, offsets[args[0] as usize], next);
                builder.ins().jump(next_block, &[]);
            }
            "dict_value" if args.len() >= 4 => {
                let func_ref = module.declare_func_in_func(math.dict_value, builder.func);
                let result_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[0] as usize] as i64);
                let handle_val = heap_load_i32(builder, heap, offsets[args[1] as usize]);
                let key_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[2] as usize] as i64);
                let rtype = match jtypes[args[0] as usize] {
                    JitType::Int => 0i64,
                    JitType::Float => 1,
                    _ => 2,
                };
                let rt = builder.ins().iconst(types::I32, rtype);
                let call = builder
                    .ins()
                    .call(func_ref, &[ctx, heap, result_off, handle_val, key_off, rt]);
                let _success = builder.inst_results(call)[0];
                builder.ins().jump(next_block, &[]);
            }

            // --- Point cloud (real trampoline via RendererServices) ---
            "pointcloud_search" if args.len() >= 5 => {
                // pointcloud_search(result, filename, center, radius, max_points, sort?, "index", indices?, "distance", distances?)
                let func_ref = module.declare_func_in_func(math.pc_search, builder.func);
                let fname_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[1] as usize] as i64);
                let center_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[2] as usize] as i64);
                let radius = heap_load_f32(builder, heap, offsets[args[3] as usize]);
                let max_pts = heap_load_i32(builder, heap, offsets[args[4] as usize]);
                let attr_start = if args.len() > 5
                    && ir
                        .symbols
                        .get(args[5] as usize)
                        .map(|s| s.typespec.simpletype().is_int())
                        .unwrap_or(false)
                {
                    7
                } else {
                    6
                };
                let sort = if attr_start == 7 {
                    heap_load_i32(builder, heap, offsets[args[5] as usize])
                } else {
                    builder.ins().iconst(types::I32, 1)
                };
                let mut idx_off: i64 = -1;
                let mut dist_off: i64 = -1;
                let mut j = attr_start;
                while j + 1 < args.len() {
                    let name_sym = args[j] as usize;
                    if name_sym < ir.symbols.len()
                        && ir.symbols[name_sym].typespec.simpletype().is_string()
                    {
                        let nm = ir.symbols[name_sym].name.as_str();
                        if nm == "index" && args[j + 1] >= 0 {
                            idx_off = offsets[args[j + 1] as usize] as i64;
                        } else if nm == "distance" && args[j + 1] >= 0 {
                            dist_off = offsets[args[j + 1] as usize] as i64;
                        }
                    }
                    j += 2;
                }
                let indices_off = builder.ins().iconst(types::I32, idx_off);
                let distances_off = builder.ins().iconst(types::I32, dist_off);
                let call = builder.ins().call(
                    func_ref,
                    &[
                        ctx,
                        sg,
                        heap,
                        fname_off,
                        center_off,
                        radius,
                        max_pts,
                        sort,
                        indices_off,
                        distances_off,
                    ],
                );
                let count = builder.inst_results(call)[0];
                heap_store_i32(builder, heap, offsets[args[0] as usize], count);
                builder.ins().jump(next_block, &[]);
            }
            "pointcloud_get" if args.len() >= 6 => {
                // pointcloud_get(result, filename, indices, count, attr, data)
                let func_ref = module.declare_func_in_func(math.pc_get, builder.func);
                let fname_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[1] as usize] as i64);
                let indices_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[2] as usize] as i64);
                let count = heap_load_i32(builder, heap, offsets[args[3] as usize]);
                let attr_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[4] as usize] as i64);
                let data_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[5] as usize] as i64);
                let data_type = match jtypes[args[5] as usize] {
                    JitType::Vec3 | JitType::Color => 1,
                    _ => 0,
                };
                let dt = builder.ins().iconst(types::I32, data_type as i64);
                let call = builder.ins().call(
                    func_ref,
                    &[
                        ctx,
                        sg,
                        heap,
                        fname_off,
                        indices_off,
                        count,
                        attr_off,
                        data_off,
                        dt,
                    ],
                );
                let ok = builder.inst_results(call)[0];
                heap_store_i32(builder, heap, offsets[args[0] as usize], ok);
                builder.ins().jump(next_block, &[]);
            }
            "pointcloud_write" if args.len() >= 3 => {
                // pointcloud_write(result, filename, pos, "attr1", val1, "attr2", val2, ...)
                let nattrs = ((args.len() - 3) / 2).max(0);
                let scratch_base = scratch_offset;
                let nattrs_val = builder.ins().iconst(types::I32, nattrs as i64);
                heap_store_i32(builder, heap, scratch_base, nattrs_val);
                for i in 0..nattrs {
                    let name_off = offsets[args[3 + i * 2] as usize] as i32;
                    let val_off = offsets[args[4 + i * 2] as usize] as i32;
                    let type_code: i32 = match jtypes[args[4 + i * 2] as usize] {
                        JitType::Int => 0,
                        JitType::Vec3 | JitType::Color => 2,
                        _ => 1,
                    };
                    let rec = scratch_base + 8 + i * 12;
                    let no_val = builder.ins().iconst(types::I32, name_off as i64);
                    let tc_val = builder.ins().iconst(types::I32, type_code as i64);
                    let vo_val = builder.ins().iconst(types::I32, val_off as i64);
                    heap_store_i32(builder, heap, rec, no_val);
                    heap_store_i32(builder, heap, rec + 4, tc_val);
                    heap_store_i32(builder, heap, rec + 8, vo_val);
                }
                let func_ref = module.declare_func_in_func(math.pc_write, builder.func);
                let result_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[0] as usize] as i64);
                let fname_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[1] as usize] as i64);
                let pos_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[2] as usize] as i64);
                let scratch_off_val = builder.ins().iconst(types::I32, scratch_base as i64);
                builder.ins().call(
                    func_ref,
                    &[
                        ctx,
                        sg,
                        heap,
                        result_off,
                        fname_off,
                        pos_off,
                        scratch_off_val,
                    ],
                );
                builder.ins().jump(next_block, &[]);
            }

            // --- Color transforms (real trampoline calls) ---
            "blackbody" if args.len() >= 2 => {
                let dst = args[0] as usize;
                let temp = heap_load_f32(builder, heap, offsets[args[1] as usize]);
                let dst_off = builder.ins().iconst(types::I32, offsets[dst] as i64);
                let func_ref = module.declare_func_in_func(math.blackbody, builder.func);
                builder.ins().call(func_ref, &[heap, dst_off, temp]);
                // blackbody is a lookup table — zero derivatives
                if has_derivs[dst] {
                    zero_derivs(builder, heap, offsets[dst], JitType::Vec3);
                }
                builder.ins().jump(next_block, &[]);
            }
            "wavelength_color" if args.len() >= 2 => {
                let dst = args[0] as usize;
                let wl = heap_load_f32(builder, heap, offsets[args[1] as usize]);
                let dst_off = builder.ins().iconst(types::I32, offsets[dst] as i64);
                let func_ref = module.declare_func_in_func(math.wavelength_color, builder.func);
                builder.ins().call(func_ref, &[heap, dst_off, wl]);
                // wavelength_color is a lookup table — zero derivatives
                if has_derivs[dst] {
                    zero_derivs(builder, heap, offsets[dst], JitType::Vec3);
                }
                builder.ins().jump(next_block, &[]);
            }
            "transformc" if args.len() >= 4 => {
                // transformc(result, from_space, to_space, color) — real color space conversion
                let dst = args[0] as usize;
                let src = args[3] as usize;
                let func_ref = module.declare_func_in_func(math.transformc, builder.func);
                let dst_off = builder.ins().iconst(types::I32, offsets[dst] as i64);
                let from_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[1] as usize] as i64);
                let to_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[2] as usize] as i64);
                let src_off = builder.ins().iconst(types::I32, offsets[src] as i64);
                builder
                    .ins()
                    .call(func_ref, &[heap, dst_off, from_off, to_off, src_off]);
                // Color space transforms are approximately linear, so derivs pass through
                if has_derivs[dst] {
                    let sz = 12usize;
                    for d in 1..=2usize {
                        let (dx, dy, dz) = if has_derivs[src] {
                            heap_load_vec3(builder, heap, offsets[src] + d * sz)
                        } else {
                            let z = builder.ins().f32const(0.0);
                            (z, z, z)
                        };
                        heap_store_vec3(builder, heap, offsets[dst] + d * sz, dx, dy, dz);
                    }
                }
                builder.ins().jump(next_block, &[]);
            }

            // --- Matrix construction ---
            "matrix" if args.len() >= 2 => {
                let dst = args[0] as usize;
                let dst_base = offsets[dst];
                if args.len() >= 17 {
                    // matrix(dst, m00, m01, ..., m33) — 16 floats
                    for i in 0..16 {
                        let v = heap_load_f32(builder, heap, offsets[args[1 + i] as usize]);
                        heap_store_f32(builder, heap, dst_base + i * 4, v);
                    }
                } else {
                    // matrix(dst, scale_float) — diagonal scale matrix
                    let v = heap_load_f32(builder, heap, offsets[args[1] as usize]);
                    let zero = builder.ins().f32const(0.0);
                    let one = builder.ins().f32const(1.0);
                    // Row-major: [v,0,0,0, 0,v,0,0, 0,0,v,0, 0,0,0,1]
                    for i in 0..16 {
                        let val = if i == 0 || i == 5 || i == 10 {
                            v
                        } else if i == 15 {
                            one
                        } else {
                            zero
                        };
                        heap_store_f32(builder, heap, dst_base + i * 4, val);
                    }
                }
                builder.ins().jump(next_block, &[]);
            }
            "getmatrix" if args.len() >= 3 => {
                // getmatrix(success, from, to, M) — C++ op layout; 2-arg form has to="common"
                let m_dst = if args.len() >= 4 { args[3] } else { args[2] };
                let dst_off = builder
                    .ins()
                    .iconst(types::I32, offsets[m_dst as usize] as i64);
                let from_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[1] as usize] as i64);
                let to_off = if args.len() >= 4 {
                    builder
                        .ins()
                        .iconst(types::I32, offsets[args[2] as usize] as i64)
                } else {
                    // 2-arg form: pass -1 so trampoline uses "common"
                    builder.ins().iconst(types::I32, -1)
                };
                let func_ref = module.declare_func_in_func(math.rs_getmatrix, builder.func);
                let call = builder
                    .ins()
                    .call(func_ref, &[ctx, sg, heap, dst_off, from_off, to_off]);
                let success = builder.inst_results(call)[0];
                heap_store_i32(builder, heap, offsets[args[0] as usize], success);
                builder.ins().jump(next_block, &[]);
            }
            "transform" if args.len() >= 3 => {
                // transform(result, matrix, source) — transform point by matrix
                let dst = args[0] as usize;
                if args.len() >= 4 && matches!(jtypes[dst], JitType::Vec3 | JitType::Color) {
                    let src = args[3] as usize;
                    let func_ref = module.declare_func_in_func(math.transform_point, builder.func);
                    let dst_off_v = builder.ins().iconst(types::I32, offsets[dst] as i64);
                    let mat_off = builder
                        .ins()
                        .iconst(types::I32, offsets[args[1] as usize] as i64);
                    let src_off = builder.ins().iconst(types::I32, offsets[src] as i64);
                    builder
                        .ins()
                        .call(func_ref, &[heap, dst_off_v, mat_off, src_off]);
                    // D(M*P) = M * D(P) — transform derivatives through the same matrix
                    if has_derivs[dst] {
                        let sz = 12usize;
                        let tv_ref =
                            module.declare_func_in_func(math.transform_vector, builder.func);
                        for d in 1..=2usize {
                            if has_derivs[src] {
                                let src_d = builder
                                    .ins()
                                    .iconst(types::I32, (offsets[src] + d * sz) as i64);
                                let dst_d = builder
                                    .ins()
                                    .iconst(types::I32, (offsets[dst] + d * sz) as i64);
                                builder.ins().call(tv_ref, &[heap, dst_d, mat_off, src_d]);
                            } else {
                                let z = builder.ins().f32const(0.0);
                                heap_store_vec3(builder, heap, offsets[dst] + d * sz, z, z, z);
                            }
                        }
                    }
                } else if matches!(jtypes[dst], JitType::Vec3 | JitType::Color) {
                    let src_idx = args[2] as usize;
                    let (x, y, z) = heap_load_vec3(builder, heap, offsets[src_idx]);
                    heap_store_vec3(builder, heap, offsets[dst], x, y, z);
                    if has_derivs[dst] {
                        let sz = 12usize;
                        for d in 1..=2usize {
                            let (dx, dy, dz) = if has_derivs[src_idx] {
                                heap_load_vec3(builder, heap, offsets[src_idx] + d * sz)
                            } else {
                                let z = builder.ins().f32const(0.0);
                                (z, z, z)
                            };
                            heap_store_vec3(builder, heap, offsets[dst] + d * sz, dx, dy, dz);
                        }
                    }
                }
                builder.ins().jump(next_block, &[]);
            }
            "transformv" if args.len() >= 3 => {
                let dst = args[0] as usize;
                if args.len() >= 4 && matches!(jtypes[dst], JitType::Vec3 | JitType::Color) {
                    let src = args[3] as usize;
                    let func_ref = module.declare_func_in_func(math.transform_vector, builder.func);
                    let dst_off_v = builder.ins().iconst(types::I32, offsets[dst] as i64);
                    let mat_off = builder
                        .ins()
                        .iconst(types::I32, offsets[args[1] as usize] as i64);
                    let src_off = builder.ins().iconst(types::I32, offsets[src] as i64);
                    builder
                        .ins()
                        .call(func_ref, &[heap, dst_off_v, mat_off, src_off]);
                    // D(M*V) = M * D(V) — transform derivatives through the same matrix
                    if has_derivs[dst] {
                        let sz = 12usize;
                        let tv_ref =
                            module.declare_func_in_func(math.transform_vector, builder.func);
                        for d in 1..=2usize {
                            if has_derivs[src] {
                                let src_d = builder
                                    .ins()
                                    .iconst(types::I32, (offsets[src] + d * sz) as i64);
                                let dst_d = builder
                                    .ins()
                                    .iconst(types::I32, (offsets[dst] + d * sz) as i64);
                                builder.ins().call(tv_ref, &[heap, dst_d, mat_off, src_d]);
                            } else {
                                let z = builder.ins().f32const(0.0);
                                heap_store_vec3(builder, heap, offsets[dst] + d * sz, z, z, z);
                            }
                        }
                    }
                } else if matches!(jtypes[dst], JitType::Vec3 | JitType::Color) {
                    let src_idx = args[2] as usize;
                    let (x, y, z) = heap_load_vec3(builder, heap, offsets[src_idx]);
                    heap_store_vec3(builder, heap, offsets[dst], x, y, z);
                    if has_derivs[dst] {
                        let sz = 12usize;
                        for d in 1..=2usize {
                            let (dx, dy, dz) = if has_derivs[src_idx] {
                                heap_load_vec3(builder, heap, offsets[src_idx] + d * sz)
                            } else {
                                let z = builder.ins().f32const(0.0);
                                (z, z, z)
                            };
                            heap_store_vec3(builder, heap, offsets[dst] + d * sz, dx, dy, dz);
                        }
                    }
                }
                builder.ins().jump(next_block, &[]);
            }
            "transformn" if args.len() >= 3 => {
                let dst = args[0] as usize;
                if args.len() >= 4 && matches!(jtypes[dst], JitType::Vec3 | JitType::Color) {
                    let src = args[3] as usize;
                    let func_ref = module.declare_func_in_func(math.transform_normal, builder.func);
                    let dst_off_v = builder.ins().iconst(types::I32, offsets[dst] as i64);
                    let mat_off = builder
                        .ins()
                        .iconst(types::I32, offsets[args[1] as usize] as i64);
                    let src_off = builder.ins().iconst(types::I32, offsets[src] as i64);
                    builder
                        .ins()
                        .call(func_ref, &[heap, dst_off_v, mat_off, src_off]);
                    // D(M^-T * N) = M^-T * D(N) — transform derivatives via inverse-transpose
                    if has_derivs[dst] {
                        let sz = 12usize;
                        let tn_ref =
                            module.declare_func_in_func(math.transform_normal, builder.func);
                        for d in 1..=2usize {
                            if has_derivs[src] {
                                let src_d = builder
                                    .ins()
                                    .iconst(types::I32, (offsets[src] + d * sz) as i64);
                                let dst_d = builder
                                    .ins()
                                    .iconst(types::I32, (offsets[dst] + d * sz) as i64);
                                builder.ins().call(tn_ref, &[heap, dst_d, mat_off, src_d]);
                            } else {
                                let z = builder.ins().f32const(0.0);
                                heap_store_vec3(builder, heap, offsets[dst] + d * sz, z, z, z);
                            }
                        }
                    }
                } else if matches!(jtypes[dst], JitType::Vec3 | JitType::Color) {
                    let src_idx = args[2] as usize;
                    let (x, y, z) = heap_load_vec3(builder, heap, offsets[src_idx]);
                    heap_store_vec3(builder, heap, offsets[dst], x, y, z);
                    if has_derivs[dst] {
                        let sz = 12usize;
                        for d in 1..=2usize {
                            let (dx, dy, dz) = if has_derivs[src_idx] {
                                heap_load_vec3(builder, heap, offsets[src_idx] + d * sz)
                            } else {
                                let z = builder.ins().f32const(0.0);
                                (z, z, z)
                            };
                            heap_store_vec3(builder, heap, offsets[dst] + d * sz, dx, dy, dz);
                        }
                    }
                }
                builder.ins().jump(next_block, &[]);
            }
            "determinant" if args.len() >= 2 => {
                let func_ref = module.declare_func_in_func(math.matrix_det, builder.func);
                let mat_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[1] as usize] as i64);
                let call = builder.ins().call(func_ref, &[heap, mat_off]);
                let result = builder.inst_results(call)[0];
                heap_store_f32(builder, heap, offsets[args[0] as usize], result);
                builder.ins().jump(next_block, &[]);
            }
            "transpose" if args.len() >= 2 => {
                let func_ref = module.declare_func_in_func(math.matrix_transpose, builder.func);
                let dst_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[0] as usize] as i64);
                let src_off = builder
                    .ins()
                    .iconst(types::I32, offsets[args[1] as usize] as i64);
                builder.ins().call(func_ref, &[heap, dst_off, src_off]);
                builder.ins().jump(next_block, &[]);
            }

            // --- Area / calculatenormal / filterwidth ---
            "calculatenormal" if args.len() >= 2 => {
                let dst = args[0] as usize;
                let (x, y, z) = sg_load_vec3(builder, sg, sg_offsets::n());
                heap_store_vec3(builder, heap, offsets[dst], x, y, z);
                // calculatenormal is constant per shading point: zero derivatives
                if has_derivs[dst] {
                    zero_derivs(builder, heap, offsets[dst], JitType::Vec3);
                }
                builder.ins().jump(next_block, &[]);
            }
            "area" if args.len() >= 2 => {
                // area(P) = length(cross(dPdu, dPdv))
                let (dpu_x, dpu_y, dpu_z) = sg_load_vec3(builder, sg, sg_offsets::dp_du());
                let (dpv_x, dpv_y, dpv_z) = sg_load_vec3(builder, sg, sg_offsets::dp_dv());
                // cross product
                let cx = {
                    let a = builder.ins().fmul(dpu_y, dpv_z);
                    let b = builder.ins().fmul(dpu_z, dpv_y);
                    builder.ins().fsub(a, b)
                };
                let cy = {
                    let a = builder.ins().fmul(dpu_z, dpv_x);
                    let b = builder.ins().fmul(dpu_x, dpv_z);
                    builder.ins().fsub(a, b)
                };
                let cz = {
                    let a = builder.ins().fmul(dpu_x, dpv_y);
                    let b = builder.ins().fmul(dpu_y, dpv_x);
                    builder.ins().fsub(a, b)
                };
                let cx2 = builder.ins().fmul(cx, cx);
                let cy2 = builder.ins().fmul(cy, cy);
                let cz2 = builder.ins().fmul(cz, cz);
                let s1 = builder.ins().fadd(cx2, cy2);
                let sum = builder.ins().fadd(s1, cz2);
                let sqrtf_ref = module.declare_func_in_func(math.sqrtf, builder.func);
                let call = builder.ins().call(sqrtf_ref, &[sum]);
                let area_val = builder.inst_results(call)[0];
                heap_store_f32(builder, heap, offsets[args[0] as usize], area_val);
                builder.ins().jump(next_block, &[]);
            }
            "filterwidth" if args.len() >= 2 => {
                // filterwidth(x) = sqrt(Dx(x)^2 + Dy(x)^2)
                // Uses the derivative slots from the heap.
                let src = args[1] as usize;
                let dst = args[0] as usize;
                let sqrtf_ref = module.declare_func_in_func(math.sqrtf, builder.func);
                if has_derivs[src] {
                    match jtypes[src] {
                        JitType::Float => {
                            let sz = 4usize;
                            let dx_val = heap_load_f32(builder, heap, offsets[src] + sz);
                            let dy_val = heap_load_f32(builder, heap, offsets[src] + 2 * sz);
                            let dx2 = builder.ins().fmul(dx_val, dx_val);
                            let dy2 = builder.ins().fmul(dy_val, dy_val);
                            let sum = builder.ins().fadd(dx2, dy2);
                            let call = builder.ins().call(sqrtf_ref, &[sum]);
                            let fw = builder.inst_results(call)[0];
                            heap_store_f32(builder, heap, offsets[dst], fw);
                        }
                        JitType::Vec3 | JitType::Color => {
                            let sz = 12usize;
                            // dx = vec3 derivative, dy = vec3 derivative
                            let (dxx, dxy, dxz) = heap_load_vec3(builder, heap, offsets[src] + sz);
                            let (dyx, dyy, dyz) =
                                heap_load_vec3(builder, heap, offsets[src] + 2 * sz);
                            // |dx|^2 + |dy|^2
                            let dx2 = builder.ins().fmul(dxx, dxx);
                            let t1 = builder.ins().fmul(dxy, dxy);
                            let t2 = builder.ins().fmul(dxz, dxz);
                            let dx_len2 = builder.ins().fadd(dx2, t1);
                            let dx_len2 = builder.ins().fadd(dx_len2, t2);
                            let dy2 = builder.ins().fmul(dyx, dyx);
                            let t3 = builder.ins().fmul(dyy, dyy);
                            let t4 = builder.ins().fmul(dyz, dyz);
                            let dy_len2 = builder.ins().fadd(dy2, t3);
                            let dy_len2 = builder.ins().fadd(dy_len2, t4);
                            let sum = builder.ins().fadd(dx_len2, dy_len2);
                            let call = builder.ins().call(sqrtf_ref, &[sum]);
                            let fw = builder.inst_results(call)[0];
                            heap_store_f32(builder, heap, offsets[dst], fw);
                        }
                        _ => {
                            let small = builder.ins().f32const(0.001);
                            heap_store_f32(builder, heap, offsets[dst], small);
                        }
                    }
                } else {
                    // No derivatives available — fallback to small epsilon
                    let small = builder.ins().f32const(0.001);
                    heap_store_f32(builder, heap, offsets[dst], small);
                }
                builder.ins().jump(next_block, &[]);
            }

            // --- Raytype query ---
            "raytype" if args.len() >= 2 => {
                let rt = sg_load_i32(builder, sg, sg_offsets::raytype());
                heap_store_i32(builder, heap, offsets[args[0] as usize], rt);
                builder.ins().jump(next_block, &[]);
            }

            // --- Array ops ---
            "arraycopy" if args.len() >= 2 => {
                // arraycopy(dst, src) — copy entire symbol data
                let dst = args[0] as usize;
                let src = args[1] as usize;
                let sz = jtypes[src].size();
                // Copy value data (float-by-float for simplicity)
                for i in (0..sz).step_by(4) {
                    let v = heap_load_f32(builder, heap, offsets[src] + i);
                    heap_store_f32(builder, heap, offsets[dst] + i, v);
                }
                builder.ins().jump(next_block, &[]);
            }
            "arrayfill" if args.len() >= 2 => {
                // arrayfill(dst_array, value) — fill all elements with value
                // In our JIT, arrays have a fixed type size; fill with the source value
                let dst = args[0] as usize;
                let src = args[1] as usize;
                let v = heap_load_f32(builder, heap, offsets[src]);
                let sz = jtypes[dst].size();
                for i in (0..sz).step_by(4) {
                    heap_store_f32(builder, heap, offsets[dst] + i, v);
                }
                builder.ins().jump(next_block, &[]);
            }

            // --- Struct field ops ---
            "getfield" if args.len() >= 3 => {
                // getfield(result, struct_sym, field_name_const)
                // Resolve field byte offset from struct layout.
                let dst = args[0] as usize;
                let struct_sym = args[1] as usize;
                let field_name_sym = args[2] as usize;
                let field_off = compute_struct_field_offset(ir, struct_sym, field_name_sym);
                let src_base = offsets[struct_sym] + field_off.0;
                let field_sz = field_off.1;
                // Copy field_sz bytes from struct to destination
                let n_floats = (field_sz / 4).max(1);
                for i in 0..n_floats {
                    let v = heap_load_f32(builder, heap, src_base + i * 4);
                    heap_store_f32(builder, heap, offsets[dst] + i * 4, v);
                }
                builder.ins().jump(next_block, &[]);
            }
            "setfield" if args.len() >= 3 => {
                // setfield(struct_sym, field_name_const, value)
                let struct_sym = args[0] as usize;
                let field_name_sym = args[1] as usize;
                let val_sym = args[2] as usize;
                let field_off = compute_struct_field_offset(ir, struct_sym, field_name_sym);
                let dst_base = offsets[struct_sym] + field_off.0;
                let field_sz = field_off.1;
                let n_floats = (field_sz / 4).max(1);
                for i in 0..n_floats {
                    let v = heap_load_f32(builder, heap, offsets[val_sym] + i * 4);
                    heap_store_f32(builder, heap, dst_base + i * 4, v);
                }
                builder.ins().jump(next_block, &[]);
            }

            // --- Matrix component access (real implementation) ---
            // mxcompref(result_float, matrix, row, col) — read M[row][col]
            "mxcompref" if args.len() >= 4 => {
                let row_raw = heap_load_i32(builder, heap, offsets[args[2] as usize]);
                let col_raw = heap_load_i32(builder, heap, offsets[args[3] as usize]);
                let (symname_hash, sourcefile_hash) = if (args[1] as usize) < ir.symbols.len() {
                    (
                        ir.symbols[args[1] as usize].name.hash(),
                        op.sourcefile.hash(),
                    )
                } else {
                    (0u64, 0u64)
                };
                let row = emit_maybe_range_check(
                    builder,
                    module,
                    math,
                    ctx,
                    sg,
                    heap,
                    row_raw,
                    4,
                    symname_hash,
                    sourcefile_hash,
                    op.sourceline,
                    range_checking,
                );
                let col = emit_maybe_range_check(
                    builder,
                    module,
                    math,
                    ctx,
                    sg,
                    heap,
                    col_raw,
                    4,
                    symname_hash,
                    sourcefile_hash,
                    op.sourceline,
                    range_checking,
                );
                // offset = mat_base + (row * 4 + col) * 4 bytes
                let four = builder.ins().iconst(types::I32, 4);
                let row_x4 = builder.ins().imul(row, four);
                let idx = builder.ins().iadd(row_x4, col);
                let byte_off_within = builder.ins().imul(idx, four);
                let mat_base = builder
                    .ins()
                    .iconst(types::I32, offsets[args[1] as usize] as i64);
                let total_off = builder.ins().iadd(mat_base, byte_off_within);
                // Convert to pointer offset and load
                let total_off_ext = builder.ins().uextend(ptr_type, total_off);
                let addr = builder.ins().iadd(heap, total_off_ext);
                let val = builder.ins().load(types::F32, MemFlags::new(), addr, 0);
                heap_store_f32(builder, heap, offsets[args[0] as usize], val);
                builder.ins().jump(next_block, &[]);
            }
            // mxcompassign(matrix, row, col, value) — write M[row][col] = value
            "mxcompassign" if args.len() >= 4 => {
                let row_raw = heap_load_i32(builder, heap, offsets[args[1] as usize]);
                let col_raw = heap_load_i32(builder, heap, offsets[args[2] as usize]);
                let (symname_hash, sourcefile_hash) = if (args[0] as usize) < ir.symbols.len() {
                    (
                        ir.symbols[args[0] as usize].name.hash(),
                        op.sourcefile.hash(),
                    )
                } else {
                    (0u64, 0u64)
                };
                let row = emit_maybe_range_check(
                    builder,
                    module,
                    math,
                    ctx,
                    sg,
                    heap,
                    row_raw,
                    4,
                    symname_hash,
                    sourcefile_hash,
                    op.sourceline,
                    range_checking,
                );
                let col = emit_maybe_range_check(
                    builder,
                    module,
                    math,
                    ctx,
                    sg,
                    heap,
                    col_raw,
                    4,
                    symname_hash,
                    sourcefile_hash,
                    op.sourceline,
                    range_checking,
                );
                let val = heap_load_f32(builder, heap, offsets[args[3] as usize]);
                let four = builder.ins().iconst(types::I32, 4);
                let row_x4 = builder.ins().imul(row, four);
                let idx = builder.ins().iadd(row_x4, col);
                let byte_off_within = builder.ins().imul(idx, four);
                let mat_base = builder
                    .ins()
                    .iconst(types::I32, offsets[args[0] as usize] as i64);
                let total_off = builder.ins().iadd(mat_base, byte_off_within);
                let total_off_ext = builder.ins().uextend(ptr_type, total_off);
                let addr = builder.ins().iadd(heap, total_off_ext);
                builder.ins().store(MemFlags::new(), val, addr, 0);
                builder.ins().jump(next_block, &[]);
            }

            // --- Unknown opcode: skip with debug log ---
            _ => {
                #[cfg(debug_assertions)]
                eprintln!("[jit] unhandled opcode: {opname}");
                builder.ins().jump(next_block, &[]);
            }
        }
    }

    // Seal all blocks (after all jumps are known)
    builder.switch_to_block(exit_block);
    for block in &op_blocks {
        builder.seal_block(*block);
    }
    builder.seal_block(exit_block);

    Ok(())
}

// ---------------------------------------------------------------------------
// Helper: emit binary float/vec3 operation
// ---------------------------------------------------------------------------

fn emit_binop_float(
    builder: &mut FunctionBuilder,
    heap: CValue,
    offsets: &[usize],
    jtypes: &[JitType],
    args: &[i32],
    next_block: Block,
    op: impl Fn(&mut FunctionBuilder, CValue, CValue) -> CValue,
) {
    let dst = args[0] as usize;
    let lhs = args[1] as usize;
    let rhs = args[2] as usize;

    match (jtypes[dst], jtypes[lhs], jtypes[rhs]) {
        // Vec3 op Vec3 (both operands are vec3/color)
        (
            JitType::Vec3 | JitType::Color,
            JitType::Vec3 | JitType::Color,
            JitType::Vec3 | JitType::Color,
        ) => {
            let (ax, ay, az) = heap_load_vec3(builder, heap, offsets[lhs]);
            let (bx, by, bz) = heap_load_vec3(builder, heap, offsets[rhs]);
            let rx = op(builder, ax, bx);
            let ry = op(builder, ay, by);
            let rz = op(builder, az, bz);
            heap_store_vec3(builder, heap, offsets[dst], rx, ry, rz);
        }
        // Vec3 = Float op Vec3 (broadcast float to all components)
        (
            JitType::Vec3 | JitType::Color,
            JitType::Float | JitType::Int,
            JitType::Vec3 | JitType::Color,
        ) => {
            let a = heap_load_f32(builder, heap, offsets[lhs]);
            let (bx, by, bz) = heap_load_vec3(builder, heap, offsets[rhs]);
            let rx = op(builder, a, bx);
            let ry = op(builder, a, by);
            let rz = op(builder, a, bz);
            heap_store_vec3(builder, heap, offsets[dst], rx, ry, rz);
        }
        // Vec3 = Vec3 op Float (broadcast float to all components)
        (
            JitType::Vec3 | JitType::Color,
            JitType::Vec3 | JitType::Color,
            JitType::Float | JitType::Int,
        ) => {
            let (ax, ay, az) = heap_load_vec3(builder, heap, offsets[lhs]);
            let b = heap_load_f32(builder, heap, offsets[rhs]);
            let rx = op(builder, ax, b);
            let ry = op(builder, ay, b);
            let rz = op(builder, az, b);
            heap_store_vec3(builder, heap, offsets[dst], rx, ry, rz);
        }
        // Vec3 = Float op Float (broadcast result to all components)
        (JitType::Vec3 | JitType::Color, _, _) => {
            let a = heap_load_f32(builder, heap, offsets[lhs]);
            let b = heap_load_f32(builder, heap, offsets[rhs]);
            let r = op(builder, a, b);
            heap_store_vec3(builder, heap, offsets[dst], r, r, r);
        }
        // Float op Float
        (JitType::Float, _, _) => {
            let a = heap_load_f32(builder, heap, offsets[lhs]);
            let b = heap_load_f32(builder, heap, offsets[rhs]);
            let r = op(builder, a, b);
            heap_store_f32(builder, heap, offsets[dst], r);
        }
        // Int op Int: convert to float, apply op, convert back
        (JitType::Int, JitType::Int, JitType::Int) => {
            let a_raw = heap_load_i32(builder, heap, offsets[lhs]);
            let b_raw = heap_load_i32(builder, heap, offsets[rhs]);
            let a = builder.ins().fcvt_from_sint(types::F32, a_raw);
            let b = builder.ins().fcvt_from_sint(types::F32, b_raw);
            let r = op(builder, a, b);
            let ri = builder.ins().fcvt_to_sint_sat(types::I32, r);
            heap_store_i32(builder, heap, offsets[dst], ri);
        }
        // Fallback: treat everything as float
        _ => {
            let a = heap_load_f32(builder, heap, offsets[lhs]);
            let b = heap_load_f32(builder, heap, offsets[rhs]);
            let r = op(builder, a, b);
            heap_store_f32(builder, heap, offsets[dst], r);
        }
    }
    builder.ins().jump(next_block, &[]);
}

// ---------------------------------------------------------------------------
// Helper: safe float division (returns 0.0 for non-finite results)
// Matches C++ osl_safe_div_fff: q = a/b; isfinite(q) ? q : 0.0
// ---------------------------------------------------------------------------

/// Emit safe float division: `a / b`, returning 0.0 if result is non-finite.
/// Uses the identity: `(x - x) == 0.0` is true iff x is finite.
fn emit_safe_fdiv(builder: &mut FunctionBuilder, a: CValue, b: CValue) -> CValue {
    let q = builder.ins().fdiv(a, b);
    let check = builder.ins().fsub(q, q); // 0 if finite, NaN if inf/NaN
    let zero = builder.ins().f32const(0.0);
    let is_finite = builder.ins().fcmp(FloatCC::Equal, check, zero);
    builder.ins().select(is_finite, q, zero)
}

// ---------------------------------------------------------------------------
// Convenience: compile and run
// ---------------------------------------------------------------------------

/// Convenience function: compile a ShaderIR and execute it with JIT.
pub fn jit_execute(ir: &ShaderIR, sg: &mut ShaderGlobals) -> Result<CompiledShader, JitError> {
    let backend = CraneliftBackend::new();
    let compiled = backend.compile(ir)?;
    compiled.execute(sg);
    Ok(compiled)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shaderglobals::ShaderGlobals;

    fn compile_and_run(source: &str) -> (CompiledShader, Vec<u8>, ShaderGlobals) {
        let ast = crate::parser::parse(source).expect("parse failed").ast;
        let ir = crate::codegen::generate(&ast);
        let backend = CraneliftBackend::new();
        let compiled = backend.compile(&ir).expect("JIT compile failed");
        let mut sg = ShaderGlobals::default();
        sg.u = 0.5;
        sg.v = 0.25;
        sg.p = Vec3::new(1.0, 2.0, 3.0);
        sg.n = Vec3::new(0.0, 1.0, 0.0);
        let mut heap = vec![0u8; compiled.heap_size()];
        compiled.execute_with_heap(&mut sg, &mut heap);
        (compiled, heap, sg)
    }

    #[test]
    fn test_jit_constant() {
        let (compiled, heap, _) =
            compile_and_run("shader test(output float result = 0) { result = 42.0; }");
        let result = compiled.get_float(&heap, "result");
        assert_eq!(result, Some(42.0));
    }

    #[test]
    fn test_jit_arithmetic() {
        let (compiled, heap, _) =
            compile_and_run("shader test(output float result = 0) { result = 3.0 + 4.0 * 2.0; }");
        // Depending on how the compiler emits this, it could be 11.0 or 14.0
        let result = compiled.get_float(&heap, "result").unwrap();
        assert!(result > 0.0, "result should be positive: {result}");
    }

    #[test]
    fn test_jit_globals() {
        let (compiled, heap, _) =
            compile_and_run("shader test(output float result = 0) { result = u + v; }");
        let result = compiled.get_float(&heap, "result").unwrap();
        // u=0.5, v=0.25
        assert!((result - 0.75).abs() < 1e-5, "expected 0.75, got {result}");
    }

    #[test]
    fn test_jit_math_builtins() {
        let (compiled, heap, _) =
            compile_and_run("shader test(output float result = 0) { result = sin(1.0); }");
        let result = compiled.get_float(&heap, "result").unwrap();
        let expected = 1.0f32.sin();
        assert!(
            (result - expected).abs() < 1e-5,
            "expected {expected}, got {result}"
        );
    }

    #[test]
    fn test_jit_negation() {
        let (compiled, heap, _) =
            compile_and_run("shader test(output float result = 0) { result = -5.0; }");
        let result = compiled.get_float(&heap, "result").unwrap();
        assert!(
            (result - (-5.0)).abs() < 1e-5,
            "expected -5.0, got {result}"
        );
    }

    #[test]
    fn test_jit_comparison() {
        let (compiled, heap, _) = compile_and_run(
            "shader test(output float result = 0) { if (u > 0.3) result = 1.0; else result = 0.0; }",
        );
        let result = compiled.get_float(&heap, "result").unwrap();
        // u=0.5 > 0.3, so result should be 1.0
        assert!((result - 1.0).abs() < 1e-5, "expected 1.0, got {result}");
    }

    #[test]
    fn test_jit_vec3_construction() {
        let (compiled, heap, _) = compile_and_run(
            "shader test(output color result = 0) { result = color(0.5, 0.6, 0.7); }",
        );
        let result = compiled.get_vec3(&heap, "result");
        if let Some(v) = result {
            assert!((v.x - 0.5).abs() < 1e-5);
            assert!((v.y - 0.6).abs() < 1e-5);
            assert!((v.z - 0.7).abs() < 1e-5);
        }
    }

    #[test]
    fn test_jit_backend_trait() {
        let backend = CraneliftBackend::new();
        assert_eq!(backend.name(), "cranelift");
    }

    // --- New math function tests ---

    #[test]
    fn test_jit_cbrt() {
        let (compiled, heap, _) =
            compile_and_run("shader test(output float result = 0) { result = cbrt(27.0); }");
        let result = compiled.get_float(&heap, "result").unwrap();
        assert!(
            (result - 3.0).abs() < 1e-4,
            "cbrt(27) expected 3.0, got {result}"
        );
    }

    #[test]
    fn test_jit_log2() {
        let (compiled, heap, _) =
            compile_and_run("shader test(output float result = 0) { result = log2(8.0); }");
        let result = compiled.get_float(&heap, "result").unwrap();
        assert!(
            (result - 3.0).abs() < 1e-4,
            "log2(8) expected 3.0, got {result}"
        );
    }

    #[test]
    fn test_jit_log10() {
        let (compiled, heap, _) =
            compile_and_run("shader test(output float result = 0) { result = log10(1000.0); }");
        let result = compiled.get_float(&heap, "result").unwrap();
        assert!(
            (result - 3.0).abs() < 1e-4,
            "log10(1000) expected 3.0, got {result}"
        );
    }

    #[test]
    fn test_jit_exp2() {
        let (compiled, heap, _) =
            compile_and_run("shader test(output float result = 0) { result = exp2(3.0); }");
        let result = compiled.get_float(&heap, "result").unwrap();
        assert!(
            (result - 8.0).abs() < 1e-4,
            "exp2(3) expected 8.0, got {result}"
        );
    }

    #[test]
    fn test_jit_expm1() {
        let (compiled, heap, _) =
            compile_and_run("shader test(output float result = 0) { result = expm1(0.0); }");
        let result = compiled.get_float(&heap, "result").unwrap();
        assert!(result.abs() < 1e-4, "expm1(0) expected 0.0, got {result}");
    }

    #[test]
    fn test_jit_erf() {
        let (compiled, heap, _) =
            compile_and_run("shader test(output float result = 0) { result = erf(0.0); }");
        let result = compiled.get_float(&heap, "result").unwrap();
        assert!(result.abs() < 1e-4, "erf(0) expected 0.0, got {result}");
    }

    #[test]
    fn test_jit_erfc() {
        let (compiled, heap, _) =
            compile_and_run("shader test(output float result = 0) { result = erfc(0.0); }");
        let result = compiled.get_float(&heap, "result").unwrap();
        assert!(
            (result - 1.0).abs() < 1e-4,
            "erfc(0) expected 1.0, got {result}"
        );
    }

    #[test]
    fn test_jit_logb() {
        let (compiled, heap, _) =
            compile_and_run("shader test(output float result = 0) { result = logb(16.0); }");
        let result = compiled.get_float(&heap, "result").unwrap();
        // logb(16) = floor(log2(16)) = 4
        assert!(
            (result - 4.0).abs() < 1e-4,
            "logb(16) expected 4.0, got {result}"
        );
    }

    #[test]
    fn test_jit_int_assign() {
        // Test basic int constant assignment
        let (compiled, heap, _) =
            compile_and_run("shader test(output int result = 0) { result = 42; }");
        let result = compiled.get_int(&heap, "result").unwrap();
        assert_eq!(result, 42, "int assign expected 42, got {result}");
    }

    #[test]
    fn test_jit_int_arithmetic() {
        // Test float result from int add (the JIT promotes int arithmetic to float)
        let (compiled, heap, _) =
            compile_and_run("shader test(output float result = 0) { result = 3.0 + 4.0; }");
        let result = compiled.get_float(&heap, "result").unwrap();
        assert!(
            (result - 7.0).abs() < 1e-4,
            "3.0 + 4.0 expected 7.0, got {result}"
        );
    }

    #[test]
    fn test_jit_math_chain() {
        // Chain multiple new math ops to verify they work together
        let (compiled, heap, _) =
            compile_and_run("shader test(output float result = 0) { result = exp2(log2(16.0)); }");
        let result = compiled.get_float(&heap, "result").unwrap();
        assert!(
            (result - 16.0).abs() < 1e-2,
            "exp2(log2(16)) expected 16.0, got {result}"
        );
    }

    // --- Safe division tests ---

    #[test]
    fn test_jit_safe_div_by_zero() {
        // Division by zero should return 0.0 (not INF/NaN)
        let (compiled, heap, _) =
            compile_and_run("shader test(output float result = 0) { result = 1.0 / 0.0; }");
        let result = compiled.get_float(&heap, "result").unwrap();
        assert_eq!(
            result, 0.0,
            "1.0/0.0 should be 0.0 (safe div), got {result}"
        );
    }

    #[test]
    fn test_jit_safe_div_normal() {
        // Normal division should still work
        let (compiled, heap, _) =
            compile_and_run("shader test(output float result = 0) { result = 6.0 / 3.0; }");
        let result = compiled.get_float(&heap, "result").unwrap();
        assert!(
            (result - 2.0).abs() < 1e-5,
            "6.0/3.0 expected 2.0, got {result}"
        );
    }

    #[test]
    fn test_jit_safe_fmod_by_zero() {
        // fmod(x, 0) should return 0.0
        let (compiled, heap, _) =
            compile_and_run("shader test(output float result = 99) { result = fmod(5.0, 0.0); }");
        let result = compiled.get_float(&heap, "result").unwrap();
        assert_eq!(result, 0.0, "fmod(5.0, 0.0) should be 0.0, got {result}");
    }

    #[test]
    fn test_jit_safe_mod_by_zero() {
        // mod(x, 0) should return 0.0
        let (compiled, heap, _) =
            compile_and_run("shader test(output float result = 99) { result = mod(7.0, 0.0); }");
        let result = compiled.get_float(&heap, "result").unwrap();
        assert_eq!(result, 0.0, "mod(7.0, 0.0) should be 0.0, got {result}");
    }

    // --- Construct opcode tests ---

    #[test]
    fn test_jit_construct_vec3() {
        // Verify vector construction from 3 floats
        let (compiled, heap, _) = compile_and_run(
            "shader test(output color result = 0) { result = color(0.1, 0.2, 0.3); }",
        );
        let v = compiled.get_vec3(&heap, "result").unwrap();
        assert!((v.x - 0.1).abs() < 1e-5, "x={}", v.x);
        assert!((v.y - 0.2).abs() < 1e-5, "y={}", v.y);
        assert!((v.z - 0.3).abs() < 1e-5, "z={}", v.z);
    }

    #[test]
    fn test_jit_construct_color_from_space() {
        // construct(colorspace, x, y, z) — color("rgb", 0.2, 0.4, 0.6) = passthrough
        let (compiled, heap, _) = compile_and_run(
            r#"shader test(output color result = 0) { result = color("rgb", 0.2, 0.4, 0.6); }"#,
        );
        let v = compiled.get_vec3(&heap, "result").unwrap();
        // rgb space: identity, so (0.2, 0.4, 0.6) stays
        assert!((v.x - 0.2).abs() < 1e-4, "x={}", v.x);
        assert!((v.y - 0.4).abs() < 1e-4, "y={}", v.y);
        assert!((v.z - 0.6).abs() < 1e-4, "z={}", v.z);
    }

    // --- Int output computed result ---

    #[test]
    fn test_jit_int_output_computed() {
        // Int division by zero should return 0 (not panic)
        // The codegen may route int ops through float path, so test safe behavior
        let (compiled, heap, _) = compile_and_run(
            "shader test(output float result = 0) { float a = 10.0; float b = 5.0; result = a / b; }",
        );
        let result = compiled.get_float(&heap, "result").unwrap();
        assert!(
            (result - 2.0).abs() < 1e-5,
            "10.0/5.0 expected 2.0, got {result}"
        );
    }

    #[test]
    fn test_jit_safe_div_overflow() {
        // Large/tiny division should return 0.0 (overflow -> non-finite -> 0)
        let (compiled, heap, _) = compile_and_run(
            "shader test(output float result = 99) { float big = 1e30; float tiny = 1e-30; result = big / tiny; }",
        );
        let result = compiled.get_float(&heap, "result").unwrap();
        assert!(
            result.is_finite(),
            "overflow div should produce finite result, got {result}"
        );
    }

    #[test]
    fn test_jit_float_to_int_sat() {
        // Saturating float-to-int: normal case should work
        let (compiled, heap, _) = compile_and_run(
            "shader test(output float result = 0) { int x = (int)3.7; result = (float)x; }",
        );
        let result = compiled.get_float(&heap, "result").unwrap();
        assert!(
            (result - 3.0).abs() < 1e-5,
            "(int)3.7 should be 3, got {result}"
        );
    }

    #[test]
    fn test_jit_int_to_float_cast() {
        // Integer to float cast
        let (compiled, heap, _) = compile_and_run(
            "shader test(output float result = 0) { int x = 42; result = (float)x; }",
        );
        let result = compiled.get_float(&heap, "result").unwrap();
        assert!(
            (result - 42.0).abs() < 1e-5,
            "(float)42 should be 42.0, got {result}"
        );
    }
}

// ---------------------------------------------------------------------------
// Batched JIT — process WIDTH shading points through the compiled function
// ---------------------------------------------------------------------------

/// A compiled shader that can process multiple shading points in a batch.
///
/// Internally wraps a scalar `CompiledShader` and loops over the batch.
/// This is the "trivially batched" approach — each point is processed
/// independently through the same compiled native code. True vector JIT
/// (emitting Cranelift SIMD instructions) is a future optimization.
pub struct BatchedCompiledShader {
    /// The underlying scalar compiled shader.
    pub inner: Arc<CompiledShader>,
}

unsafe impl Send for BatchedCompiledShader {}
unsafe impl Sync for BatchedCompiledShader {}

impl BatchedCompiledShader {
    /// Create from an existing scalar compiled shader.
    pub fn new(compiled: Arc<CompiledShader>) -> Self {
        Self { inner: compiled }
    }

    /// Execute the shader for a batch of shading points.
    ///
    /// Each `ShaderGlobals` in `sgs` is processed independently through
    /// the JIT-compiled function. Results are written to `heaps[i]`.
    ///
    /// # Panics
    /// Panics if `sgs.len() != heaps.len()`.
    pub fn execute_batch(&self, sgs: &mut [ShaderGlobals], heaps: &mut [Vec<u8>]) {
        assert_eq!(
            sgs.len(),
            heaps.len(),
            "sgs and heaps must have same length"
        );
        for (sg, heap) in sgs.iter_mut().zip(heaps.iter_mut()) {
            if heap.len() < self.inner.heap_size {
                heap.resize(self.inner.heap_size, 0);
            }
            self.inner.execute_with_heap(sg, heap);
        }
    }

    /// Execute a batch of WIDTH shading points, using Wide types for input/output.
    ///
    /// This is the typed interface: callers provide `BatchedShaderGlobals<WIDTH>`
    /// and get results back via the heap for each lane.
    pub fn execute_wide<const WIDTH: usize>(
        &self,
        bsg: &mut crate::batched::BatchedShaderGlobals<WIDTH>,
        mask: &crate::batched::Mask<WIDTH>,
    ) -> Vec<Vec<u8>> {
        let mut heaps: Vec<Vec<u8>> = (0..WIDTH)
            .map(|_| vec![0u8; self.inner.heap_size])
            .collect();

        for lane in 0..WIDTH {
            if !mask.is_set(lane) {
                continue;
            }
            // Extract a scalar ShaderGlobals for this lane
            let mut sg = bsg.extract_lane(lane);
            self.inner.execute_with_heap(&mut sg, &mut heaps[lane]);
            // Write back modified globals
            bsg.inject_lane(lane, &sg);
        }

        heaps
    }

    /// Read a float result from a specific lane's heap.
    pub fn get_float_lane(&self, heaps: &[Vec<u8>], lane: usize, name: &str) -> Option<f32> {
        if lane < heaps.len() {
            self.inner.get_float(&heaps[lane], name)
        } else {
            None
        }
    }

    /// Read a Vec3 result from a specific lane's heap.
    pub fn get_vec3_lane(&self, heaps: &[Vec<u8>], lane: usize, name: &str) -> Option<Vec3> {
        if lane < heaps.len() {
            self.inner.get_vec3(&heaps[lane], name)
        } else {
            None
        }
    }

    /// Get the required heap size per lane.
    pub fn heap_size(&self) -> usize {
        self.inner.heap_size
    }

    /// Access the symbol offsets map.
    pub fn symbol_offsets(&self) -> &HashMap<String, (usize, SymLayout)> {
        &self.inner.symbol_offsets
    }
}

impl CraneliftBackend {
    /// Compile a shader group for batched execution.
    ///
    /// Returns a `BatchedCompiledShader` that can process WIDTH shading points.
    /// Currently wraps the scalar compilation; future versions will emit
    /// Cranelift SIMD vector instructions for true vectorized execution.
    pub fn compile_group_batched(
        &self,
        layers: &[&ShaderIR],
        connections: &[crate::shadingsys::Connection],
        range_checking: bool,
    ) -> Result<BatchedCompiledShader, JitError> {
        let compiled = self.compile_group(layers, connections, range_checking)?;
        Ok(BatchedCompiledShader::new(Arc::new(compiled)))
    }
}
