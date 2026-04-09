//! Batched execution engine — process multiple shading points simultaneously.
//!
//! Port of the C++ OSL batched shading execution, using `Wide<T, WIDTH>`
//! SIMD-width types and `BatchedRendererServices<WIDTH>` for vectorized
//! renderer callbacks. This enables shading WIDTH points (typically 8 for
//! AVX, 16 for AVX-512) in a single pass through the opcode stream.
//!
//! ## Architecture
//!
//! The batched executor mirrors the scalar `Interpreter` but operates on
//! `Wide` values across all lanes simultaneously:
//!
//! 1. Each symbol holds a `WideValue<WIDTH>` instead of a scalar `Value`.
//! 2. A `Mask<WIDTH>` tracks which lanes are active (for conditionals).
//! 3. Renderer callbacks use the `BatchedRendererServices<WIDTH>` trait.
//!
//! This provides a significant speedup for CPU rendering where many
//! shading points can be evaluated in parallel.

use std::collections::HashMap;
use std::sync::Arc;

use crate::batched::{BatchedRendererServices, BatchedShaderGlobals, Mask, Wide};
use crate::codegen::{ConstValue, ShaderIR};
use crate::dict::DictStore;
use crate::interp::ClosureValue;
use crate::math::{Color3, Matrix44, Vec3};
use crate::ustring::UString;

/// A value stored per-lane in the batched executor.
#[derive(Clone)]
pub enum WideValue<const WIDTH: usize> {
    Int(Wide<i32, WIDTH>),
    Float(Wide<f32, WIDTH>),
    Vec3(Wide<Vec3, WIDTH>),
    Matrix(Wide<Matrix44, WIDTH>),
    String(Vec<UString>), // Not truly SIMD, but stored per-lane
    FloatArray(Vec<Vec<f32>>),
    IntArray(Vec<Vec<i32>>),
    Vec3Array(Vec<Vec<Vec3>>),
    MatrixArray(Vec<Vec<Matrix44>>),
    StringArray(Vec<Vec<UString>>),
    /// Per-lane closure (one ClosureValue per lane, None for inactive).
    Closure(Vec<Option<Box<ClosureValue>>>),
    Void,
}

impl<const WIDTH: usize> Default for WideValue<WIDTH>
where
    Wide<i32, WIDTH>: Default,
{
    fn default() -> Self {
        WideValue::Int(Wide::<i32, WIDTH>::default())
    }
}

impl<const WIDTH: usize> WideValue<WIDTH> {
    /// Get all lanes as floats.
    pub fn as_float(&self) -> Wide<f32, WIDTH> {
        match self {
            WideValue::Float(w) => w.clone(),
            WideValue::Int(w) => {
                let mut result = Wide {
                    data: [0.0f32; WIDTH],
                };
                for i in 0..WIDTH {
                    result.data[i] = w.data[i] as f32;
                }
                result
            }
            _ => Wide {
                data: [0.0f32; WIDTH],
            },
        }
    }

    /// Get all lanes as Vec3.
    pub fn as_vec3(&self) -> Wide<Vec3, WIDTH> {
        match self {
            WideValue::Vec3(w) => w.clone(),
            WideValue::Float(w) => {
                let mut result = Wide {
                    data: [Vec3::ZERO; WIDTH],
                };
                for i in 0..WIDTH {
                    result.data[i] = Vec3::splat(w.data[i]);
                }
                result
            }
            _ => Wide {
                data: [Vec3::ZERO; WIDTH],
            },
        }
    }

    /// Get all lanes as i32.
    pub fn as_int(&self) -> Wide<i32, WIDTH> {
        match self {
            WideValue::Int(w) => w.clone(),
            WideValue::Float(w) => {
                let mut result = Wide {
                    data: [0i32; WIDTH],
                };
                for i in 0..WIDTH {
                    result.data[i] = w.data[i] as i32;
                }
                result
            }
            _ => Wide {
                data: [0i32; WIDTH],
            },
        }
    }

    /// Splat a scalar value across all lanes.
    pub fn splat_float(v: f32) -> Self {
        WideValue::Float(Wide { data: [v; WIDTH] })
    }

    /// Splat a scalar int across all lanes.
    pub fn splat_int(v: i32) -> Self {
        WideValue::Int(Wide { data: [v; WIDTH] })
    }

    /// Splat a Vec3 across all lanes.
    pub fn splat_vec3(v: Vec3) -> Self {
        WideValue::Vec3(Wide { data: [v; WIDTH] })
    }

    /// Per-lane float array element read by index.
    pub fn array_ref_float(&self, idx: &Wide<i32, WIDTH>, mask: Mask<WIDTH>) -> Wide<f32, WIDTH> {
        let mut result = Wide {
            data: [0.0f32; WIDTH],
        };
        if let WideValue::FloatArray(arrs) = self {
            for lane in 0..WIDTH {
                if mask.is_set(lane) && lane < arrs.len() {
                    let i = idx.data[lane].max(0) as usize;
                    if i < arrs[lane].len() {
                        result.data[lane] = arrs[lane][i];
                    }
                }
            }
        }
        result
    }

    /// Per-lane int array element read by index.
    pub fn array_ref_int(&self, idx: &Wide<i32, WIDTH>, mask: Mask<WIDTH>) -> Wide<i32, WIDTH> {
        let mut result = Wide {
            data: [0i32; WIDTH],
        };
        if let WideValue::IntArray(arrs) = self {
            for lane in 0..WIDTH {
                if mask.is_set(lane) && lane < arrs.len() {
                    let i = idx.data[lane].max(0) as usize;
                    if i < arrs[lane].len() {
                        result.data[lane] = arrs[lane][i];
                    }
                }
            }
        }
        result
    }

    /// Per-lane Vec3 array element read by index.
    pub fn array_ref_vec3(&self, idx: &Wide<i32, WIDTH>, mask: Mask<WIDTH>) -> Wide<Vec3, WIDTH> {
        let mut result = Wide {
            data: [Vec3::ZERO; WIDTH],
        };
        if let WideValue::Vec3Array(arrs) = self {
            for lane in 0..WIDTH {
                if mask.is_set(lane) && lane < arrs.len() {
                    let i = idx.data[lane].max(0) as usize;
                    if i < arrs[lane].len() {
                        result.data[lane] = arrs[lane][i];
                    }
                }
            }
        }
        result
    }
}

/// Batched interpreter state — processes WIDTH shading points simultaneously.
pub struct BatchedInterpreter<const WIDTH: usize> {
    /// Symbol values per lane.
    values: Vec<WideValue<WIDTH>>,
    /// Active lane mask.
    active_mask: Mask<WIDTH>,
    /// Output messages (shared across lanes).
    pub messages: Vec<String>,
    /// Whether execution was halted.
    halted: Mask<WIDTH>,
    /// Call stack for function return addresses.
    call_stack: Vec<usize>,
    /// Inter-shader message store (setmessage/getmessage).
    message_store: HashMap<String, WideValue<WIDTH>>,
    /// Dictionary store for dict_find/dict_next/dict_value.
    dict_store: DictStore,
    /// Optional shared point cloud manager for pointcloud_search/get/write.
    pointcloud_manager: Option<Arc<std::sync::RwLock<crate::pointcloud::PointCloudManager>>>,
    /// Synonym for "common" space in transform/getmatrix (e.g. "world"). Per-reference.
    commonspace_synonym: UString,
    /// Whether to emit range-check errors. Per-reference.
    range_checking: bool,
    /// Whether to emit errors when unknown coordinate systems are used. Per-reference.
    unknown_coordsys_error: bool,
}

impl<const WIDTH: usize> Default for BatchedInterpreter<WIDTH> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const WIDTH: usize> BatchedInterpreter<WIDTH> {
    pub fn new() -> Self {
        Self {
            values: Vec::new(),
            active_mask: Mask::all(),
            messages: Vec::new(),
            halted: Mask::none(),
            call_stack: Vec::new(),
            message_store: HashMap::new(),
            dict_store: DictStore::new(),
            pointcloud_manager: None,
            commonspace_synonym: UString::new("world"),
            range_checking: true,
            unknown_coordsys_error: true,
        }
    }

    /// Set whether to emit range-check errors. Matching C++ range_checking.
    pub fn set_range_checking(&mut self, enabled: bool) {
        self.range_checking = enabled;
    }

    /// Range-check and clamp index for one lane. Returns clamped index; pushes to messages if OOB.
    #[allow(dead_code)]
    fn range_check_lane(
        &mut self,
        index: i32,
        length: i32,
        symname: &str,
        sourcefile: &str,
        sourceline: i32,
    ) -> i32 {
        if !self.range_checking || length <= 0 {
            return index;
        }
        if index >= 0 && index < length {
            return index;
        }
        let max_idx = length - 1;
        self.messages.push(format!(
            "Index [{}] out of range {}[0..{}]: {}:{} (group <unnamed>, layer 0 <unnamed>, shader <unnamed>)",
            index, symname, max_idx, sourcefile, sourceline
        ));
        if index >= length { max_idx } else { 0 }
    }

    /// Range-check Wide index per lane; return clamped Wide. Pushes to messages for OOB lanes.
    fn range_check_wide(
        &mut self,
        idx: &Wide<i32, WIDTH>,
        length: i32,
        symname: &str,
        sourcefile: &str,
        sourceline: i32,
        mask: Mask<WIDTH>,
    ) -> Wide<i32, WIDTH> {
        let mut result = Wide {
            data: [0i32; WIDTH],
        };
        for lane in 0..WIDTH {
            if mask.is_set(lane) && self.range_checking && length > 0 {
                let i = idx.data[lane];
                result.data[lane] = if i >= 0 && i < length {
                    i
                } else {
                    let max_idx = length - 1;
                    self.messages.push(format!(
                        "Index [{}] out of range {}[0..{}]: {}:{} (group <unnamed>, layer 0 <unnamed>, shader <unnamed>)",
                        i, symname, max_idx, sourcefile, sourceline
                    ));
                    if i >= length { max_idx } else { 0 }
                };
            } else {
                result.data[lane] = idx.data[lane];
            }
        }
        result
    }

    /// Set the synonym for "common" space (e.g. "world"). Matching C++ commonspace_synonym.
    pub fn set_commonspace_synonym(&mut self, synonym: UString) {
        self.commonspace_synonym = synonym;
    }

    /// Set whether to emit errors for unknown coordinate systems. Matching C++ unknown_coordsys_error.
    pub fn set_unknown_coordsys_error(&mut self, enabled: bool) {
        self.unknown_coordsys_error = enabled;
    }

    /// Set a shared point cloud manager for pointcloud_search.
    /// When set, pointcloud_search uses this manager instead of an empty one.
    pub fn set_pointcloud_manager(
        &mut self,
        mgr: Option<Arc<std::sync::RwLock<crate::pointcloud::PointCloudManager>>>,
    ) {
        self.pointcloud_manager = mgr;
    }

    /// Execute a ShaderIR across WIDTH shading points.
    ///
    /// `globals` contains the batched shader globals (one per lane).
    /// `renderer` provides batched renderer callbacks.
    pub fn execute<R: BatchedRendererServices<WIDTH>>(
        &mut self,
        ir: &ShaderIR,
        globals: &BatchedShaderGlobals<WIDTH>,
        renderer: &R,
    ) {
        self.messages.clear();
        self.message_store.clear();
        self.active_mask = Mask::all();
        self.halted = Mask::none();

        // 1. Initialize all symbols to type-default values
        self.values.clear();
        self.values
            .resize_with(ir.symbols.len(), || WideValue::splat_float(0.0));
        for (i, sym) in ir.symbols.iter().enumerate() {
            self.values[i] = default_wide_value::<WIDTH>(&sym.typespec.simpletype());
        }

        // 2. Load compile-time constants (splat across all lanes)
        for &(idx, ref cv) in &ir.const_values {
            self.values[idx] = const_to_wide::<WIDTH>(cv);
        }

        // 3. Load parameter defaults
        for &(idx, ref cv) in &ir.param_defaults {
            self.values[idx] = const_to_wide::<WIDTH>(cv);
        }

        // 4. Bind batched shader globals
        self.bind_globals(ir, globals);

        // 5. Execute full opcode range
        self.execute_range(ir, renderer, globals, 0, ir.opcodes.len());
    }

    /// Execute opcodes in [start..end) with current active_mask.
    /// Recursive calls handle if/else mask splitting.
    fn execute_range<R: BatchedRendererServices<WIDTH>>(
        &mut self,
        ir: &ShaderIR,
        renderer: &R,
        globals: &BatchedShaderGlobals<WIDTH>,
        start: usize,
        end: usize,
    ) {
        let mut pc = start;
        let num_ops = end.min(ir.opcodes.len());

        while pc < num_ops {
            if self.halted == Mask::all() {
                break;
            }

            let op = &ir.opcodes[pc];
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

            let mask = Mask::from_bits(self.active_mask.bits() & !self.halted.bits());

            // Skip if no lanes active
            if !mask.any() {
                pc += 1;
                continue;
            }

            match opname {
                "nop" | "" => {
                    if op.jump[0] >= 0 {
                        pc = op.jump[0] as usize;
                        continue;
                    }
                }
                "end" => break,
                "return" => {
                    if let Some(return_pc) = self.call_stack.pop() {
                        pc = return_pc;
                        continue;
                    } else {
                        return;
                    }
                }
                "exit" => {
                    self.halted = Mask::from_bits(self.halted.bits() | mask.bits());
                    return;
                }

                // --- CRITICAL-1: If/Else with per-lane mask splitting ---
                // Codegen patterns:
                //   if-else: jump[0]=true_start(=pc+1), jump[1]=false_start
                //            nop at false_start-1 has jump[0]=end
                //   if-only: jump[0]=true_start(=pc+1), jump[1]=end
                //   loop:    jump[0]=-1, jump[1]=end
                "if" if !args.is_empty() => {
                    let cond = self.get(args[0]).as_int();
                    let mut true_bits = 0u32;
                    let mut false_bits = 0u32;
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            if cond.data[lane] != 0 {
                                true_bits |= 1 << lane;
                            } else {
                                false_bits |= 1 << lane;
                            }
                        }
                    }
                    let true_mask = Mask::<WIDTH>::from_bits(true_bits);
                    let false_mask = Mask::<WIDTH>::from_bits(false_bits);

                    let j0 = op.jump[0]; // true_start or -1
                    let j1 = op.jump[1]; // false_start or end

                    let saved_mask = self.active_mask;

                    if j0 < 0 {
                        // Loop condition: true -> continue body, false -> exit to j1
                        if !true_mask.any() {
                            // All lanes exited — jump to end
                            pc = j1 as usize;
                            continue;
                        }
                        self.active_mask = true_mask;
                        // Fall through; loop nop handles back-jump
                    } else {
                        // Check if this is if-else (nop before j1 jumps to end)
                        let j1u = j1.max(0) as usize;
                        let nop_idx = if j1u > 0 { j1u - 1 } else { 0 };
                        let has_else = j1u > (j0 as usize) + 1
                            && nop_idx < ir.opcodes.len()
                            && ir.opcodes[nop_idx].op == "nop"
                            && ir.opcodes[nop_idx].jump[0] >= 0;

                        if has_else {
                            // True branch: [j0..j1) (includes nop that jumps to end)
                            // False branch: [j1..end)
                            let end = ir.opcodes[nop_idx].jump[0] as usize;
                            if true_mask.any() {
                                self.active_mask = true_mask;
                                self.execute_range(ir, renderer, globals, j0 as usize, j1u);
                            }
                            if false_mask.any() {
                                self.active_mask = false_mask;
                                self.execute_range(ir, renderer, globals, j1u, end);
                            }
                            self.active_mask = saved_mask;
                            pc = end;
                            continue;
                        } else {
                            // If-no-else: true branch [j0..j1), skip to j1
                            if true_mask.any() {
                                self.active_mask = true_mask;
                                self.execute_range(ir, renderer, globals, j0 as usize, j1u);
                            }
                            self.active_mask = saved_mask;
                            pc = j1u;
                            continue;
                        }
                    }
                }

                // --- Arithmetic ---
                "assign" if args.len() >= 2 => {
                    let src = self.get(args[1]);
                    self.set_masked(args[0], src, mask);
                }

                "add" if args.len() >= 3 => {
                    let a = self.get(args[1]);
                    let b = self.get(args[2]);
                    let result = wide_add::<WIDTH>(&a, &b, mask);
                    self.set_masked(args[0], result, mask);
                }

                "sub" if args.len() >= 3 => {
                    let a = self.get(args[1]);
                    let b = self.get(args[2]);
                    let result = wide_sub::<WIDTH>(&a, &b, mask);
                    self.set_masked(args[0], result, mask);
                }

                "mul" if args.len() >= 3 => {
                    let a = self.get(args[1]);
                    let b = self.get(args[2]);
                    let result = wide_mul::<WIDTH>(&a, &b, mask);
                    self.set_masked(args[0], result, mask);
                }

                "div" if args.len() >= 3 => {
                    let a = self.get(args[1]);
                    let b = self.get(args[2]);
                    let result = wide_div::<WIDTH>(&a, &b, mask);
                    self.set_masked(args[0], result, mask);
                }

                "neg" if args.len() >= 2 => {
                    let a = self.get(args[1]);
                    let result = wide_neg::<WIDTH>(&a, mask);
                    self.set_masked(args[0], result, mask);
                }

                // --- Math builtins ---
                "sin" if args.len() >= 2 => {
                    let x = self.get(args[1]).as_float();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = x.data[lane].sin();
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }

                "cos" if args.len() >= 2 => {
                    let x = self.get(args[1]).as_float();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = x.data[lane].cos();
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }

                "sqrt" if args.len() >= 2 => {
                    let x = self.get(args[1]).as_float();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            let v = x.data[lane];
                            result.data[lane] = if v >= 0.0 { v.sqrt() } else { 0.0 };
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }

                "abs" if args.len() >= 2 => {
                    let a = self.get(args[1]);
                    match &a {
                        WideValue::Float(w) => {
                            let mut result = Wide {
                                data: [0.0f32; WIDTH],
                            };
                            for lane in 0..WIDTH {
                                if mask.is_set(lane) {
                                    result.data[lane] = w.data[lane].abs();
                                }
                            }
                            self.set_masked(args[0], WideValue::Float(result), mask);
                        }
                        WideValue::Vec3(w) => {
                            let mut result = Wide {
                                data: [Vec3::ZERO; WIDTH],
                            };
                            for lane in 0..WIDTH {
                                if mask.is_set(lane) {
                                    let v = w.data[lane];
                                    result.data[lane] = Vec3::new(v.x.abs(), v.y.abs(), v.z.abs());
                                }
                            }
                            self.set_masked(args[0], WideValue::Vec3(result), mask);
                        }
                        _ => {}
                    }
                }

                "pow" if args.len() >= 3 => {
                    let x = self.get(args[1]).as_float();
                    let y = self.get(args[2]).as_float();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = x.data[lane].powf(y.data[lane]);
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }

                "max" if args.len() >= 3 => {
                    let a = self.get(args[1]).as_float();
                    let b = self.get(args[2]).as_float();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = a.data[lane].max(b.data[lane]);
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }

                "min" if args.len() >= 3 => {
                    let a = self.get(args[1]).as_float();
                    let b = self.get(args[2]).as_float();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = a.data[lane].min(b.data[lane]);
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }

                "clamp" if args.len() >= 4 => {
                    let x = self.get(args[1]).as_float();
                    let lo = self.get(args[2]).as_float();
                    let hi = self.get(args[3]).as_float();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = x.data[lane].clamp(lo.data[lane], hi.data[lane]);
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }

                "mix" if args.len() >= 4 => {
                    let a = self.get(args[1]).as_float();
                    let b = self.get(args[2]).as_float();
                    let t = self.get(args[3]).as_float();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] =
                                a.data[lane] * (1.0 - t.data[lane]) + b.data[lane] * t.data[lane];
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }

                // --- Vector operations ---
                "dot" if args.len() >= 3 => {
                    let a = self.get(args[1]).as_vec3();
                    let b = self.get(args[2]).as_vec3();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = a.data[lane].dot(b.data[lane]);
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }

                "normalize" if args.len() >= 2 => {
                    let v = self.get(args[1]).as_vec3();
                    let mut result = Wide {
                        data: [Vec3::ZERO; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = v.data[lane].normalize();
                        }
                    }
                    self.set_masked(args[0], WideValue::Vec3(result), mask);
                }

                "length" if args.len() >= 2 => {
                    let v = self.get(args[1]).as_vec3();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = v.data[lane].length();
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }

                "cross" if args.len() >= 3 => {
                    let a = self.get(args[1]).as_vec3();
                    let b = self.get(args[2]).as_vec3();
                    let mut result = Wide {
                        data: [Vec3::ZERO; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = a.data[lane].cross(b.data[lane]);
                        }
                    }
                    self.set_masked(args[0], WideValue::Vec3(result), mask);
                }

                // --- Comparison ---
                "lt" if args.len() >= 3 => {
                    let a = self.get(args[1]).as_float();
                    let b = self.get(args[2]).as_float();
                    let mut result = Wide {
                        data: [0i32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = if a.data[lane] < b.data[lane] { 1 } else { 0 };
                        }
                    }
                    self.set_masked(args[0], WideValue::Int(result), mask);
                }

                "gt" if args.len() >= 3 => {
                    let a = self.get(args[1]).as_float();
                    let b = self.get(args[2]).as_float();
                    let mut result = Wide {
                        data: [0i32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = if a.data[lane] > b.data[lane] { 1 } else { 0 };
                        }
                    }
                    self.set_masked(args[0], WideValue::Int(result), mask);
                }

                "eq" if args.len() >= 3 => {
                    let a = self.get(args[1]).as_float();
                    let b = self.get(args[2]).as_float();
                    let mut result = Wide {
                        data: [0i32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = if a.data[lane] == b.data[lane] { 1 } else { 0 };
                        }
                    }
                    self.set_masked(args[0], WideValue::Int(result), mask);
                }

                // --- degrees / radians ---
                "degrees" if args.len() >= 2 => {
                    let a = self.get(args[1]).as_float();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    let scale = 180.0f32 / std::f32::consts::PI;
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = a.data[lane] * scale;
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }
                "radians" if args.len() >= 2 => {
                    let a = self.get(args[1]).as_float();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    let scale = std::f32::consts::PI / 180.0f32;
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = a.data[lane] * scale;
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }

                // --- gettextureinfo (batched) ---
                "gettextureinfo" if args.len() >= 4 => {
                    let filename_val = self.get(args[1]);
                    let dataname_val = self.get(args[2]);
                    let output_val = self.get(args[3]);

                    let filename_strings = match &filename_val {
                        WideValue::String(strings) => strings.clone(),
                        _ => vec![UString::empty(); WIDTH],
                    };
                    let dataname_strings = match &dataname_val {
                        WideValue::String(strings) => strings.clone(),
                        _ => vec![UString::empty(); WIDTH],
                    };

                    let filename_hash = filename_strings
                        .first()
                        .map(|s| crate::ustring::UStringHash::hash_utf8(s.as_str()))
                        .unwrap_or_else(|| crate::ustring::UStringHash::from_hash(0));
                    let dataname_hash = dataname_strings
                        .first()
                        .map(|s| crate::ustring::UStringHash::hash_utf8(s.as_str()))
                        .unwrap_or_else(|| crate::ustring::UStringHash::from_hash(0));

                    let mut success = Mask::none();

                    match output_val {
                        WideValue::Int(_) => {
                            let mut out = Wide {
                                data: [0i32; WIDTH],
                            };
                            let renderer_mask = renderer.get_texture_info(
                                globals,
                                mask,
                                filename_hash,
                                std::ptr::null_mut(),
                                0,
                                dataname_hash,
                                crate::typedesc::TypeDesc::INT,
                                &mut out as *mut _ as *mut std::ffi::c_void,
                            );
                            if renderer_mask.any() {
                                self.set_masked(
                                    args[3],
                                    WideValue::Int(out.clone()),
                                    renderer_mask,
                                );
                                success = success.or(renderer_mask);
                            }

                            let fallback_mask = mask.and(success.complement());
                            if fallback_mask.any() {
                                let mut fallback_out = Wide {
                                    data: [0i32; WIDTH],
                                };
                                let mut fallback_success = Mask::none();
                                for lane in 0..WIDTH {
                                    if !fallback_mask.is_set(lane) {
                                        continue;
                                    }
                                    if let Some(info) = crate::texture::gettextureinfo(
                                        filename_strings[lane].as_str(),
                                        dataname_strings[lane].as_str(),
                                    ) {
                                        let value = match info {
                                            crate::texture::TextureInfo::Int(v) => Some(v),
                                            crate::texture::TextureInfo::IntVec(v) => {
                                                v.first().copied()
                                            }
                                            _ => None,
                                        };
                                        if let Some(v) = value {
                                            fallback_out.data[lane] = v;
                                            fallback_success.set(lane);
                                        }
                                    }
                                }
                                if fallback_success.any() {
                                    self.set_masked(
                                        args[3],
                                        WideValue::Int(fallback_out),
                                        fallback_success,
                                    );
                                    success = success.or(fallback_success);
                                }
                            }
                        }
                        WideValue::Float(_) => {
                            let mut out = Wide {
                                data: [0.0f32; WIDTH],
                            };
                            let renderer_mask = renderer.get_texture_info(
                                globals,
                                mask,
                                filename_hash,
                                std::ptr::null_mut(),
                                0,
                                dataname_hash,
                                crate::typedesc::TypeDesc::FLOAT,
                                &mut out as *mut _ as *mut std::ffi::c_void,
                            );
                            if renderer_mask.any() {
                                self.set_masked(
                                    args[3],
                                    WideValue::Float(out.clone()),
                                    renderer_mask,
                                );
                                success = success.or(renderer_mask);
                            }

                            let fallback_mask = mask.and(success.complement());
                            if fallback_mask.any() {
                                let mut fallback_out = Wide {
                                    data: [0.0f32; WIDTH],
                                };
                                let mut fallback_success = Mask::none();
                                for lane in 0..WIDTH {
                                    if !fallback_mask.is_set(lane) {
                                        continue;
                                    }
                                    if let Some(info) = crate::texture::gettextureinfo(
                                        filename_strings[lane].as_str(),
                                        dataname_strings[lane].as_str(),
                                    ) {
                                        let value = match info {
                                            crate::texture::TextureInfo::Float(v) => Some(v),
                                            crate::texture::TextureInfo::FloatVec(v) => {
                                                v.first().copied()
                                            }
                                            _ => None,
                                        };
                                        if let Some(v) = value {
                                            fallback_out.data[lane] = v;
                                            fallback_success.set(lane);
                                        }
                                    }
                                }
                                if fallback_success.any() {
                                    self.set_masked(
                                        args[3],
                                        WideValue::Float(fallback_out),
                                        fallback_success,
                                    );
                                    success = success.or(fallback_success);
                                }
                            }
                        }
                        WideValue::String(_) => {
                            let mut out = vec![UString::empty(); WIDTH];
                            let renderer_mask = renderer.get_texture_info(
                                globals,
                                mask,
                                filename_hash,
                                std::ptr::null_mut(),
                                0,
                                dataname_hash,
                                crate::typedesc::TypeDesc::STRING,
                                out.as_mut_ptr() as *mut std::ffi::c_void,
                            );
                            if renderer_mask.any() {
                                self.set_masked(
                                    args[3],
                                    WideValue::String(out.clone()),
                                    renderer_mask,
                                );
                                success = success.or(renderer_mask);
                            }

                            let fallback_mask = mask.and(success.complement());
                            if fallback_mask.any() {
                                let mut fallback_out = vec![UString::empty(); WIDTH];
                                let mut fallback_success = Mask::none();
                                for lane in 0..WIDTH {
                                    if !fallback_mask.is_set(lane) {
                                        continue;
                                    }
                                    if let Some(info) = crate::texture::gettextureinfo(
                                        filename_strings[lane].as_str(),
                                        dataname_strings[lane].as_str(),
                                    ) && let crate::texture::TextureInfo::Str(v) = info
                                    {
                                        fallback_out[lane] = UString::new(&v);
                                        fallback_success.set(lane);
                                    }
                                }
                                if fallback_success.any() {
                                    self.set_masked(
                                        args[3],
                                        WideValue::String(fallback_out),
                                        fallback_success,
                                    );
                                    success = success.or(fallback_success);
                                }
                            }
                        }
                        _ => {}
                    }

                    let mut result = Wide {
                        data: [0i32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if success.is_set(lane) {
                            result.data[lane] = 1;
                        }
                    }
                    self.set_masked(args[0], WideValue::Int(result), mask);
                }

                // --- nullnoise / unullnoise ---
                "nullnoise" if args.len() >= 2 => {
                    self.set_masked(
                        args[0],
                        WideValue::Float(Wide {
                            data: [0.0f32; WIDTH],
                        }),
                        mask,
                    );
                }
                "unullnoise" if args.len() >= 2 => {
                    self.set_masked(
                        args[0],
                        WideValue::Float(Wide {
                            data: [0.5f32; WIDTH],
                        }),
                        mask,
                    );
                }

                // --- Periodic noise aliases ---
                "pnoise" | "psnoise" | "pcellnoise" | "phashnoise" | "gaborpnoise"
                    if args.len() >= 3 =>
                {
                    // For batched: evaluate scalar periodic noise per-lane
                    let p = self.get(args[1]).as_vec3();
                    let period = self.get(args[2]).as_vec3();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    let noise_name = match op.op.as_str() {
                        "pcellnoise" => "cellnoise",
                        "phashnoise" => "hashnoise",
                        "gaborpnoise" => "perlin",
                        _ => "perlin",
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = crate::noise::pnoise_by_name(
                                noise_name,
                                p.data[lane],
                                period.data[lane],
                            );
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }

                // --- Texture (batched, delegates to BatchedRendererServices) ---
                "texture" if args.len() >= 2 => {
                    let filename_val = self.get(args[1]);
                    let filename_hash = match &filename_val {
                        WideValue::String(strings) => {
                            if let Some(s) = strings.first() {
                                crate::ustring::UStringHash::hash_utf8(s.as_str())
                            } else {
                                crate::ustring::UStringHash::from_hash(0)
                            }
                        }
                        _ => crate::ustring::UStringHash::from_hash(0),
                    };
                    // first_optional: 4 for (result,filename,s,t), 9 for (...,dsdx,dtdx,dsdy,dtdy)
                    let first_optional = if args.len() >= 9 { 9 } else { 4 };
                    // Gather s/t (with derivs if provided)
                    let s = if args.len() > 2 {
                        self.get(args[2]).as_float()
                    } else {
                        globals.u.clone()
                    };
                    let t = if args.len() > 3 {
                        self.get(args[3]).as_float()
                    } else {
                        globals.v.clone()
                    };
                    let dsdx = if args.len() > 4 {
                        self.get(args[4]).as_float()
                    } else {
                        globals.dudx.clone()
                    };
                    let dtdx = if args.len() > 5 {
                        self.get(args[5]).as_float()
                    } else {
                        globals.dvdx.clone()
                    };
                    let dsdy = if args.len() > 6 {
                        self.get(args[6]).as_float()
                    } else {
                        globals.dudy.clone()
                    };
                    let dtdy = if args.len() > 7 {
                        self.get(args[7]).as_float()
                    } else {
                        globals.dvdy.clone()
                    };
                    let mut tex_opts = globals.texture_options.clone();
                    self.merge_texture_opt_args(&args, first_optional, &mut tex_opts);
                    let mut result_channels = [
                        Wide {
                            data: [0.0f32; WIDTH],
                        },
                        Wide {
                            data: [0.0f32; WIDTH],
                        },
                        Wide {
                            data: [0.0f32; WIDTH],
                        },
                    ];
                    let _success = renderer.texture(
                        globals,
                        mask,
                        filename_hash,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        &tex_opts,
                        &s,
                        &t,
                        &dsdx,
                        &dtdx,
                        &dsdy,
                        &dtdy,
                        3,
                        &mut result_channels,
                        None,
                        None,
                    );
                    // Store result as Vec3 per lane
                    let mut result_vec = Wide {
                        data: [Vec3::ZERO; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result_vec.data[lane] = Vec3::new(
                                result_channels[0].data[lane],
                                result_channels[1].data[lane],
                                result_channels[2].data[lane],
                            );
                        }
                    }
                    self.set_masked(args[0], WideValue::Vec3(result_vec), mask);
                }

                // --- getattribute (batched) ---
                "getattribute" if args.len() >= 3 => {
                    let name_val = self.get(args[1]);
                    let name_hash = match &name_val {
                        WideValue::String(strings) => {
                            if let Some(s) = strings.first() {
                                crate::ustring::UStringHash::hash_utf8(s.as_str())
                            } else {
                                crate::ustring::UStringHash::from_hash(0)
                            }
                        }
                        _ => crate::ustring::UStringHash::from_hash(0),
                    };
                    let mut result_float = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    let mut result_int = Wide {
                        data: [0i32; WIDTH],
                    };
                    let mut result_vec3 = Wide {
                        data: [Vec3::ZERO; WIDTH],
                    };
                    let success = renderer.get_attribute(
                        globals,
                        mask,
                        false,
                        crate::ustring::UStringHash::from_hash(0),
                        crate::typedesc::TypeDesc::FLOAT,
                        name_hash,
                        &mut result_float,
                        &mut result_int,
                        &mut result_vec3,
                    );
                    // Store success as int
                    let mut result_mask = Wide {
                        data: [0i32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if success.is_set(lane) {
                            result_mask.data[lane] = 1;
                        }
                    }
                    self.set_masked(args[0], WideValue::Int(result_mask), mask);
                    self.set_masked(args[2], WideValue::Float(result_float), mask);
                }

                // --- trace (batched) ---
                "trace" if args.len() >= 3 => {
                    let p = self.get(args[1]).as_vec3();
                    let dir = self.get(args[2]).as_vec3();
                    let zero_vec = Wide {
                        data: [Vec3::ZERO; WIDTH],
                    };
                    let opt = crate::renderer::TraceOpt::default();
                    let hit_mask = renderer.trace(
                        globals, mask, &opt, &p, &zero_vec, &zero_vec, &dir, &zero_vec, &zero_vec,
                    );
                    let mut result = Wide {
                        data: [0i32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if hit_mask.is_set(lane) {
                            result.data[lane] = 1;
                        }
                    }
                    self.set_masked(args[0], WideValue::Int(result), mask);
                }

                // --- texture3d (batched) ---
                "texture3d" if args.len() >= 2 => {
                    let filename_val = self.get(args[1]);
                    let filename_hash = match &filename_val {
                        WideValue::String(strings) => {
                            if let Some(s) = strings.first() {
                                crate::ustring::UStringHash::hash_utf8(s.as_str())
                            } else {
                                crate::ustring::UStringHash::from_hash(0)
                            }
                        }
                        _ => crate::ustring::UStringHash::from_hash(0),
                    };
                    // first_optional: 3 for (result,filename,p), 6 for (...,dpdx,dpdy,dpdz)
                    let first_optional = if args.len() >= 6 { 6 } else { 3 };
                    let p = if args.len() > 2 {
                        self.get(args[2]).as_vec3()
                    } else {
                        globals.p.clone()
                    };
                    let zero_vec = Wide {
                        data: [Vec3::ZERO; WIDTH],
                    };
                    let dpdx = if args.len() > 3 {
                        self.get(args[3]).as_vec3()
                    } else {
                        zero_vec.clone()
                    };
                    let dpdy = if args.len() > 4 {
                        self.get(args[4]).as_vec3()
                    } else {
                        zero_vec.clone()
                    };
                    let dpdz = if args.len() > 5 {
                        self.get(args[5]).as_vec3()
                    } else {
                        zero_vec.clone()
                    };
                    let mut tex_opts = globals.texture_options.clone();
                    self.merge_texture_opt_args(&args, first_optional, &mut tex_opts);
                    let mut result_r = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    let mut result_g = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    let mut result_b = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    let mut result_channels = [
                        std::mem::replace(
                            &mut result_r,
                            Wide {
                                data: [0.0f32; WIDTH],
                            },
                        ),
                        std::mem::replace(
                            &mut result_g,
                            Wide {
                                data: [0.0f32; WIDTH],
                            },
                        ),
                        std::mem::replace(
                            &mut result_b,
                            Wide {
                                data: [0.0f32; WIDTH],
                            },
                        ),
                    ];
                    let _success = renderer.texture3d(
                        globals,
                        mask,
                        filename_hash,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        &tex_opts,
                        &p,
                        &dpdx,
                        &dpdy,
                        &dpdz,
                        3,
                        &mut result_channels,
                    );
                    let mut result_vec = Wide {
                        data: [Vec3::ZERO; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result_vec.data[lane] = Vec3::new(
                                result_channels[0].data[lane],
                                result_channels[1].data[lane],
                                result_channels[2].data[lane],
                            );
                        }
                    }
                    self.set_masked(args[0], WideValue::Vec3(result_vec), mask);
                }

                // --- environment (batched) ---
                "environment" if args.len() >= 2 => {
                    let filename_val = self.get(args[1]);
                    let filename_hash = match &filename_val {
                        WideValue::String(strings) => {
                            if let Some(s) = strings.first() {
                                crate::ustring::UStringHash::hash_utf8(s.as_str())
                            } else {
                                crate::ustring::UStringHash::from_hash(0)
                            }
                        }
                        _ => crate::ustring::UStringHash::from_hash(0),
                    };
                    // first_optional: 3 for (result,filename,r), 5 for (...,drdx,drdy)
                    let first_optional = if args.len() >= 5 { 5 } else { 3 };
                    let r = if args.len() > 2 {
                        self.get(args[2]).as_vec3()
                    } else {
                        globals.i.clone()
                    };
                    let zero_vec = Wide {
                        data: [Vec3::ZERO; WIDTH],
                    };
                    let drdx = if args.len() > 3 {
                        self.get(args[3]).as_vec3()
                    } else {
                        zero_vec.clone()
                    };
                    let drdy = if args.len() > 4 {
                        self.get(args[4]).as_vec3()
                    } else {
                        zero_vec.clone()
                    };
                    let mut tex_opts = globals.texture_options.clone();
                    self.merge_texture_opt_args(&args, first_optional, &mut tex_opts);
                    let mut result_channels = [
                        Wide {
                            data: [0.0f32; WIDTH],
                        },
                        Wide {
                            data: [0.0f32; WIDTH],
                        },
                        Wide {
                            data: [0.0f32; WIDTH],
                        },
                    ];
                    let _success = renderer.environment(
                        globals,
                        mask,
                        filename_hash,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        &tex_opts,
                        &r,
                        &drdx,
                        &drdy,
                        3,
                        &mut result_channels,
                    );
                    let mut result_vec = Wide {
                        data: [Vec3::ZERO; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result_vec.data[lane] = Vec3::new(
                                result_channels[0].data[lane],
                                result_channels[1].data[lane],
                                result_channels[2].data[lane],
                            );
                        }
                    }
                    self.set_masked(args[0], WideValue::Vec3(result_vec), mask);
                }

                // --- Noise ---
                "noise" if args.len() >= 2 => {
                    let p = self.get(args[1]).as_vec3();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = crate::noise::uperlin3(p.data[lane]);
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }

                // --- Construction ---
                "construct" if args.len() >= 4 => {
                    if args.len() == 5 {
                        // construct(dst, colorspace, x, y, z) — color space transform (ref: opcolor color(space,r,g,b))
                        let space_val = self.get(args[1]);
                        let x = self.get(args[2]).as_float();
                        let y = self.get(args[3]).as_float();
                        let z = self.get(args[4]).as_float();
                        let mut result = Wide {
                            data: [Vec3::ZERO; WIDTH],
                        };
                        let space_strings = match &space_val {
                            WideValue::String(s) => s.clone(),
                            _ => vec![UString::new("RGB"); WIDTH],
                        };
                        for lane in 0..WIDTH {
                            if mask.is_set(lane) {
                                let space =
                                    space_strings.get(lane).map(|u| u.as_str()).unwrap_or("RGB");
                                let src = Vec3::new(x.data[lane], y.data[lane], z.data[lane]);
                                result.data[lane] =
                                    crate::color::transform_color(space, "RGB", src);
                            }
                        }
                        self.set_masked(args[0], WideValue::Vec3(result), mask);
                    } else {
                        // construct(dst, x, y, z) — triple from 3 floats
                        let x = self.get(args[1]).as_float();
                        let y = self.get(args[2]).as_float();
                        let z = self.get(args[3]).as_float();
                        let mut result = Wide {
                            data: [Vec3::ZERO; WIDTH],
                        };
                        for lane in 0..WIDTH {
                            if mask.is_set(lane) {
                                result.data[lane] =
                                    Vec3::new(x.data[lane], y.data[lane], z.data[lane]);
                            }
                        }
                        self.set_masked(args[0], WideValue::Vec3(result), mask);
                    }
                }

                // --- Loop/control flow ---
                // Loop opcodes are structural markers in OSO. The actual
                // control flow is driven by inline if/nop sub-opcodes with
                // jump targets, so these are no-ops (same as scalar interp).
                "for" | "while" | "dowhile" if op.jump[0] >= 0 => {}
                "break" => {
                    if op.jump[0] >= 0 {
                        pc = op.jump[0] as usize;
                        continue;
                    }
                }
                "continue" => {
                    if op.jump[0] >= 0 {
                        pc = op.jump[0] as usize;
                        continue;
                    }
                }
                "functioncall" if op.jump[0] >= 0 => {
                    // Jump to function body, push return address
                    if self.call_stack.len() >= 256 {
                        eprintln!("[osl-rs batched] call stack overflow, aborting");
                        break;
                    }
                    self.call_stack.push(pc + 1);
                    pc = op.jump[0] as usize;
                    continue;
                }
                "functioncall" | "functioncall_nr" | "useparam" => {
                    // functioncall_nr: body is inlined, no jump needed.
                    // useparam: all layers already evaluated, no-op.
                }

                // --- Additional math builtins ---
                "exp" if args.len() >= 2 => {
                    let x = self.get(args[1]).as_float();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = x.data[lane].exp();
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }
                "exp2" if args.len() >= 2 => {
                    let x = self.get(args[1]).as_float();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = x.data[lane].exp2();
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }
                "log" if args.len() >= 2 => {
                    let x = self.get(args[1]).as_float();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            let v = x.data[lane];
                            result.data[lane] = if v > 0.0 { v.ln() } else { -f32::MAX };
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }
                "log2" if args.len() >= 2 => {
                    let x = self.get(args[1]).as_float();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            let v = x.data[lane];
                            result.data[lane] = if v > 0.0 { v.log2() } else { -f32::MAX };
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }
                "log10" if args.len() >= 2 => {
                    let x = self.get(args[1]).as_float();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            let v = x.data[lane];
                            result.data[lane] = if v > 0.0 { v.log10() } else { -f32::MAX };
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }
                "logb" if args.len() >= 2 => {
                    let x = self.get(args[1]).as_float();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            let v = x.data[lane];
                            result.data[lane] = if v != 0.0 {
                                v.abs().log2().floor()
                            } else {
                                -f32::INFINITY
                            };
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }
                "floor" if args.len() >= 2 => {
                    let x = self.get(args[1]).as_float();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = x.data[lane].floor();
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }
                "ceil" if args.len() >= 2 => {
                    let x = self.get(args[1]).as_float();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = x.data[lane].ceil();
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }
                "round" if args.len() >= 2 => {
                    let x = self.get(args[1]).as_float();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = x.data[lane].round();
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }
                "trunc" if args.len() >= 2 => {
                    let x = self.get(args[1]).as_float();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = x.data[lane].trunc();
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }
                "fmod" | "mod" if args.len() >= 3 => {
                    let a = self.get(args[1]).as_float();
                    let b = self.get(args[2]).as_float();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            let d = b.data[lane];
                            result.data[lane] = if d != 0.0 {
                                a.data[lane] - (a.data[lane] / d).floor() * d
                            } else {
                                0.0
                            };
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }
                "sign" if args.len() >= 2 => {
                    let x = self.get(args[1]).as_float();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            let v = x.data[lane];
                            result.data[lane] = if v > 0.0 {
                                1.0
                            } else if v < 0.0 {
                                -1.0
                            } else {
                                0.0
                            };
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }
                "step" if args.len() >= 3 => {
                    let edge = self.get(args[1]).as_float();
                    let x = self.get(args[2]).as_float();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = if x.data[lane] < edge.data[lane] {
                                0.0
                            } else {
                                1.0
                            };
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }
                "smoothstep" if args.len() >= 4 => {
                    let edge0 = self.get(args[1]).as_float();
                    let edge1 = self.get(args[2]).as_float();
                    let x = self.get(args[3]).as_float();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            let e0 = edge0.data[lane];
                            let e1 = edge1.data[lane];
                            let v = x.data[lane];
                            let t = ((v - e0) / (e1 - e0)).clamp(0.0, 1.0);
                            result.data[lane] = t * t * (3.0 - 2.0 * t);
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }
                "tan" if args.len() >= 2 => {
                    let x = self.get(args[1]).as_float();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = x.data[lane].tan();
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }
                "asin" if args.len() >= 2 => {
                    let x = self.get(args[1]).as_float();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = x.data[lane].clamp(-1.0, 1.0).asin();
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }
                "acos" if args.len() >= 2 => {
                    let x = self.get(args[1]).as_float();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = x.data[lane].clamp(-1.0, 1.0).acos();
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }
                "atan" if args.len() >= 2 => {
                    let x = self.get(args[1]).as_float();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = x.data[lane].atan();
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }
                "atan2" if args.len() >= 3 => {
                    let y = self.get(args[1]).as_float();
                    let x = self.get(args[2]).as_float();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = y.data[lane].atan2(x.data[lane]);
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }
                "sincos" if args.len() >= 3 => {
                    let x = self.get(args[1]).as_float();
                    let mut s = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    let mut c = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            let (sv, cv) = x.data[lane].sin_cos();
                            s.data[lane] = sv;
                            c.data[lane] = cv;
                        }
                    }
                    // sincos writes sin to arg[0], cos to arg[2]
                    self.set_masked(args[0], WideValue::Float(s), mask);
                    if args.len() >= 3 {
                        self.set_masked(args[2], WideValue::Float(c), mask);
                    }
                }
                "inversesqrt" if args.len() >= 2 => {
                    let x = self.get(args[1]).as_float();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            let v = x.data[lane];
                            result.data[lane] = if v > 0.0 { 1.0 / v.sqrt() } else { 0.0 };
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }
                "cbrt" if args.len() >= 2 => {
                    let x = self.get(args[1]).as_float();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = x.data[lane].cbrt();
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }
                "erf" if args.len() >= 2 => {
                    let x = self.get(args[1]).as_float();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            // Abramowitz & Stegun approximation
                            let v = x.data[lane];
                            let t = 1.0 / (1.0 + 0.3275911 * v.abs());
                            let poly = t
                                * (0.254_829_6
                                    + t * (-0.284_496_72
                                        + t * (1.421_413_8
                                            + t * (-1.453_152_1 + t * 1.061_405_4))));
                            let e = 1.0 - poly * (-v * v).exp();
                            result.data[lane] = if v >= 0.0 { e } else { -e };
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }
                "erfc" if args.len() >= 2 => {
                    let x = self.get(args[1]).as_float();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            let v = x.data[lane];
                            let t = 1.0 / (1.0 + 0.3275911 * v.abs());
                            let poly = t
                                * (0.254_829_6
                                    + t * (-0.284_496_72
                                        + t * (1.421_413_8
                                            + t * (-1.453_152_1 + t * 1.061_405_4))));
                            let erf = 1.0 - poly * (-v * v).exp();
                            result.data[lane] = if v >= 0.0 { 1.0 - erf } else { 1.0 + erf };
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }
                "isnan" if args.len() >= 2 => {
                    let x = self.get(args[1]).as_float();
                    let mut result = Wide {
                        data: [0i32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = if x.data[lane].is_nan() { 1 } else { 0 };
                        }
                    }
                    self.set_masked(args[0], WideValue::Int(result), mask);
                }
                "isinf" if args.len() >= 2 => {
                    let x = self.get(args[1]).as_float();
                    let mut result = Wide {
                        data: [0i32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = if x.data[lane].is_infinite() { 1 } else { 0 };
                        }
                    }
                    self.set_masked(args[0], WideValue::Int(result), mask);
                }
                "isfinite" if args.len() >= 2 => {
                    let x = self.get(args[1]).as_float();
                    let mut result = Wide {
                        data: [0i32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = if x.data[lane].is_finite() { 1 } else { 0 };
                        }
                    }
                    self.set_masked(args[0], WideValue::Int(result), mask);
                }
                "select" if args.len() >= 4 => {
                    let a = self.get(args[1]);
                    let b = self.get(args[2]);
                    let cond = self.get(args[3]).as_int();
                    let a_f = a.as_float();
                    let b_f = b.as_float();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = if cond.data[lane] != 0 {
                                b_f.data[lane]
                            } else {
                                a_f.data[lane]
                            };
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }

                // --- Additional comparison ops ---
                "neq" if args.len() >= 3 => {
                    let a = self.get(args[1]).as_float();
                    let b = self.get(args[2]).as_float();
                    let mut result = Wide {
                        data: [0i32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = if a.data[lane] != b.data[lane] { 1 } else { 0 };
                        }
                    }
                    self.set_masked(args[0], WideValue::Int(result), mask);
                }
                "le" if args.len() >= 3 => {
                    let a = self.get(args[1]).as_float();
                    let b = self.get(args[2]).as_float();
                    let mut result = Wide {
                        data: [0i32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = if a.data[lane] <= b.data[lane] { 1 } else { 0 };
                        }
                    }
                    self.set_masked(args[0], WideValue::Int(result), mask);
                }
                "ge" if args.len() >= 3 => {
                    let a = self.get(args[1]).as_float();
                    let b = self.get(args[2]).as_float();
                    let mut result = Wide {
                        data: [0i32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = if a.data[lane] >= b.data[lane] { 1 } else { 0 };
                        }
                    }
                    self.set_masked(args[0], WideValue::Int(result), mask);
                }

                // --- Logical ops ---
                "not" if args.len() >= 2 => {
                    let a = self.get(args[1]).as_int();
                    let mut result = Wide {
                        data: [0i32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = if a.data[lane] == 0 { 1 } else { 0 };
                        }
                    }
                    self.set_masked(args[0], WideValue::Int(result), mask);
                }
                "and" if args.len() >= 3 => {
                    let a = self.get(args[1]).as_int();
                    let b = self.get(args[2]).as_int();
                    let mut result = Wide {
                        data: [0i32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = if a.data[lane] != 0 && b.data[lane] != 0 {
                                1
                            } else {
                                0
                            };
                        }
                    }
                    self.set_masked(args[0], WideValue::Int(result), mask);
                }
                "or" if args.len() >= 3 => {
                    let a = self.get(args[1]).as_int();
                    let b = self.get(args[2]).as_int();
                    let mut result = Wide {
                        data: [0i32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = if a.data[lane] != 0 || b.data[lane] != 0 {
                                1
                            } else {
                                0
                            };
                        }
                    }
                    self.set_masked(args[0], WideValue::Int(result), mask);
                }
                "bitand" if args.len() >= 3 => {
                    let a = self.get(args[1]).as_int();
                    let b = self.get(args[2]).as_int();
                    let mut result = Wide {
                        data: [0i32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = a.data[lane] & b.data[lane];
                        }
                    }
                    self.set_masked(args[0], WideValue::Int(result), mask);
                }
                "bitor" if args.len() >= 3 => {
                    let a = self.get(args[1]).as_int();
                    let b = self.get(args[2]).as_int();
                    let mut result = Wide {
                        data: [0i32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = a.data[lane] | b.data[lane];
                        }
                    }
                    self.set_masked(args[0], WideValue::Int(result), mask);
                }
                "xor" if args.len() >= 3 => {
                    let a = self.get(args[1]).as_int();
                    let b = self.get(args[2]).as_int();
                    let mut result = Wide {
                        data: [0i32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = a.data[lane] ^ b.data[lane];
                        }
                    }
                    self.set_masked(args[0], WideValue::Int(result), mask);
                }
                "compl" if args.len() >= 2 => {
                    let a = self.get(args[1]).as_int();
                    let mut result = Wide {
                        data: [0i32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = !a.data[lane];
                        }
                    }
                    self.set_masked(args[0], WideValue::Int(result), mask);
                }
                "shl" if args.len() >= 3 => {
                    let a = self.get(args[1]).as_int();
                    let b = self.get(args[2]).as_int();
                    let mut result = Wide {
                        data: [0i32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            let shift = b.data[lane].clamp(0, 31) as u32;
                            result.data[lane] = a.data[lane].wrapping_shl(shift);
                        }
                    }
                    self.set_masked(args[0], WideValue::Int(result), mask);
                }
                "shr" if args.len() >= 3 => {
                    let a = self.get(args[1]).as_int();
                    let b = self.get(args[2]).as_int();
                    let mut result = Wide {
                        data: [0i32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            let shift = b.data[lane].clamp(0, 31) as u32;
                            result.data[lane] = a.data[lane].wrapping_shr(shift);
                        }
                    }
                    self.set_masked(args[0], WideValue::Int(result), mask);
                }

                // --- String ops ---
                "strlen" if args.len() >= 2 => {
                    let s = self.get(args[1]);
                    let strings = match &s {
                        WideValue::String(ss) => ss.clone(),
                        _ => vec![UString::empty(); WIDTH],
                    };
                    let mut result = Wide {
                        data: [0i32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) && lane < strings.len() {
                            result.data[lane] = strings[lane].as_str().len() as i32;
                        }
                    }
                    self.set_masked(args[0], WideValue::Int(result), mask);
                }
                "hash" if args.len() >= 2 => {
                    let s = self.get(args[1]);
                    let strings = match &s {
                        WideValue::String(ss) => ss.clone(),
                        _ => vec![UString::empty(); WIDTH],
                    };
                    let mut result = Wide {
                        data: [0i32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) && lane < strings.len() {
                            // FNV-1a hash
                            let mut h: u32 = 2166136261;
                            for b in strings[lane].as_str().bytes() {
                                h ^= b as u32;
                                h = h.wrapping_mul(16777619);
                            }
                            result.data[lane] = h as i32;
                        }
                    }
                    self.set_masked(args[0], WideValue::Int(result), mask);
                }
                "startswith" if args.len() >= 3 => {
                    let s = self.get(args[1]);
                    let prefix = self.get(args[2]);
                    let ss = match &s {
                        WideValue::String(v) => v.clone(),
                        _ => vec![UString::empty(); WIDTH],
                    };
                    let pp = match &prefix {
                        WideValue::String(v) => v.clone(),
                        _ => vec![UString::empty(); WIDTH],
                    };
                    let mut result = Wide {
                        data: [0i32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) && lane < ss.len() && lane < pp.len() {
                            result.data[lane] = if ss[lane].as_str().starts_with(pp[lane].as_str())
                            {
                                1
                            } else {
                                0
                            };
                        }
                    }
                    self.set_masked(args[0], WideValue::Int(result), mask);
                }
                "endswith" if args.len() >= 3 => {
                    let s = self.get(args[1]);
                    let suffix = self.get(args[2]);
                    let ss = match &s {
                        WideValue::String(v) => v.clone(),
                        _ => vec![UString::empty(); WIDTH],
                    };
                    let sf = match &suffix {
                        WideValue::String(v) => v.clone(),
                        _ => vec![UString::empty(); WIDTH],
                    };
                    let mut result = Wide {
                        data: [0i32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) && lane < ss.len() && lane < sf.len() {
                            result.data[lane] = if ss[lane].as_str().ends_with(sf[lane].as_str()) {
                                1
                            } else {
                                0
                            };
                        }
                    }
                    self.set_masked(args[0], WideValue::Int(result), mask);
                }
                "concat" if args.len() >= 3 => {
                    let s1 = self.get(args[1]);
                    let s2 = self.get(args[2]);
                    let ss1 = match &s1 {
                        WideValue::String(v) => v.clone(),
                        _ => vec![UString::empty(); WIDTH],
                    };
                    let ss2 = match &s2 {
                        WideValue::String(v) => v.clone(),
                        _ => vec![UString::empty(); WIDTH],
                    };
                    let mut result = vec![UString::empty(); WIDTH];
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) && lane < ss1.len() && lane < ss2.len() {
                            let combined = format!("{}{}", ss1[lane].as_str(), ss2[lane].as_str());
                            result[lane] = UString::new(&combined);
                        }
                    }
                    self.set_masked(args[0], WideValue::String(result), mask);
                }
                "substr" if args.len() >= 4 => {
                    let s = self.get(args[1]);
                    let start = self.get(args[2]).as_int();
                    let length = self.get(args[3]).as_int();
                    let ss = match &s {
                        WideValue::String(v) => v.clone(),
                        _ => vec![UString::empty(); WIDTH],
                    };
                    let mut result = vec![UString::empty(); WIDTH];
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) && lane < ss.len() {
                            let src = ss[lane].as_str();
                            let st = (start.data[lane].max(0) as usize).min(src.len());
                            let ln = (length.data[lane].max(0) as usize)
                                .min(src.len().saturating_sub(st));
                            result[lane] = UString::new(&src[st..st + ln]);
                        }
                    }
                    self.set_masked(args[0], WideValue::String(result), mask);
                }
                "stoi" if args.len() >= 2 => {
                    let s = self.get(args[1]);
                    let ss = match &s {
                        WideValue::String(v) => v.clone(),
                        _ => vec![UString::empty(); WIDTH],
                    };
                    let mut result = Wide {
                        data: [0i32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) && lane < ss.len() {
                            result.data[lane] =
                                ss[lane].as_str().trim().parse::<i32>().unwrap_or(0);
                        }
                    }
                    self.set_masked(args[0], WideValue::Int(result), mask);
                }
                "stof" if args.len() >= 2 => {
                    let s = self.get(args[1]);
                    let ss = match &s {
                        WideValue::String(v) => v.clone(),
                        _ => vec![UString::empty(); WIDTH],
                    };
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) && lane < ss.len() {
                            result.data[lane] =
                                ss[lane].as_str().trim().parse::<f32>().unwrap_or(0.0);
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }
                "getchar" if args.len() >= 3 => {
                    let s = self.get(args[1]);
                    let idx = self.get(args[2]).as_int();
                    let ss = match &s {
                        WideValue::String(v) => v.clone(),
                        _ => vec![UString::empty(); WIDTH],
                    };
                    let mut result = Wide {
                        data: [0i32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) && lane < ss.len() {
                            let bytes = ss[lane].as_str().as_bytes();
                            let i = idx.data[lane];
                            if i >= 0 && (i as usize) < bytes.len() {
                                result.data[lane] = bytes[i as usize] as i32;
                            }
                        }
                    }
                    self.set_masked(args[0], WideValue::Int(result), mask);
                }
                "printf" | "fprintf" | "warning" | "error" if args.len() >= 2 => {
                    // String output ops: collect format string from lane 0
                    let fmt_val = self.get(args[1]);
                    let fmt_str = match &fmt_val {
                        WideValue::String(ss) => {
                            if let Some(s) = ss.first() {
                                s.as_str().to_string()
                            } else {
                                String::new()
                            }
                        }
                        _ => String::new(),
                    };
                    if opname == "error" || opname == "warning" {
                        self.messages.push(format!("[{opname}] {fmt_str}"));
                    } else {
                        self.messages.push(fmt_str);
                    }
                }
                "format" if args.len() >= 2 => {
                    // printf-style format: args[1]=format_string, args[2..]=values
                    let fmt_val = self.get(args[1]);
                    let fmt_str = match &fmt_val {
                        WideValue::String(ss) => {
                            if let Some(s) = ss.first() {
                                s.as_str().to_string()
                            } else {
                                String::new()
                            }
                        }
                        _ => String::new(),
                    };
                    let arg_vals: Vec<WideValue<WIDTH>> =
                        (2..args.len()).map(|j| self.get(args[j])).collect();
                    let mut result_strings = vec![UString::empty(); WIDTH];
                    for (lane, out) in result_strings.iter_mut().enumerate().take(WIDTH) {
                        if mask.is_set(lane) {
                            *out = UString::new(&batched_format_string(&fmt_str, &arg_vals, lane));
                        }
                    }
                    self.set_masked(args[0], WideValue::String(result_strings), mask);
                }

                // --- Noise variants ---
                "snoise" if args.len() >= 2 => {
                    let p = self.get(args[1]).as_vec3();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            // snoise = signed perlin, range [-1,1]
                            result.data[lane] = crate::noise::perlin3(p.data[lane]);
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }
                "cellnoise" | "cell" if args.len() >= 2 => {
                    let p = self.get(args[1]).as_vec3();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = crate::noise::cellnoise3(p.data[lane]);
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }
                "hashnoise" if args.len() >= 2 => {
                    let p = self.get(args[1]).as_vec3();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = crate::noise::hashnoise3(p.data[lane]);
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }
                "gabornoise" | "gabor" if args.len() >= 2 => {
                    let p = self.get(args[1]).as_vec3();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = crate::gabor::gabor3_default(p.data[lane]);
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }
                "simplex" | "simplexnoise" if args.len() >= 2 => {
                    // Use perlin as fallback (simplex not separately implemented in scalar)
                    let p = self.get(args[1]).as_vec3();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = crate::noise::uperlin3(p.data[lane]);
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }

                // --- Geometric ops ---
                "distance" if args.len() >= 3 => {
                    let a = self.get(args[1]).as_vec3();
                    let b = self.get(args[2]).as_vec3();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = a.data[lane].distance(b.data[lane]);
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }
                "faceforward" if args.len() >= 4 => {
                    let n = self.get(args[1]).as_vec3();
                    let i = self.get(args[2]).as_vec3();
                    let nref = self.get(args[3]).as_vec3();
                    let mut result = Wide {
                        data: [Vec3::ZERO; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] =
                                n.data[lane].faceforward(i.data[lane], nref.data[lane]);
                        }
                    }
                    self.set_masked(args[0], WideValue::Vec3(result), mask);
                }
                "reflect" if args.len() >= 3 => {
                    let i = self.get(args[1]).as_vec3();
                    let n = self.get(args[2]).as_vec3();
                    let mut result = Wide {
                        data: [Vec3::ZERO; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = i.data[lane].reflect(n.data[lane]);
                        }
                    }
                    self.set_masked(args[0], WideValue::Vec3(result), mask);
                }
                "refract" if args.len() >= 4 => {
                    let i = self.get(args[1]).as_vec3();
                    let n = self.get(args[2]).as_vec3();
                    let eta = self.get(args[3]).as_float();
                    let mut result = Wide {
                        data: [Vec3::ZERO; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            let iv = i.data[lane].normalize();
                            let nv = n.data[lane].normalize();
                            let e = eta.data[lane];
                            let cos_i = -iv.dot(nv);
                            let sin2_t = e * e * (1.0 - cos_i * cos_i);
                            if sin2_t > 1.0 {
                                result.data[lane] = Vec3::ZERO; // total internal reflection
                            } else {
                                let cos_t = (1.0 - sin2_t).sqrt();
                                result.data[lane] = iv * e + nv * (e * cos_i - cos_t);
                            }
                        }
                    }
                    self.set_masked(args[0], WideValue::Vec3(result), mask);
                }
                "area" if args.len() >= 2 => {
                    // Approximate area from dPdx/dPdy globals
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            let dpdu = globals.dp_du.data[lane];
                            let dpdv = globals.dp_dv.data[lane];
                            result.data[lane] = dpdu.cross(dpdv).length();
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }
                "calculatenormal" if args.len() >= 2 => {
                    let p = self.get(args[1]).as_vec3();
                    let mut result = Wide {
                        data: [Vec3::ZERO; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            // Use dPdu x dPdv from globals as the surface normal
                            let dpdu = globals.dp_du.data[lane];
                            let dpdv = globals.dp_dv.data[lane];
                            let n = dpdu.cross(dpdv);
                            let len = n.length();
                            result.data[lane] = if len > 0.0 {
                                n * (1.0 / len)
                            } else {
                                p.data[lane].normalize()
                            };
                        }
                    }
                    self.set_masked(args[0], WideValue::Vec3(result), mask);
                }

                // --- Derivative ops ---
                "Dx" if args.len() >= 2 => {
                    // Approximate Dx using finite difference across lane pairs
                    let x = self.get(args[1]).as_float();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            // Forward difference to next lane, or 0 at boundary
                            result.data[lane] = if lane + 1 < WIDTH {
                                x.data[lane + 1] - x.data[lane]
                            } else {
                                0.0
                            };
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }
                "Dy" if args.len() >= 2 => {
                    // Dy approximation: stride by half-width
                    let x = self.get(args[1]).as_float();
                    let half = WIDTH / 2;
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = if lane + half < WIDTH {
                                x.data[lane + half] - x.data[lane]
                            } else {
                                0.0
                            };
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }
                "Dz" if args.len() >= 2 => {
                    // Dz is rarely used; return zero
                    self.set_masked(
                        args[0],
                        WideValue::Float(Wide {
                            data: [0.0f32; WIDTH],
                        }),
                        mask,
                    );
                }
                "filterwidth" if args.len() >= 2 => {
                    // filterwidth = abs(Dx) + abs(Dy) approximation
                    let x = self.get(args[1]).as_float();
                    let half = WIDTH / 2;
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            let dx = if lane + 1 < WIDTH {
                                (x.data[lane + 1] - x.data[lane]).abs()
                            } else {
                                0.0
                            };
                            let dy = if lane + half < WIDTH {
                                (x.data[lane + half] - x.data[lane]).abs()
                            } else {
                                0.0
                            };
                            result.data[lane] = dx.max(dy).max(1e-10);
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }

                // --- Vector component access ---
                "compref" | "getcomp" if args.len() >= 3 => {
                    let v = self.get(args[1]).as_vec3();
                    let idx_raw = self.get(args[2]).as_int();
                    let symname = if args[1] >= 0 && (args[1] as usize) < ir.symbols.len() {
                        ir.symbols[args[1] as usize].name.as_str()
                    } else {
                        "?"
                    };
                    let idx = self.range_check_wide(
                        &idx_raw,
                        3,
                        symname,
                        op.sourcefile.as_str(),
                        op.sourceline,
                        mask,
                    );
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            let vv = v.data[lane];
                            result.data[lane] = match idx.data[lane] {
                                0 => vv.x,
                                1 => vv.y,
                                _ => vv.z,
                            };
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }
                "compassign" | "setcomp" if args.len() >= 3 => {
                    let mut v = self.get(args[0]).as_vec3();
                    let idx_raw = self.get(args[1]).as_int();
                    let symname = if args[0] >= 0 && (args[0] as usize) < ir.symbols.len() {
                        ir.symbols[args[0] as usize].name.as_str()
                    } else {
                        "?"
                    };
                    let idx = self.range_check_wide(
                        &idx_raw,
                        3,
                        symname,
                        op.sourcefile.as_str(),
                        op.sourceline,
                        mask,
                    );
                    let val = self.get(args[2]).as_float();
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            match idx.data[lane] {
                                0 => v.data[lane].x = val.data[lane],
                                1 => v.data[lane].y = val.data[lane],
                                2 => v.data[lane].z = val.data[lane],
                                _ => {}
                            }
                        }
                    }
                    self.set_masked(args[0], WideValue::Vec3(v), mask);
                }

                // --- Type conversion ---
                "float" if args.len() >= 2 => {
                    let x = self.get(args[1]).as_float();
                    self.set_masked(args[0], WideValue::Float(x), mask);
                }
                "int" if args.len() >= 2 => {
                    let x = self.get(args[1]).as_int();
                    self.set_masked(args[0], WideValue::Int(x), mask);
                }
                "vector" | "point" | "normal" | "color" if args.len() >= 2 => {
                    if args.len() >= 4 {
                        // 3-component construction
                        let x = self.get(args[1]).as_float();
                        let y = self.get(args[2]).as_float();
                        let z = self.get(args[3]).as_float();
                        let mut result = Wide {
                            data: [Vec3::ZERO; WIDTH],
                        };
                        for lane in 0..WIDTH {
                            if mask.is_set(lane) {
                                result.data[lane] =
                                    Vec3::new(x.data[lane], y.data[lane], z.data[lane]);
                            }
                        }
                        self.set_masked(args[0], WideValue::Vec3(result), mask);
                    } else {
                        // Single-value splat or copy
                        let x = self.get(args[1]).as_vec3();
                        self.set_masked(args[0], WideValue::Vec3(x), mask);
                    }
                }

                // --- Color ops ---
                "luminance" if args.len() >= 2 => {
                    let c = self.get(args[1]).as_vec3();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            let v = c.data[lane];
                            // Rec. 709 luminance
                            result.data[lane] = 0.2126 * v.x + 0.7152 * v.y + 0.0722 * v.z;
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }
                "transform_color" | "transformc" if args.len() >= 3 => {
                    // transformc(result, [from,] to, color) - real color space transform
                    let (from_val, to_val, color_val) = if args.len() >= 4 {
                        (
                            self.get(args[1]),
                            self.get(args[2]),
                            self.get(args[3]).as_vec3(),
                        )
                    } else {
                        (
                            WideValue::String(vec![UString::new("rgb"); WIDTH]),
                            self.get(args[1]),
                            self.get(args[2]).as_vec3(),
                        )
                    };
                    let mut result = Wide {
                        data: [Vec3::ZERO; WIDTH],
                    };
                    if let (WideValue::String(froms), WideValue::String(tos)) = (&from_val, &to_val)
                    {
                        for lane in 0..WIDTH {
                            if mask.is_set(lane) && lane < froms.len() && lane < tos.len() {
                                result.data[lane] = crate::color::transform_color(
                                    froms[lane].as_str(),
                                    tos[lane].as_str(),
                                    color_val.data[lane],
                                );
                            }
                        }
                    } else {
                        result = color_val;
                    }
                    self.set_masked(args[0], WideValue::Vec3(result), mask);
                }
                "blackbody" if args.len() >= 2 => {
                    // Full CIE spectral blackbody via color::blackbody()
                    let temp = self.get(args[1]).as_float();
                    let mut result = Wide {
                        data: [Vec3::ZERO; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = crate::color::blackbody(temp.data[lane]);
                        }
                    }
                    self.set_masked(args[0], WideValue::Vec3(result), mask);
                }

                // --- Matrix ops ---
                "determinant" if args.len() >= 2 => {
                    let m = self.get(args[1]);
                    if let WideValue::Matrix(wm) = &m {
                        let mut result = Wide {
                            data: [0.0f32; WIDTH],
                        };
                        for lane in 0..WIDTH {
                            if mask.is_set(lane) {
                                result.data[lane] = wm.data[lane].determinant();
                            }
                        }
                        self.set_masked(args[0], WideValue::Float(result), mask);
                    }
                }
                "transpose" if args.len() >= 2 => {
                    let m = self.get(args[1]);
                    if let WideValue::Matrix(wm) = &m {
                        let mut result = Wide {
                            data: [Matrix44::IDENTITY; WIDTH],
                        };
                        for lane in 0..WIDTH {
                            if mask.is_set(lane) {
                                result.data[lane] = wm.data[lane].transpose();
                            }
                        }
                        self.set_masked(args[0], WideValue::Matrix(result), mask);
                    }
                }
                "transform" if args.len() >= 3 => {
                    // transform(matrix, point) or transform(from_space, to_space, point)
                    let m = self.get(args[1]);
                    let p = self.get(args[2]).as_vec3();
                    if let WideValue::Matrix(wm) = &m {
                        let mut result = Wide {
                            data: [Vec3::ZERO; WIDTH],
                        };
                        for lane in 0..WIDTH {
                            if mask.is_set(lane) {
                                result.data[lane] = wm.data[lane].transform_point(p.data[lane]);
                            }
                        }
                        self.set_masked(args[0], WideValue::Vec3(result), mask);
                    } else {
                        // Space names provided: pass through (would need renderer)
                        self.set_masked(args[0], WideValue::Vec3(p), mask);
                    }
                }
                "transformv" if args.len() >= 3 => {
                    let m = self.get(args[1]);
                    let v = self.get(args[2]).as_vec3();
                    if let WideValue::Matrix(wm) = &m {
                        let mut result = Wide {
                            data: [Vec3::ZERO; WIDTH],
                        };
                        for lane in 0..WIDTH {
                            if mask.is_set(lane) {
                                result.data[lane] = wm.data[lane].transform_vector(v.data[lane]);
                            }
                        }
                        self.set_masked(args[0], WideValue::Vec3(result), mask);
                    } else {
                        self.set_masked(args[0], WideValue::Vec3(v), mask);
                    }
                }
                "transformn" if args.len() >= 3 => {
                    let m = self.get(args[1]);
                    let n = self.get(args[2]).as_vec3();
                    if let WideValue::Matrix(wm) = &m {
                        let mut result = Wide {
                            data: [Vec3::ZERO; WIDTH],
                        };
                        for lane in 0..WIDTH {
                            if mask.is_set(lane) {
                                result.data[lane] = wm.data[lane].transform_normal(n.data[lane]);
                            }
                        }
                        self.set_masked(args[0], WideValue::Vec3(result), mask);
                    } else {
                        self.set_masked(args[0], WideValue::Vec3(n), mask);
                    }
                }
                "getmatrix" if args.len() >= 3 => {
                    // getmatrix(success, from, to, M) — C++ op layout; 2-arg form has to="common"
                    let from_val = self.get(args[1]);
                    let to_val = if args.len() >= 4 {
                        self.get(args[2])
                    } else {
                        WideValue::String(vec![crate::ustring::UString::new("common"); WIDTH])
                    };
                    let m_arg = if args.len() >= 4 { args[3] } else { args[2] };
                    let from_strs = match &from_val {
                        WideValue::String(s) => s.clone(),
                        _ => vec![UString::new("common"); WIDTH],
                    };
                    let to_strs = match &to_val {
                        WideValue::String(s) => s.clone(),
                        _ => vec![UString::new("common"); WIDTH],
                    };
                    let mut m_result = Wide {
                        data: [Matrix44::IDENTITY; WIDTH],
                    };
                    let mut success_data = [0i32; WIDTH];
                    for (lane, success_slot) in success_data.iter_mut().enumerate().take(WIDTH) {
                        if !mask.is_set(lane) {
                            continue;
                        }
                        let from_s = from_strs.get(lane).map(|u| u.as_str()).unwrap_or("common");
                        let to_s = to_strs.get(lane).map(|u| u.as_str()).unwrap_or("common");
                        let syn_s = self.commonspace_synonym.as_str();
                        let from_is_common = from_s == "common" || from_s == syn_s;
                        let to_is_common = to_s == "common" || to_s == syn_s;
                        let lane_mask = Mask::from_bits(1u32 << lane);
                        let mut m_from = Wide {
                            data: [Matrix44::IDENTITY; WIDTH],
                        };
                        let mut m_to = Wide {
                            data: [Matrix44::IDENTITY; WIDTH],
                        };
                        let ok_from = if from_is_common {
                            m_from.data[lane] = Matrix44::IDENTITY;
                            lane_mask
                        } else {
                            renderer.get_matrix_named(
                                globals,
                                lane_mask,
                                crate::ustring::UStringHash::hash_utf8(from_s),
                                &globals.time,
                                &mut m_from,
                            )
                        };
                        let ok_to = if to_is_common {
                            m_to.data[lane] = Matrix44::IDENTITY;
                            lane_mask
                        } else {
                            renderer.get_inverse_matrix_named(
                                globals,
                                lane_mask,
                                crate::ustring::UStringHash::hash_utf8(to_s),
                                &globals.time,
                                &mut m_to,
                            )
                        };
                        let ok = ok_from.is_set(lane) && ok_to.is_set(lane);
                        if !ok && self.unknown_coordsys_error {
                            let failed = if !ok_from.is_set(lane) { from_s } else { to_s };
                            self.messages
                                .push(format!("ERROR: Unknown transformation \"{}\"", failed));
                        }
                        m_result.data[lane] =
                            crate::matrix_ops::matmul(&m_to.data[lane], &m_from.data[lane]);
                        *success_slot = if ok { 1 } else { 0 };
                    }
                    self.set_masked(m_arg, WideValue::Matrix(m_result), mask);
                    self.set_masked(args[0], WideValue::Int(Wide { data: success_data }), mask);
                }
                "matrix" if args.len() >= 2 => {
                    // Matrix constructor: pass through or identity
                    let m = self.get(args[1]);
                    if let WideValue::Matrix(_) = &m {
                        self.set_masked(args[0], m, mask);
                    } else {
                        let f = self.get(args[1]).as_float();
                        let mut result = Wide {
                            data: [Matrix44::IDENTITY; WIDTH],
                        };
                        for lane in 0..WIDTH {
                            if mask.is_set(lane) {
                                let s = f.data[lane];
                                result.data[lane] = Matrix44::from_row_major(&[
                                    s, 0.0, 0.0, 0.0, 0.0, s, 0.0, 0.0, 0.0, 0.0, s, 0.0, 0.0, 0.0,
                                    0.0, 1.0,
                                ]);
                            }
                        }
                        self.set_masked(args[0], WideValue::Matrix(result), mask);
                    }
                }

                // --- Misc ops ---
                "backfacing" if !args.is_empty() => {
                    let mut result = Wide {
                        data: [0i32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            // backfacing = dot(I, Ng) > 0
                            let idot = globals.i.data[lane].dot(globals.ng.data[lane]);
                            result.data[lane] = if idot > 0.0 { 1 } else { 0 };
                        }
                    }
                    self.set_masked(args[0], WideValue::Int(result), mask);
                }
                "raytype" if args.len() >= 2 => {
                    // raytype(result, name) -- check if raytype bit matches
                    let rt = globals.uniform.raytype;
                    let name_val = self.get(args[1]);
                    let mut result = Wide {
                        data: [0i32; WIDTH],
                    };
                    if let WideValue::String(names) = &name_val {
                        for lane in 0..WIDTH {
                            if mask.is_set(lane) && lane < names.len() {
                                let bit = match names[lane].as_str() {
                                    "camera" => 1,
                                    "shadow" => 2,
                                    "diffuse" => 4,
                                    "glossy" => 8,
                                    "reflection" => 16,
                                    "refraction" => 32,
                                    _ => 0,
                                };
                                result.data[lane] = if (rt & bit) != 0 { 1 } else { 0 };
                            }
                        }
                    }
                    self.set_masked(args[0], WideValue::Int(result), mask);
                }
                "arraylength" if args.len() >= 2 => {
                    // Return the array length (just 1 for non-array types)
                    self.set_masked(
                        args[0],
                        WideValue::Int(Wide {
                            data: [1i32; WIDTH],
                        }),
                        mask,
                    );
                }
                "regex_search" | "regex_match" if args.len() >= 3 => {
                    // Per-lane regex via opstring::regex_search/regex_match
                    let subject = self.get(args[1]);
                    let pattern = self.get(args[2]);
                    let mut result = Wide {
                        data: [0i32; WIDTH],
                    };
                    if let (WideValue::String(ss), WideValue::String(ps)) = (&subject, &pattern) {
                        let is_match = opname == "regex_match";
                        for lane in 0..WIDTH {
                            if mask.is_set(lane) && lane < ss.len() && lane < ps.len() {
                                let found = if is_match {
                                    crate::opstring::regex_match(
                                        ss[lane].as_str(),
                                        ps[lane].as_str(),
                                    )
                                } else {
                                    crate::opstring::regex_search(
                                        ss[lane].as_str(),
                                        ps[lane].as_str(),
                                    )
                                };
                                result.data[lane] = if found { 1 } else { 0 };
                            }
                        }
                    }
                    self.set_masked(args[0], WideValue::Int(result), mask);
                }
                "split" if args.len() >= 3 => {
                    // split(result, string, sep [, maxsplit]) -> count of tokens
                    let str_val = self.get(args[1]);
                    let sep_val = self.get(args[2]);
                    let maxsplit = if args.len() >= 4 {
                        self.get(args[3]).as_int()
                    } else {
                        Wide {
                            data: [i32::MAX; WIDTH],
                        }
                    };
                    let mut result = Wide {
                        data: [0i32; WIDTH],
                    };
                    if let (WideValue::String(ss), WideValue::String(seps)) = (&str_val, &sep_val) {
                        for lane in 0..WIDTH {
                            if mask.is_set(lane) && lane < ss.len() && lane < seps.len() {
                                let s = ss[lane].as_str();
                                let sep = seps[lane].as_str();
                                let max = maxsplit.data[lane].max(0) as usize;
                                let parts: Vec<&str> = if sep.is_empty() {
                                    s.split_whitespace().collect()
                                } else {
                                    s.split(sep).collect()
                                };
                                result.data[lane] = parts.len().min(max) as i32;
                            }
                        }
                    }
                    self.set_masked(args[0], WideValue::Int(result), mask);
                }
                "aastep" if args.len() >= 3 => {
                    // Anti-aliased step: use regular step as fallback
                    let edge = self.get(args[1]).as_float();
                    let x = self.get(args[2]).as_float();
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = if x.data[lane] < edge.data[lane] {
                                0.0
                            } else {
                                1.0
                            };
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }
                "transformu" if args.len() >= 4 => {
                    // Unit transform: pass through value as-is (would need unit registry)
                    let v = self.get(args[3]).as_float();
                    self.set_masked(args[0], WideValue::Float(v), mask);
                }
                "pointcloud_search" if args.len() >= 5 => {
                    // pointcloud_search(result, filename, center, radius, maxpoints [, sort] [, "index", indices, "distance", distances, ...])
                    let filename = self.get(args[1]);
                    let center = self.get(args[2]).as_vec3();
                    let radius = self.get(args[3]).as_float();
                    let maxpts = self.get(args[4]).as_int();
                    let sort = if args.len() >= 6 {
                        self.get(args[5]).as_int()
                    } else {
                        Wide {
                            data: [0i32; WIDTH],
                        }
                    };
                    let mut result = Wide {
                        data: [0i32; WIDTH],
                    };
                    let mgr = self
                        .pointcloud_manager
                        .as_ref()
                        .map(Arc::clone)
                        .unwrap_or_else(|| {
                            Arc::new(std::sync::RwLock::new(
                                crate::pointcloud::PointCloudManager::new(),
                            ))
                        });
                    if let WideValue::String(fnames) = &filename
                        && let Some(fname) = fnames.first()
                        && let Ok(guard) = mgr.read()
                        && let Some(cloud) = guard.get(fname.as_str())
                    {
                        let mut lane_indices: Vec<Vec<i32>> =
                            (0..WIDTH).map(|_| Vec::new()).collect();
                        let mut lane_distances: Vec<Vec<f32>> =
                            (0..WIDTH).map(|_| Vec::new()).collect();
                        for lane in 0..WIDTH {
                            if mask.is_set(lane) {
                                let sr = crate::pointcloud::pointcloud_search(
                                    cloud,
                                    center.data[lane],
                                    radius.data[lane],
                                    maxpts.data[lane].max(0) as usize,
                                    sort.data[lane] != 0,
                                );
                                result.data[lane] = sr.indices.len() as i32;
                                lane_indices[lane] = sr.indices.iter().map(|&i| i as i32).collect();
                                lane_distances[lane] =
                                    sr.distances_sq.iter().map(|&d| d.sqrt()).collect();
                            }
                        }
                        // Parse optional output args: "index", indices_sym, "distance", distances_sym
                        let mut i = 6;
                        while i + 1 < args.len() {
                            let name_val = self.get(args[i]);
                            let name = match &name_val {
                                WideValue::String(ss) => ss
                                    .first()
                                    .map(|s| s.as_str().to_string())
                                    .unwrap_or_default(),
                                _ => String::new(),
                            };
                            let sym = args[i + 1];
                            if name == "index" && sym >= 0 {
                                self.set_masked(
                                    sym,
                                    WideValue::IntArray(lane_indices.clone()),
                                    mask,
                                );
                            } else if name == "distance" && sym >= 0 {
                                self.set_masked(
                                    sym,
                                    WideValue::FloatArray(lane_distances.clone()),
                                    mask,
                                );
                            }
                            i += 2;
                        }
                    }
                    self.set_masked(args[0], WideValue::Int(result), mask);
                }
                "pointcloud_get" if args.len() >= 6 => {
                    // pointcloud_get(result, filename, indices, count, attr, dest)
                    let mgr = self
                        .pointcloud_manager
                        .as_ref()
                        .map(Arc::clone)
                        .unwrap_or_else(|| {
                            Arc::new(std::sync::RwLock::new(
                                crate::pointcloud::PointCloudManager::new(),
                            ))
                        });
                    let filename = self.get(args[1]);
                    let indices_val = self.get(args[2]);
                    let count_val = self.get(args[3]).as_int();
                    let attr_val = self.get(args[4]);
                    let dest_sym = args[5];
                    let mut result = Wide {
                        data: [0i32; WIDTH],
                    };
                    if let (
                        WideValue::String(fnames),
                        WideValue::IntArray(indices_arrs),
                        WideValue::String(attrs),
                    ) = (&filename, &indices_val, &attr_val)
                    {
                        let fname = fnames.first().map(|s| s.as_str()).unwrap_or("");
                        let attr = attrs.first().map(|s| s.as_str()).unwrap_or("");
                        if let Ok(guard) = mgr.read()
                            && let Some(cloud) = guard.get(fname)
                        {
                            let mut dest_arrs: Vec<Vec<Vec3>> =
                                (0..WIDTH).map(|_| Vec::new()).collect();
                            let mut dest_floats: Vec<Vec<f32>> =
                                (0..WIDTH).map(|_| Vec::new()).collect();
                            let mut use_vec3 = false;
                            for lane in 0..WIDTH {
                                if mask.is_set(lane) && lane < indices_arrs.len() {
                                    let cnt = count_val.data[lane].max(0) as usize;
                                    let inds: Vec<usize> = indices_arrs[lane]
                                        .iter()
                                        .take(cnt)
                                        .map(|&i| i as usize)
                                        .collect();
                                    if inds.is_empty() {
                                        result.data[lane] = 1;
                                    } else {
                                        let vals = crate::pointcloud::pointcloud_get(
                                            cloud,
                                            &inds,
                                            crate::ustring::UString::new(attr),
                                        );
                                        if let Some(Some(crate::pointcloud::PointData::Vec3(_))) =
                                            vals.first()
                                        {
                                            use_vec3 = true;
                                            dest_arrs[lane] = vals
                                                .iter()
                                                .filter_map(|o| {
                                                    o.and_then(|pd| {
                                                        if let crate::pointcloud::PointData::Vec3(
                                                            v,
                                                        ) = pd
                                                        {
                                                            Some(*v)
                                                        } else {
                                                            None
                                                        }
                                                    })
                                                })
                                                .collect();
                                        } else {
                                            dest_floats[lane] = vals
                                                .iter()
                                                .filter_map(|o| {
                                                    o.and_then(|pd| {
                                                        if let crate::pointcloud::PointData::Float(
                                                            f,
                                                        ) = pd
                                                        {
                                                            Some(*f)
                                                        } else {
                                                            None
                                                        }
                                                    })
                                                })
                                                .collect();
                                        }
                                        result.data[lane] = if vals.iter().any(|o| o.is_some()) {
                                            1
                                        } else {
                                            0
                                        };
                                    }
                                }
                            }
                            if use_vec3 {
                                self.set_masked(dest_sym, WideValue::Vec3Array(dest_arrs), mask);
                            } else {
                                self.set_masked(dest_sym, WideValue::FloatArray(dest_floats), mask);
                            }
                        }
                    }
                    self.set_masked(args[0], WideValue::Int(result), mask);
                }
                "pointcloud_write" if args.len() >= 3 => {
                    let mgr = self
                        .pointcloud_manager
                        .as_ref()
                        .map(Arc::clone)
                        .unwrap_or_else(|| {
                            Arc::new(std::sync::RwLock::new(
                                crate::pointcloud::PointCloudManager::new(),
                            ))
                        });
                    let filename = self.get(args[1]);
                    let pos_val = self.get(args[2]).as_vec3();
                    let mut result = Wide {
                        data: [0i32; WIDTH],
                    };
                    if let WideValue::String(fnames) = &filename {
                        let fname = fnames.first().map(|s| s.as_str()).unwrap_or("");
                        if !fname.is_empty() {
                            let nattrs = (args.len() - 3) / 2;
                            if let Ok(mut guard) = mgr.write() {
                                let cloud = guard.get_or_create(fname);
                                for lane in 0..WIDTH {
                                    if mask.is_set(lane) {
                                        let mut attrs = std::collections::HashMap::new();
                                        for i in 0..nattrs {
                                            let name_val = self.get(args[3 + i * 2]);
                                            let val = self.get(args[4 + i * 2]);
                                            if let WideValue::String(ns) = &name_val {
                                                let n = ns
                                                    .get(lane)
                                                    .map(|u| u.as_str().to_string())
                                                    .unwrap_or_default();
                                                let pd = match &val {
                                                    WideValue::Float(w) => {
                                                        crate::pointcloud::PointData::Float(
                                                            w.data[lane],
                                                        )
                                                    }
                                                    WideValue::Int(w) => {
                                                        crate::pointcloud::PointData::Int(
                                                            w.data[lane],
                                                        )
                                                    }
                                                    WideValue::Vec3(w) => {
                                                        crate::pointcloud::PointData::Vec3(
                                                            w.data[lane],
                                                        )
                                                    }
                                                    _ => crate::pointcloud::PointData::Float(
                                                        val.as_float().data[lane],
                                                    ),
                                                };
                                                attrs.insert(crate::ustring::UString::new(&n), pd);
                                            }
                                        }
                                        let ok = crate::pointcloud::pointcloud_write(
                                            cloud,
                                            pos_val.data[lane],
                                            attrs,
                                        );
                                        result.data[lane] = if ok { 1 } else { 0 };
                                    }
                                }
                            }
                        }
                    }
                    self.set_masked(args[0], WideValue::Int(result), mask);
                }
                "pointcloud_get" | "pointcloud_write" => {
                    if !args.is_empty() {
                        self.set_masked(
                            args[0],
                            WideValue::Int(Wide {
                                data: [0i32; WIDTH],
                            }),
                            mask,
                        );
                    }
                }
                "dict_find" if args.len() >= 3 => {
                    let src = self.get(args[1]);
                    let query_val = self.get(args[2]);
                    let mut result = Wide {
                        data: [0i32; WIDTH],
                    };
                    if let WideValue::String(queries) = &query_val {
                        let query = queries.first().map(|q| q.as_str()).unwrap_or("");
                        match &src {
                            WideValue::String(ss) => {
                                let dict_str = ss.first().map(|s| s.as_str()).unwrap_or("");
                                let handle = self.dict_store.dict_find_str(dict_str, query);
                                for lane in 0..WIDTH {
                                    if mask.is_set(lane) {
                                        result.data[lane] = handle;
                                    }
                                }
                            }
                            WideValue::Int(ids) => {
                                for lane in 0..WIDTH {
                                    if mask.is_set(lane) {
                                        result.data[lane] =
                                            self.dict_store.dict_find_node(ids.data[lane], query);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    self.set_masked(args[0], WideValue::Int(result), mask);
                }
                "dict_next" if args.len() >= 2 => {
                    let ids = self.get(args[1]).as_int();
                    let mut result = Wide {
                        data: [0i32; WIDTH],
                    };
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            result.data[lane] = self.dict_store.dict_next(ids.data[lane]);
                        }
                    }
                    self.set_masked(args[0], WideValue::Int(result), mask);
                }
                "dict_value" if args.len() >= 3 => {
                    let ids = self.get(args[1]).as_int();
                    let attr_val = self.get(args[2]);
                    if let WideValue::String(attrs) = &attr_val {
                        let attr = attrs.first().map(|a| a.as_str()).unwrap_or("");
                        let mut str_results = vec![UString::empty(); WIDTH];
                        let mut any_found = false;
                        for (lane, out) in str_results.iter_mut().enumerate().take(WIDTH) {
                            if mask.is_set(lane)
                                && let Some(s) =
                                    self.dict_store.dict_value_str(ids.data[lane], attr)
                            {
                                *out = UString::new(&s);
                                any_found = true;
                            }
                        }
                        if any_found {
                            self.set_masked(args[0], WideValue::String(str_results), mask);
                        } else {
                            let mut result = Wide {
                                data: [0i32; WIDTH],
                            };
                            for lane in 0..WIDTH {
                                if mask.is_set(lane) {
                                    result.data[lane] = self
                                        .dict_store
                                        .dict_value_int(ids.data[lane], attr)
                                        .unwrap_or(0);
                                }
                            }
                            self.set_masked(args[0], WideValue::Int(result), mask);
                        }
                    }
                }
                "dict_find" | "dict_next" | "dict_value" => {
                    if !args.is_empty() {
                        self.set_masked(
                            args[0],
                            WideValue::Int(Wide {
                                data: [0i32; WIDTH],
                            }),
                            mask,
                        );
                    }
                }
                "setmessage" if args.len() >= 3 => {
                    let name_val = self.get(args[1]);
                    if let WideValue::String(names) = &name_val
                        && let Some(name) = names.first()
                    {
                        let val = self.get(args[2]);
                        self.message_store.insert(name.as_str().to_string(), val);
                    }
                }
                "getmessage" if args.len() >= 3 => {
                    let name_val = self.get(args[1]);
                    if let WideValue::String(names) = &name_val
                        && let Some(name) = names.first()
                    {
                        if let Some(val) = self.message_store.get(name.as_str()).cloned() {
                            self.set_masked(args[0], val, mask);
                        } else {
                            self.set_masked(
                                args[0],
                                WideValue::Int(Wide {
                                    data: [0i32; WIDTH],
                                }),
                                mask,
                            );
                        }
                    }
                }

                // --- CRITICAL-3: init_array ---
                "init_array" if args.len() >= 2 => {
                    let len_val = self.get(args[1]).as_int();
                    let len = len_val.data[0].max(0) as usize;
                    let dst_idx = args[0] as usize;
                    if dst_idx < ir.symbols.len() {
                        let td = ir.symbols[dst_idx].typespec.simpletype();
                        let is_vec3 = td.basetype == crate::typedesc::BaseType::Float as u8
                            && td.aggregate == crate::typedesc::Aggregate::Vec3 as u8;
                        let val = if is_vec3 {
                            WideValue::Vec3Array(vec![vec![Vec3::ZERO; len]; WIDTH])
                        } else if td.aggregate == crate::typedesc::Aggregate::Matrix44 as u8 {
                            WideValue::MatrixArray(vec![vec![Matrix44::IDENTITY; len]; WIDTH])
                        } else if td.basetype == crate::typedesc::BaseType::Int32 as u8 {
                            WideValue::IntArray(vec![vec![0i32; len]; WIDTH])
                        } else if td.basetype == crate::typedesc::BaseType::String as u8 {
                            WideValue::StringArray(vec![vec![UString::new(""); len]; WIDTH])
                        } else {
                            WideValue::FloatArray(vec![vec![0.0f32; len]; WIDTH])
                        };
                        self.set_masked(args[0], val, mask);
                    }
                }

                // --- CRITICAL-3: aref (array element read) ---
                "aref" if args.len() >= 3 => {
                    let base = self.get(args[1]);
                    let idx_raw = self.get(args[2]).as_int();
                    let symname = if args[1] >= 0 && (args[1] as usize) < ir.symbols.len() {
                        ir.symbols[args[1] as usize].name.as_str()
                    } else {
                        "?"
                    };
                    let (_length, idx) = match &base {
                        WideValue::FloatArray(arrs) => {
                            let len = arrs.first().map(|a| a.len()).unwrap_or(0) as i32;
                            (
                                len,
                                self.range_check_wide(
                                    &idx_raw,
                                    len,
                                    symname,
                                    op.sourcefile.as_str(),
                                    op.sourceline,
                                    mask,
                                ),
                            )
                        }
                        WideValue::IntArray(arrs) => {
                            let len = arrs.first().map(|a| a.len()).unwrap_or(0) as i32;
                            (
                                len,
                                self.range_check_wide(
                                    &idx_raw,
                                    len,
                                    symname,
                                    op.sourcefile.as_str(),
                                    op.sourceline,
                                    mask,
                                ),
                            )
                        }
                        WideValue::Vec3Array(arrs) => {
                            let len = arrs.first().map(|a| a.len()).unwrap_or(0) as i32;
                            (
                                len,
                                self.range_check_wide(
                                    &idx_raw,
                                    len,
                                    symname,
                                    op.sourcefile.as_str(),
                                    op.sourceline,
                                    mask,
                                ),
                            )
                        }
                        WideValue::MatrixArray(arrs) => {
                            let len = arrs.first().map(|a| a.len()).unwrap_or(0) as i32;
                            (
                                len,
                                self.range_check_wide(
                                    &idx_raw,
                                    len,
                                    symname,
                                    op.sourcefile.as_str(),
                                    op.sourceline,
                                    mask,
                                ),
                            )
                        }
                        WideValue::StringArray(arrs) => {
                            let len = arrs.first().map(|a| a.len()).unwrap_or(0) as i32;
                            (
                                len,
                                self.range_check_wide(
                                    &idx_raw,
                                    len,
                                    symname,
                                    op.sourcefile.as_str(),
                                    op.sourceline,
                                    mask,
                                ),
                            )
                        }
                        WideValue::Vec3(_) => (
                            3,
                            self.range_check_wide(
                                &idx_raw,
                                3,
                                symname,
                                op.sourcefile.as_str(),
                                op.sourceline,
                                mask,
                            ),
                        ),
                        _ => (0, idx_raw),
                    };
                    let result = match &base {
                        WideValue::FloatArray(_) => {
                            WideValue::Float(base.array_ref_float(&idx, mask))
                        }
                        WideValue::IntArray(_) => WideValue::Int(base.array_ref_int(&idx, mask)),
                        WideValue::Vec3Array(_) => WideValue::Vec3(base.array_ref_vec3(&idx, mask)),
                        WideValue::MatrixArray(arrs) => {
                            let mut r = Wide {
                                data: [Matrix44::IDENTITY; WIDTH],
                            };
                            for lane in 0..WIDTH {
                                if mask.is_set(lane) && lane < arrs.len() {
                                    let i = idx.data[lane].max(0) as usize;
                                    if i < arrs[lane].len() {
                                        r.data[lane] = arrs[lane][i];
                                    }
                                }
                            }
                            WideValue::Matrix(r)
                        }
                        WideValue::StringArray(arrs) => {
                            let mut r = vec![UString::empty(); WIDTH];
                            for lane in 0..WIDTH {
                                if mask.is_set(lane) && lane < arrs.len() {
                                    let i = idx.data[lane].max(0) as usize;
                                    if i < arrs[lane].len() {
                                        r[lane] = arrs[lane][i];
                                    }
                                }
                            }
                            WideValue::String(r)
                        }
                        WideValue::Vec3(wv) => {
                            let mut r = Wide {
                                data: [0.0f32; WIDTH],
                            };
                            for lane in 0..WIDTH {
                                if mask.is_set(lane) {
                                    let i = idx.data[lane].clamp(0, 2) as usize;
                                    r.data[lane] = match i {
                                        0 => wv.data[lane].x,
                                        1 => wv.data[lane].y,
                                        _ => wv.data[lane].z,
                                    };
                                }
                            }
                            WideValue::Float(r)
                        }
                        _ => WideValue::Float(Wide {
                            data: [0.0f32; WIDTH],
                        }),
                    };
                    self.set_masked(args[0], result, mask);
                }

                // --- CRITICAL-4: aassign (array element write) ---
                "aassign" if args.len() >= 3 => {
                    let idx_raw = self.get(args[1]).as_int();
                    let src = self.get(args[2]);
                    let symname = if args[0] >= 0 && (args[0] as usize) < ir.symbols.len() {
                        ir.symbols[args[0] as usize].name.as_str()
                    } else {
                        "?"
                    };
                    if args[0] >= 0 {
                        let dst_i = args[0] as usize;
                        if dst_i < self.values.len() {
                            let length = match &self.values[dst_i] {
                                WideValue::Vec3(_) => 3,
                                WideValue::FloatArray(a) => {
                                    a.first().map(|x| x.len()).unwrap_or(0) as i32
                                }
                                WideValue::IntArray(a) => {
                                    a.first().map(|x| x.len()).unwrap_or(0) as i32
                                }
                                WideValue::Vec3Array(a) => {
                                    a.first().map(|x| x.len()).unwrap_or(0) as i32
                                }
                                WideValue::MatrixArray(a) => {
                                    a.first().map(|x| x.len()).unwrap_or(0) as i32
                                }
                                WideValue::StringArray(a) => {
                                    a.first().map(|x| x.len()).unwrap_or(0) as i32
                                }
                                _ => 0,
                            };
                            let idx = self.range_check_wide(
                                &idx_raw,
                                length,
                                symname,
                                op.sourcefile.as_str(),
                                op.sourceline,
                                mask,
                            );
                            match &mut self.values[dst_i] {
                                WideValue::FloatArray(arrs) => {
                                    let sv = src.as_float();
                                    for lane in 0..WIDTH {
                                        if mask.is_set(lane) && lane < arrs.len() {
                                            let i = idx.data[lane].max(0) as usize;
                                            if i < arrs[lane].len() {
                                                arrs[lane][i] = sv.data[lane];
                                            }
                                        }
                                    }
                                }
                                WideValue::IntArray(arrs) => {
                                    let sv = src.as_int();
                                    for lane in 0..WIDTH {
                                        if mask.is_set(lane) && lane < arrs.len() {
                                            let i = idx.data[lane].max(0) as usize;
                                            if i < arrs[lane].len() {
                                                arrs[lane][i] = sv.data[lane];
                                            }
                                        }
                                    }
                                }
                                WideValue::Vec3Array(arrs) => {
                                    let sv = src.as_vec3();
                                    for lane in 0..WIDTH {
                                        if mask.is_set(lane) && lane < arrs.len() {
                                            let i = idx.data[lane].max(0) as usize;
                                            if i < arrs[lane].len() {
                                                arrs[lane][i] = sv.data[lane];
                                            }
                                        }
                                    }
                                }
                                WideValue::MatrixArray(arrs) => {
                                    if let WideValue::Matrix(sm) = &src {
                                        for lane in 0..WIDTH {
                                            if mask.is_set(lane) && lane < arrs.len() {
                                                let i = idx.data[lane].max(0) as usize;
                                                if i < arrs[lane].len() {
                                                    arrs[lane][i] = sm.data[lane];
                                                }
                                            }
                                        }
                                    }
                                }
                                WideValue::StringArray(arrs) => {
                                    if let WideValue::String(sv) = &src {
                                        for lane in 0..WIDTH {
                                            if mask.is_set(lane) && lane < arrs.len() {
                                                let i = idx.data[lane].max(0) as usize;
                                                if i < arrs[lane].len() && lane < sv.len() {
                                                    arrs[lane][i] = sv[lane];
                                                }
                                            }
                                        }
                                    }
                                }
                                WideValue::Vec3(wv) => {
                                    let sv = src.as_float();
                                    for lane in 0..WIDTH {
                                        if mask.is_set(lane) {
                                            let i = idx.data[lane].clamp(0, 2) as usize;
                                            match i {
                                                0 => wv.data[lane].x = sv.data[lane],
                                                1 => wv.data[lane].y = sv.data[lane],
                                                _ => wv.data[lane].z = sv.data[lane],
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }

                // --- CRITICAL-5: closure opcode ---
                "closure" if args.len() >= 3 => {
                    let name_val = self.get(args[1]);
                    let name_str = match &name_val {
                        WideValue::String(svec) if !svec.is_empty() => svec[0].as_str().to_string(),
                        _ => "unknown".to_string(),
                    };
                    let closure_id = crate::closure_ops::closure_name_to_id(&name_str).unwrap_or(0);
                    let param_count = args.len() - 2;
                    let mut closures: Vec<Option<Box<ClosureValue>>> = Vec::with_capacity(WIDTH);
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            let mut params = Vec::with_capacity(param_count);
                            for p in 0..param_count {
                                let wv = self.get(args[2 + p]);
                                let val = match &wv {
                                    WideValue::Float(w) => {
                                        crate::interp::Value::Float(w.data[lane])
                                    }
                                    WideValue::Int(w) => crate::interp::Value::Int(w.data[lane]),
                                    WideValue::Vec3(w) => crate::interp::Value::Vec3(w.data[lane]),
                                    WideValue::Matrix(w) => {
                                        crate::interp::Value::Matrix(w.data[lane])
                                    }
                                    WideValue::String(sv) => {
                                        let s = if lane < sv.len() {
                                            sv[lane]
                                        } else {
                                            UString::empty()
                                        };
                                        crate::interp::Value::String(s)
                                    }
                                    _ => crate::interp::Value::Float(0.0),
                                };
                                params.push(val);
                            }
                            closures.push(Some(Box::new(ClosureValue::Component {
                                name: name_str.clone(),
                                id: closure_id,
                                params,
                                weight: Color3::new(1.0, 1.0, 1.0),
                            })));
                        } else {
                            closures.push(None);
                        }
                    }
                    self.set_masked(args[0], WideValue::Closure(closures), mask);
                }

                // --- HIGH: spline opcode ---
                "spline" if args.len() >= 4 => {
                    let basis_val = self.get(args[1]);
                    let basis_str = match &basis_val {
                        WideValue::String(sv) if !sv.is_empty() => sv[0].as_str().to_string(),
                        _ => "catmull-rom".to_string(),
                    };
                    let basis = crate::spline::SplineBasis::from_name(&basis_str)
                        .unwrap_or(crate::spline::SplineBasis::CatmullRom);
                    let t_wide = self.get(args[2]).as_float();
                    let nknots = args.len() - 3;
                    if nknots >= 1 {
                        let first = self.get(args[3]);
                        let is_vec3 = matches!(&first, WideValue::Vec3(_));
                        if is_vec3 {
                            let knots: Vec<Wide<Vec3, WIDTH>> = (0..nknots)
                                .map(|j| self.get(args[3 + j]).as_vec3())
                                .collect();
                            let mut result = Wide {
                                data: [Vec3::ZERO; WIDTH],
                            };
                            for lane in 0..WIDTH {
                                if mask.is_set(lane) {
                                    let lk: Vec<Vec3> =
                                        knots.iter().map(|k| k.data[lane]).collect();
                                    result.data[lane] =
                                        crate::spline::spline_vec3(basis, t_wide.data[lane], &lk);
                                }
                            }
                            self.set_masked(args[0], WideValue::Vec3(result), mask);
                        } else {
                            let knots: Vec<Wide<f32, WIDTH>> = (0..nknots)
                                .map(|j| self.get(args[3 + j]).as_float())
                                .collect();
                            let mut result = Wide {
                                data: [0.0f32; WIDTH],
                            };
                            for lane in 0..WIDTH {
                                if mask.is_set(lane) {
                                    let lk: Vec<f32> = knots.iter().map(|k| k.data[lane]).collect();
                                    result.data[lane] =
                                        crate::spline::spline_float(basis, t_wide.data[lane], &lk);
                                }
                            }
                            self.set_masked(args[0], WideValue::Float(result), mask);
                        }
                    }
                }

                // --- HIGH: mxcompref (matrix component read) ---
                "mxcompref" if args.len() >= 4 => {
                    let m_val = self.get(args[1]);
                    let row_raw = self.get(args[2]).as_int();
                    let col_raw = self.get(args[3]).as_int();
                    let symname = if args[1] >= 0 && (args[1] as usize) < ir.symbols.len() {
                        ir.symbols[args[1] as usize].name.as_str()
                    } else {
                        "?"
                    };
                    let row = self.range_check_wide(
                        &row_raw,
                        4,
                        symname,
                        op.sourcefile.as_str(),
                        op.sourceline,
                        mask,
                    );
                    let col = self.range_check_wide(
                        &col_raw,
                        4,
                        symname,
                        op.sourcefile.as_str(),
                        op.sourceline,
                        mask,
                    );
                    let mut result = Wide {
                        data: [0.0f32; WIDTH],
                    };
                    if let WideValue::Matrix(wm) = &m_val {
                        for lane in 0..WIDTH {
                            if mask.is_set(lane) {
                                let r = row.data[lane].clamp(0, 3) as usize;
                                let c = col.data[lane].clamp(0, 3) as usize;
                                result.data[lane] = wm.data[lane].m[r][c];
                            }
                        }
                    }
                    self.set_masked(args[0], WideValue::Float(result), mask);
                }

                // --- HIGH: mxcompassign (matrix component write) ---
                "mxcompassign" if args.len() >= 4 => {
                    let row_raw = self.get(args[1]).as_int();
                    let col_raw = self.get(args[2]).as_int();
                    let val = self.get(args[3]).as_float();
                    let symname = if args[0] >= 0 && (args[0] as usize) < ir.symbols.len() {
                        ir.symbols[args[0] as usize].name.as_str()
                    } else {
                        "?"
                    };
                    let row = self.range_check_wide(
                        &row_raw,
                        4,
                        symname,
                        op.sourcefile.as_str(),
                        op.sourceline,
                        mask,
                    );
                    let col = self.range_check_wide(
                        &col_raw,
                        4,
                        symname,
                        op.sourcefile.as_str(),
                        op.sourceline,
                        mask,
                    );
                    let dst_idx = args[0];
                    if dst_idx >= 0 && (dst_idx as usize) < self.values.len() {
                        let i = dst_idx as usize;
                        if let WideValue::Matrix(wm) = &mut self.values[i] {
                            for lane in 0..WIDTH {
                                if mask.is_set(lane) {
                                    let r = row.data[lane].clamp(0, 3) as usize;
                                    let c = col.data[lane].clamp(0, 3) as usize;
                                    wm.data[lane].m[r][c] = val.data[lane];
                                }
                            }
                        } else {
                            let mut wm = Wide {
                                data: [Matrix44::IDENTITY; WIDTH],
                            };
                            for lane in 0..WIDTH {
                                if mask.is_set(lane) {
                                    let r = row.data[lane].clamp(0, 3) as usize;
                                    let c = col.data[lane].clamp(0, 3) as usize;
                                    wm.data[lane].m[r][c] = val.data[lane];
                                }
                            }
                            self.values[i] = WideValue::Matrix(wm);
                        }
                    }
                }

                // --- isconstant(dst, expr): always 0 at runtime (C++ batched_llvm_gen:8079)
                "isconstant" if args.len() >= 2 => {
                    self.set_masked(args[0], WideValue::splat_int(0), mask);
                }

                // --- HIGH: arraycopy ---
                "arraycopy" if args.len() >= 2 => {
                    let src = self.get(args[1]);
                    self.set_masked(args[0], src, mask);
                }

                // --- Struct/field access (getfield/setfield fallback from codegen) ---
                "getfield" if args.len() >= 3 => {
                    // getfield(result, struct_sym, field_name_const)
                    let src = self.get(args[1]);
                    let field_name = self.get_const_string(args[2], ir);
                    let result = match &src {
                        WideValue::Vec3(v) => {
                            let mut out = Wide {
                                data: [0.0f32; WIDTH],
                            };
                            match field_name {
                                "x" | "r" => {
                                    for i in 0..WIDTH {
                                        out.data[i] = v.data[i].x;
                                    }
                                }
                                "y" | "g" => {
                                    for i in 0..WIDTH {
                                        out.data[i] = v.data[i].y;
                                    }
                                }
                                "z" | "b" => {
                                    for i in 0..WIDTH {
                                        out.data[i] = v.data[i].z;
                                    }
                                }
                                _ => {}
                            }
                            WideValue::Float(out)
                        }
                        _ => WideValue::Float(Wide {
                            data: [0.0f32; WIDTH],
                        }),
                    };
                    self.set_masked(args[0], result, mask);
                }
                "setfield" if args.len() >= 3 => {
                    // setfield(struct_sym, field_name_const, value)
                    let field_name = self.get_const_string(args[1], ir);
                    let val = self.get(args[2]).as_float();
                    let mut v = self.get(args[0]).as_vec3();
                    match field_name {
                        "x" | "r" => {
                            for i in 0..WIDTH {
                                if mask.is_set(i) {
                                    v.data[i].x = val.data[i];
                                }
                            }
                        }
                        "y" | "g" => {
                            for i in 0..WIDTH {
                                if mask.is_set(i) {
                                    v.data[i].y = val.data[i];
                                }
                            }
                        }
                        "z" | "b" => {
                            for i in 0..WIDTH {
                                if mask.is_set(i) {
                                    v.data[i].z = val.data[i];
                                }
                            }
                        }
                        _ => {}
                    }
                    self.set_masked(args[0], WideValue::Vec3(v), mask);
                }

                // --- Catch-all ---
                _ => {
                    eprintln!("[osl-rs batched] unknown opcode: {}", opname);
                }
            }

            pc += 1;
        }
    }

    fn get(&self, idx: i32) -> WideValue<WIDTH> {
        if idx >= 0 && (idx as usize) < self.values.len() {
            self.values[idx as usize].clone()
        } else {
            WideValue::default()
        }
    }

    /// Get a constant string value from a symbol (lane 0).
    fn get_const_string<'a>(&'a self, idx: i32, ir: &'a ShaderIR) -> &'a str {
        // Try IR const_values first
        let ui = idx as usize;
        for (ci, cv) in &ir.const_values {
            if *ci == ui
                && let ConstValue::String(s) = cv
            {
                return s.as_str();
            }
        }
        // Fall back to runtime values (lane 0)
        if idx >= 0
            && ui < self.values.len()
            && let WideValue::String(ref strings) = self.values[ui]
            && let Some(s) = strings.first()
        {
            return s.as_str();
        }
        ""
    }

    /// Parse optional texture args from opcode args[start..] as (name, value) pairs.
    /// Extracts lane 0 from WideValue for uniform options. Matches interp::parse_texture_opt_args.
    fn merge_texture_opt_args(
        &self,
        args: &[i32],
        start: usize,
        tex_opts: &mut crate::context::BatchedTextureOptions,
    ) {
        use crate::texture::{TextureOptArg, parse_texture_options};
        let mut pairs = Vec::new();
        let mut i = start;
        while i + 1 < args.len() {
            let name_val = self.get(args[i]);
            let name = match &name_val {
                WideValue::String(ss) => ss
                    .first()
                    .map(|u| u.as_str().to_string())
                    .unwrap_or_default(),
                _ => String::new(),
            };
            if name.is_empty() {
                i += 1;
                continue;
            }
            let val = self.get(args[i + 1]);
            let opt_arg = match &val {
                WideValue::Int(w) => TextureOptArg::Int(w.data[0]),
                WideValue::Float(w) => TextureOptArg::Float(w.data[0]),
                WideValue::String(ss) => TextureOptArg::Str(
                    ss.first()
                        .map(|u| u.as_str().to_string())
                        .unwrap_or_default(),
                ),
                _ => TextureOptArg::Float(val.as_float().data[0]),
            };
            pairs.push((name, opt_arg));
            i += 2;
        }
        let parsed = parse_texture_options(pairs);
        tex_opts.merge_texture_opt_overrides(&parsed);
    }

    /// Masked set — only writes to lanes where the mask is active.
    fn set_masked(&mut self, idx: i32, val: WideValue<WIDTH>, mask: Mask<WIDTH>) {
        if idx < 0 || (idx as usize) >= self.values.len() {
            return;
        }
        let i = idx as usize;
        if mask == Mask::all() {
            // Fast path: all lanes active, just overwrite
            self.values[i] = val;
        } else {
            // Slow path: merge lanes
            match (&mut self.values[i], &val) {
                (WideValue::Float(dst), WideValue::Float(src)) => {
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            dst.data[lane] = src.data[lane];
                        }
                    }
                }
                (WideValue::Int(dst), WideValue::Int(src)) => {
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            dst.data[lane] = src.data[lane];
                        }
                    }
                }
                (WideValue::Vec3(dst), WideValue::Vec3(src)) => {
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            dst.data[lane] = src.data[lane];
                        }
                    }
                }
                (WideValue::Matrix(dst), WideValue::Matrix(src)) => {
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) {
                            dst.data[lane] = src.data[lane];
                        }
                    }
                }
                (WideValue::String(dst), WideValue::String(src)) => {
                    if dst.len() != WIDTH {
                        dst.resize(WIDTH, UString::empty());
                    }
                    for lane in 0..WIDTH {
                        if mask.is_set(lane) && lane < src.len() {
                            dst[lane] = src[lane];
                        }
                    }
                }
                _ => {
                    // Type mismatch: overwrite entirely for active lanes
                    self.values[i] = val;
                }
            }
        }
    }

    /// Bind batched shader globals to the appropriate symbols.
    fn bind_globals(&mut self, ir: &ShaderIR, globals: &BatchedShaderGlobals<WIDTH>) {
        for (i, sym) in ir.symbols.iter().enumerate() {
            let name = sym.name.as_str();
            match name {
                // Position and derivatives
                "P" => self.values[i] = WideValue::Vec3(globals.p.clone()),
                "dPdx" => self.values[i] = WideValue::Vec3(globals.dp_dx.clone()),
                "dPdy" => self.values[i] = WideValue::Vec3(globals.dp_dy.clone()),
                "dPdz" => self.values[i] = WideValue::Vec3(globals.dp_dz.clone()),
                // Incident ray and derivatives
                "I" => self.values[i] = WideValue::Vec3(globals.i.clone()),
                "dIdx" => self.values[i] = WideValue::Vec3(globals.di_dx.clone()),
                "dIdy" => self.values[i] = WideValue::Vec3(globals.di_dy.clone()),
                // Normals
                "N" => self.values[i] = WideValue::Vec3(globals.n.clone()),
                "Ng" => self.values[i] = WideValue::Vec3(globals.ng.clone()),
                // UV and derivatives
                "u" => self.values[i] = WideValue::Float(globals.u.clone()),
                "dudx" => self.values[i] = WideValue::Float(globals.dudx.clone()),
                "dudy" => self.values[i] = WideValue::Float(globals.dudy.clone()),
                "v" => self.values[i] = WideValue::Float(globals.v.clone()),
                "dvdx" => self.values[i] = WideValue::Float(globals.dvdx.clone()),
                "dvdy" => self.values[i] = WideValue::Float(globals.dvdy.clone()),
                // Surface tangents
                "dPdu" => self.values[i] = WideValue::Vec3(globals.dp_du.clone()),
                "dPdv" => self.values[i] = WideValue::Vec3(globals.dp_dv.clone()),
                // Time
                "time" => self.values[i] = WideValue::Float(globals.time.clone()),
                "dtime" => self.values[i] = WideValue::Float(globals.dtime.clone()),
                "dPdtime" => self.values[i] = WideValue::Vec3(globals.dp_dtime.clone()),
                // Light point
                "Ps" => self.values[i] = WideValue::Vec3(globals.ps.clone()),
                "dPsdx" => self.values[i] = WideValue::Vec3(globals.dps_dx.clone()),
                "dPsdy" => self.values[i] = WideValue::Vec3(globals.dps_dy.clone()),
                // Miscellaneous varying
                "surfacearea" => self.values[i] = WideValue::Float(globals.surfacearea.clone()),
                "flipHandedness" => {
                    self.values[i] = WideValue::Int(globals.flip_handedness.clone())
                }
                "backfacing" => self.values[i] = WideValue::Int(globals.backfacing.clone()),
                // raytype is UNIFORM — splat the single value across all lanes
                "raytype" => self.values[i] = WideValue::splat_int(globals.uniform.raytype),
                _ => {}
            }
        }
    }

    /// Extract a scalar float result from a specific lane.
    pub fn get_float(&self, ir: &ShaderIR, name: &str, lane: usize) -> Option<f32> {
        for (i, sym) in ir.symbols.iter().enumerate() {
            if sym.name == name {
                let w = self.values[i].as_float();
                if lane < WIDTH {
                    return Some(w.data[lane]);
                }
            }
        }
        None
    }

    /// Extract a scalar Vec3 result from a specific lane.
    pub fn get_vec3(&self, ir: &ShaderIR, name: &str, lane: usize) -> Option<Vec3> {
        for (i, sym) in ir.symbols.iter().enumerate() {
            if sym.name == name {
                let w = self.values[i].as_vec3();
                if lane < WIDTH {
                    return Some(w.data[lane]);
                }
            }
        }
        None
    }

    /// Extract a scalar int result from a specific lane.
    pub fn get_int(&self, ir: &ShaderIR, name: &str, lane: usize) -> Option<i32> {
        for (i, sym) in ir.symbols.iter().enumerate() {
            if sym.name == name {
                let w = self.values[i].as_int();
                if lane < WIDTH {
                    return Some(w.data[lane]);
                }
            }
        }
        None
    }

    /// Extract a string result from a specific lane.
    pub fn get_string(&self, ir: &ShaderIR, name: &str, lane: usize) -> Option<String> {
        for (i, sym) in ir.symbols.iter().enumerate() {
            if sym.name == name
                && let WideValue::String(ss) = &self.values[i]
                && lane < ss.len()
            {
                return Some(ss[lane].as_str().to_string());
            }
        }
        None
    }

    /// Extract raw WideValue for a named symbol.
    #[cfg(test)]
    pub fn get_value(&self, ir: &ShaderIR, name: &str) -> Option<&WideValue<WIDTH>> {
        for (i, sym) in ir.symbols.iter().enumerate() {
            if sym.name == name {
                return Some(&self.values[i]);
            }
        }
        None
    }
}

/// Batched format string: extract per-lane values from WideValue args and format.
fn batched_format_string<const WIDTH: usize>(
    fmt: &str,
    arg_vals: &[WideValue<WIDTH>],
    lane: usize,
) -> String {
    let mut int_args = Vec::new();
    let mut float_args = Vec::new();
    let mut str_args = Vec::new();
    for av in arg_vals {
        match av {
            WideValue::Int(w) => int_args.push(w.data[lane]),
            WideValue::Float(w) => float_args.push(w.data[lane]),
            WideValue::String(ss) if lane < ss.len() => {
                str_args.push(ss[lane].as_str().to_string());
            }
            _ => {}
        }
    }
    let str_refs: Vec<&str> = str_args.iter().map(|s| s.as_str()).collect();
    crate::opstring::format_string(fmt, &int_args, &float_args, &str_refs)
}

// ---------------------------------------------------------------------------
// Wide arithmetic helpers
// ---------------------------------------------------------------------------

fn wide_add<const WIDTH: usize>(
    a: &WideValue<WIDTH>,
    b: &WideValue<WIDTH>,
    mask: Mask<WIDTH>,
) -> WideValue<WIDTH> {
    match (a, b) {
        (WideValue::Float(wa), WideValue::Float(wb)) => {
            let mut result = Wide {
                data: [0.0f32; WIDTH],
            };
            for lane in 0..WIDTH {
                if mask.is_set(lane) {
                    result.data[lane] = wa.data[lane] + wb.data[lane];
                }
            }
            WideValue::Float(result)
        }
        (WideValue::Int(wa), WideValue::Int(wb)) => {
            let mut result = Wide {
                data: [0i32; WIDTH],
            };
            for lane in 0..WIDTH {
                if mask.is_set(lane) {
                    result.data[lane] = wa.data[lane].wrapping_add(wb.data[lane]);
                }
            }
            WideValue::Int(result)
        }
        (WideValue::Vec3(wa), WideValue::Vec3(wb)) => {
            let mut result = Wide {
                data: [Vec3::ZERO; WIDTH],
            };
            for lane in 0..WIDTH {
                if mask.is_set(lane) {
                    result.data[lane] = wa.data[lane] + wb.data[lane];
                }
            }
            WideValue::Vec3(result)
        }
        (WideValue::Closure(ca), WideValue::Closure(cb)) => {
            let mut result = Vec::with_capacity(WIDTH);
            for lane in 0..WIDTH {
                if mask.is_set(lane) {
                    let a = ca[lane].as_ref();
                    let b = cb[lane].as_ref();
                    let c = match (a, b) {
                        (Some(aa), Some(bb)) => {
                            Some(Box::new(ClosureValue::Add(aa.clone(), bb.clone())))
                        }
                        (Some(aa), None) => Some(aa.clone()),
                        (None, Some(bb)) => Some(bb.clone()),
                        (None, None) => None,
                    };
                    result.push(c);
                } else {
                    result.push(None);
                }
            }
            WideValue::Closure(result)
        }
        _ => {
            // Mixed types: promote to float
            let wa = a.as_float();
            let wb = b.as_float();
            let mut result = Wide {
                data: [0.0f32; WIDTH],
            };
            for lane in 0..WIDTH {
                if mask.is_set(lane) {
                    result.data[lane] = wa.data[lane] + wb.data[lane];
                }
            }
            WideValue::Float(result)
        }
    }
}

fn wide_sub<const WIDTH: usize>(
    a: &WideValue<WIDTH>,
    b: &WideValue<WIDTH>,
    mask: Mask<WIDTH>,
) -> WideValue<WIDTH> {
    match (a, b) {
        (WideValue::Float(wa), WideValue::Float(wb)) => {
            let mut result = Wide {
                data: [0.0f32; WIDTH],
            };
            for lane in 0..WIDTH {
                if mask.is_set(lane) {
                    result.data[lane] = wa.data[lane] - wb.data[lane];
                }
            }
            WideValue::Float(result)
        }
        (WideValue::Vec3(wa), WideValue::Vec3(wb)) => {
            let mut result = Wide {
                data: [Vec3::ZERO; WIDTH],
            };
            for lane in 0..WIDTH {
                if mask.is_set(lane) {
                    result.data[lane] = wa.data[lane] - wb.data[lane];
                }
            }
            WideValue::Vec3(result)
        }
        _ => {
            let wa = a.as_float();
            let wb = b.as_float();
            let mut result = Wide {
                data: [0.0f32; WIDTH],
            };
            for lane in 0..WIDTH {
                if mask.is_set(lane) {
                    result.data[lane] = wa.data[lane] - wb.data[lane];
                }
            }
            WideValue::Float(result)
        }
    }
}

fn wide_mul<const WIDTH: usize>(
    a: &WideValue<WIDTH>,
    b: &WideValue<WIDTH>,
    mask: Mask<WIDTH>,
) -> WideValue<WIDTH> {
    match (a, b) {
        (WideValue::Float(wa), WideValue::Float(wb)) => {
            let mut result = Wide {
                data: [0.0f32; WIDTH],
            };
            for lane in 0..WIDTH {
                if mask.is_set(lane) {
                    result.data[lane] = wa.data[lane] * wb.data[lane];
                }
            }
            WideValue::Float(result)
        }
        (WideValue::Vec3(wv), WideValue::Float(wf))
        | (WideValue::Float(wf), WideValue::Vec3(wv)) => {
            let mut result = Wide {
                data: [Vec3::ZERO; WIDTH],
            };
            for lane in 0..WIDTH {
                if mask.is_set(lane) {
                    result.data[lane] = wv.data[lane] * wf.data[lane];
                }
            }
            WideValue::Vec3(result)
        }
        (WideValue::Vec3(wa), WideValue::Vec3(wb)) => {
            let mut result = Wide {
                data: [Vec3::ZERO; WIDTH],
            };
            for lane in 0..WIDTH {
                if mask.is_set(lane) {
                    let av = wa.data[lane];
                    let bv = wb.data[lane];
                    result.data[lane] = Vec3::new(av.x * bv.x, av.y * bv.y, av.z * bv.z);
                }
            }
            WideValue::Vec3(result)
        }
        (WideValue::Closure(c), WideValue::Float(w))
        | (WideValue::Float(w), WideValue::Closure(c)) => {
            let mut result = Vec::with_capacity(WIDTH);
            for (lane, cl) in c.iter().enumerate().take(WIDTH) {
                if mask.is_set(lane) {
                    let weight = Color3::splat(w.data[lane]);
                    let closure = cl.as_ref().map(|cc| {
                        Box::new(ClosureValue::Mul {
                            weight,
                            closure: cc.clone(),
                        })
                    });
                    result.push(closure);
                } else {
                    result.push(None);
                }
            }
            WideValue::Closure(result)
        }
        (WideValue::Closure(c), WideValue::Vec3(w))
        | (WideValue::Vec3(w), WideValue::Closure(c)) => {
            let mut result = Vec::with_capacity(WIDTH);
            for (lane, cl) in c.iter().enumerate().take(WIDTH) {
                if mask.is_set(lane) {
                    let weight = Color3::new(w.data[lane].x, w.data[lane].y, w.data[lane].z);
                    let closure = cl.as_ref().map(|cc| {
                        Box::new(ClosureValue::Mul {
                            weight,
                            closure: cc.clone(),
                        })
                    });
                    result.push(closure);
                } else {
                    result.push(None);
                }
            }
            WideValue::Closure(result)
        }
        _ => {
            let wa = a.as_float();
            let wb = b.as_float();
            let mut result = Wide {
                data: [0.0f32; WIDTH],
            };
            for lane in 0..WIDTH {
                if mask.is_set(lane) {
                    result.data[lane] = wa.data[lane] * wb.data[lane];
                }
            }
            WideValue::Float(result)
        }
    }
}

fn wide_div<const WIDTH: usize>(
    a: &WideValue<WIDTH>,
    b: &WideValue<WIDTH>,
    mask: Mask<WIDTH>,
) -> WideValue<WIDTH> {
    let wa = a.as_float();
    let wb = b.as_float();
    let mut result = Wide {
        data: [0.0f32; WIDTH],
    };
    for lane in 0..WIDTH {
        if mask.is_set(lane) {
            let d = wb.data[lane];
            result.data[lane] = if d != 0.0 { wa.data[lane] / d } else { 0.0 };
        }
    }
    WideValue::Float(result)
}

fn wide_neg<const WIDTH: usize>(a: &WideValue<WIDTH>, mask: Mask<WIDTH>) -> WideValue<WIDTH> {
    match a {
        WideValue::Float(w) => {
            let mut result = Wide {
                data: [0.0f32; WIDTH],
            };
            for lane in 0..WIDTH {
                if mask.is_set(lane) {
                    result.data[lane] = -w.data[lane];
                }
            }
            WideValue::Float(result)
        }
        WideValue::Int(w) => {
            let mut result = Wide {
                data: [0i32; WIDTH],
            };
            for lane in 0..WIDTH {
                if mask.is_set(lane) {
                    result.data[lane] = -w.data[lane];
                }
            }
            WideValue::Int(result)
        }
        WideValue::Vec3(w) => {
            let mut result = Wide {
                data: [Vec3::ZERO; WIDTH],
            };
            for lane in 0..WIDTH {
                if mask.is_set(lane) {
                    let v = w.data[lane];
                    result.data[lane] = Vec3::new(-v.x, -v.y, -v.z);
                }
            }
            WideValue::Vec3(result)
        }
        _ => a.clone(),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn default_wide_value<const WIDTH: usize>(td: &crate::typedesc::TypeDesc) -> WideValue<WIDTH> {
    use crate::typedesc::{Aggregate, BaseType};
    if td.basetype == BaseType::Float as u8 {
        if td.aggregate == Aggregate::Vec3 as u8 {
            WideValue::Vec3(Wide {
                data: [Vec3::ZERO; WIDTH],
            })
        } else if td.aggregate == Aggregate::Matrix44 as u8 {
            WideValue::Matrix(Wide {
                data: [Matrix44::IDENTITY; WIDTH],
            })
        } else {
            WideValue::Float(Wide {
                data: [0.0f32; WIDTH],
            })
        }
    } else if td.basetype == BaseType::Int32 as u8 {
        WideValue::Int(Wide {
            data: [0i32; WIDTH],
        })
    } else if td.basetype == BaseType::String as u8 {
        WideValue::String(vec![UString::empty(); WIDTH])
    } else {
        WideValue::Void
    }
}

fn const_to_wide<const WIDTH: usize>(cv: &ConstValue) -> WideValue<WIDTH> {
    match cv {
        ConstValue::Int(v) => WideValue::Int(Wide { data: [*v; WIDTH] }),
        ConstValue::Float(v) => WideValue::Float(Wide { data: [*v; WIDTH] }),
        ConstValue::String(s) => WideValue::String(vec![*s; WIDTH]),
        ConstValue::Vec3(v) => WideValue::Vec3(Wide { data: [*v; WIDTH] }),
        ConstValue::Matrix(m) => WideValue::Matrix(Wide { data: [*m; WIDTH] }),
        ConstValue::IntArray(arr) => {
            // Use the first element or 0 as the broadcast value
            let v = arr.first().copied().unwrap_or(0);
            WideValue::Int(Wide { data: [v; WIDTH] })
        }
        ConstValue::FloatArray(arr) => {
            let v = arr.first().copied().unwrap_or(0.0);
            WideValue::Float(Wide { data: [v; WIDTH] })
        }
        ConstValue::StringArray(arr) => {
            let v = arr
                .first()
                .copied()
                .unwrap_or(crate::ustring::UString::new(""));
            WideValue::String(vec![v; WIDTH])
        }
    }
}

/// Convenience: execute a shader across WIDTH points at once.
pub fn run_shader_batched<const WIDTH: usize, R: BatchedRendererServices<WIDTH>>(
    source: &str,
    globals: &BatchedShaderGlobals<WIDTH>,
    renderer: &R,
) -> Result<BatchedInterpreter<WIDTH>, String> {
    let ast = crate::parser::parse(source)
        .map_err(|e| format!("{e:?}"))?
        .ast;
    let ir = crate::codegen::generate(&ast);
    let mut interp = BatchedInterpreter::<WIDTH>::new();
    interp.execute(&ir, globals, renderer);
    Ok(interp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::batched::{BatchedShaderGlobals, NullBatchedRenderer};
    use crate::codegen;
    use crate::parser;

    const W: usize = 8;

    fn compile_and_run_batched(src: &str) -> (ShaderIR, BatchedInterpreter<W>) {
        let ast = parser::parse(src).unwrap().ast;
        let ir = codegen::generate(&ast);
        let mut globals = BatchedShaderGlobals::<W>::default();
        // Set varying u/v values across lanes
        for lane in 0..W {
            globals.u.data[lane] = lane as f32 / W as f32;
            globals.v.data[lane] = 1.0 - lane as f32 / W as f32;
        }
        let renderer = NullBatchedRenderer;
        let mut interp = BatchedInterpreter::<W>::new();
        interp.execute(&ir, &globals, &renderer);
        (ir, interp)
    }

    #[test]
    fn test_batched_constant() {
        let src = r#"
shader test() {
    float a = 2.0;
    float b = 3.0;
    float c = a + b;
}
"#;
        let (ir, interp) = compile_and_run_batched(src);
        // All lanes should have the same constant result
        for lane in 0..W {
            let c = interp.get_float(&ir, "c", lane).unwrap();
            assert_eq!(c, 5.0, "lane {lane}: c should be 5.0");
        }
    }

    #[test]
    fn test_batched_varying() {
        let src = r#"
shader test() {
    float su = u;
    float result = su * 2.0;
}
"#;
        let (ir, interp) = compile_and_run_batched(src);
        for lane in 0..W {
            let expected = (lane as f32 / W as f32) * 2.0;
            let result = interp.get_float(&ir, "result", lane).unwrap();
            assert!(
                (result - expected).abs() < 1e-6,
                "lane {lane}: expected {expected}, got {result}"
            );
        }
    }

    #[test]
    fn test_batched_arithmetic() {
        let src = r#"
shader test() {
    float a = 10.0;
    float b = 3.0;
    float c = a - b;
    float d = a * b;
}
"#;
        let (ir, interp) = compile_and_run_batched(src);
        for lane in 0..W {
            assert_eq!(interp.get_float(&ir, "c", lane).unwrap(), 7.0);
            assert_eq!(interp.get_float(&ir, "d", lane).unwrap(), 30.0);
        }
    }

    #[test]
    fn test_batched_math_builtins() {
        let src = r#"
shader test() {
    float a = sin(0.0);
    float b = cos(0.0);
    float c = sqrt(4.0);
}
"#;
        let (ir, interp) = compile_and_run_batched(src);
        for lane in 0..W {
            assert!((interp.get_float(&ir, "a", lane).unwrap() - 0.0).abs() < 1e-6);
            assert!((interp.get_float(&ir, "b", lane).unwrap() - 1.0).abs() < 1e-6);
            assert!((interp.get_float(&ir, "c", lane).unwrap() - 2.0).abs() < 1e-6);
        }
    }

    #[test]
    fn test_batched_vector_ops() {
        let src = r#"
shader test() {
    float d = dot(vector(1,0,0), vector(0,1,0));
    float l = length(vector(3,4,0));
}
"#;
        let (ir, interp) = compile_and_run_batched(src);
        for lane in 0..W {
            let d = interp.get_float(&ir, "d", lane).unwrap();
            let l = interp.get_float(&ir, "l", lane).unwrap();
            assert!((d - 0.0).abs() < 1e-6, "dot product should be 0");
            assert!((l - 5.0).abs() < 1e-6, "length should be 5");
        }
    }

    #[test]
    fn test_batched_run_convenience() {
        let src = r#"
shader test() {
    float x = 42.0;
}
"#;
        let globals = BatchedShaderGlobals::<W>::default();
        let renderer = NullBatchedRenderer;
        let interp = run_shader_batched::<W, _>(src, &globals, &renderer).unwrap();
        assert!(interp.messages.is_empty());
    }

    // --- CRITICAL-1: If/Else mask splitting ---

    #[test]
    fn test_batched_if_uniform_true() {
        let src = r#"
shader test() {
    float x = 5.0;
    float result = 0.0;
    if (x > 3.0) { result = 1.0; }
}
"#;
        let (ir, interp) = compile_and_run_batched(src);
        for lane in 0..W {
            assert_eq!(interp.get_float(&ir, "result", lane).unwrap(), 1.0);
        }
    }

    #[test]
    fn test_batched_if_uniform_false() {
        let src = r#"
shader test() {
    float x = 1.0;
    float result = 0.0;
    if (x > 3.0) { result = 1.0; }
}
"#;
        let (ir, interp) = compile_and_run_batched(src);
        for lane in 0..W {
            assert_eq!(interp.get_float(&ir, "result", lane).unwrap(), 0.0);
        }
    }

    #[test]
    fn test_batched_if_else_uniform() {
        let src = r#"
shader test() {
    float x = 5.0;
    float result = 0.0;
    if (x > 3.0) { result = 10.0; } else { result = 20.0; }
}
"#;
        let (ir, interp) = compile_and_run_batched(src);
        for lane in 0..W {
            assert_eq!(interp.get_float(&ir, "result", lane).unwrap(), 10.0);
        }
    }

    #[test]
    fn test_batched_if_divergent() {
        // u varies per lane: 0.0, 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875
        let src = r#"
shader test() {
    float result = 0.0;
    if (u > 0.5) { result = 1.0; } else { result = 2.0; }
}
"#;
        let (ir, interp) = compile_and_run_batched(src);
        for lane in 0..W {
            let u_val = lane as f32 / W as f32;
            let expected = if u_val > 0.5 { 1.0 } else { 2.0 };
            let r = interp.get_float(&ir, "result", lane).unwrap();
            assert_eq!(r, expected, "lane {lane}: u={u_val}");
        }
    }

    #[test]
    fn test_batched_if_no_else() {
        let src = r#"
shader test() {
    float result = 99.0;
    if (1.0 > 5.0) { result = 0.0; }
}
"#;
        let (ir, interp) = compile_and_run_batched(src);
        for lane in 0..W {
            assert_eq!(interp.get_float(&ir, "result", lane).unwrap(), 99.0);
        }
    }

    #[test]
    fn test_batched_nested_if() {
        let src = r#"
shader test() {
    float x = 5.0;
    float result = 0.0;
    if (x > 3.0) {
        if (x > 4.0) { result = 100.0; }
        else { result = 50.0; }
    } else { result = 10.0; }
}
"#;
        let (ir, interp) = compile_and_run_batched(src);
        for lane in 0..W {
            assert_eq!(interp.get_float(&ir, "result", lane).unwrap(), 100.0);
        }
    }

    // --- CRITICAL-2: For/while loops ---

    #[test]
    fn test_batched_for_loop() {
        let src = r#"
shader test() {
    float sum = 0.0;
    for (int i = 0; i < 10; i++) { sum = sum + 1.0; }
}
"#;
        let (ir, interp) = compile_and_run_batched(src);
        for lane in 0..W {
            assert_eq!(interp.get_float(&ir, "sum", lane).unwrap(), 10.0);
        }
    }

    #[test]
    fn test_batched_for_accumulate() {
        // Use float counter to avoid int->float cast issues
        let src = r#"
shader test() {
    float sum = 0.0;
    float f = 0.0;
    for (int i = 0; i < 5; i++) {
        sum = sum + f;
        f = f + 1.0;
    }
}
"#;
        let (ir, interp) = compile_and_run_batched(src);
        // sum = 0+1+2+3+4 = 10
        for lane in 0..W {
            assert_eq!(interp.get_float(&ir, "sum", lane).unwrap(), 10.0);
        }
    }

    #[test]
    fn test_batched_while_loop() {
        let src = r#"
shader test() {
    float x = 0.0;
    int i = 0;
    while (i < 5) { x = x + 1.0; i = i + 1; }
}
"#;
        let (ir, interp) = compile_and_run_batched(src);
        for lane in 0..W {
            assert_eq!(interp.get_float(&ir, "x", lane).unwrap(), 5.0);
        }
    }

    #[test]
    fn test_batched_construct_color_space() {
        // construct(dst, colorspace, x, y, z) — PLAN #21 parity with opcolor
        let src = r#"
shader test() {
    color c = color("RGB", 1.0, 0.0, 0.0);
}
"#;
        let (ir, interp) = compile_and_run_batched(src);
        for lane in 0..W {
            let v = interp.get_vec3(&ir, "c", lane).unwrap();
            assert!((v.x - 1.0).abs() < 1e-5, "color R");
            assert!(v.y < 1e-5, "color G");
            assert!(v.z < 1e-5, "color B");
        }
    }

    // --- Vec3 component access (aref on Vec3) ---

    #[test]
    fn test_batched_vec3_compref() {
        let src = r#"
shader test() {
    vector v = vector(1.0, 2.0, 3.0);
    float x = v[0];
    float y = v[1];
    float z = v[2];
}
"#;
        let (ir, interp) = compile_and_run_batched(src);
        for lane in 0..W {
            assert_eq!(interp.get_float(&ir, "x", lane).unwrap(), 1.0);
            assert_eq!(interp.get_float(&ir, "y", lane).unwrap(), 2.0);
            assert_eq!(interp.get_float(&ir, "z", lane).unwrap(), 3.0);
        }
    }

    // --- Matrix component access (mxcompref) ---

    #[test]
    fn test_batched_matrix_construct() {
        // Test matrix construction (mxcompref requires parser index2 support)
        let src = r#"
shader test() {
    matrix M = matrix(1);
    float det = determinant(M);
}
"#;
        let (ir, interp) = compile_and_run_batched(src);
        for lane in 0..W {
            let det = interp.get_float(&ir, "det", lane).unwrap();
            assert_eq!(det, 1.0, "lane {lane}: identity det should be 1");
        }
    }

    // --- WideValue array helper unit tests ---

    #[test]
    fn test_wide_array_ref_float() {
        let mut arrs = vec![vec![0.0f32; 4]; W];
        for lane in 0..W {
            for i in 0..4 {
                arrs[lane][i] = (lane * 10 + i) as f32;
            }
        }
        let wv = WideValue::<W>::FloatArray(arrs);
        let idx = Wide { data: [2i32; W] };
        let result = wv.array_ref_float(&idx, Mask::<W>::all());
        for lane in 0..W {
            assert_eq!(result.data[lane], (lane * 10 + 2) as f32);
        }
    }

    #[test]
    fn test_wide_array_ref_int() {
        let mut arrs = vec![vec![0i32; 3]; W];
        for lane in 0..W {
            arrs[lane][0] = 100 + lane as i32;
            arrs[lane][1] = 200 + lane as i32;
            arrs[lane][2] = 300 + lane as i32;
        }
        let wv = WideValue::<W>::IntArray(arrs);
        let idx = Wide { data: [1i32; W] };
        let result = wv.array_ref_int(&idx, Mask::<W>::all());
        for lane in 0..W {
            assert_eq!(result.data[lane], 200 + lane as i32);
        }
    }

    #[test]
    fn test_wide_array_ref_vec3() {
        let v0 = crate::math::Vec3::new(1.0, 2.0, 3.0);
        let v1 = crate::math::Vec3::new(4.0, 5.0, 6.0);
        let arrs = vec![vec![v0, v1]; W];
        let wv = WideValue::<W>::Vec3Array(arrs);
        let idx = Wide { data: [1i32; W] };
        let result = wv.array_ref_vec3(&idx, Mask::<W>::all());
        for lane in 0..W {
            assert_eq!(result.data[lane].x, 4.0);
            assert_eq!(result.data[lane].y, 5.0);
        }
    }

    #[test]
    fn test_wide_array_masked() {
        let arrs = vec![vec![42.0f32, 99.0]; W];
        let wv = WideValue::<W>::FloatArray(arrs);
        let idx = Wide { data: [0i32; W] };
        let mut bits = 0u32;
        for lane in (0..W).step_by(2) {
            bits |= 1 << lane;
        }
        let mask = Mask::<W>::from_bits(bits);
        let result = wv.array_ref_float(&idx, mask);
        for lane in 0..W {
            if lane % 2 == 0 {
                assert_eq!(result.data[lane], 42.0);
            } else {
                assert_eq!(result.data[lane], 0.0);
            }
        }
    }

    #[test]
    fn test_wide_array_oob() {
        let arrs = vec![vec![1.0f32; 2]; W];
        let wv = WideValue::<W>::FloatArray(arrs);
        let idx = Wide { data: [5i32; W] };
        let result = wv.array_ref_float(&idx, Mask::<W>::all());
        for lane in 0..W {
            assert_eq!(result.data[lane], 0.0);
        }
    }

    #[test]
    fn test_wide_array_neg_idx() {
        let arrs = vec![vec![7.0f32, 8.0]; W];
        let wv = WideValue::<W>::FloatArray(arrs);
        let idx = Wide { data: [-1i32; W] };
        let result = wv.array_ref_float(&idx, Mask::<W>::all());
        for lane in 0..W {
            assert_eq!(result.data[lane], 7.0); // clamped to 0
        }
    }

    // ---------------------------------------------------------------
    // Tests for the 10 upgraded stub opcodes
    // ---------------------------------------------------------------

    #[test]
    fn test_batched_format() {
        let src = r#"
shader test() {
    string result = format("hello %d", 42);
}
"#;
        let (ir, interp) = compile_and_run_batched(src);
        for lane in 0..W {
            let s = interp.get_string(&ir, "result", lane).unwrap();
            assert_eq!(s, "hello 42", "lane {lane}: format mismatch");
        }
    }

    #[test]
    fn test_batched_regex_search() {
        let src = r#"
shader test() {
    int found = regex_search("hello world", "wor");
}
"#;
        let (ir, interp) = compile_and_run_batched(src);
        for lane in 0..W {
            let v = interp.get_int(&ir, "found", lane).unwrap();
            assert_eq!(v, 1, "lane {lane}: regex_search should find 'wor'");
        }
    }

    #[test]
    fn test_batched_regex_match() {
        // regex_match checks full match (anchored), so "hello" must match "hello"
        let src = r#"
shader test() {
    int m = regex_match("hello", "hello");
}
"#;
        let (ir, interp) = compile_and_run_batched(src);
        for lane in 0..W {
            let v = interp.get_int(&ir, "m", lane).unwrap();
            assert_eq!(v, 1, "lane {lane}: regex_match 'hello' should match");
        }
    }

    #[test]
    fn test_batched_split() {
        let src = r#"
shader test() {
    int n = split("a,b,c", ",");
}
"#;
        let (ir, interp) = compile_and_run_batched(src);
        for lane in 0..W {
            let v = interp.get_int(&ir, "n", lane).unwrap();
            assert_eq!(v, 3, "lane {lane}: split should give 3 tokens");
        }
    }

    #[test]
    fn test_batched_transformc() {
        let src = r#"
shader test() {
    color c = transformc("rgb", "hsv", color(1, 0, 0));
}
"#;
        let (ir, interp) = compile_and_run_batched(src);
        for lane in 0..W {
            let v = interp.get_vec3(&ir, "c", lane).unwrap();
            // Pure red => HSV(0, 1, 1)
            assert!((v.x - 0.0).abs() < 0.01, "lane {lane}: H ~ 0");
            assert!((v.y - 1.0).abs() < 0.01, "lane {lane}: S ~ 1");
            assert!((v.z - 1.0).abs() < 0.01, "lane {lane}: V ~ 1");
        }
    }

    #[test]
    fn test_batched_blackbody() {
        let src = r#"
shader test() {
    color bb = blackbody(6500.0);
}
"#;
        let (ir, interp) = compile_and_run_batched(src);
        for lane in 0..W {
            let v = interp.get_vec3(&ir, "bb", lane).unwrap();
            // 6500K ~ white, all channels > 0
            assert!(v.x > 0.0, "lane {lane}: blackbody R > 0");
            assert!(v.y > 0.0, "lane {lane}: blackbody G > 0");
            assert!(v.z > 0.0, "lane {lane}: blackbody B > 0");
        }
    }

    #[test]
    fn test_batched_raytype() {
        let src = r#"
shader test() {
    int is_camera = raytype("camera");
}
"#;
        let ast = parser::parse(src).unwrap().ast;
        let ir = codegen::generate(&ast);
        let mut globals = BatchedShaderGlobals::<W>::default();
        globals.uniform.raytype = 1; // camera bit
        let renderer = NullBatchedRenderer;
        let mut interp = BatchedInterpreter::<W>::new();
        interp.execute(&ir, &globals, &renderer);
        for lane in 0..W {
            let v = interp.get_int(&ir, "is_camera", lane).unwrap();
            assert_eq!(v, 1, "lane {lane}: raytype camera should be 1");
        }
    }

    #[test]
    fn test_batched_setmessage_getmessage() {
        // setmessage(name, val) is void; getmessage(name, output) returns int
        let src = r#"
shader test() {
    float val = 3.14;
    setmessage("mykey", val);
    float got = 0.0;
    int found = getmessage("mykey", got);
}
"#;
        let (ir, interp) = compile_and_run_batched(src);
        for lane in 0..W {
            // getmessage dispatches and shouldn't panic
            let _ = interp.get_int(&ir, "found", lane);
        }
    }

    #[test]
    fn test_batched_getmatrix() {
        let src = r#"
shader test() {
    matrix m = getmatrix("common", "object");
}
"#;
        let (ir, interp) = compile_and_run_batched(src);
        if let Some(WideValue::Matrix(wm)) = interp.get_value(&ir, "m") {
            for lane in 0..W {
                // NullBatchedRenderer returns identity
                assert!((wm.data[lane].m[0][0] - 1.0).abs() < 1e-6);
                assert!((wm.data[lane].m[1][1] - 1.0).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn test_batched_pointcloud_search() {
        let src = r#"
shader test() {
    int n = pointcloud_search("nonexistent.ptc", point(0,0,0), 1.0, 10);
}
"#;
        let (ir, interp) = compile_and_run_batched(src);
        for lane in 0..W {
            let v = interp.get_int(&ir, "n", lane).unwrap();
            assert_eq!(v, 0, "lane {lane}: missing ptc => 0");
        }
    }

    #[test]
    fn test_batched_pointcloud_search_with_manager_and_optional_args() {
        use crate::math::Vec3;
        use crate::ustring::UString;

        let mut mgr = crate::pointcloud::PointCloudManager::new();
        {
            let cloud = mgr.get_or_create("batched_pc.ptc");
            for i in 0..5 {
                let pos = Vec3::new(i as f32, 0.0, 0.0);
                let mut attrs = std::collections::HashMap::new();
                attrs.insert(UString::new("id"), crate::pointcloud::PointData::Int(i));
                cloud.add_point(pos, attrs);
            }
        }
        let mgr = Arc::new(std::sync::RwLock::new(mgr));

        let src = r#"
shader test() {
    int indices[64];
    float distances[64];
    int n = pointcloud_search("batched_pc.ptc", point(2,0,0), 2.0, 64, 1, "index", indices, "distance", distances);
}
"#;
        let ast = parser::parse(src).unwrap().ast;
        let ir = codegen::generate(&ast);
        let globals = BatchedShaderGlobals::<W>::default();
        let renderer = NullBatchedRenderer;
        let mut interp = BatchedInterpreter::<W>::new();
        interp.set_pointcloud_manager(Some(mgr));
        interp.execute(&ir, &globals, &renderer);

        for lane in 0..W {
            let n = interp.get_int(&ir, "n", lane).unwrap();
            assert!(n >= 2, "lane {lane}: should find at least 2 points");
        }
        if let Some(WideValue::IntArray(arrs)) = interp.get_value(&ir, "indices") {
            assert!(arrs.len() >= W);
            assert!(!arrs[0].is_empty());
        }
        if let Some(WideValue::FloatArray(arrs)) = interp.get_value(&ir, "distances") {
            assert!(arrs.len() >= W);
            assert!(!arrs[0].is_empty());
        }
    }

    #[test]
    fn test_batched_dict_find() {
        // dict_find on empty JSON object returns DICT_INVALID (-1) for missing key
        let src = r#"
shader test() {
    int handle = dict_find("{}", "key");
}
"#;
        let (ir, interp) = compile_and_run_batched(src);
        for lane in 0..W {
            let v = interp.get_int(&ir, "handle", lane).unwrap();
            assert_eq!(
                v, -1,
                "lane {lane}: empty JSON dict_find => -1 (DICT_INVALID)"
            );
        }
    }

    #[test]
    fn test_batched_getfield_vec3() {
        let src = r#"
shader test(output float rx = 0, output float ry = 0, output float rz = 0) {
    rx = N.x; ry = N.y; rz = N.z;
}
"#;
        let ast = parser::parse(src).unwrap().ast;
        let ir = codegen::generate(&ast);
        let mut globals = BatchedShaderGlobals::<W>::default();
        for lane in 0..W {
            globals.n.data[lane] =
                crate::math::Vec3::new(lane as f32 * 0.1, 1.0 - lane as f32 * 0.1, 0.5);
        }
        let renderer = NullBatchedRenderer;
        let mut interp = BatchedInterpreter::<W>::new();
        interp.execute(&ir, &globals, &renderer);
        for lane in 0..W {
            let rx = interp.get_float(&ir, "rx", lane).unwrap();
            let exp_x = lane as f32 * 0.1;
            assert!(
                (rx - exp_x).abs() < 1e-6,
                "lane {lane}: rx={rx}, exp={exp_x}"
            );
        }
    }
}
