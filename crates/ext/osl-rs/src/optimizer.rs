//! Runtime optimizer — constant folding, dead code elimination, peephole opts.
//!
//! Port of `runtimeoptimize.cpp`. Operates on ShaderIR to reduce opcode count
//! and simplify expressions before execution.

use crate::codegen::{ConstValue, ShaderIR};
use crate::symbol::{SymType, Symbol};
use crate::typedesc::TypeDesc;
use crate::typespec::TypeSpec;
use crate::ustring::UString;

/// Optimization statistics.
#[derive(Debug, Clone, Default)]
pub struct OptStats {
    pub constant_folds: u32,
    pub dead_ops_eliminated: u32,
    pub temps_coalesced: u32,
    pub peephole_opts: u32,
    pub copies_propagated: u32,
    pub useless_assigns: u32,
    pub total_passes: u32,
}

/// Optimization level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OptLevel {
    /// No optimization.
    O0,
    /// Basic optimization (constant folding + DCE).
    O1,
    /// Full optimization (O1 + peephole + coalescing).
    O2,
}

/// Optimize a ShaderIR in place.
pub fn optimize(ir: &mut ShaderIR, level: OptLevel) -> OptStats {
    let mut stats = OptStats::default();

    if level == OptLevel::O0 {
        return stats;
    }

    // Pre-pass: simplify params + resolve isconnected + coerce constants
    if level >= OptLevel::O1 {
        simplify_params(ir, &mut stats);
        resolve_isconnected(ir, &mut stats);
        coerce_assigned_constant(ir, &mut stats);
        find_params_holding_globals(ir, &mut stats);
    }

    // Multiple passes until convergence
    loop {
        stats.total_passes += 1;
        let changed_cf = constant_fold(ir, &mut stats);
        let changed_dce = dead_code_eliminate(ir, &mut stats);
        let changed_stale = stale_assign_eliminate(ir, &mut stats);
        let changed_mm = middleman_eliminate(ir, &mut stats);
        let changed_up = useparam_eliminate(ir, &mut stats);
        let changed_ph = if level >= OptLevel::O2 {
            peephole(ir, &mut stats)
        } else {
            false
        };
        let changed_ph2 = if level >= OptLevel::O2 {
            peephole2(ir, &mut stats)
        } else {
            false
        };
        let changed_pa = if level >= OptLevel::O2 {
            peephole_arith(ir, &mut stats)
        } else {
            false
        };
        let changed_cp = if level >= OptLevel::O1 {
            copy_propagate(ir, &mut stats)
        } else {
            false
        };
        let changed_ua = if level >= OptLevel::O2 {
            useless_assign_elim(ir, &mut stats)
        } else {
            false
        };
        let changed_mix = if level >= OptLevel::O2 {
            opt_mix(ir, &mut stats)
        } else {
            false
        };
        let changed_out = if level >= OptLevel::O2 {
            outparam_assign_elision(ir, &mut stats)
        } else {
            false
        };
        let changed_rm = if level >= OptLevel::O2 {
            remove_unused_params(ir, &mut stats)
        } else {
            false
        };
        let _changed_ga = if level >= OptLevel::O2 {
            opt_fold_getattribute(ir, &mut stats)
        } else {
            false
        };

        if !changed_cf
            && !changed_dce
            && !changed_stale
            && !changed_mm
            && !changed_up
            && !changed_ph
            && !changed_ph2
            && !changed_pa
            && !changed_cp
            && !changed_ua
            && !changed_mix
            && !changed_out
            && !changed_rm
        {
            break;
        }
        if stats.total_passes > 20 {
            break; // safety limit
        }
    }

    if level >= OptLevel::O2 {
        coalesce_temps(ir, &mut stats);
    }

    // Collapse nops and unused symbols after all optimization
    collapse_syms(ir);
    collapse_ops(ir);

    stats
}

/// Look up a constant float value from const_values.
fn get_const_float(ir: &ShaderIR, sym_idx: usize) -> Option<f32> {
    use crate::codegen::ConstValue;
    for &(idx, ref cv) in &ir.const_values {
        if idx == sym_idx {
            return match cv {
                ConstValue::Float(v) => Some(*v),
                ConstValue::Int(v) => Some(*v as f32),
                _ => None,
            };
        }
    }
    None
}

/// Look up a constant int value from const_values.
fn get_const_int(ir: &ShaderIR, sym_idx: usize) -> Option<i32> {
    use crate::codegen::ConstValue;
    for &(idx, ref cv) in &ir.const_values {
        if idx == sym_idx {
            return match cv {
                ConstValue::Int(v) => Some(*v),
                _ => None,
            };
        }
    }
    None
}

/// Create or find a constant float in the IR.
fn ensure_const_float(ir: &mut ShaderIR, val: f32) -> i32 {
    use crate::codegen::ConstValue;
    use crate::symbol::Symbol;
    use crate::typedesc::TypeDesc;
    use crate::typespec::TypeSpec;

    // Check if we already have this constant
    for &(idx, ref cv) in &ir.const_values {
        if let ConstValue::Float(v) = cv {
            if (*v - val).abs() < f32::EPSILON {
                return idx as i32;
            }
        }
    }

    // Create a new constant
    let idx = ir.symbols.len();
    let name = format!("$opt_f{val}");
    let mut sym = Symbol::new(
        UString::new(&name),
        TypeSpec::from_simple(TypeDesc::FLOAT),
        SymType::Const,
    );
    sym.initializers = 1;
    ir.symbols.push(sym);
    ir.const_values.push((idx, ConstValue::Float(val)));
    idx as i32
}

/// Create or find a constant int in the IR.
fn ensure_const_int(ir: &mut ShaderIR, val: i32) -> i32 {
    use crate::codegen::ConstValue;
    use crate::symbol::Symbol;
    use crate::typedesc::TypeDesc;
    use crate::typespec::TypeSpec;

    for &(idx, ref cv) in &ir.const_values {
        if let ConstValue::Int(v) = cv {
            if *v == val {
                return idx as i32;
            }
        }
    }

    let idx = ir.symbols.len();
    let name = format!("$opt_i{val}");
    let mut sym = Symbol::new(
        UString::new(&name),
        TypeSpec::from_simple(TypeDesc::INT),
        SymType::Const,
    );
    sym.initializers = 1;
    ir.symbols.push(sym);
    ir.const_values.push((idx, ConstValue::Int(val)));
    idx as i32
}

/// Look up a constant string value from const_values.
fn get_const_str(ir: &ShaderIR, sym_idx: usize) -> Option<String> {
    use crate::codegen::ConstValue;
    for &(idx, ref cv) in &ir.const_values {
        if idx == sym_idx {
            return match cv {
                ConstValue::String(s) => Some(s.as_str().to_string()),
                _ => None,
            };
        }
    }
    None
}

/// Look up a constant Vec3 value from const_values.
fn get_const_vec3(ir: &ShaderIR, sym_idx: usize) -> Option<crate::math::Vec3> {
    use crate::codegen::ConstValue;
    for &(idx, ref cv) in &ir.const_values {
        if idx == sym_idx {
            return match cv {
                ConstValue::Vec3(v) => Some(*v),
                _ => None,
            };
        }
    }
    None
}

/// Look up a constant float array from const_values.
fn get_const_float_array(ir: &ShaderIR, sym_idx: usize) -> Option<Vec<f32>> {
    use crate::codegen::ConstValue;
    for &(idx, ref cv) in &ir.const_values {
        if idx == sym_idx {
            return match cv {
                ConstValue::FloatArray(v) => Some(v.clone()),
                _ => None,
            };
        }
    }
    None
}

/// Look up a constant int array from const_values.
fn get_const_int_array(ir: &ShaderIR, sym_idx: usize) -> Option<Vec<i32>> {
    use crate::codegen::ConstValue;
    for &(idx, ref cv) in &ir.const_values {
        if idx == sym_idx {
            return match cv {
                ConstValue::IntArray(v) => Some(v.clone()),
                _ => None,
            };
        }
    }
    None
}

/// Create or find a constant string in the IR.
fn ensure_const_str(ir: &mut ShaderIR, val: &str) -> i32 {
    use crate::codegen::ConstValue;
    use crate::symbol::Symbol;
    use crate::typedesc::TypeDesc;
    use crate::typespec::TypeSpec;

    for &(idx, ref cv) in &ir.const_values {
        if let ConstValue::String(s) = cv {
            if s == val {
                return idx as i32;
            }
        }
    }

    let idx = ir.symbols.len();
    let name = format!("$opt_s{val}");
    let mut sym = Symbol::new(
        UString::new(&name),
        TypeSpec::from_simple(TypeDesc::STRING),
        SymType::Const,
    );
    sym.initializers = 1;
    ir.symbols.push(sym);
    ir.const_values
        .push((idx, ConstValue::String(UString::new(val))));
    idx as i32
}

/// Create or find a constant Vec3 in the IR.
fn ensure_const_vec3(ir: &mut ShaderIR, v: crate::math::Vec3) -> i32 {
    use crate::codegen::ConstValue;
    use crate::symbol::Symbol;
    use crate::typedesc::TypeDesc;
    use crate::typespec::TypeSpec;

    let idx = ir.symbols.len();
    let name = format!("$opt_v{}_{}_{}", v.x, v.y, v.z);
    let mut sym = Symbol::new(
        UString::new(&name),
        TypeSpec::from_simple(TypeDesc::COLOR),
        SymType::Const,
    );
    sym.initializers = 1;
    ir.symbols.push(sym);
    ir.const_values.push((idx, ConstValue::Vec3(v)));
    idx as i32
}

/// Create a constant float array in the IR.
fn ensure_const_float_array(ir: &mut ShaderIR, vals: &[f32]) -> i32 {
    use crate::codegen::ConstValue;
    use crate::symbol::Symbol;
    use crate::typedesc::TypeDesc;
    use crate::typespec::TypeSpec;

    let idx = ir.symbols.len();
    let name = format!("$opt_fa{}", vals.len());
    let mut sym = Symbol::new(
        UString::new(&name),
        TypeSpec::from_simple(TypeDesc::FLOAT.array(vals.len() as i32)),
        SymType::Const,
    );
    sym.initializers = vals.len() as i32;
    ir.symbols.push(sym);
    ir.const_values
        .push((idx, ConstValue::FloatArray(vals.to_vec())));
    idx as i32
}

/// Create a constant matrix in the IR.
fn ensure_const_matrix(ir: &mut ShaderIR, m: crate::math::Matrix44) -> i32 {
    use crate::codegen::ConstValue;
    use crate::symbol::Symbol;
    use crate::typedesc::TypeDesc;
    use crate::typespec::TypeSpec;

    let idx = ir.symbols.len();
    let name = format!("$opt_mx{}", idx);
    let mut sym = Symbol::new(
        UString::new(&name),
        TypeSpec::from_simple(TypeDesc::MATRIX),
        SymType::Const,
    );
    sym.initializers = 1;
    ir.symbols.push(sym);
    ir.const_values.push((idx, ConstValue::Matrix(m)));
    idx as i32
}

/// Constant folding: evaluate ops with constant operands at compile time.
fn constant_fold(ir: &mut ShaderIR, stats: &mut OptStats) -> bool {
    let mut changed = false;

    for i in 0..ir.opcodes.len() {
        let op = &ir.opcodes[i];
        let opname = op.op.as_str().to_string();

        // Skip already-eliminated ops
        if opname == "nop" || opname.is_empty() {
            continue;
        }

        let nargs = op.nargs as usize;
        let firstarg = op.firstarg as usize;

        if nargs < 1 {
            continue;
        }

        // --- Fold binary arithmetic with two constants ---
        if matches!(opname.as_str(), "add" | "sub" | "mul" | "div" | "mod") && nargs >= 3 {
            let dst_idx = ir.args[firstarg] as usize;
            let src1_idx = ir.args[firstarg + 1] as usize;
            let src2_idx = ir.args[firstarg + 2] as usize;

            if src1_idx < ir.symbols.len()
                && src2_idx < ir.symbols.len()
                && ir.symbols[src1_idx].symtype == SymType::Const
                && ir.symbols[src2_idx].symtype == SymType::Const
            {
                // Try float folding
                if let (Some(v1), Some(v2)) =
                    (get_const_float(ir, src1_idx), get_const_float(ir, src2_idx))
                {
                    let result = match opname.as_str() {
                        "add" => Some(v1 + v2),
                        "sub" => Some(v1 - v2),
                        "mul" => Some(v1 * v2),
                        // OSL semantics: A / 0 => 0
                        "div" => {
                            if v2 != 0.0 {
                                Some(v1 / v2)
                            } else {
                                Some(0.0)
                            }
                        }
                        // OSL semantics: A % 0 => 0
                        "mod" => {
                            if v2 != 0.0 {
                                Some(v1 % v2)
                            } else {
                                Some(0.0)
                            }
                        }
                        _ => None,
                    };
                    if let Some(result_val) = result {
                        let const_idx = ensure_const_float(ir, result_val);
                        // Replace op with: assign dst, const
                        ir.opcodes[i].op = UString::new("assign");
                        ir.opcodes[i].nargs = 2;
                        ir.args[firstarg] = dst_idx as i32;
                        ir.args[firstarg + 1] = const_idx;
                        stats.constant_folds += 1;
                        changed = true;
                        continue;
                    }
                }

                // Try int folding
                if let (Some(v1), Some(v2)) =
                    (get_const_int(ir, src1_idx), get_const_int(ir, src2_idx))
                {
                    let result = match opname.as_str() {
                        "add" => Some(v1.wrapping_add(v2)),
                        "sub" => Some(v1.wrapping_sub(v2)),
                        "mul" => Some(v1.wrapping_mul(v2)),
                        // OSL semantics: A / 0 => 0
                        "div" => {
                            if v2 != 0 {
                                Some(v1 / v2)
                            } else {
                                Some(0)
                            }
                        }
                        // OSL semantics: A % 0 => 0
                        "mod" => {
                            if v2 != 0 {
                                Some(v1 % v2)
                            } else {
                                Some(0)
                            }
                        }
                        _ => None,
                    };
                    if let Some(result_val) = result {
                        let const_idx = ensure_const_int(ir, result_val);
                        ir.opcodes[i].op = UString::new("assign");
                        ir.opcodes[i].nargs = 2;
                        ir.args[firstarg] = dst_idx as i32;
                        ir.args[firstarg + 1] = const_idx;
                        stats.constant_folds += 1;
                        changed = true;
                        continue;
                    }
                }
            }
        }

        // --- Fold comparison with two constants ---
        if matches!(opname.as_str(), "eq" | "neq" | "lt" | "gt" | "le" | "ge") && nargs >= 3 {
            let dst_idx = ir.args[firstarg] as usize;
            let src1_idx = ir.args[firstarg + 1] as usize;
            let src2_idx = ir.args[firstarg + 2] as usize;

            if src1_idx < ir.symbols.len()
                && src2_idx < ir.symbols.len()
                && ir.symbols[src1_idx].symtype == SymType::Const
                && ir.symbols[src2_idx].symtype == SymType::Const
            {
                if let (Some(v1), Some(v2)) =
                    (get_const_float(ir, src1_idx), get_const_float(ir, src2_idx))
                {
                    // C++ uses exact memcmp (equal_consts), not epsilon comparison
                    let result = match opname.as_str() {
                        "eq" => Some(v1 == v2),
                        "neq" => Some(v1 != v2),
                        "lt" => Some(v1 < v2),
                        "gt" => Some(v1 > v2),
                        "le" => Some(v1 <= v2),
                        "ge" => Some(v1 >= v2),
                        _ => None,
                    };
                    if let Some(result_val) = result {
                        let const_idx = ensure_const_int(ir, if result_val { 1 } else { 0 });
                        ir.opcodes[i].op = UString::new("assign");
                        ir.opcodes[i].nargs = 2;
                        ir.args[firstarg] = dst_idx as i32;
                        ir.args[firstarg + 1] = const_idx;
                        stats.constant_folds += 1;
                        changed = true;
                        continue;
                    }
                }

                // Try int comparison (C++ handles int-int, int-float, float-int)
                if let (Some(v1), Some(v2)) =
                    (get_const_int(ir, src1_idx), get_const_int(ir, src2_idx))
                {
                    let result = match opname.as_str() {
                        "eq" => Some(v1 == v2),
                        "neq" => Some(v1 != v2),
                        "lt" => Some(v1 < v2),
                        "gt" => Some(v1 > v2),
                        "le" => Some(v1 <= v2),
                        "ge" => Some(v1 >= v2),
                        _ => None,
                    };
                    if let Some(result_val) = result {
                        let const_idx = ensure_const_int(ir, if result_val { 1 } else { 0 });
                        ir.opcodes[i].op = UString::new("assign");
                        ir.opcodes[i].nargs = 2;
                        ir.args[firstarg] = dst_idx as i32;
                        ir.args[firstarg + 1] = const_idx;
                        stats.constant_folds += 1;
                        changed = true;
                        continue;
                    }
                }
            }
        }

        // --- Fold negation of constant ---
        if opname == "neg" && nargs >= 2 {
            let dst_idx = ir.args[firstarg] as usize;
            let src_idx = ir.args[firstarg + 1] as usize;

            if src_idx < ir.symbols.len() && ir.symbols[src_idx].symtype == SymType::Const {
                if let Some(v) = get_const_float(ir, src_idx) {
                    let const_idx = ensure_const_float(ir, -v);
                    ir.opcodes[i].op = UString::new("assign");
                    ir.opcodes[i].nargs = 2;
                    ir.args[firstarg] = dst_idx as i32;
                    ir.args[firstarg + 1] = const_idx;
                    stats.constant_folds += 1;
                    changed = true;
                    continue;
                }
                // C++ also handles int negation
                if let Some(v) = get_const_int(ir, src_idx) {
                    let const_idx = ensure_const_int(ir, -v);
                    ir.opcodes[i].op = UString::new("assign");
                    ir.opcodes[i].nargs = 2;
                    ir.args[firstarg] = dst_idx as i32;
                    ir.args[firstarg + 1] = const_idx;
                    stats.constant_folds += 1;
                    changed = true;
                    continue;
                }
            }
        }

        // --- Fold unary math builtins on constants ---
        if matches!(
            opname.as_str(),
            "sin"
                | "cos"
                | "tan"
                | "asin"
                | "acos"
                | "atan"
                | "sinh"
                | "cosh"
                | "tanh"
                | "exp"
                | "exp2"
                | "expm1"
                | "log"
                | "log2"
                | "log10"
                | "sqrt"
                | "inversesqrt"
                | "cbrt"
                | "abs"
                | "fabs"
                | "floor"
                | "ceil"
                | "round"
                | "trunc"
                | "sign"
                | "radians"
                | "degrees"
                | "erf"
                | "erfc"
        ) && nargs >= 2
        {
            let dst_idx = ir.args[firstarg] as usize;
            let src_idx = ir.args[firstarg + 1] as usize;

            if src_idx < ir.symbols.len() && ir.symbols[src_idx].symtype == SymType::Const {
                if let Some(v) = get_const_float(ir, src_idx) {
                    let result = match opname.as_str() {
                        "sin" => Some(v.sin()),
                        "cos" => Some(v.cos()),
                        "tan" => Some(v.tan()),
                        "asin" => Some(v.clamp(-1.0, 1.0).asin()),
                        "acos" => Some(v.clamp(-1.0, 1.0).acos()),
                        "atan" => Some(v.atan()),
                        "sinh" => Some(v.sinh()),
                        "cosh" => Some(v.cosh()),
                        "tanh" => Some(v.tanh()),
                        "exp" => Some(v.exp()),
                        "exp2" => Some(v.exp2()),
                        "expm1" => Some(v.exp() - 1.0),
                        "log" => {
                            if v > 0.0 {
                                Some(v.ln())
                            } else {
                                None
                            }
                        }
                        "log2" => {
                            if v > 0.0 {
                                Some(v.log2())
                            } else {
                                None
                            }
                        }
                        "log10" => {
                            if v > 0.0 {
                                Some(v.log10())
                            } else {
                                None
                            }
                        }
                        "sqrt" => {
                            if v >= 0.0 {
                                Some(v.sqrt())
                            } else {
                                None
                            }
                        }
                        "inversesqrt" => {
                            if v > 0.0 {
                                Some(1.0 / v.sqrt())
                            } else {
                                None
                            }
                        }
                        "cbrt" => Some(v.cbrt()),
                        "abs" | "fabs" => Some(v.abs()),
                        "floor" => Some(v.floor()),
                        "ceil" => Some(v.ceil()),
                        "round" => Some(v.round()),
                        "trunc" => Some(v.trunc()),
                        "sign" => Some(if v > 0.0 {
                            1.0
                        } else if v < 0.0 {
                            -1.0
                        } else {
                            0.0
                        }),
                        "radians" => Some(v * std::f32::consts::PI / 180.0),
                        "degrees" => Some(v * 180.0 / std::f32::consts::PI),
                        "erf" => Some(libm::erff(v)),
                        "erfc" => Some(libm::erfcf(v)),
                        _ => None,
                    };
                    if let Some(result_val) = result {
                        let const_idx = ensure_const_float(ir, result_val);
                        ir.opcodes[i].op = UString::new("assign");
                        ir.opcodes[i].nargs = 2;
                        ir.args[firstarg] = dst_idx as i32;
                        ir.args[firstarg + 1] = const_idx;
                        stats.constant_folds += 1;
                        changed = true;
                        continue;
                    }
                }
            }
        }

        // --- Fold: abs/fabs for int constant (C++ constfold_abs handles int) ---
        if matches!(opname.as_str(), "abs" | "fabs") && nargs >= 2 {
            let dst_idx = ir.args[firstarg] as usize;
            let src_idx = ir.args[firstarg + 1] as usize;

            if src_idx < ir.symbols.len() && ir.symbols[src_idx].symtype == SymType::Const {
                if let Some(v) = get_const_int(ir, src_idx) {
                    let const_idx = ensure_const_int(ir, v.abs());
                    ir.opcodes[i].op = UString::new("assign");
                    ir.opcodes[i].nargs = 2;
                    ir.args[firstarg] = dst_idx as i32;
                    ir.args[firstarg + 1] = const_idx;
                    stats.constant_folds += 1;
                    changed = true;
                    continue;
                }
            }
        }

        // --- Fold bitwise ops: bitand, bitor, xor, compl (C++ constfold_bitand etc.) ---
        if matches!(opname.as_str(), "bitand" | "bitor" | "xor") && nargs >= 3 {
            let dst_idx = ir.args[firstarg] as usize;
            let src1_idx = ir.args[firstarg + 1] as usize;
            let src2_idx = ir.args[firstarg + 2] as usize;

            if src1_idx < ir.symbols.len()
                && src2_idx < ir.symbols.len()
                && ir.symbols[src1_idx].symtype == SymType::Const
                && ir.symbols[src2_idx].symtype == SymType::Const
            {
                if let (Some(v1), Some(v2)) =
                    (get_const_int(ir, src1_idx), get_const_int(ir, src2_idx))
                {
                    let result = match opname.as_str() {
                        "bitand" => Some(v1 & v2),
                        "bitor" => Some(v1 | v2),
                        "xor" => Some(v1 ^ v2),
                        _ => None,
                    };
                    if let Some(result_val) = result {
                        let const_idx = ensure_const_int(ir, result_val);
                        ir.opcodes[i].op = UString::new("assign");
                        ir.opcodes[i].nargs = 2;
                        ir.args[firstarg] = dst_idx as i32;
                        ir.args[firstarg + 1] = const_idx;
                        stats.constant_folds += 1;
                        changed = true;
                        continue;
                    }
                }
            }
        }

        // --- Fold: compl (bitwise complement, unary) ---
        if opname == "compl" && nargs >= 2 {
            let dst_idx = ir.args[firstarg] as usize;
            let src_idx = ir.args[firstarg + 1] as usize;

            if src_idx < ir.symbols.len() && ir.symbols[src_idx].symtype == SymType::Const {
                if let Some(v) = get_const_int(ir, src_idx) {
                    let const_idx = ensure_const_int(ir, !v);
                    ir.opcodes[i].op = UString::new("assign");
                    ir.opcodes[i].nargs = 2;
                    ir.args[firstarg] = dst_idx as i32;
                    ir.args[firstarg + 1] = const_idx;
                    stats.constant_folds += 1;
                    changed = true;
                    continue;
                }
            }
        }

        // --- Fold logical ops: and, or (C++ constfold_and, constfold_or) ---
        if matches!(opname.as_str(), "and" | "or") && nargs >= 3 {
            let dst_idx = ir.args[firstarg] as usize;
            let src1_idx = ir.args[firstarg + 1] as usize;
            let src2_idx = ir.args[firstarg + 2] as usize;

            if src1_idx < ir.symbols.len()
                && src2_idx < ir.symbols.len()
                && ir.symbols[src1_idx].symtype == SymType::Const
                && ir.symbols[src2_idx].symtype == SymType::Const
            {
                if let (Some(v1), Some(v2)) =
                    (get_const_int(ir, src1_idx), get_const_int(ir, src2_idx))
                {
                    let result = match opname.as_str() {
                        "and" => Some(if v1 != 0 && v2 != 0 { 1 } else { 0 }),
                        "or" => Some(if v1 != 0 || v2 != 0 { 1 } else { 0 }),
                        _ => None,
                    };
                    if let Some(result_val) = result {
                        let const_idx = ensure_const_int(ir, result_val);
                        ir.opcodes[i].op = UString::new("assign");
                        ir.opcodes[i].nargs = 2;
                        ir.args[firstarg] = dst_idx as i32;
                        ir.args[firstarg + 1] = const_idx;
                        stats.constant_folds += 1;
                        changed = true;
                        continue;
                    }
                }
            }
        }

        // --- Fold: clamp(val, lo, hi) with all constants ---
        if opname == "clamp" && nargs >= 4 {
            let dst_idx = ir.args[firstarg] as usize;
            let val_idx = ir.args[firstarg + 1] as usize;
            let lo_idx = ir.args[firstarg + 2] as usize;
            let hi_idx = ir.args[firstarg + 3] as usize;

            if val_idx < ir.symbols.len()
                && lo_idx < ir.symbols.len()
                && hi_idx < ir.symbols.len()
                && ir.symbols[val_idx].symtype == SymType::Const
                && ir.symbols[lo_idx].symtype == SymType::Const
                && ir.symbols[hi_idx].symtype == SymType::Const
            {
                if let (Some(val), Some(lo), Some(hi)) = (
                    get_const_float(ir, val_idx),
                    get_const_float(ir, lo_idx),
                    get_const_float(ir, hi_idx),
                ) {
                    let result = val.clamp(lo, hi);
                    let const_idx = ensure_const_float(ir, result);
                    ir.opcodes[i].op = UString::new("assign");
                    ir.opcodes[i].nargs = 2;
                    ir.args[firstarg] = dst_idx as i32;
                    ir.args[firstarg + 1] = const_idx;
                    stats.constant_folds += 1;
                    changed = true;
                    continue;
                }
            }
        }

        // --- Fold binary math builtins: atan2, pow, min, max, step ---
        if matches!(
            opname.as_str(),
            "atan2" | "pow" | "min" | "max" | "step" | "fmod" | "safe_div"
        ) && nargs >= 3
        {
            let dst_idx = ir.args[firstarg] as usize;
            let src1_idx = ir.args[firstarg + 1] as usize;
            let src2_idx = ir.args[firstarg + 2] as usize;

            if src1_idx < ir.symbols.len()
                && src2_idx < ir.symbols.len()
                && ir.symbols[src1_idx].symtype == SymType::Const
                && ir.symbols[src2_idx].symtype == SymType::Const
            {
                if let (Some(v1), Some(v2)) =
                    (get_const_float(ir, src1_idx), get_const_float(ir, src2_idx))
                {
                    let result = match opname.as_str() {
                        "atan2" => Some(v1.atan2(v2)),
                        "pow" => Some(v1.powf(v2)),
                        "min" => Some(v1.min(v2)),
                        "max" => Some(v1.max(v2)),
                        "step" => Some(if v2 < v1 { 0.0 } else { 1.0 }),
                        "fmod" => {
                            if v2 != 0.0 {
                                Some(v1 % v2)
                            } else {
                                None
                            }
                        }
                        "safe_div" => {
                            if v2 != 0.0 {
                                Some(v1 / v2)
                            } else {
                                Some(0.0)
                            }
                        }
                        _ => None,
                    };
                    if let Some(result_val) = result {
                        let const_idx = ensure_const_float(ir, result_val);
                        ir.opcodes[i].op = UString::new("assign");
                        ir.opcodes[i].nargs = 2;
                        ir.args[firstarg] = dst_idx as i32;
                        ir.args[firstarg + 1] = const_idx;
                        stats.constant_folds += 1;
                        changed = true;
                        continue;
                    }
                }
            }
        }

        // --- Fold: assign x, x → nop ---
        if opname == "assign" && nargs >= 2 {
            let dst = ir.args[firstarg];
            let src = ir.args[firstarg + 1];
            if dst == src {
                ir.opcodes[i].op = UString::new("nop");
                ir.opcodes[i].nargs = 0;
                stats.dead_ops_eliminated += 1;
                changed = true;
                continue;
            }
        }

        // --- Fold: mul x, y, 1 → assign x, y ---
        if opname == "mul" && nargs >= 3 {
            let dst_idx = ir.args[firstarg] as usize;
            let src1_idx = ir.args[firstarg + 1] as usize;
            let src2_idx = ir.args[firstarg + 2] as usize;

            if src2_idx < ir.symbols.len() && ir.symbols[src2_idx].symtype == SymType::Const {
                if let Some(v) = get_const_float(ir, src2_idx) {
                    if (v - 1.0).abs() < f32::EPSILON {
                        ir.opcodes[i].op = UString::new("assign");
                        ir.opcodes[i].nargs = 2;
                        ir.args[firstarg] = dst_idx as i32;
                        ir.args[firstarg + 1] = src1_idx as i32;
                        stats.constant_folds += 1;
                        changed = true;
                        continue;
                    }
                    if v == 0.0 {
                        let zero_idx = ensure_const_float(ir, 0.0);
                        ir.opcodes[i].op = UString::new("assign");
                        ir.opcodes[i].nargs = 2;
                        ir.args[firstarg] = dst_idx as i32;
                        ir.args[firstarg + 1] = zero_idx;
                        stats.constant_folds += 1;
                        changed = true;
                        continue;
                    }
                }
            }
        }

        // --- Fold: add x, y, 0 → assign x, y ---
        if opname == "add" && nargs >= 3 {
            let dst_idx = ir.args[firstarg] as usize;
            let src1_idx = ir.args[firstarg + 1] as usize;
            let src2_idx = ir.args[firstarg + 2] as usize;

            if src2_idx < ir.symbols.len() && ir.symbols[src2_idx].symtype == SymType::Const {
                if let Some(v) = get_const_float(ir, src2_idx) {
                    if v == 0.0 {
                        ir.opcodes[i].op = UString::new("assign");
                        ir.opcodes[i].nargs = 2;
                        ir.args[firstarg] = dst_idx as i32;
                        ir.args[firstarg + 1] = src1_idx as i32;
                        stats.constant_folds += 1;
                        changed = true;
                        continue;
                    }
                }
            }
        }

        // --- Fold: div x, y, 1 → assign x, y ---
        if opname == "div" && nargs >= 3 {
            let dst_idx = ir.args[firstarg] as usize;
            let src1_idx = ir.args[firstarg + 1] as usize;
            let src2_idx = ir.args[firstarg + 2] as usize;

            if src2_idx < ir.symbols.len() && ir.symbols[src2_idx].symtype == SymType::Const {
                if let Some(v) = get_const_float(ir, src2_idx) {
                    if (v - 1.0).abs() < f32::EPSILON {
                        ir.opcodes[i].op = UString::new("assign");
                        ir.opcodes[i].nargs = 2;
                        ir.args[firstarg] = dst_idx as i32;
                        ir.args[firstarg + 1] = src1_idx as i32;
                        stats.constant_folds += 1;
                        changed = true;
                        continue;
                    }
                }
            }
        }

        // --- Fold: if with constant condition ---
        // OSL codegen layout: jump[0] = true_start, jump[1] = false_start (or end)
        // True branch = opcodes[jump[0]..jump[1])
        // False branch = opcodes[jump[1]..end)
        if opname == "if" && nargs >= 1 {
            let cond_idx = ir.args[firstarg] as usize;
            if cond_idx < ir.symbols.len() && ir.symbols[cond_idx].symtype == SymType::Const {
                if let Some(cv) = get_const_int(ir, cond_idx) {
                    let true_start = ir.opcodes[i].jump[0];
                    let false_start = ir.opcodes[i].jump[1];
                    if cv != 0 {
                        // Condition is true: nop the if, nop the false branch
                        ir.opcodes[i].op = UString::new("nop");
                        ir.opcodes[i].nargs = 0;
                        // Find end of else-block: the NOP before false_start
                        // has jump[0] = end_of_if (our codegen inserts it)
                        if false_start >= 0 {
                            let fs = false_start as usize;
                            // Look for end marker: NOP just before false_start
                            let else_end = if fs > 0 && ir.opcodes[fs - 1].jump[0] >= 0 {
                                ir.opcodes[fs - 1].jump[0] as usize
                            } else {
                                ir.opcodes.len() // fallback: nop to end
                            };
                            for j in fs..else_end {
                                if j < ir.opcodes.len() {
                                    ir.opcodes[j].op = UString::new("nop");
                                    ir.opcodes[j].nargs = 0;
                                }
                            }
                        }
                        stats.constant_folds += 1;
                        changed = true;
                        continue;
                    } else {
                        // Condition is false: nop the if, nop the true branch
                        ir.opcodes[i].op = UString::new("nop");
                        ir.opcodes[i].nargs = 0;
                        // Nop true branch: opcodes[true_start..false_start)
                        if true_start >= 0 && false_start >= 0 {
                            let ts = true_start as usize;
                            let fs = false_start as usize;
                            for j in ts..fs {
                                if j < ir.opcodes.len() {
                                    ir.opcodes[j].op = UString::new("nop");
                                    ir.opcodes[j].nargs = 0;
                                }
                            }
                        }
                        stats.constant_folds += 1;
                        changed = true;
                        continue;
                    }
                }
            }
        }

        // --- Fold: dot(const_vec3, const_vec3) ---
        if opname == "dot" && nargs >= 3 {
            let dst_idx = ir.args[firstarg] as usize;
            let a_idx = ir.args[firstarg + 1] as usize;
            let b_idx = ir.args[firstarg + 2] as usize;
            if a_idx < ir.symbols.len()
                && b_idx < ir.symbols.len()
                && ir.symbols[a_idx].symtype == SymType::Const
                && ir.symbols[b_idx].symtype == SymType::Const
            {
                if let (Some(a), Some(b)) = (get_const_vec3(ir, a_idx), get_const_vec3(ir, b_idx)) {
                    let result = a.x * b.x + a.y * b.y + a.z * b.z;
                    let ci = ensure_const_float(ir, result);
                    ir.opcodes[i].op = UString::new("assign");
                    ir.opcodes[i].nargs = 2;
                    ir.args[firstarg] = dst_idx as i32;
                    ir.args[firstarg + 1] = ci;
                    stats.constant_folds += 1;
                    changed = true;
                    continue;
                }
            }
        }

        // --- Fold: compref(dst, const_vec3, const_int) ---
        if opname == "compref" && nargs >= 3 {
            let dst_idx = ir.args[firstarg] as usize;
            let v_idx = ir.args[firstarg + 1] as usize;
            let c_idx = ir.args[firstarg + 2] as usize;
            if v_idx < ir.symbols.len()
                && c_idx < ir.symbols.len()
                && ir.symbols[v_idx].symtype == SymType::Const
                && ir.symbols[c_idx].symtype == SymType::Const
            {
                if let (Some(v), Some(ci)) = (get_const_vec3(ir, v_idx), get_const_int(ir, c_idx)) {
                    let val = match ci {
                        0 => v.x,
                        1 => v.y,
                        _ => v.z,
                    };
                    let ri = ensure_const_float(ir, val);
                    ir.opcodes[i].op = UString::new("assign");
                    ir.opcodes[i].nargs = 2;
                    ir.args[firstarg] = dst_idx as i32;
                    ir.args[firstarg + 1] = ri;
                    stats.constant_folds += 1;
                    changed = true;
                    continue;
                }
            }
        }

        // --- Fold: compassign sequence (3 consecutive compassign with const indices 0,1,2) ---
        if opname == "compassign" && nargs >= 3 {
            let v_idx = ir.args[firstarg] as usize;
            let c_idx = ir.args[firstarg + 1] as usize;
            if c_idx < ir.symbols.len() && ir.symbols[c_idx].symtype == SymType::Const {
                if let Some(comp) = get_const_int(ir, c_idx) {
                    if comp == 0 && i + 2 < ir.opcodes.len() {
                        // Check if next two are compassign for same dst with indices 1 and 2
                        let op1 = &ir.opcodes[i + 1];
                        let op2 = &ir.opcodes[i + 2];
                        if op1.op == "compassign"
                            && op2.op == "compassign"
                            && op1.nargs >= 3
                            && op2.nargs >= 3
                        {
                            let v1 = ir.args[op1.firstarg as usize] as usize;
                            let c1 = ir.args[op1.firstarg as usize + 1] as usize;
                            let v2 = ir.args[op2.firstarg as usize] as usize;
                            let c2 = ir.args[op2.firstarg as usize + 1] as usize;
                            if v1 == v_idx
                                && v2 == v_idx
                                && c1 < ir.symbols.len()
                                && c2 < ir.symbols.len()
                                && ir.symbols[c1].symtype == SymType::Const
                                && ir.symbols[c2].symtype == SymType::Const
                            {
                                if let (Some(1), Some(2)) =
                                    (get_const_int(ir, c1), get_const_int(ir, c2))
                                {
                                    // All three are const-index compassign: get values
                                    let val0_idx = ir.args[firstarg + 2] as usize;
                                    let val1_idx = ir.args[op1.firstarg as usize + 2] as usize;
                                    let val2_idx = ir.args[op2.firstarg as usize + 2] as usize;
                                    if val0_idx < ir.symbols.len()
                                        && val1_idx < ir.symbols.len()
                                        && val2_idx < ir.symbols.len()
                                        && ir.symbols[val0_idx].symtype == SymType::Const
                                        && ir.symbols[val1_idx].symtype == SymType::Const
                                        && ir.symbols[val2_idx].symtype == SymType::Const
                                    {
                                        if let (Some(x), Some(y), Some(z)) = (
                                            get_const_float(ir, val0_idx),
                                            get_const_float(ir, val1_idx),
                                            get_const_float(ir, val2_idx),
                                        ) {
                                            let vec_ci = ensure_const_vec3(
                                                ir,
                                                crate::math::Vec3 { x, y, z },
                                            );
                                            ir.opcodes[i].op = UString::new("assign");
                                            ir.opcodes[i].nargs = 2;
                                            ir.args[firstarg] = v_idx as i32;
                                            ir.args[firstarg + 1] = vec_ci;
                                            ir.opcodes[i + 1].op = UString::new("nop");
                                            ir.opcodes[i + 1].nargs = 0;
                                            ir.opcodes[i + 2].op = UString::new("nop");
                                            ir.opcodes[i + 2].nargs = 0;
                                            stats.constant_folds += 1;
                                            changed = true;
                                            continue;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // --- Fold: mxcompassign sequence (16 consecutive with const row/col 0..3) ---
        if opname == "mxcompassign" && nargs >= 4 {
            let mx_idx = ir.args[firstarg] as usize;
            let r_idx = ir.args[firstarg + 1] as usize;
            if r_idx < ir.symbols.len() && ir.symbols[r_idx].symtype == SymType::Const {
                if let Some(0) = get_const_int(ir, r_idx) {
                    // Check if next 15 ops are mxcompassign for same matrix
                    if i + 15 < ir.opcodes.len() {
                        let mut vals = [0.0f32; 16];
                        let mut all_ok = true;
                        for k in 0..16 {
                            let op_k = &ir.opcodes[i + k];
                            if op_k.op != "mxcompassign" || op_k.nargs < 4 {
                                all_ok = false;
                                break;
                            }
                            let m = ir.args[op_k.firstarg as usize] as usize;
                            let r = ir.args[op_k.firstarg as usize + 1] as usize;
                            let c = ir.args[op_k.firstarg as usize + 2] as usize;
                            let v = ir.args[op_k.firstarg as usize + 3] as usize;
                            if m != mx_idx {
                                all_ok = false;
                                break;
                            }
                            if r >= ir.symbols.len()
                                || c >= ir.symbols.len()
                                || v >= ir.symbols.len()
                            {
                                all_ok = false;
                                break;
                            }
                            if ir.symbols[r].symtype != SymType::Const
                                || ir.symbols[c].symtype != SymType::Const
                                || ir.symbols[v].symtype != SymType::Const
                            {
                                all_ok = false;
                                break;
                            }
                            if let (Some(ri), Some(ci), Some(fv)) = (
                                get_const_int(ir, r),
                                get_const_int(ir, c),
                                get_const_float(ir, v),
                            ) {
                                let idx_k = (ri * 4 + ci) as usize;
                                if idx_k < 16 {
                                    vals[idx_k] = fv;
                                } else {
                                    all_ok = false;
                                    break;
                                }
                            } else {
                                all_ok = false;
                                break;
                            }
                        }
                        if all_ok {
                            let mut m = crate::math::Matrix44::default();
                            for row in 0..4 {
                                for col in 0..4 {
                                    m.m[row][col] = vals[row * 4 + col];
                                }
                            }
                            let mc = ensure_const_matrix(ir, m);
                            ir.opcodes[i].op = UString::new("assign");
                            ir.opcodes[i].nargs = 2;
                            ir.args[firstarg] = mx_idx as i32;
                            ir.args[firstarg + 1] = mc;
                            for k in 1..16 {
                                ir.opcodes[i + k].op = UString::new("nop");
                                ir.opcodes[i + k].nargs = 0;
                            }
                            stats.constant_folds += 1;
                            changed = true;
                            continue;
                        }
                    }
                }
            }
        }

        // --- Fold: aref(dst, const_array, const_index) ---
        if opname == "aref" && nargs >= 3 {
            let dst_idx = ir.args[firstarg] as usize;
            let arr_idx = ir.args[firstarg + 1] as usize;
            let idx_idx = ir.args[firstarg + 2] as usize;
            if arr_idx < ir.symbols.len()
                && idx_idx < ir.symbols.len()
                && ir.symbols[arr_idx].symtype == SymType::Const
                && ir.symbols[idx_idx].symtype == SymType::Const
            {
                if let Some(index) = get_const_int(ir, idx_idx) {
                    let index = index as usize;
                    // Try float array
                    if let Some(arr) = get_const_float_array(ir, arr_idx) {
                        if index < arr.len() {
                            let ci = ensure_const_float(ir, arr[index]);
                            ir.opcodes[i].op = UString::new("assign");
                            ir.opcodes[i].nargs = 2;
                            ir.args[firstarg] = dst_idx as i32;
                            ir.args[firstarg + 1] = ci;
                            stats.constant_folds += 1;
                            changed = true;
                            continue;
                        }
                    }
                    // Try int array
                    if let Some(arr) = get_const_int_array(ir, arr_idx) {
                        if index < arr.len() {
                            let ci = ensure_const_int(ir, arr[index]);
                            ir.opcodes[i].op = UString::new("assign");
                            ir.opcodes[i].nargs = 2;
                            ir.args[firstarg] = dst_idx as i32;
                            ir.args[firstarg + 1] = ci;
                            stats.constant_folds += 1;
                            changed = true;
                            continue;
                        }
                    }
                }
            }
        }

        // --- Fold: arraylength(dst, array) ---
        if opname == "arraylength" && nargs >= 2 {
            let dst_idx = ir.args[firstarg] as usize;
            let arr_idx = ir.args[firstarg + 1] as usize;
            if arr_idx < ir.symbols.len() {
                let arrlen = ir.symbols[arr_idx].typespec.simpletype().arraylen;
                if arrlen > 0 {
                    let ci = ensure_const_int(ir, arrlen);
                    ir.opcodes[i].op = UString::new("assign");
                    ir.opcodes[i].nargs = 2;
                    ir.args[firstarg] = dst_idx as i32;
                    ir.args[firstarg + 1] = ci;
                    stats.constant_folds += 1;
                    changed = true;
                    continue;
                }
            }
        }

        // --- Fold: aassign sequence (N consecutive aassign with const indices 0..N-1) ---
        if opname == "aassign" && nargs >= 3 {
            let arr_idx = ir.args[firstarg] as usize;
            let idx0 = ir.args[firstarg + 1] as usize;
            if idx0 < ir.symbols.len() && ir.symbols[idx0].symtype == SymType::Const {
                if let Some(0) = get_const_int(ir, idx0) {
                    // Determine array length from type
                    let arrlen = ir.symbols[arr_idx].typespec.simpletype().arraylen as usize;
                    if arrlen > 0 && i + arrlen - 1 < ir.opcodes.len() {
                        let mut vals = Vec::with_capacity(arrlen);
                        let mut all_ok = true;
                        for k in 0..arrlen {
                            let op_k = &ir.opcodes[i + k];
                            if op_k.op != "aassign" || op_k.nargs < 3 {
                                all_ok = false;
                                break;
                            }
                            let a = ir.args[op_k.firstarg as usize] as usize;
                            let ci = ir.args[op_k.firstarg as usize + 1] as usize;
                            let vi = ir.args[op_k.firstarg as usize + 2] as usize;
                            if a != arr_idx {
                                all_ok = false;
                                break;
                            }
                            if ci >= ir.symbols.len() || vi >= ir.symbols.len() {
                                all_ok = false;
                                break;
                            }
                            if ir.symbols[ci].symtype != SymType::Const
                                || ir.symbols[vi].symtype != SymType::Const
                            {
                                all_ok = false;
                                break;
                            }
                            if let (Some(idx_k), Some(fv)) =
                                (get_const_int(ir, ci), get_const_float(ir, vi))
                            {
                                if idx_k as usize != k {
                                    all_ok = false;
                                    break;
                                }
                                vals.push(fv);
                            } else {
                                all_ok = false;
                                break;
                            }
                        }
                        if all_ok && vals.len() == arrlen {
                            let ac = ensure_const_float_array(ir, &vals);
                            ir.opcodes[i].op = UString::new("assign");
                            ir.opcodes[i].nargs = 2;
                            ir.args[firstarg] = arr_idx as i32;
                            ir.args[firstarg + 1] = ac;
                            for k in 1..arrlen {
                                ir.opcodes[i + k].op = UString::new("nop");
                                ir.opcodes[i + k].nargs = 0;
                            }
                            stats.constant_folds += 1;
                            changed = true;
                            continue;
                        }
                    }
                }
            }
        }

        // --- Fold: string builtins with constant args ---

        // strlen(dst, const_str)
        if opname == "strlen" && nargs >= 2 {
            let dst_idx = ir.args[firstarg] as usize;
            let s_idx = ir.args[firstarg + 1] as usize;
            if s_idx < ir.symbols.len() && ir.symbols[s_idx].symtype == SymType::Const {
                if let Some(s) = get_const_str(ir, s_idx) {
                    let ci = ensure_const_int(ir, s.len() as i32);
                    ir.opcodes[i].op = UString::new("assign");
                    ir.opcodes[i].nargs = 2;
                    ir.args[firstarg] = dst_idx as i32;
                    ir.args[firstarg + 1] = ci;
                    stats.constant_folds += 1;
                    changed = true;
                    continue;
                }
            }
        }

        // hash(dst, const_str)
        if opname == "hash" && nargs >= 2 {
            let dst_idx = ir.args[firstarg] as usize;
            let s_idx = ir.args[firstarg + 1] as usize;
            if s_idx < ir.symbols.len() && ir.symbols[s_idx].symtype == SymType::Const {
                if let Some(s) = get_const_str(ir, s_idx) {
                    // Simple hash: use FNV-1a
                    let mut h = 0x811c9dc5u32;
                    for b in s.bytes() {
                        h ^= b as u32;
                        h = h.wrapping_mul(0x01000193);
                    }
                    let ci = ensure_const_int(ir, h as i32);
                    ir.opcodes[i].op = UString::new("assign");
                    ir.opcodes[i].nargs = 2;
                    ir.args[firstarg] = dst_idx as i32;
                    ir.args[firstarg + 1] = ci;
                    stats.constant_folds += 1;
                    changed = true;
                    continue;
                }
            }
        }

        // getchar(dst, const_str, const_int)
        if opname == "getchar" && nargs >= 3 {
            let dst_idx = ir.args[firstarg] as usize;
            let s_idx = ir.args[firstarg + 1] as usize;
            let i_idx = ir.args[firstarg + 2] as usize;
            if s_idx < ir.symbols.len()
                && i_idx < ir.symbols.len()
                && ir.symbols[s_idx].symtype == SymType::Const
                && ir.symbols[i_idx].symtype == SymType::Const
            {
                if let (Some(s), Some(ci)) = (get_const_str(ir, s_idx), get_const_int(ir, i_idx)) {
                    let val = s.as_bytes().get(ci as usize).copied().unwrap_or(0) as i32;
                    let ri = ensure_const_int(ir, val);
                    ir.opcodes[i].op = UString::new("assign");
                    ir.opcodes[i].nargs = 2;
                    ir.args[firstarg] = dst_idx as i32;
                    ir.args[firstarg + 1] = ri;
                    stats.constant_folds += 1;
                    changed = true;
                    continue;
                }
            }
        }

        // endswith(dst, const_str, const_str)
        if opname == "endswith" && nargs >= 3 {
            let dst_idx = ir.args[firstarg] as usize;
            let s_idx = ir.args[firstarg + 1] as usize;
            let suf_idx = ir.args[firstarg + 2] as usize;
            if s_idx < ir.symbols.len()
                && suf_idx < ir.symbols.len()
                && ir.symbols[s_idx].symtype == SymType::Const
                && ir.symbols[suf_idx].symtype == SymType::Const
            {
                if let (Some(s), Some(suf)) = (get_const_str(ir, s_idx), get_const_str(ir, suf_idx))
                {
                    let val = if s.ends_with(&suf) { 1 } else { 0 };
                    let ci = ensure_const_int(ir, val);
                    ir.opcodes[i].op = UString::new("assign");
                    ir.opcodes[i].nargs = 2;
                    ir.args[firstarg] = dst_idx as i32;
                    ir.args[firstarg + 1] = ci;
                    stats.constant_folds += 1;
                    changed = true;
                    continue;
                }
            }
        }

        // startswith(dst, const_str, const_str)
        if opname == "startswith" && nargs >= 3 {
            let dst_idx = ir.args[firstarg] as usize;
            let s_idx = ir.args[firstarg + 1] as usize;
            let pre_idx = ir.args[firstarg + 2] as usize;
            if s_idx < ir.symbols.len()
                && pre_idx < ir.symbols.len()
                && ir.symbols[s_idx].symtype == SymType::Const
                && ir.symbols[pre_idx].symtype == SymType::Const
            {
                if let (Some(s), Some(pre)) = (get_const_str(ir, s_idx), get_const_str(ir, pre_idx))
                {
                    let val = if s.starts_with(&pre) { 1 } else { 0 };
                    let ci = ensure_const_int(ir, val);
                    ir.opcodes[i].op = UString::new("assign");
                    ir.opcodes[i].nargs = 2;
                    ir.args[firstarg] = dst_idx as i32;
                    ir.args[firstarg + 1] = ci;
                    stats.constant_folds += 1;
                    changed = true;
                    continue;
                }
            }
        }

        // stoi(dst, const_str)
        if opname == "stoi" && nargs >= 2 {
            let dst_idx = ir.args[firstarg] as usize;
            let s_idx = ir.args[firstarg + 1] as usize;
            if s_idx < ir.symbols.len() && ir.symbols[s_idx].symtype == SymType::Const {
                if let Some(s) = get_const_str(ir, s_idx) {
                    let val = s.trim().parse::<i32>().unwrap_or(0);
                    let ci = ensure_const_int(ir, val);
                    ir.opcodes[i].op = UString::new("assign");
                    ir.opcodes[i].nargs = 2;
                    ir.args[firstarg] = dst_idx as i32;
                    ir.args[firstarg + 1] = ci;
                    stats.constant_folds += 1;
                    changed = true;
                    continue;
                }
            }
        }

        // stof(dst, const_str)
        if opname == "stof" && nargs >= 2 {
            let dst_idx = ir.args[firstarg] as usize;
            let s_idx = ir.args[firstarg + 1] as usize;
            if s_idx < ir.symbols.len() && ir.symbols[s_idx].symtype == SymType::Const {
                if let Some(s) = get_const_str(ir, s_idx) {
                    let val = s.trim().parse::<f32>().unwrap_or(0.0);
                    let ci = ensure_const_float(ir, val);
                    ir.opcodes[i].op = UString::new("assign");
                    ir.opcodes[i].nargs = 2;
                    ir.args[firstarg] = dst_idx as i32;
                    ir.args[firstarg + 1] = ci;
                    stats.constant_folds += 1;
                    changed = true;
                    continue;
                }
            }
        }

        // concat(dst, const_str...) - variable args
        if opname == "concat" && nargs >= 3 {
            let dst_idx = ir.args[firstarg] as usize;
            let mut all_const = true;
            let mut parts = Vec::new();
            for j in 1..nargs {
                let si = ir.args[firstarg + j] as usize;
                if si >= ir.symbols.len() || ir.symbols[si].symtype != SymType::Const {
                    all_const = false;
                    break;
                }
                if let Some(s) = get_const_str(ir, si) {
                    parts.push(s);
                } else {
                    all_const = false;
                    break;
                }
            }
            if all_const {
                let result: String = parts.concat();
                let ci = ensure_const_str(ir, &result);
                ir.opcodes[i].op = UString::new("assign");
                ir.opcodes[i].nargs = 2;
                ir.args[firstarg] = dst_idx as i32;
                ir.args[firstarg + 1] = ci;
                stats.constant_folds += 1;
                changed = true;
                continue;
            }
        }

        // substr(dst, const_str, const_start, const_len)
        if opname == "substr" && nargs >= 4 {
            let dst_idx = ir.args[firstarg] as usize;
            let s_idx = ir.args[firstarg + 1] as usize;
            let start_idx = ir.args[firstarg + 2] as usize;
            let len_idx = ir.args[firstarg + 3] as usize;
            if s_idx < ir.symbols.len()
                && start_idx < ir.symbols.len()
                && len_idx < ir.symbols.len()
                && ir.symbols[s_idx].symtype == SymType::Const
                && ir.symbols[start_idx].symtype == SymType::Const
                && ir.symbols[len_idx].symtype == SymType::Const
            {
                if let (Some(s), Some(start), Some(len)) = (
                    get_const_str(ir, s_idx),
                    get_const_int(ir, start_idx),
                    get_const_int(ir, len_idx),
                ) {
                    let start = start.max(0) as usize;
                    let len = len.max(0) as usize;
                    let end = (start + len).min(s.len());
                    let result = if start < s.len() { &s[start..end] } else { "" };
                    let ci = ensure_const_str(ir, result);
                    ir.opcodes[i].op = UString::new("assign");
                    ir.opcodes[i].nargs = 2;
                    ir.args[firstarg] = dst_idx as i32;
                    ir.args[firstarg + 1] = ci;
                    stats.constant_folds += 1;
                    changed = true;
                    continue;
                }
            }
        }

        // format(dst, const_fmt, const_args...) - simple %d/%f/%s substitution
        if opname == "format" && nargs >= 3 {
            let dst_idx = ir.args[firstarg] as usize;
            let fmt_idx = ir.args[firstarg + 1] as usize;
            if fmt_idx < ir.symbols.len() && ir.symbols[fmt_idx].symtype == SymType::Const {
                if let Some(fmt_str) = get_const_str(ir, fmt_idx) {
                    let mut all_const = true;
                    let mut arg_vals: Vec<String> = Vec::new();
                    for j in 2..nargs {
                        let ai = ir.args[firstarg + j] as usize;
                        if ai >= ir.symbols.len() || ir.symbols[ai].symtype != SymType::Const {
                            all_const = false;
                            break;
                        }
                        if let Some(iv) = get_const_int(ir, ai) {
                            arg_vals.push(iv.to_string());
                        } else if let Some(fv) = get_const_float(ir, ai) {
                            arg_vals.push(format!("{fv}"));
                        } else if let Some(sv) = get_const_str(ir, ai) {
                            arg_vals.push(sv);
                        } else {
                            all_const = false;
                            break;
                        }
                    }
                    if all_const {
                        // Simple substitution: replace %d, %f, %s, %g with arg values
                        let mut result = String::new();
                        let mut chars = fmt_str.chars().peekable();
                        let mut arg_i = 0;
                        while let Some(c) = chars.next() {
                            if c == '%' {
                                if let Some(&spec) = chars.peek() {
                                    if matches!(spec, 'd' | 'f' | 's' | 'g' | 'i') {
                                        chars.next();
                                        if arg_i < arg_vals.len() {
                                            result.push_str(&arg_vals[arg_i]);
                                            arg_i += 1;
                                        }
                                        continue;
                                    }
                                }
                            }
                            result.push(c);
                        }
                        let ci = ensure_const_str(ir, &result);
                        ir.opcodes[i].op = UString::new("assign");
                        ir.opcodes[i].nargs = 2;
                        ir.args[firstarg] = dst_idx as i32;
                        ir.args[firstarg + 1] = ci;
                        stats.constant_folds += 1;
                        changed = true;
                        continue;
                    }
                }
            }
        }

        // regex_search(dst, const_str, const_pattern) - only fold literal patterns (no regex metacharacters)
        if opname == "regex_search" && nargs >= 3 {
            let dst_idx = ir.args[firstarg] as usize;
            let subj_idx = ir.args[firstarg + 1] as usize;
            let pat_idx = ir.args[firstarg + 2] as usize;
            if subj_idx < ir.symbols.len()
                && pat_idx < ir.symbols.len()
                && ir.symbols[subj_idx].symtype == SymType::Const
                && ir.symbols[pat_idx].symtype == SymType::Const
            {
                if let (Some(subj), Some(pat)) =
                    (get_const_str(ir, subj_idx), get_const_str(ir, pat_idx))
                {
                    // Only fold literal patterns (no regex metacharacters)
                    let is_literal = !pat.contains(|c: char| ".*+?[](){}|\\^$".contains(c));
                    if is_literal {
                        let val = if subj.contains(&pat) { 1 } else { 0 };
                        let ci = ensure_const_int(ir, val);
                        ir.opcodes[i].op = UString::new("assign");
                        ir.opcodes[i].nargs = 2;
                        ir.args[firstarg] = dst_idx as i32;
                        ir.args[firstarg + 1] = ci;
                        stats.constant_folds += 1;
                        changed = true;
                        continue;
                    }
                }
            }
        }

        // split(dst, const_str, result, [const_sep]) - fold to count of splits
        if opname == "split" && nargs >= 3 {
            let dst_idx = ir.args[firstarg] as usize;
            let s_idx = ir.args[firstarg + 1] as usize;
            if s_idx < ir.symbols.len() && ir.symbols[s_idx].symtype == SymType::Const {
                if let Some(s) = get_const_str(ir, s_idx) {
                    let sep = if nargs >= 4 {
                        let sep_idx = ir.args[firstarg + 3] as usize;
                        if sep_idx < ir.symbols.len()
                            && ir.symbols[sep_idx].symtype == SymType::Const
                        {
                            get_const_str(ir, sep_idx)
                        } else {
                            None
                        }
                    } else {
                        Some(" \t\n".to_string()) // default whitespace
                    };
                    if let Some(sep) = sep {
                        let count = if sep.len() == 1 {
                            s.split(sep.chars().next().unwrap()).count() as i32
                        } else {
                            s.split_whitespace().count() as i32
                        };
                        let ci = ensure_const_int(ir, count);
                        ir.opcodes[i].op = UString::new("assign");
                        ir.opcodes[i].nargs = 2;
                        ir.args[firstarg] = dst_idx as i32;
                        ir.args[firstarg + 1] = ci;
                        stats.constant_folds += 1;
                        changed = true;
                        continue;
                    }
                }
            }
        }

        // --- Fold: select(dst, a, b, cond) ---
        // C++ constfold_select: cond==0 => assign a, cond!=0 => assign b, a==b => assign a
        if opname == "select" && nargs >= 4 {
            let dst_idx = ir.args[firstarg] as usize;
            let a_idx = ir.args[firstarg + 1];
            let b_idx = ir.args[firstarg + 2];
            let c_idx = ir.args[firstarg + 3] as usize;

            // If a == b (same symbol), always assign a regardless of condition
            if a_idx == b_idx {
                ir.opcodes[i].op = UString::new("assign");
                ir.opcodes[i].nargs = 2;
                ir.args[firstarg] = dst_idx as i32;
                ir.args[firstarg + 1] = a_idx;
                stats.constant_folds += 1;
                changed = true;
                continue;
            }

            if c_idx < ir.symbols.len() && ir.symbols[c_idx].symtype == SymType::Const {
                if let Some(cv) = get_const_int(ir, c_idx) {
                    let src = if cv == 0 { a_idx } else { b_idx };
                    ir.opcodes[i].op = UString::new("assign");
                    ir.opcodes[i].nargs = 2;
                    ir.args[firstarg] = dst_idx as i32;
                    ir.args[firstarg + 1] = src;
                    stats.constant_folds += 1;
                    changed = true;
                    continue;
                }
                // Also check float condition (treat 0.0 as false)
                if let Some(cv) = get_const_float(ir, c_idx) {
                    let src = if cv == 0.0 { a_idx } else { b_idx };
                    ir.opcodes[i].op = UString::new("assign");
                    ir.opcodes[i].nargs = 2;
                    ir.args[firstarg] = dst_idx as i32;
                    ir.args[firstarg + 1] = src;
                    stats.constant_folds += 1;
                    changed = true;
                    continue;
                }
            }
        }

        // --- Fold: sincos(angle, sin_dst, cos_dst) with constant angle ---
        // C++ constfold_sincos: compute both sin and cos at compile time
        if opname == "sincos" && nargs >= 3 {
            let angle_idx = ir.args[firstarg] as usize;
            if angle_idx < ir.symbols.len() && ir.symbols[angle_idx].symtype == SymType::Const {
                if let Some(angle) = get_const_float(ir, angle_idx) {
                    let sin_val = angle.sin();
                    let cos_val = angle.cos();
                    let sin_dst = ir.args[firstarg + 1];
                    let cos_dst = ir.args[firstarg + 2];
                    let sin_ci = ensure_const_float(ir, sin_val);
                    let cos_ci = ensure_const_float(ir, cos_val);
                    // Replace sincos with assign to sin_dst
                    ir.opcodes[i].op = UString::new("assign");
                    ir.opcodes[i].nargs = 2;
                    ir.args[firstarg] = sin_dst;
                    ir.args[firstarg + 1] = sin_ci;
                    // Insert a new assign op for cos_dst right after
                    let cos_fa = ir.args.len() as i32;
                    ir.args.push(cos_dst);
                    ir.args.push(cos_ci);
                    let mut cos_op = crate::symbol::Opcode::new(
                        UString::new("assign"),
                        UString::default(),
                        cos_fa,
                        2,
                    );
                    cos_op.argwrite = 1;
                    cos_op.argread = !1u32;
                    ir.opcodes.insert(i + 1, cos_op);
                    stats.constant_folds += 1;
                    changed = true;
                    continue;
                }
            }
        }

        // --- Fold: normalize(dst, const_vec3) ---
        // C++ constfold_normalize: normalize constant vector at compile time
        if opname == "normalize" && nargs >= 2 {
            let dst_idx = ir.args[firstarg] as usize;
            let v_idx = ir.args[firstarg + 1] as usize;
            if v_idx < ir.symbols.len() && ir.symbols[v_idx].symtype == SymType::Const {
                if let Some(v) = get_const_vec3(ir, v_idx) {
                    let len = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
                    let result = if len > 0.0 {
                        crate::math::Vec3 {
                            x: v.x / len,
                            y: v.y / len,
                            z: v.z / len,
                        }
                    } else {
                        crate::math::Vec3 {
                            x: 0.0,
                            y: 0.0,
                            z: 0.0,
                        }
                    };
                    let ci = ensure_const_vec3(ir, result);
                    ir.opcodes[i].op = UString::new("assign");
                    ir.opcodes[i].nargs = 2;
                    ir.args[firstarg] = dst_idx as i32;
                    ir.args[firstarg + 1] = ci;
                    stats.constant_folds += 1;
                    changed = true;
                    continue;
                }
            }
        }

        // --- Fold: Dx/Dy/Dz of constant => 0 ---
        // C++ constfold_deriv: derivative of any constant is zero
        if matches!(opname.as_str(), "Dx" | "Dy" | "Dz") && nargs >= 2 {
            let dst_idx = ir.args[firstarg] as usize;
            let src_idx = ir.args[firstarg + 1] as usize;
            if src_idx < ir.symbols.len() && ir.symbols[src_idx].symtype == SymType::Const {
                let zero = ensure_const_float(ir, 0.0);
                ir.opcodes[i].op = UString::new("assign");
                ir.opcodes[i].nargs = 2;
                ir.args[firstarg] = dst_idx as i32;
                ir.args[firstarg + 1] = zero;
                stats.constant_folds += 1;
                changed = true;
                continue;
            }
        }

        // --- Fold: isconstant(dst, src) ---
        // C++ constfold_isconstant: if src is known constant => assign 1
        // (we never fold to 0; further optimization may discover more constants)
        if opname == "isconstant" && nargs >= 2 {
            let dst_idx = ir.args[firstarg] as usize;
            let src_idx = ir.args[firstarg + 1] as usize;
            if src_idx < ir.symbols.len() && ir.symbols[src_idx].symtype == SymType::Const {
                let one = ensure_const_int(ir, 1);
                ir.opcodes[i].op = UString::new("assign");
                ir.opcodes[i].nargs = 2;
                ir.args[firstarg] = dst_idx as i32;
                ir.args[firstarg + 1] = one;
                stats.constant_folds += 1;
                changed = true;
                continue;
            }
        }

        // --- Fold: functioncall with empty body ---
        // C++ constfold_functioncall: if body is only nops/return, eliminate
        if opname == "functioncall" && nargs >= 1 {
            let end = ir.opcodes[i].jump[0];
            if end >= 0 {
                let end_idx = end as usize;
                let mut has_anything_else = false;
                let mut has_return = false;
                for j in (i + 1)..end_idx.min(ir.opcodes.len()) {
                    let inner = ir.opcodes[j].op.as_str();
                    if inner == "return" {
                        has_return = true;
                    } else if inner != "nop" && !inner.is_empty() {
                        has_anything_else = true;
                    }
                }
                if !has_anything_else {
                    // Body is empty (only nops/returns): nop the whole block
                    for j in i..end_idx.min(ir.opcodes.len()) {
                        if ir.opcodes[j].op != "nop" {
                            ir.opcodes[j].op = UString::new("nop");
                            ir.opcodes[j].nargs = 0;
                            stats.constant_folds += 1;
                            changed = true;
                        }
                    }
                    continue;
                } else if !has_return {
                    // No return: transmute to functioncall_nr (plan #52). Preserves side effects.
                    ir.opcodes[i].op = UString::new("functioncall_nr");
                    stats.constant_folds += 1;
                    changed = true;
                    continue;
                }
            }
        }
    }

    // --- Constant propagation + symbol aliasing pass ---
    // Track two kinds of mappings:
    // 1. propagated: sym -> const_sym (constant propagation)
    // 2. aliases: sym -> alias_sym (symbol aliasing, matching C++ block_aliases)
    //
    // When we see `assign dst, src`, dst aliases to src.
    // If src is a constant, dst is also a known constant.
    // We clear aliases when the aliased symbol is written.
    let mut propagated = std::collections::HashMap::<usize, usize>::new();
    let mut aliases = std::collections::HashMap::<usize, usize>::new();

    for i in 0..ir.opcodes.len() {
        let op = &ir.opcodes[i];
        let opname = op.op.as_str().to_string();
        if opname == "nop" || opname.is_empty() {
            continue;
        }

        let nargs = op.nargs as usize;
        let firstarg = op.firstarg as usize;

        // Track assignments for aliasing
        if opname == "assign" && nargs >= 2 {
            let dst_idx = ir.args[firstarg] as usize;
            let src_idx = ir.args[firstarg + 1] as usize;

            if src_idx < ir.symbols.len() && dst_idx < ir.symbols.len() {
                // Constant propagation: track or invalidate
                if ir.symbols[src_idx].symtype == SymType::Const
                    && ir.symbols[dst_idx].symtype == SymType::Local
                {
                    propagated.insert(dst_idx, src_idx);
                } else {
                    // Non-const assign invalidates any previous const propagation
                    propagated.remove(&dst_idx);
                }

                // Symbol aliasing: if src is a param/local/global,
                // dst is an alias for src (until dst is written again)
                if ir.symbols[dst_idx].symtype == SymType::Local
                    || ir.symbols[dst_idx].symtype == SymType::Temp
                {
                    // Dealias: follow the chain
                    let mut final_src = src_idx;
                    while let Some(&next) = aliases.get(&final_src) {
                        if next == final_src {
                            break;
                        }
                        final_src = next;
                    }
                    aliases.insert(dst_idx, final_src);
                }
            }
        }

        // Non-assign ops that write to a symbol clear its alias
        if opname != "assign" && nargs >= 1 {
            let dst_idx = ir.args[firstarg] as usize;
            if dst_idx < ir.symbols.len() {
                aliases.remove(&dst_idx);
                propagated.remove(&dst_idx);
            }
        }

        // Control flow ops (if, for, while, etc.) clear all block-local state.
        // Both aliases and const-propagation info are invalidated because
        // a branch may have reassigned locals.
        if matches!(
            opname.as_str(),
            "if" | "for" | "while" | "dowhile" | "functioncall" | "return" | "break" | "continue"
        ) {
            aliases.clear();
            propagated.clear();
        }
    }

    // Apply propagation: replace uses of propagated locals with their constant source
    // Also apply aliases: replace uses of aliased locals with their alias target
    if !propagated.is_empty() || !aliases.is_empty() {
        for i in 0..ir.opcodes.len() {
            let op = &ir.opcodes[i];
            let opname = op.op.as_str();
            if opname == "nop" || opname.is_empty() {
                continue;
            }
            // Skip assignment targets — only replace read operands
            if opname == "assign" {
                continue;
            }

            let nargs = op.nargs as usize;
            let firstarg = op.firstarg as usize;

            // Replace source operands (not the destination at index 0)
            for j in 1..nargs {
                let arg_pos = firstarg + j;
                if arg_pos >= ir.args.len() {
                    break;
                }
                let sym_idx = ir.args[arg_pos] as usize;

                // Prefer constant propagation over aliasing
                if let Some(&const_idx) = propagated.get(&sym_idx) {
                    ir.args[arg_pos] = const_idx as i32;
                    changed = true;
                } else if let Some(&alias_idx) = aliases.get(&sym_idx) {
                    if alias_idx != sym_idx {
                        ir.args[arg_pos] = alias_idx as i32;
                        changed = true;
                    }
                }
            }
        }
    }

    changed
}

/// Stale assignment elimination: remove assignments that are overwritten
/// before being read. Matches the C++ `stale_syms` mechanism.
fn stale_assign_eliminate(ir: &mut ShaderIR, stats: &mut OptStats) -> bool {
    let mut changed = false;

    // Track: for each symbol, the instruction index where it was last "simply" assigned
    let mut stale: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();

    for i in 0..ir.opcodes.len() {
        let op = &ir.opcodes[i];
        let opname = op.op.as_str().to_string();
        if opname == "nop" || opname.is_empty() {
            continue;
        }

        let nargs = op.nargs as usize;
        let firstarg = op.firstarg as usize;

        // Control flow: clear stale info (entering new basic block)
        if matches!(
            opname.as_str(),
            "if" | "for" | "while" | "dowhile" | "functioncall" | "return" | "break" | "continue"
        ) {
            stale.clear();
            continue;
        }

        // Check if any read operands un-stale their symbols
        for j in 1..nargs {
            if firstarg + j >= ir.args.len() {
                break;
            }
            let sym_idx = ir.args[firstarg + j] as usize;
            stale.remove(&sym_idx);
        }

        // Check if destination is a simple assign
        if nargs >= 1 {
            let dst_idx = ir.args[firstarg] as usize;
            if dst_idx < ir.symbols.len() {
                let sym = &ir.symbols[dst_idx];
                let is_simple = matches!(
                    opname.as_str(),
                    "assign" | "add" | "sub" | "mul" | "div" | "neg"
                ) && (sym.symtype == SymType::Local
                    || sym.symtype == SymType::Temp);

                if is_simple {
                    // If this symbol was already stale (assigned but not read),
                    // the previous assignment can be eliminated
                    if let Some(prev_op_idx) = stale.get(&dst_idx) {
                        let prev_op_idx = *prev_op_idx;
                        ir.opcodes[prev_op_idx].op = UString::new("nop");
                        ir.opcodes[prev_op_idx].nargs = 0;
                        stats.dead_ops_eliminated += 1;
                        changed = true;
                    }
                    // Mark this assignment as stale
                    stale.insert(dst_idx, i);
                } else {
                    // Non-simple write: un-stale the symbol
                    stale.remove(&dst_idx);
                }
            }
        }
    }

    changed
}

/// Dead code elimination: remove ops whose results are never read.
fn dead_code_eliminate(ir: &mut ShaderIR, stats: &mut OptStats) -> bool {
    let mut changed = false;

    // Mark which symbols are read
    let n_syms = ir.symbols.len();
    let mut sym_read = vec![false; n_syms];
    let mut sym_written = vec![false; n_syms];

    for op in &ir.opcodes {
        let opname = op.op.as_str();
        if opname == "nop" || opname.is_empty() {
            continue;
        }

        let nargs = op.nargs as usize;
        let firstarg = op.firstarg as usize;

        for j in 0..nargs {
            if firstarg + j >= ir.args.len() {
                break;
            }
            let sym_idx = ir.args[firstarg + j] as usize;
            if sym_idx >= n_syms {
                continue;
            }

            if j == 0 && (op.argwrite & 1) != 0 {
                sym_written[sym_idx] = true;
            }
            if j == 0 && (op.argread & 1) != 0 {
                sym_read[sym_idx] = true;
            }
            if j > 0 {
                sym_read[sym_idx] = true;
            }
        }
    }

    // Eliminate ops that write to temps that are never read
    for i in 0..ir.opcodes.len() {
        let op = &ir.opcodes[i];
        let opname = op.op.as_str();
        if opname == "nop" || opname.is_empty() {
            continue;
        }
        if opname == "return" || opname == "if" {
            continue;
        }

        let nargs = op.nargs as usize;
        let firstarg = op.firstarg as usize;
        if nargs == 0 {
            continue;
        }

        let dst_idx = ir.args[firstarg] as usize;
        if dst_idx >= n_syms {
            continue;
        }

        let sym = &ir.symbols[dst_idx];
        if sym.symtype == SymType::Temp && !sym_read[dst_idx] {
            ir.opcodes[i].op = UString::new("nop");
            ir.opcodes[i].nargs = 0;
            stats.dead_ops_eliminated += 1;
            changed = true;
        }
    }

    changed
}

/// Peephole optimizations: simple pattern-based transforms.
fn peephole(ir: &mut ShaderIR, stats: &mut OptStats) -> bool {
    let mut changed = false;

    for i in 0..ir.opcodes.len().saturating_sub(1) {
        let op1_name = ir.opcodes[i].op.as_str().to_string();
        let op2_name = ir.opcodes[i + 1].op.as_str().to_string();

        // Pattern: assign a, b; assign b, a → eliminate second assign
        if op1_name == "assign" && op2_name == "assign" {
            let nargs1 = ir.opcodes[i].nargs as usize;
            let nargs2 = ir.opcodes[i + 1].nargs as usize;
            let fa1 = ir.opcodes[i].firstarg as usize;
            let fa2 = ir.opcodes[i + 1].firstarg as usize;

            if nargs1 >= 2 && nargs2 >= 2 {
                let a1_dst = ir.args[fa1];
                let a1_src = ir.args[fa1 + 1];
                let a2_dst = ir.args[fa2];
                let a2_src = ir.args[fa2 + 1];

                if a1_dst == a2_src && a1_src == a2_dst {
                    ir.opcodes[i + 1].op = UString::new("nop");
                    ir.opcodes[i + 1].nargs = 0;
                    stats.peephole_opts += 1;
                    changed = true;
                }
            }
        }
    }

    changed
}

/// Middleman elimination: if A = B and then C = A, replace with C = B directly.
/// This is the "middleman" optimization from the C++ reference.
fn middleman_eliminate(ir: &mut ShaderIR, stats: &mut OptStats) -> bool {
    let mut changed = false;
    // Track latest simple assignment: dst_sym → src_sym
    let mut assign_map: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();

    for i in 0..ir.opcodes.len() {
        let op = &ir.opcodes[i];
        let opname = op.op.as_str().to_string();
        if opname == "nop" || opname.is_empty() {
            continue;
        }

        // Clear on control flow
        if matches!(
            opname.as_str(),
            "if" | "for" | "while" | "dowhile" | "functioncall" | "return" | "break" | "continue"
        ) {
            assign_map.clear();
            continue;
        }

        let nargs = op.nargs as usize;
        let firstarg = op.firstarg as usize;

        if opname == "assign" && nargs >= 2 {
            let dst_idx = ir.args[firstarg] as usize;
            let src_idx = ir.args[firstarg + 1] as usize;
            if dst_idx < ir.symbols.len() && src_idx < ir.symbols.len() {
                // Follow chain: if src was assigned from something, use the original
                let final_src = if let Some(&orig) = assign_map.get(&src_idx) {
                    orig
                } else {
                    src_idx
                };
                if final_src != src_idx {
                    ir.args[firstarg + 1] = final_src as i32;
                    changed = true;
                    stats.peephole_opts += 1;
                }
                assign_map.insert(dst_idx, final_src);
            }
        } else {
            // Non-assign writes clear the mapping for that sym
            if nargs >= 1 {
                let dst_idx = ir.args[firstarg] as usize;
                assign_map.remove(&dst_idx);
            }
        }
    }

    changed
}

/// Eliminate useparam opcodes that are unnecessary (when the param symbol
/// has already been initialized). Matches C++ `useless_useparam_elim`.
fn useparam_eliminate(ir: &mut ShaderIR, stats: &mut OptStats) -> bool {
    let mut changed = false;
    for i in 0..ir.opcodes.len() {
        let op = &ir.opcodes[i];
        if op.op != "useparam" {
            continue;
        }
        let nargs = op.nargs as usize;
        let firstarg = op.firstarg as usize;
        if nargs < 1 {
            continue;
        }
        let sym_idx = ir.args[firstarg] as usize;
        if sym_idx >= ir.symbols.len() {
            continue;
        }
        let sym = &ir.symbols[sym_idx];
        // If this param has no connections (initbegin == initend) and
        // has a known constant default, the useparam is unnecessary
        if sym.symtype == SymType::Param && sym.initbegin == sym.initend {
            ir.opcodes[i].op = UString::new("nop");
            ir.opcodes[i].nargs = 0;
            stats.dead_ops_eliminated += 1;
            changed = true;
        }
    }
    changed
}

/// Collapse nops: remove all nop opcodes and renumber jump targets.
/// Matches C++ `collapse_syms` + `collapse_ops`.
pub fn collapse_ops(ir: &mut ShaderIR) {
    // Build a mapping from old opcode indices to new indices
    let n_ops = ir.opcodes.len();
    let mut new_index: Vec<i32> = Vec::with_capacity(n_ops);
    let mut new_idx = 0i32;
    for op in ir.opcodes.iter() {
        new_index.push(new_idx);
        if op.op != "nop" && !op.op.is_empty() {
            new_idx += 1;
        }
    }
    // Sentinel: old n_ops maps to new_idx
    let sentinel = new_idx;

    // Remap jump targets
    for op in ir.opcodes.iter_mut() {
        for j in 0..4 {
            if op.jump[j] >= 0 {
                if (op.jump[j] as usize) < n_ops {
                    op.jump[j] = new_index[op.jump[j] as usize];
                } else {
                    op.jump[j] = sentinel;
                }
            }
        }
    }

    // Remap param init ranges
    for sym in ir.symbols.iter_mut() {
        if sym.symtype == SymType::Param || sym.symtype == SymType::OutputParam {
            if sym.initbegin >= 0 {
                sym.initbegin = if (sym.initbegin as usize) < n_ops {
                    new_index[sym.initbegin as usize]
                } else {
                    sentinel
                };
            }
            if sym.initend >= 0 {
                sym.initend = if (sym.initend as usize) < n_ops {
                    new_index[sym.initend as usize]
                } else {
                    sentinel
                };
            }
        }
    }

    // Remove nop opcodes
    ir.opcodes.retain(|op| op.op != "nop" && !op.op.is_empty());
}

/// Coalesce temporary variables: merge temps with non-overlapping lifetimes.
fn coalesce_temps(ir: &mut ShaderIR, stats: &mut OptStats) {
    // Track lifetimes of temp symbols
    let n_syms = ir.symbols.len();
    let mut first_use = vec![i32::MAX; n_syms];
    let mut last_use = vec![-1i32; n_syms];

    for (op_idx, op) in ir.opcodes.iter().enumerate() {
        if op.op == "nop" {
            continue;
        }
        let nargs = op.nargs as usize;
        let firstarg = op.firstarg as usize;

        for j in 0..nargs {
            if firstarg + j >= ir.args.len() {
                break;
            }
            let sym_idx = ir.args[firstarg + j] as usize;
            if sym_idx >= n_syms {
                continue;
            }
            first_use[sym_idx] = first_use[sym_idx].min(op_idx as i32);
            last_use[sym_idx] = last_use[sym_idx].max(op_idx as i32);
        }
    }

    // Find pairs of temps with non-overlapping lifetimes and same type
    let temps: Vec<usize> = (0..n_syms)
        .filter(|&i| ir.symbols[i].symtype == SymType::Temp && last_use[i] >= 0)
        .collect();

    for i in 0..temps.len() {
        for j in (i + 1)..temps.len() {
            let a = temps[i];
            let b = temps[j];

            if ir.symbols[a].typespec != ir.symbols[b].typespec {
                continue;
            }
            if last_use[a] < first_use[b] || last_use[b] < first_use[a] {
                // Non-overlapping: replace all uses of b with a
                let b_idx = b as i32;
                let a_idx = a as i32;
                for arg in &mut ir.args {
                    if *arg == b_idx {
                        *arg = a_idx;
                    }
                }
                stats.temps_coalesced += 1;
            }
        }
    }
}

/// Simplify params: turn instance parameters into constants if they have a
/// known value and lockgeom=1 (default). Matches C++ `simplify_params`.
fn simplify_params(ir: &mut ShaderIR, stats: &mut OptStats) -> bool {
    let mut changed = false;
    for sym_idx in 0..ir.symbols.len() {
        if ir.symbols[sym_idx].symtype != SymType::Param {
            continue;
        }
        // Check if this param has a constant default and isn't connected
        // (initbegin == initend means no initialization code needed)
        if ir.symbols[sym_idx].initbegin != ir.symbols[sym_idx].initend {
            continue;
        }
        // Find if there's a constant value for this param
        let has_const = ir.param_defaults.iter().any(|&(i, _)| i == sym_idx);
        if !has_const {
            continue;
        }
        // If we find this param is only ever read (never written to),
        // we can effectively treat it as a constant for optimization purposes.
        let param_sym = sym_idx as i32;
        let is_written = ir.opcodes.iter().any(|op| {
            if op.op == "nop" || op.nargs == 0 {
                return false;
            }
            let firstarg = op.firstarg as usize;
            // First arg of most ops is the destination (written)
            if firstarg < ir.args.len() && ir.args[firstarg] == param_sym {
                let opname = op.op.as_str();
                opname != "useparam" // useparam doesn't really write
            } else {
                false
            }
        });
        if !is_written {
            // Don't override explicit [[ int lockgeom = 0 ]] from AST metadata
            let has_explicit_lockgeom = ir.symbols[sym_idx]
                .metadata
                .iter()
                .any(|(_, name, _)| name == "lockgeom");
            if !has_explicit_lockgeom {
                ir.symbols[sym_idx].is_lockgeom = true;
                changed = true;
                stats.constant_folds += 1;
            }
        }
    }
    changed
}

/// Track variable lifetimes: determine first_use and last_use for every symbol.
/// Matches C++ `track_variable_lifetimes`.
pub fn track_variable_lifetimes(ir: &ShaderIR) -> Vec<(i32, i32)> {
    let n = ir.symbols.len();
    let mut lifetimes: Vec<(i32, i32)> = vec![(i32::MAX, -1); n];
    for (op_idx, op) in ir.opcodes.iter().enumerate() {
        if op.op == "nop" {
            continue;
        }
        let nargs = op.nargs as usize;
        let firstarg = op.firstarg as usize;
        for j in 0..nargs {
            if firstarg + j >= ir.args.len() {
                break;
            }
            let sym_idx = ir.args[firstarg + j] as usize;
            if sym_idx >= n {
                continue;
            }
            lifetimes[sym_idx].0 = lifetimes[sym_idx].0.min(op_idx as i32);
            lifetimes[sym_idx].1 = lifetimes[sym_idx].1.max(op_idx as i32);
        }
    }
    lifetimes
}

/// Resolve isconnected() calls into constant assignments.
/// Matches C++ `resolve_isconnected`.
fn resolve_isconnected(ir: &mut ShaderIR, stats: &mut OptStats) -> bool {
    let mut changed = false;
    for i in 0..ir.opcodes.len() {
        if ir.opcodes[i].op != "isconnected" {
            continue;
        }
        let nargs = ir.opcodes[i].nargs as usize;
        if nargs < 2 {
            continue;
        }
        let firstarg = ir.opcodes[i].firstarg as usize;
        let sym_idx = ir.args[firstarg + 1] as usize;
        if sym_idx >= ir.symbols.len() {
            continue;
        }
        // In the interpreter, params are "connected" if they have overrides
        // from upstream layers. Without actual connection info, we default
        // to returning 0 (not connected).
        let result_idx = ir.args[firstarg];
        // Turn into assign result=0
        ir.opcodes[i].op = UString::new("assign");
        ir.opcodes[i].nargs = 2;
        // We need a zero constant
        let zero_idx = find_or_add_const_int(ir, 0);
        ir.args[firstarg + 1] = zero_idx;
        stats.constant_folds += 1;
        changed = true;
        let _ = result_idx; // suppress warning
    }
    changed
}

/// Peephole optimization on pairs of adjacent instructions.
/// Matches C++ `peephole2`.
fn peephole2(ir: &mut ShaderIR, stats: &mut OptStats) -> bool {
    let mut changed = false;
    let n = ir.opcodes.len();
    if n < 2 {
        return false;
    }
    for i in 0..n - 1 {
        if ir.opcodes[i].op == "nop" || ir.opcodes[i + 1].op == "nop" {
            continue;
        }
        let op1_name = ir.opcodes[i].op.as_str().to_string();
        let op2_name = ir.opcodes[i + 1].op.as_str().to_string();
        let fa1 = ir.opcodes[i].firstarg as usize;
        let na1 = ir.opcodes[i].nargs as usize;
        let fa2 = ir.opcodes[i + 1].firstarg as usize;
        let na2 = ir.opcodes[i + 1].nargs as usize;

        // Pattern: assign A, B; assign B, A → nop the second
        if op1_name == "assign" && op2_name == "assign" && na1 >= 2 && na2 >= 2 {
            let dst1 = ir.args[fa1];
            let src1 = ir.args[fa1 + 1];
            let dst2 = ir.args[fa2];
            let src2 = ir.args[fa2 + 1];
            if dst1 == src2 && src1 == dst2 {
                // Redundant round-trip assignment
                ir.opcodes[i + 1].op = UString::new("nop");
                ir.opcodes[i + 1].nargs = 0;
                stats.peephole_opts += 1;
                changed = true;
            }
        }

        // Pattern: neg A, B; neg C, A → assign C, B (double negation)
        if op1_name == "neg" && op2_name == "neg" && na1 >= 2 && na2 >= 2 {
            let dst1 = ir.args[fa1];
            let src1 = ir.args[fa1 + 1];
            let _dst2 = ir.args[fa2];
            let src2 = ir.args[fa2 + 1];
            if dst1 == src2 {
                ir.opcodes[i].op = UString::new("nop");
                ir.opcodes[i].nargs = 0;
                ir.opcodes[i + 1].op = UString::new("assign");
                ir.args[fa2 + 1] = src1;
                stats.peephole_opts += 1;
                changed = true;
            }
        }
    }
    changed
}

/// Helper: find or add a constant int to the IR.
fn find_or_add_const_int(ir: &mut ShaderIR, val: i32) -> i32 {
    for &(idx, ref cv) in &ir.const_values {
        if let ConstValue::Int(v) = cv {
            if *v == val {
                return idx as i32;
            }
        }
    }
    let idx = ir.symbols.len();
    let name = format!("$opt_const_{val}");
    let mut sym = Symbol::new(
        UString::new(&name),
        TypeSpec::from_simple(TypeDesc::INT),
        SymType::Const,
    );
    sym.initializers = 1;
    ir.symbols.push(sym);
    ir.const_values.push((idx, ConstValue::Int(val)));
    idx as i32
}

/// Collapse unused symbols: remove symbols that are never referenced.
/// Matches C++ `collapse_syms`.
pub fn collapse_syms(ir: &mut ShaderIR) {
    // Track which symbols are referenced
    let n = ir.symbols.len();
    let mut referenced = vec![false; n];
    for arg in &ir.args {
        let idx = *arg as usize;
        if idx < n {
            referenced[idx] = true;
        }
    }
    // Also mark params, globals, and output params as referenced
    for (i, sym) in ir.symbols.iter().enumerate() {
        if sym.symtype == SymType::Param
            || sym.symtype == SymType::OutputParam
            || sym.symtype == SymType::Global
        {
            referenced[i] = true;
        }
    }
    // Mark symbols used in const_values and param_defaults
    for &(idx, _) in &ir.const_values {
        if idx < n {
            referenced[idx] = true;
        }
    }
    for &(idx, _) in &ir.param_defaults {
        if idx < n {
            referenced[idx] = true;
        }
    }
    // For now, we don't actually remove symbols (that would require renumbering
    // all args), but we could mark them as dead for diagnostic purposes.
    // This matches the C++ behavior where collapse_syms is primarily about
    // renumbering after optimization.
}

/// Find parameters that simply hold global values (e.g., param float u = u).
/// Matches C++ `find_params_holding_globals`.
fn find_params_holding_globals(ir: &mut ShaderIR, _stats: &mut OptStats) -> bool {
    use crate::symbol::ValueSource;
    let mut changed = false;

    // Collect (param_idx, global_idx) pairs to replace.
    // Per C++ reference: param must be connected_down, valuesource==Default,
    // written exactly once by an unconditional assign from a global.
    let mut replacements = Vec::new();

    for sym_idx in 0..ir.symbols.len() {
        let sym = &ir.symbols[sym_idx];
        if sym.symtype != SymType::Param && sym.symtype != SymType::OutputParam {
            continue;
        }
        if !sym.connected_down {
            continue;
        }
        if sym.valuesource != ValueSource::Default {
            continue;
        }
        // Must be written exactly once
        if sym.firstwrite < 0 || sym.firstwrite != sym.lastwrite {
            continue;
        }

        let opnum = sym.firstwrite as usize;
        if opnum >= ir.opcodes.len() {
            continue;
        }
        let op = &ir.opcodes[opnum];
        if op.op != "assign" || op.nargs < 2 {
            continue;
        }

        // Check op is unconditional (not inside a conditional/loop)
        // Simple heuristic: op must not have any jump targets pointing past it
        // For now, check it's before any control-flow op
        let in_conditional = ir.opcodes[..opnum].iter().any(|prev| {
            let pname = prev.op.as_str();
            if matches!(pname, "if" | "for" | "while" | "dowhile") {
                // Check if opnum is within the jump range
                prev.jump.iter().any(|&j| j > opnum as i32 && j > 0)
            } else {
                false
            }
        });
        if in_conditional {
            continue;
        }

        let fa = op.firstarg as usize;
        if fa + 1 >= ir.args.len() {
            continue;
        }
        let src_idx = ir.args[fa + 1] as usize;
        if src_idx >= ir.symbols.len() {
            continue;
        }
        if ir.symbols[src_idx].symtype != SymType::Global {
            continue;
        }

        replacements.push((sym_idx, src_idx));
    }

    // Apply: replace read-position references to param with global
    for &(param_idx, global_idx) in &replacements {
        let param_i32 = param_idx as i32;
        let global_i32 = global_idx as i32;
        for i in 0..ir.opcodes.len() {
            let op = &ir.opcodes[i];
            if op.op == "nop" || op.nargs == 0 {
                continue;
            }
            let nargs = op.nargs as usize;
            let fa = op.firstarg as usize;
            // Replace source args only (index 1+), not the destination (index 0)
            for j in 1..nargs {
                let pos = fa + j;
                if pos >= ir.args.len() {
                    break;
                }
                if ir.args[pos] == param_i32 {
                    ir.args[pos] = global_i32;
                    changed = true;
                }
            }
        }
    }
    changed
}

/// Coerce assigned constants: if we have `assign triple, float_const`, replace
/// with a direct triple constant. Matches C++ `coerce_assigned_constant`.
fn coerce_assigned_constant(ir: &mut ShaderIR, stats: &mut OptStats) -> bool {
    let mut changed = false;
    for i in 0..ir.opcodes.len() {
        if ir.opcodes[i].op != "assign" {
            continue;
        }
        let nargs = ir.opcodes[i].nargs as usize;
        if nargs < 2 {
            continue;
        }
        let fa = ir.opcodes[i].firstarg as usize;
        let dst_idx = ir.args[fa] as usize;
        let src_idx = ir.args[fa + 1] as usize;
        if dst_idx >= ir.symbols.len() || src_idx >= ir.symbols.len() {
            continue;
        }
        let dst_type = ir.symbols[dst_idx].typespec.simpletype();
        let src_type = ir.symbols[src_idx].typespec.simpletype();

        // float → triple coercion
        if dst_type.aggregate == crate::typedesc::Aggregate::Vec3 as u8
            && src_type.aggregate == crate::typedesc::Aggregate::Scalar as u8
            && ir.symbols[src_idx].symtype == SymType::Const
        {
            if let Some(fval) = get_const_float(ir, src_idx) {
                // Create a Vec3 constant and replace
                let v = crate::math::Vec3::new(fval, fval, fval);
                let new_idx = ir.symbols.len();
                let name = format!("$coerce_v{fval}");
                let mut sym = Symbol::new(
                    UString::new(&name),
                    ir.symbols[dst_idx].typespec,
                    SymType::Const,
                );
                sym.initializers = 1;
                ir.symbols.push(sym);
                ir.const_values.push((new_idx, ConstValue::Vec3(v)));
                ir.args[fa + 1] = new_idx as i32;
                stats.constant_folds += 1;
                changed = true;
            }
        }
    }
    changed
}

/// Track variable dependencies for dead code elimination with derivatives.
/// Matches C++ `track_variable_dependencies`.
pub fn track_variable_dependencies(ir: &ShaderIR) -> Vec<Vec<usize>> {
    let n = ir.symbols.len();
    let mut deps: Vec<Vec<usize>> = vec![Vec::new(); n];

    for op in &ir.opcodes {
        if op.op == "nop" || op.nargs == 0 {
            continue;
        }
        let nargs = op.nargs as usize;
        let firstarg = op.firstarg as usize;
        if nargs < 2 || firstarg >= ir.args.len() {
            continue;
        }
        let dst = ir.args[firstarg] as usize;
        if dst >= n {
            continue;
        }
        // All source args (args[1..]) are dependencies of the destination
        for j in 1..nargs {
            if firstarg + j >= ir.args.len() {
                break;
            }
            let src = ir.args[firstarg + j] as usize;
            if src < n && src != dst {
                deps[dst].push(src);
            }
        }
    }
    deps
}

/// Eliminate output param assignments when the output isn't connected downstream.
/// Matches C++ `outparam_assign_elision`.
fn outparam_assign_elision(ir: &mut ShaderIR, stats: &mut OptStats) -> bool {
    let mut changed = false;
    // Find output params that are never read by another layer
    // (in single-layer shaders, outputs that aren't renderer_output can be eliminated)
    for sym_idx in 0..ir.symbols.len() {
        if ir.symbols[sym_idx].symtype != SymType::OutputParam {
            continue;
        }
        if ir.symbols[sym_idx].renderer_output || ir.symbols[sym_idx].connected_down {
            continue;
        }
        // This output param is unused — nop all assignments to it
        let param_i32 = sym_idx as i32;
        for i in 0..ir.opcodes.len() {
            if ir.opcodes[i].op == "nop" || ir.opcodes[i].nargs == 0 {
                continue;
            }
            let fa = ir.opcodes[i].firstarg as usize;
            if fa < ir.args.len() && ir.args[fa] == param_i32 {
                let opname = ir.opcodes[i].op.as_str();
                if opname == "assign"
                    || opname == "add"
                    || opname == "sub"
                    || opname == "mul"
                    || opname == "div"
                {
                    ir.opcodes[i].op = UString::new("nop");
                    ir.opcodes[i].nargs = 0;
                    stats.dead_ops_eliminated += 1;
                    changed = true;
                }
            }
        }
    }
    changed
}

/// Optimize mix(a, a, t) → a, mix(a, b, 0) → a, mix(a, b, 1) → b.
/// Matches C++ `opt_mix`.
fn opt_mix(ir: &mut ShaderIR, stats: &mut OptStats) -> bool {
    let mut changed = false;
    for i in 0..ir.opcodes.len() {
        if ir.opcodes[i].op != "mix" {
            continue;
        }
        let nargs = ir.opcodes[i].nargs as usize;
        if nargs < 4 {
            continue;
        }
        let fa = ir.opcodes[i].firstarg as usize;
        let _result = ir.args[fa];
        let a = ir.args[fa + 1] as usize;
        let b = ir.args[fa + 2] as usize;
        let t = ir.args[fa + 3] as usize;

        // mix(a, a, t) → assign result, a
        if a == b {
            ir.opcodes[i].op = UString::new("assign");
            ir.opcodes[i].nargs = 2;
            // args already has result and a in the right spots
            stats.constant_folds += 1;
            changed = true;
            continue;
        }

        // mix(a, b, 0) → assign result, a
        if let Some(tv) = get_const_float(ir, t) {
            if tv == 0.0 {
                ir.opcodes[i].op = UString::new("assign");
                ir.opcodes[i].nargs = 2;
                stats.constant_folds += 1;
                changed = true;
            } else if tv == 1.0 {
                // mix(a, b, 1) → assign result, b
                ir.opcodes[i].op = UString::new("assign");
                ir.opcodes[i].nargs = 2;
                ir.args[fa + 1] = b as i32;
                stats.constant_folds += 1;
                changed = true;
            }
        }
    }
    changed
}

/// Fold getattribute calls with known constant attribute names.
/// Matches C++ `opt_fold_getattribute`.
fn opt_fold_getattribute(ir: &mut ShaderIR, _stats: &mut OptStats) -> bool {
    // In general, getattribute results depend on the renderer, so we
    // can't fold them at compile time. However, we can fold
    // getattribute("osl:version", ...) to a constant.
    let changed = false;
    for i in 0..ir.opcodes.len() {
        if ir.opcodes[i].op != "getattribute" {
            continue;
        }
        let nargs = ir.opcodes[i].nargs as usize;
        if nargs < 3 {
            continue;
        }
        let fa = ir.opcodes[i].firstarg as usize;
        let name_idx = ir.args[fa + 1] as usize;
        if name_idx >= ir.symbols.len() {
            continue;
        }
        // Check if it's a known attribute
        if ir.symbols[name_idx].symtype == SymType::Const {
            let attr_name = ir.symbols[name_idx].name.as_str();
            if attr_name.contains("osl:version") || attr_name.contains("$const") {
                // Could fold to a known value — but for now, just skip
                // (renderer-dependent)
            }
        }
    }
    changed
}

/// Mark outgoing connections: track which outputs are used by downstream layers.
/// Matches C++ `mark_outgoing_connections`.
pub fn mark_outgoing_connections(ir: &mut ShaderIR, connected_outputs: &[&str]) {
    for sym in ir.symbols.iter_mut() {
        if sym.symtype == SymType::OutputParam {
            if connected_outputs.contains(&sym.name.as_str()) {
                sym.connected_down = true;
            }
        }
    }
}

/// Remove unused parameters: turn params that are never read into nop initializers.
/// Matches C++ `remove_unused_params`.
fn remove_unused_params(ir: &mut ShaderIR, stats: &mut OptStats) -> bool {
    let mut changed = false;
    let lifetimes = track_variable_lifetimes(ir);

    for (sym_idx, lt) in lifetimes.iter().enumerate() {
        if sym_idx >= ir.symbols.len() {
            break;
        }
        if ir.symbols[sym_idx].symtype != SymType::Param {
            continue;
        }
        // If this param is never read (lastread == -1), it's unused
        if lt.1 < 0 {
            // NOP out the useparam opcodes for this sym
            let param_i32 = sym_idx as i32;
            for i in 0..ir.opcodes.len() {
                if ir.opcodes[i].op == "useparam" {
                    let fa = ir.opcodes[i].firstarg as usize;
                    if fa < ir.args.len() && ir.args[fa] == param_i32 {
                        ir.opcodes[i].op = UString::new("nop");
                        ir.opcodes[i].nargs = 0;
                        stats.dead_ops_eliminated += 1;
                        changed = true;
                    }
                }
            }
        }
    }
    changed
}

// ---------------------------------------------------------------------------
// Peephole arithmetic identity optimizations
// ---------------------------------------------------------------------------

/// Peephole arithmetic identity rewrites on single instructions.
///
/// Patterns:
/// - `mul(dst, x, 1.0)` -> `assign(dst, x)`
/// - `mul(dst, x, 0.0)` -> `assign(dst, 0.0)`
/// - `add(dst, x, 0.0)` -> `assign(dst, x)`
/// - `div(dst, x, 1.0)` -> `assign(dst, x)`
/// - `assign(x, x)` -> nop
/// - `mul(dst, x, 2.0)` -> `add(dst, x, x)`
fn peephole_arith(ir: &mut ShaderIR, stats: &mut OptStats) -> bool {
    let mut changed = false;

    for i in 0..ir.opcodes.len() {
        let opname = ir.opcodes[i].op.as_str().to_string();
        if opname == "nop" || opname.is_empty() {
            continue;
        }
        let nargs = ir.opcodes[i].nargs as usize;
        let fa = ir.opcodes[i].firstarg as usize;

        // Self-assign: assign x, x -> nop
        if opname == "assign" && nargs >= 2 {
            let dst = ir.args[fa];
            let src = ir.args[fa + 1];
            if dst == src {
                ir.opcodes[i].op = UString::new("nop");
                ir.opcodes[i].nargs = 0;
                stats.peephole_opts += 1;
                changed = true;
                continue;
            }
        }

        // sub(dst, A, A) -> assign(dst, 0) (C++ constfold_sub: A-A=0)
        if opname == "sub" && nargs >= 3 {
            let src1_idx = ir.args[fa + 1] as usize;
            let src2_idx = ir.args[fa + 2] as usize;
            if src1_idx == src2_idx {
                let zero = ensure_const_float(ir, 0.0);
                ir.opcodes[i].op = UString::new("assign");
                ir.opcodes[i].nargs = 2;
                ir.args[fa + 1] = zero;
                stats.peephole_opts += 1;
                changed = true;
                continue;
            }
        }

        // Binary ops with a constant operand
        if nargs >= 3 && matches!(opname.as_str(), "mul" | "add" | "sub" | "div" | "mod") {
            let _dst_idx = ir.args[fa] as usize;
            let src1_idx = ir.args[fa + 1] as usize;
            let src2_idx = ir.args[fa + 2] as usize;
            if src1_idx >= ir.symbols.len() || src2_idx >= ir.symbols.len() {
                continue;
            }

            let c1 = if ir.symbols[src1_idx].symtype == SymType::Const {
                get_const_float(ir, src1_idx)
            } else {
                None
            };
            let c2 = if ir.symbols[src2_idx].symtype == SymType::Const {
                get_const_float(ir, src2_idx)
            } else {
                None
            };

            match opname.as_str() {
                "mul" => {
                    // mul(dst, x, 1.0) or mul(dst, 1.0, x) -> assign(dst, x)
                    if c2 == Some(1.0) {
                        ir.opcodes[i].op = UString::new("assign");
                        ir.opcodes[i].nargs = 2;
                        // args[fa] = dst, args[fa+1] = src1 (already correct)
                        stats.peephole_opts += 1;
                        changed = true;
                    } else if c1 == Some(1.0) {
                        ir.opcodes[i].op = UString::new("assign");
                        ir.opcodes[i].nargs = 2;
                        ir.args[fa + 1] = src2_idx as i32;
                        stats.peephole_opts += 1;
                        changed = true;
                    }
                    // mul(dst, x, 0.0) or mul(dst, 0.0, x) -> assign(dst, 0.0)
                    else if c2 == Some(0.0) {
                        let zero = ensure_const_float(ir, 0.0);
                        ir.opcodes[i].op = UString::new("assign");
                        ir.opcodes[i].nargs = 2;
                        ir.args[fa + 1] = zero;
                        stats.peephole_opts += 1;
                        changed = true;
                    } else if c1 == Some(0.0) {
                        let zero = ensure_const_float(ir, 0.0);
                        ir.opcodes[i].op = UString::new("assign");
                        ir.opcodes[i].nargs = 2;
                        ir.args[fa + 1] = zero;
                        stats.peephole_opts += 1;
                        changed = true;
                    }
                    // mul(dst, x, 2.0) -> add(dst, x, x)
                    else if c2 == Some(2.0) {
                        ir.opcodes[i].op = UString::new("add");
                        ir.args[fa + 2] = src1_idx as i32; // add(dst, x, x)
                        stats.peephole_opts += 1;
                        changed = true;
                    } else if c1 == Some(2.0) {
                        ir.opcodes[i].op = UString::new("add");
                        ir.args[fa + 1] = src2_idx as i32;
                        ir.args[fa + 2] = src2_idx as i32; // add(dst, x, x)
                        stats.peephole_opts += 1;
                        changed = true;
                    }
                }
                "add" => {
                    // add(dst, x, 0.0) -> assign(dst, x)
                    if c2 == Some(0.0) {
                        ir.opcodes[i].op = UString::new("assign");
                        ir.opcodes[i].nargs = 2;
                        stats.peephole_opts += 1;
                        changed = true;
                    } else if c1 == Some(0.0) {
                        ir.opcodes[i].op = UString::new("assign");
                        ir.opcodes[i].nargs = 2;
                        ir.args[fa + 1] = src2_idx as i32;
                        stats.peephole_opts += 1;
                        changed = true;
                    }
                }
                "sub" => {
                    // sub(dst, x, 0.0) -> assign(dst, x)
                    if c2 == Some(0.0) {
                        ir.opcodes[i].op = UString::new("assign");
                        ir.opcodes[i].nargs = 2;
                        stats.peephole_opts += 1;
                        changed = true;
                    }
                }
                "div" => {
                    // div(dst, x, 1.0) -> assign(dst, x)
                    if c2 == Some(1.0) {
                        ir.opcodes[i].op = UString::new("assign");
                        ir.opcodes[i].nargs = 2;
                        stats.peephole_opts += 1;
                        changed = true;
                    }
                    // OSL semantics: div(dst, x, 0.0) -> assign(dst, 0.0)
                    else if c2 == Some(0.0) {
                        let zero = ensure_const_float(ir, 0.0);
                        ir.opcodes[i].op = UString::new("assign");
                        ir.opcodes[i].nargs = 2;
                        ir.args[fa + 1] = zero;
                        stats.peephole_opts += 1;
                        changed = true;
                    }
                    // div(dst, 0.0, x) -> assign(dst, 0.0)
                    else if c1 == Some(0.0) {
                        let zero = ensure_const_float(ir, 0.0);
                        ir.opcodes[i].op = UString::new("assign");
                        ir.opcodes[i].nargs = 2;
                        ir.args[fa + 1] = zero;
                        stats.peephole_opts += 1;
                        changed = true;
                    }
                }
                "mod" => {
                    // OSL semantics: mod(dst, x, 0.0) -> assign(dst, 0.0)
                    if c2 == Some(0.0) {
                        let zero = ensure_const_float(ir, 0.0);
                        ir.opcodes[i].op = UString::new("assign");
                        ir.opcodes[i].nargs = 2;
                        ir.args[fa + 1] = zero;
                        stats.peephole_opts += 1;
                        changed = true;
                    }
                    // mod(dst, 0.0, x) -> assign(dst, 0.0)
                    else if c1 == Some(0.0) {
                        let zero = ensure_const_float(ir, 0.0);
                        ir.opcodes[i].op = UString::new("assign");
                        ir.opcodes[i].nargs = 2;
                        ir.args[fa + 1] = zero;
                        stats.peephole_opts += 1;
                        changed = true;
                    }
                }
                _ => {}
            }
        }
    }
    changed
}

// ---------------------------------------------------------------------------
// Copy propagation
// ---------------------------------------------------------------------------

/// Copy propagation: after `assign dst, src`, replace reads of `dst` with `src`
/// in subsequent ops until `dst` is reassigned.
///
/// This extends middleman_eliminate by propagating copies into ALL op types,
/// not just other assign instructions.
fn copy_propagate(ir: &mut ShaderIR, stats: &mut OptStats) -> bool {
    let mut changed = false;
    // alias_map: sym_idx -> replacement sym_idx
    let mut alias: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();

    for i in 0..ir.opcodes.len() {
        let opname = ir.opcodes[i].op.as_str().to_string();
        if opname == "nop" || opname.is_empty() {
            continue;
        }

        // Clear aliases on control flow boundaries
        if matches!(
            opname.as_str(),
            "if" | "for" | "while" | "dowhile" | "functioncall" | "return" | "break" | "continue"
        ) {
            alias.clear();
            continue;
        }

        let nargs = ir.opcodes[i].nargs as usize;
        let fa = ir.opcodes[i].firstarg as usize;

        // Replace read operands with their aliases
        for j in 1..nargs {
            if fa + j >= ir.args.len() {
                break;
            }
            let sym_idx = ir.args[fa + j] as usize;
            if let Some(&replacement) = alias.get(&sym_idx) {
                if replacement < ir.symbols.len() {
                    ir.args[fa + j] = replacement as i32;
                    stats.copies_propagated += 1;
                    changed = true;
                }
            }
        }

        // Track new assign aliases or invalidate on write
        if nargs >= 1 {
            let dst_idx = ir.args[fa] as usize;
            if opname == "assign" && nargs >= 2 {
                let src_idx = ir.args[fa + 1] as usize;
                if dst_idx < ir.symbols.len() && src_idx < ir.symbols.len() {
                    let sym = &ir.symbols[dst_idx];
                    // Only propagate for locals and temps
                    if sym.symtype == SymType::Local || sym.symtype == SymType::Temp {
                        // Follow chain: if src itself has an alias, use that
                        let final_src = alias.get(&src_idx).copied().unwrap_or(src_idx);
                        alias.insert(dst_idx, final_src);
                        continue;
                    }
                }
            }
            // Any other write to dst invalidates its alias
            alias.remove(&dst_idx);
            // Also invalidate anything that was aliased TO dst_idx
            alias.retain(|_, v| *v != dst_idx);
        }
    }
    changed
}

// ---------------------------------------------------------------------------
// Useless assignment elimination (cross-BB)
// ---------------------------------------------------------------------------

/// Eliminate useless assignments: if a local/temp is written at position i
/// and written again at position j with no reads of that symbol between i and j,
/// the write at i is dead and can be removed.
///
/// This complements `stale_assign_eliminate` which only works within basic blocks.
/// This pass does a global forward scan per written symbol.
fn useless_assign_elim(ir: &mut ShaderIR, stats: &mut OptStats) -> bool {
    let mut changed = false;
    let n_ops = ir.opcodes.len();

    // Collect all write locations for each local/temp symbol
    let mut writes: std::collections::HashMap<usize, Vec<usize>> = std::collections::HashMap::new();

    for i in 0..n_ops {
        let opname = ir.opcodes[i].op.as_str();
        if opname == "nop" || opname.is_empty() {
            continue;
        }
        let nargs = ir.opcodes[i].nargs as usize;
        let fa = ir.opcodes[i].firstarg as usize;
        if nargs == 0 {
            continue;
        }

        // Only simple ops that write to arg[0]
        if !matches!(
            opname,
            "assign" | "add" | "sub" | "mul" | "div" | "neg" | "mod"
        ) {
            continue;
        }

        let dst_idx = ir.args[fa] as usize;
        if dst_idx < ir.symbols.len() {
            let sym = &ir.symbols[dst_idx];
            if sym.symtype == SymType::Local || sym.symtype == SymType::Temp {
                writes.entry(dst_idx).or_default().push(i);
            }
        }
    }

    // For each symbol with multiple writes, check if any write is followed
    // by another write with no intervening read.
    for (sym_idx, write_locs) in &writes {
        if write_locs.len() < 2 {
            continue;
        }

        for w in 0..write_locs.len() - 1 {
            let w_pos = write_locs[w];
            let next_w_pos = write_locs[w + 1];

            // Check if there is a control flow op between the two writes;
            // if so, skip (conservative: the read might be conditional).
            let mut has_read = false;
            let mut has_cf = false;

            for k in (w_pos + 1)..next_w_pos {
                let kop = ir.opcodes[k].op.as_str();
                if kop == "nop" || kop.is_empty() {
                    continue;
                }
                if matches!(
                    kop,
                    "if" | "for"
                        | "while"
                        | "dowhile"
                        | "functioncall"
                        | "return"
                        | "break"
                        | "continue"
                ) {
                    has_cf = true;
                    break;
                }
                // Check if this op reads sym_idx
                let knargs = ir.opcodes[k].nargs as usize;
                let kfa = ir.opcodes[k].firstarg as usize;
                for j in 1..knargs {
                    if kfa + j < ir.args.len() {
                        if ir.args[kfa + j] as usize == *sym_idx {
                            has_read = true;
                            break;
                        }
                    }
                }
                // Also check if arg[0] is read (e.g., compound assign)
                if knargs >= 1 && kfa < ir.args.len() {
                    if ir.args[kfa] as usize == *sym_idx && (ir.opcodes[k].argread & 1) != 0 {
                        has_read = true;
                    }
                }
                if has_read {
                    break;
                }
            }

            if !has_read && !has_cf {
                // The write at w_pos is useless
                ir.opcodes[w_pos].op = UString::new("nop");
                ir.opcodes[w_pos].nargs = 0;
                stats.useless_assigns += 1;
                changed = true;
            }
        }
    }
    changed
}

// ---------------------------------------------------------------------------
// Advanced cross-connection optimizations (Task 4)
// ---------------------------------------------------------------------------

/// Propagate constant outputs from upstream layers into downstream layer inputs.
///
/// This is the cross-layer constant propagation optimization from the C++
/// reference (`ShadingSystemImpl::optimize_group`). When an upstream layer
/// produces a known-constant output, all downstream connections from that
/// output can be replaced with the constant value directly, potentially
/// enabling further single-layer optimizations.
///
/// `layers` is a mutable slice of (ShaderIR, connected_outputs) pairs.
/// `connections` lists all inter-layer connections.
///
/// Returns the number of propagations performed.
pub fn propagate_constants_across_connections(
    layers: &mut [(ShaderIR, Vec<String>)],
    connections: &[crate::shadingsys::Connection],
) -> usize {
    let mut propagations = 0;

    // For each connection, check if the source output is a compile-time constant
    for conn in connections {
        let src_layer = conn.src_layer as usize;
        let dst_layer = conn.dst_layer as usize;

        if src_layer >= layers.len() || dst_layer >= layers.len() {
            continue;
        }

        let src_param_name = conn.src_param.as_str();
        let dst_param_name = conn.dst_param.as_str();

        // Find the source symbol and check if it's a constant
        let const_value = {
            let src_ir = &layers[src_layer].0;
            let src_sym_idx = src_ir.symbols.iter().position(|s| s.name == src_param_name);
            if let Some(idx) = src_sym_idx {
                let sym = &src_ir.symbols[idx];
                // Check if the symbol has been simplified to a constant
                if sym.symtype == SymType::Const || sym.is_lockgeom {
                    // Look up its constant value
                    src_ir
                        .const_values
                        .iter()
                        .chain(src_ir.param_defaults.iter())
                        .find(|&&(i, _)| i == idx)
                        .map(|(_, cv)| cv.clone())
                } else {
                    None
                }
            } else {
                None
            }
        };

        // If we found a constant, propagate it to the destination
        if let Some(cv) = const_value {
            let dst_ir = &mut layers[dst_layer].0;
            let dst_sym_idx = dst_ir.symbols.iter().position(|s| s.name == dst_param_name);
            if let Some(idx) = dst_sym_idx {
                // Remove existing defaults for this param
                dst_ir.param_defaults.retain(|&(i, _)| i != idx);
                // Set the constant value
                dst_ir.param_defaults.push((idx, cv));
                propagations += 1;
            }
        }
    }

    propagations
}

/// Specialize a shader IR for known uniform parameter values.
///
/// When a parameter has a known value at shade-time (e.g., from a
/// ReParameter call or from constant propagation), this function replaces
/// the parameter symbol with a constant, enabling further optimization.
///
/// `uniform_params` maps parameter names to their known constant values.
///
/// Returns the number of parameters specialized.
///
/// Called automatically by `optimize_group_with_params()` as Phase 0.
/// Use that function to pass renderer-supplied param values (e.g. via ReParameter).
pub fn specialize_params(
    ir: &mut ShaderIR,
    uniform_params: &std::collections::HashMap<String, ConstValue>,
) -> usize {
    let mut specialized = 0;

    for (name, value) in uniform_params {
        let uname = UString::new(name);
        let sym_idx = ir.symbols.iter().position(|s| {
            (s.symtype == SymType::Param || s.symtype == SymType::OutputParam) && s.name == uname
        });

        if let Some(idx) = sym_idx {
            // Check that this param isn't written to by any instruction
            let param_i32 = idx as i32;
            let is_written = ir.opcodes.iter().any(|op| {
                if op.op == "nop" || op.nargs == 0 {
                    return false;
                }
                let firstarg = op.firstarg as usize;
                if firstarg < ir.args.len() && ir.args[firstarg] == param_i32 {
                    let opname = op.op.as_str();
                    opname != "useparam"
                } else {
                    false
                }
            });

            if !is_written {
                // Replace with constant
                ir.symbols[idx].symtype = SymType::Const;
                ir.symbols[idx].is_lockgeom = true;
                ir.param_defaults.retain(|&(i, _)| i != idx);
                ir.const_values.retain(|&(i, _)| i != idx);
                ir.const_values.push((idx, value.clone()));
                specialized += 1;
            }
        }
    }

    specialized
}

/// Optimize a shader group with cross-connection constant propagation.
///
/// This performs multi-layer optimization:
/// 1. Optimize each layer individually
/// 2. Propagate constants across connections
/// 3. Re-optimize affected downstream layers
/// 4. Repeat until no more propagations occur
///
/// Returns total optimization stats across all layers.
/// See also: `optimize_group_with_params` for specializing known uniform values first.
pub fn optimize_group(
    layers: &mut [(ShaderIR, Vec<String>)],
    connections: &[crate::shadingsys::Connection],
    level: OptLevel,
) -> OptStats {
    optimize_group_with_params(
        layers,
        connections,
        level,
        &std::collections::HashMap::new(),
    )
}

/// Optimize a shader group with parameter specialization then cross-connection propagation.
///
/// Phase 0: Specialize each layer with `uniform_params` — converts known constant params
/// into constants before any other optimization, enabling maximum constant folding.
/// Phase 1+2: Same as `optimize_group` (per-layer opt + cross-connection propagation).
///
/// `uniform_params` maps parameter name -> known constant value supplied by the renderer
/// (e.g. via ReParameter). Empty map skips specialization and behaves like `optimize_group`.
pub fn optimize_group_with_params(
    layers: &mut [(ShaderIR, Vec<String>)],
    connections: &[crate::shadingsys::Connection],
    level: OptLevel,
    uniform_params: &std::collections::HashMap<String, ConstValue>,
) -> OptStats {
    let mut total_stats = OptStats::default();

    // Phase 0: Specialize known uniform parameters into constants per layer.
    // Only runs when uniform_params is non-empty (renderer-supplied instance values).
    if !uniform_params.is_empty() {
        for (ir, _) in layers.iter_mut() {
            let n = specialize_params(ir, uniform_params);
            total_stats.constant_folds += n as u32;
        }
    }

    // Phase 1: Initial per-layer optimization
    for (ir, connected_outputs) in layers.iter_mut() {
        // Mark which outputs are connected downstream
        mark_outgoing_connections(
            ir,
            &connected_outputs
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>(),
        );
        let stats = optimize(ir, level);
        total_stats.constant_folds += stats.constant_folds;
        total_stats.dead_ops_eliminated += stats.dead_ops_eliminated;
        total_stats.temps_coalesced += stats.temps_coalesced;
        total_stats.peephole_opts += stats.peephole_opts;
        total_stats.copies_propagated += stats.copies_propagated;
        total_stats.useless_assigns += stats.useless_assigns;
        total_stats.total_passes += stats.total_passes;
    }

    // Phase 2: Cross-connection constant propagation (iterate until stable)
    for _ in 0..5 {
        let propagations = propagate_constants_across_connections(layers, connections);
        if propagations == 0 {
            break;
        }
        total_stats.constant_folds += propagations as u32;

        // Re-optimize layers that received new constants
        for (ir, connected_outputs) in layers.iter_mut() {
            mark_outgoing_connections(
                ir,
                &connected_outputs
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>(),
            );
            let stats = optimize(ir, level);
            total_stats.constant_folds += stats.constant_folds;
            total_stats.dead_ops_eliminated += stats.dead_ops_eliminated;
            total_stats.temps_coalesced += stats.temps_coalesced;
            total_stats.peephole_opts += stats.peephole_opts;
            total_stats.copies_propagated += stats.copies_propagated;
            total_stats.useless_assigns += stats.useless_assigns;
            total_stats.total_passes += stats.total_passes;
        }
    }

    total_stats
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen;
    use crate::parser;
    use crate::symbol::Opcode;

    fn compile(src: &str) -> ShaderIR {
        let ast = parser::parse(src).unwrap().ast;
        codegen::generate(&ast)
    }

    #[test]
    fn test_optimize_noop() {
        let mut ir = compile("shader test() {}");
        let stats = optimize(&mut ir, OptLevel::O0);
        assert_eq!(stats.total_passes, 0);
    }

    #[test]
    fn test_optimize_basic() {
        let mut ir = compile(
            r#"
shader test(float a = 1.0) {
    float b = a;
    float c = b + 1.0;
}
"#,
        );
        let stats = optimize(&mut ir, OptLevel::O2);
        assert!(stats.total_passes > 0);
    }

    #[test]
    fn test_dce() {
        let mut ir = compile(
            r#"
shader test(float a = 1.0) {
    float b = a + 1.0;
    float c = a + 2.0;
}
"#,
        );
        let initial_ops = ir.opcodes.iter().filter(|o| o.op != "nop").count();
        optimize(&mut ir, OptLevel::O1);
        let final_ops = ir
            .opcodes
            .iter()
            .filter(|o| o.op != "nop" && !o.op.is_empty())
            .count();
        // DCE should be able to remove dead code for unused locals
        assert!(final_ops <= initial_ops);
    }

    // --- Helper to build IR manually for unit tests ---

    fn make_ir() -> ShaderIR {
        ShaderIR::default()
    }

    fn add_sym(ir: &mut ShaderIR, name: &str, st: SymType) -> i32 {
        let idx = ir.symbols.len();
        ir.symbols.push(Symbol::new(
            UString::new(name),
            TypeSpec::from_simple(TypeDesc::FLOAT),
            st,
        ));
        idx as i32
    }

    fn add_const_f(ir: &mut ShaderIR, val: f32) -> i32 {
        let idx = ir.symbols.len();
        let name = format!("$c_{val}");
        let mut sym = Symbol::new(
            UString::new(&name),
            TypeSpec::from_simple(TypeDesc::FLOAT),
            SymType::Const,
        );
        sym.initializers = 1;
        ir.symbols.push(sym);
        ir.const_values.push((idx, ConstValue::Float(val)));
        idx as i32
    }

    fn add_sym_int(ir: &mut ShaderIR, name: &str, st: SymType) -> i32 {
        let idx = ir.symbols.len();
        ir.symbols.push(Symbol::new(
            UString::new(name),
            TypeSpec::from_simple(TypeDesc::INT),
            st,
        ));
        idx as i32
    }

    fn add_const_i(ir: &mut ShaderIR, val: i32) -> i32 {
        let idx = ir.symbols.len();
        let name = format!("$ci_{val}");
        let mut sym = Symbol::new(
            UString::new(&name),
            TypeSpec::from_simple(TypeDesc::INT),
            SymType::Const,
        );
        sym.initializers = 1;
        ir.symbols.push(sym);
        ir.const_values.push((idx, ConstValue::Int(val)));
        idx as i32
    }

    fn emit_op(ir: &mut ShaderIR, name: &str, args: &[i32]) {
        let fa = ir.args.len() as i32;
        for &a in args {
            ir.args.push(a);
        }
        let mut op = Opcode::new(
            UString::new(name),
            UString::default(),
            fa,
            args.len() as i32,
        );
        // First arg is written, rest are read
        if !args.is_empty() {
            op.argwrite = 1;
            op.argread = !1u32;
        }
        ir.opcodes.push(op);
    }

    fn count_ops(ir: &ShaderIR, name: &str) -> usize {
        ir.opcodes.iter().filter(|o| o.op == name).count()
    }

    // --- Peephole arithmetic tests ---

    #[test]
    fn test_peephole_arith_self_assign() {
        let mut ir = make_ir();
        let x = add_sym(&mut ir, "x", SymType::Local);
        emit_op(&mut ir, "assign", &[x, x]); // assign x, x -> nop
        let mut stats = OptStats::default();
        assert!(peephole_arith(&mut ir, &mut stats));
        assert_eq!(count_ops(&ir, "nop"), 1);
        assert_eq!(count_ops(&ir, "assign"), 0);
        assert!(stats.peephole_opts > 0);
    }

    #[test]
    fn test_peephole_arith_mul_by_one() {
        let mut ir = make_ir();
        let dst = add_sym(&mut ir, "dst", SymType::Local);
        let x = add_sym(&mut ir, "x", SymType::Local);
        let one = add_const_f(&mut ir, 1.0);
        emit_op(&mut ir, "mul", &[dst, x, one]); // mul(dst, x, 1.0) -> assign(dst, x)
        let mut stats = OptStats::default();
        assert!(peephole_arith(&mut ir, &mut stats));
        assert_eq!(count_ops(&ir, "assign"), 1);
        assert_eq!(count_ops(&ir, "mul"), 0);
    }

    #[test]
    fn test_peephole_arith_mul_by_zero() {
        let mut ir = make_ir();
        let dst = add_sym(&mut ir, "dst", SymType::Local);
        let x = add_sym(&mut ir, "x", SymType::Local);
        let zero = add_const_f(&mut ir, 0.0);
        emit_op(&mut ir, "mul", &[dst, x, zero]); // mul(dst, x, 0.0) -> assign(dst, 0.0)
        let mut stats = OptStats::default();
        assert!(peephole_arith(&mut ir, &mut stats));
        assert_eq!(count_ops(&ir, "assign"), 1);
        assert_eq!(count_ops(&ir, "mul"), 0);
    }

    #[test]
    fn test_peephole_arith_add_zero() {
        let mut ir = make_ir();
        let dst = add_sym(&mut ir, "dst", SymType::Local);
        let x = add_sym(&mut ir, "x", SymType::Local);
        let zero = add_const_f(&mut ir, 0.0);
        emit_op(&mut ir, "add", &[dst, x, zero]); // add(dst, x, 0.0) -> assign(dst, x)
        let mut stats = OptStats::default();
        assert!(peephole_arith(&mut ir, &mut stats));
        assert_eq!(count_ops(&ir, "assign"), 1);
        assert_eq!(count_ops(&ir, "add"), 0);
    }

    #[test]
    fn test_peephole_arith_div_by_one() {
        let mut ir = make_ir();
        let dst = add_sym(&mut ir, "dst", SymType::Local);
        let x = add_sym(&mut ir, "x", SymType::Local);
        let one = add_const_f(&mut ir, 1.0);
        emit_op(&mut ir, "div", &[dst, x, one]); // div(dst, x, 1.0) -> assign(dst, x)
        let mut stats = OptStats::default();
        assert!(peephole_arith(&mut ir, &mut stats));
        assert_eq!(count_ops(&ir, "assign"), 1);
        assert_eq!(count_ops(&ir, "div"), 0);
    }

    #[test]
    fn test_peephole_arith_mul_by_two() {
        let mut ir = make_ir();
        let dst = add_sym(&mut ir, "dst", SymType::Local);
        let x = add_sym(&mut ir, "x", SymType::Local);
        let two = add_const_f(&mut ir, 2.0);
        emit_op(&mut ir, "mul", &[dst, x, two]); // mul(dst, x, 2.0) -> add(dst, x, x)
        let mut stats = OptStats::default();
        assert!(peephole_arith(&mut ir, &mut stats));
        assert_eq!(count_ops(&ir, "add"), 1);
        assert_eq!(count_ops(&ir, "mul"), 0);
        // Verify the add uses x, x
        let fa = ir.opcodes[0].firstarg as usize;
        assert_eq!(ir.args[fa + 1], x);
        assert_eq!(ir.args[fa + 2], x);
    }

    // --- Copy propagation tests ---

    #[test]
    fn test_copy_propagate_basic() {
        // assign b, a; add c, b, x -> add c, a, x
        let mut ir = make_ir();
        let a = add_sym(&mut ir, "a", SymType::Param);
        let b = add_sym(&mut ir, "b", SymType::Local);
        let c = add_sym(&mut ir, "c", SymType::Local);
        let x = add_sym(&mut ir, "x", SymType::Param);
        emit_op(&mut ir, "assign", &[b, a]); // b = a
        emit_op(&mut ir, "add", &[c, b, x]); // c = b + x -> c = a + x
        let mut stats = OptStats::default();
        assert!(copy_propagate(&mut ir, &mut stats));
        // b should be replaced with a in the add
        let fa = ir.opcodes[1].firstarg as usize;
        assert_eq!(ir.args[fa + 1], a);
        assert!(stats.copies_propagated > 0);
    }

    #[test]
    fn test_copy_propagate_chain() {
        // assign b, a; assign c, b; add d, c, x -> add d, a, x
        let mut ir = make_ir();
        let a = add_sym(&mut ir, "a", SymType::Param);
        let b = add_sym(&mut ir, "b", SymType::Local);
        let c = add_sym(&mut ir, "c", SymType::Local);
        let d = add_sym(&mut ir, "d", SymType::Local);
        let x = add_sym(&mut ir, "x", SymType::Param);
        emit_op(&mut ir, "assign", &[b, a]); // b = a
        emit_op(&mut ir, "assign", &[c, b]); // c = b (-> a)
        emit_op(&mut ir, "add", &[d, c, x]); // d = c + x -> d = a + x
        let mut stats = OptStats::default();
        assert!(copy_propagate(&mut ir, &mut stats));
        let fa = ir.opcodes[2].firstarg as usize;
        assert_eq!(ir.args[fa + 1], a);
    }

    #[test]
    fn test_copy_propagate_invalidate() {
        // assign b, a; add b, b, x; add c, b, x
        // b is overwritten by the add, so the second add should still use b
        let mut ir = make_ir();
        let a = add_sym(&mut ir, "a", SymType::Param);
        let b = add_sym(&mut ir, "b", SymType::Local);
        let c = add_sym(&mut ir, "c", SymType::Local);
        let x = add_sym(&mut ir, "x", SymType::Param);
        emit_op(&mut ir, "assign", &[b, a]); // b = a
        emit_op(&mut ir, "add", &[b, b, x]); // b = b + x (overwrites b)
        emit_op(&mut ir, "add", &[c, b, x]); // c = b + x (b no longer alias of a)
        let mut stats = OptStats::default();
        copy_propagate(&mut ir, &mut stats);
        // The second add's src1 should still be b (not a)
        let fa = ir.opcodes[2].firstarg as usize;
        assert_eq!(ir.args[fa + 1], b);
    }

    // --- Useless assignment elimination tests ---

    #[test]
    fn test_useless_assign_basic() {
        // assign b, a; assign b, x -> first assign is useless
        let mut ir = make_ir();
        let a = add_sym(&mut ir, "a", SymType::Param);
        let b = add_sym(&mut ir, "b", SymType::Local);
        let x = add_sym(&mut ir, "x", SymType::Param);
        emit_op(&mut ir, "assign", &[b, a]); // b = a (useless)
        emit_op(&mut ir, "assign", &[b, x]); // b = x
        let mut stats = OptStats::default();
        assert!(useless_assign_elim(&mut ir, &mut stats));
        assert_eq!(ir.opcodes[0].op.as_str(), "nop");
        assert_eq!(ir.opcodes[1].op.as_str(), "assign");
        assert!(stats.useless_assigns > 0);
    }

    #[test]
    fn test_useless_assign_with_read() {
        // assign b, a; add c, b, x; assign b, x
        // b is read between writes, so first assign is NOT useless
        let mut ir = make_ir();
        let a = add_sym(&mut ir, "a", SymType::Param);
        let b = add_sym(&mut ir, "b", SymType::Local);
        let c = add_sym(&mut ir, "c", SymType::Local);
        let x = add_sym(&mut ir, "x", SymType::Param);
        emit_op(&mut ir, "assign", &[b, a]); // b = a
        emit_op(&mut ir, "add", &[c, b, x]); // c = b + x (reads b!)
        emit_op(&mut ir, "assign", &[b, x]); // b = x
        let mut stats = OptStats::default();
        let changed = useless_assign_elim(&mut ir, &mut stats);
        assert!(!changed);
        assert_eq!(ir.opcodes[0].op.as_str(), "assign"); // still there
    }

    #[test]
    fn test_useless_assign_control_flow() {
        // assign b, a; if ...; assign b, x
        // Control flow between writes -> conservative, keep both
        let mut ir = make_ir();
        let a = add_sym(&mut ir, "a", SymType::Param);
        let b = add_sym(&mut ir, "b", SymType::Local);
        let x = add_sym(&mut ir, "x", SymType::Param);
        let cond = add_const_f(&mut ir, 1.0);
        emit_op(&mut ir, "assign", &[b, a]);
        // Insert an "if" op between the two assigns
        {
            let fa = ir.args.len() as i32;
            ir.args.push(cond);
            let op = Opcode::new(UString::new("if"), UString::default(), fa, 1);
            ir.opcodes.push(op);
        }
        emit_op(&mut ir, "assign", &[b, x]);
        let mut stats = OptStats::default();
        let changed = useless_assign_elim(&mut ir, &mut stats);
        assert!(!changed); // must not eliminate due to control flow
    }

    // --- Integration: full O2 optimization with new passes ---

    #[test]
    fn test_optimize_o2_peephole_arith() {
        let mut ir = compile(
            r#"
shader test(float a = 1.0) {
    float b = a * 1.0;
    float c = a + 0.0;
    float d = a / 1.0;
}
"#,
        );
        let stats = optimize(&mut ir, OptLevel::O2);
        assert!(stats.total_passes > 0);
        // All identity ops should be simplified
        assert_eq!(count_ops(&ir, "mul"), 0);
        assert_eq!(count_ops(&ir, "div"), 0);
    }

    // ===== Parity fixes tests =====

    // --- BUG FIX: div by zero folds to 0 (OSL semantics) ---

    #[test]
    fn test_fold_float_div_by_zero() {
        let mut ir = make_ir();
        let dst = add_sym(&mut ir, "dst", SymType::Local);
        let a = add_const_f(&mut ir, 5.0);
        let zero = add_const_f(&mut ir, 0.0);
        emit_op(&mut ir, "div", &[dst, a, zero]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "assign"), 1);
        assert_eq!(count_ops(&ir, "div"), 0);
        assert!(stats.constant_folds > 0);
    }

    #[test]
    fn test_fold_int_div_by_zero() {
        let mut ir = make_ir();
        let dst = add_sym_int(&mut ir, "dst", SymType::Local);
        let a = add_const_i(&mut ir, 7);
        let zero = add_const_i(&mut ir, 0);
        emit_op(&mut ir, "div", &[dst, a, zero]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "assign"), 1);
        assert!(stats.constant_folds > 0);
    }

    #[test]
    fn test_fold_float_mod_by_zero() {
        let mut ir = make_ir();
        let dst = add_sym(&mut ir, "dst", SymType::Local);
        let a = add_const_f(&mut ir, 3.0);
        let zero = add_const_f(&mut ir, 0.0);
        emit_op(&mut ir, "mod", &[dst, a, zero]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "assign"), 1);
        assert_eq!(count_ops(&ir, "mod"), 0);
        assert!(stats.constant_folds > 0);
    }

    // --- BUG FIX: eq/neq exact comparison ---

    #[test]
    fn test_fold_eq_exact() {
        let mut ir = make_ir();
        let dst = add_sym_int(&mut ir, "dst", SymType::Local);
        let a = add_const_f(&mut ir, 1.0);
        let b = add_const_f(&mut ir, 1.0);
        emit_op(&mut ir, "eq", &[dst, a, b]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "assign"), 1);
        assert!(stats.constant_folds > 0);
    }

    #[test]
    fn test_fold_neq_exact() {
        let mut ir = make_ir();
        let dst = add_sym_int(&mut ir, "dst", SymType::Local);
        let a = add_const_f(&mut ir, 1.0);
        let b = add_const_f(&mut ir, 2.0);
        emit_op(&mut ir, "neq", &[dst, a, b]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "assign"), 1);
    }

    // --- Int comparison folding ---

    #[test]
    fn test_fold_int_eq() {
        let mut ir = make_ir();
        let dst = add_sym_int(&mut ir, "dst", SymType::Local);
        let a = add_const_i(&mut ir, 5);
        let b = add_const_i(&mut ir, 5);
        emit_op(&mut ir, "eq", &[dst, a, b]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "assign"), 1);
        assert!(stats.constant_folds > 0);
    }

    #[test]
    fn test_fold_int_lt() {
        let mut ir = make_ir();
        let dst = add_sym_int(&mut ir, "dst", SymType::Local);
        let a = add_const_i(&mut ir, 3);
        let b = add_const_i(&mut ir, 7);
        emit_op(&mut ir, "lt", &[dst, a, b]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "assign"), 1);
    }

    // --- Int negation folding ---

    #[test]
    fn test_fold_neg_int() {
        let mut ir = make_ir();
        let dst = add_sym_int(&mut ir, "dst", SymType::Local);
        let a = add_const_i(&mut ir, 42);
        emit_op(&mut ir, "neg", &[dst, a]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "assign"), 1);
        assert_eq!(count_ops(&ir, "neg"), 0);
        assert!(stats.constant_folds > 0);
    }

    // --- Int abs folding ---

    #[test]
    fn test_fold_abs_int() {
        let mut ir = make_ir();
        let dst = add_sym_int(&mut ir, "dst", SymType::Local);
        let a = add_const_i(&mut ir, -10);
        emit_op(&mut ir, "abs", &[dst, a]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "assign"), 1);
        assert_eq!(count_ops(&ir, "abs"), 0);
        assert!(stats.constant_folds > 0);
    }

    // --- Bitwise ops folding ---

    #[test]
    fn test_fold_bitand() {
        let mut ir = make_ir();
        let dst = add_sym_int(&mut ir, "dst", SymType::Local);
        let a = add_const_i(&mut ir, 0xFF);
        let b = add_const_i(&mut ir, 0x0F);
        emit_op(&mut ir, "bitand", &[dst, a, b]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "assign"), 1);
        assert_eq!(count_ops(&ir, "bitand"), 0);
        assert!(stats.constant_folds > 0);
    }

    #[test]
    fn test_fold_bitor() {
        let mut ir = make_ir();
        let dst = add_sym_int(&mut ir, "dst", SymType::Local);
        let a = add_const_i(&mut ir, 0xF0);
        let b = add_const_i(&mut ir, 0x0F);
        emit_op(&mut ir, "bitor", &[dst, a, b]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "assign"), 1);
        assert_eq!(count_ops(&ir, "bitor"), 0);
    }

    #[test]
    fn test_fold_xor() {
        let mut ir = make_ir();
        let dst = add_sym_int(&mut ir, "dst", SymType::Local);
        let a = add_const_i(&mut ir, 0xFF);
        let b = add_const_i(&mut ir, 0x0F);
        emit_op(&mut ir, "xor", &[dst, a, b]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "assign"), 1);
        assert_eq!(count_ops(&ir, "xor"), 0);
    }

    #[test]
    fn test_fold_compl() {
        let mut ir = make_ir();
        let dst = add_sym_int(&mut ir, "dst", SymType::Local);
        let a = add_const_i(&mut ir, 0);
        emit_op(&mut ir, "compl", &[dst, a]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "assign"), 1);
        assert_eq!(count_ops(&ir, "compl"), 0);
        assert!(stats.constant_folds > 0);
    }

    // --- Logical ops folding ---

    #[test]
    fn test_fold_and() {
        let mut ir = make_ir();
        let dst = add_sym_int(&mut ir, "dst", SymType::Local);
        let a = add_const_i(&mut ir, 1);
        let b = add_const_i(&mut ir, 0);
        emit_op(&mut ir, "and", &[dst, a, b]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "assign"), 1);
        assert_eq!(count_ops(&ir, "and"), 0);
        assert!(stats.constant_folds > 0);
    }

    #[test]
    fn test_fold_or() {
        let mut ir = make_ir();
        let dst = add_sym_int(&mut ir, "dst", SymType::Local);
        let a = add_const_i(&mut ir, 0);
        let b = add_const_i(&mut ir, 1);
        emit_op(&mut ir, "or", &[dst, a, b]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "assign"), 1);
        assert_eq!(count_ops(&ir, "or"), 0);
    }

    // --- Clamp constant folding ---

    #[test]
    fn test_fold_clamp() {
        let mut ir = make_ir();
        let dst = add_sym(&mut ir, "dst", SymType::Local);
        let val = add_const_f(&mut ir, 5.0);
        let lo = add_const_f(&mut ir, 0.0);
        let hi = add_const_f(&mut ir, 3.0);
        emit_op(&mut ir, "clamp", &[dst, val, lo, hi]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "assign"), 1);
        assert_eq!(count_ops(&ir, "clamp"), 0);
        assert!(stats.constant_folds > 0);
    }

    // --- Peephole: sub A-A -> 0 ---

    #[test]
    fn test_peephole_sub_self() {
        let mut ir = make_ir();
        let dst = add_sym(&mut ir, "dst", SymType::Local);
        let x = add_sym(&mut ir, "x", SymType::Local);
        emit_op(&mut ir, "sub", &[dst, x, x]);
        let mut stats = OptStats::default();
        assert!(peephole_arith(&mut ir, &mut stats));
        assert_eq!(count_ops(&ir, "assign"), 1);
        assert_eq!(count_ops(&ir, "sub"), 0);
        assert!(stats.peephole_opts > 0);
    }

    // --- Peephole: sub(dst, x, 0) -> assign(dst, x) ---

    #[test]
    fn test_peephole_sub_zero() {
        let mut ir = make_ir();
        let dst = add_sym(&mut ir, "dst", SymType::Local);
        let x = add_sym(&mut ir, "x", SymType::Local);
        let zero = add_const_f(&mut ir, 0.0);
        emit_op(&mut ir, "sub", &[dst, x, zero]);
        let mut stats = OptStats::default();
        assert!(peephole_arith(&mut ir, &mut stats));
        assert_eq!(count_ops(&ir, "assign"), 1);
        assert_eq!(count_ops(&ir, "sub"), 0);
    }

    // --- Peephole: div(dst, x, 0) -> assign(dst, 0) (OSL) ---

    #[test]
    fn test_peephole_div_by_zero() {
        let mut ir = make_ir();
        let dst = add_sym(&mut ir, "dst", SymType::Local);
        let x = add_sym(&mut ir, "x", SymType::Local);
        let zero = add_const_f(&mut ir, 0.0);
        emit_op(&mut ir, "div", &[dst, x, zero]);
        let mut stats = OptStats::default();
        assert!(peephole_arith(&mut ir, &mut stats));
        assert_eq!(count_ops(&ir, "assign"), 1);
        assert_eq!(count_ops(&ir, "div"), 0);
        assert!(stats.peephole_opts > 0);
    }

    // --- Peephole: div(dst, 0, x) -> assign(dst, 0) ---

    #[test]
    fn test_peephole_div_zero_numerator() {
        let mut ir = make_ir();
        let dst = add_sym(&mut ir, "dst", SymType::Local);
        let x = add_sym(&mut ir, "x", SymType::Local);
        let zero = add_const_f(&mut ir, 0.0);
        emit_op(&mut ir, "div", &[dst, zero, x]);
        let mut stats = OptStats::default();
        assert!(peephole_arith(&mut ir, &mut stats));
        assert_eq!(count_ops(&ir, "assign"), 1);
        assert_eq!(count_ops(&ir, "div"), 0);
    }

    // --- Peephole: mod(dst, x, 0) -> assign(dst, 0) (OSL) ---

    #[test]
    fn test_peephole_mod_by_zero() {
        let mut ir = make_ir();
        let dst = add_sym(&mut ir, "dst", SymType::Local);
        let x = add_sym(&mut ir, "x", SymType::Local);
        let zero = add_const_f(&mut ir, 0.0);
        emit_op(&mut ir, "mod", &[dst, x, zero]);
        let mut stats = OptStats::default();
        assert!(peephole_arith(&mut ir, &mut stats));
        assert_eq!(count_ops(&ir, "assign"), 1);
        assert_eq!(count_ops(&ir, "mod"), 0);
        assert!(stats.peephole_opts > 0);
    }

    // --- Peephole: mod(dst, 0, x) -> assign(dst, 0) ---

    #[test]
    fn test_peephole_mod_zero_numerator() {
        let mut ir = make_ir();
        let dst = add_sym(&mut ir, "dst", SymType::Local);
        let x = add_sym(&mut ir, "x", SymType::Local);
        let zero = add_const_f(&mut ir, 0.0);
        emit_op(&mut ir, "mod", &[dst, zero, x]);
        let mut stats = OptStats::default();
        assert!(peephole_arith(&mut ir, &mut stats));
        assert_eq!(count_ops(&ir, "assign"), 1);
        assert_eq!(count_ops(&ir, "mod"), 0);
    }

    // ===== New constant folding rule tests =====

    fn add_const_str(ir: &mut ShaderIR, val: &str) -> i32 {
        let idx = ir.symbols.len();
        let name = format!("$cs_{val}");
        let mut sym = Symbol::new(
            UString::new(&name),
            TypeSpec::from_simple(TypeDesc::STRING),
            SymType::Const,
        );
        sym.initializers = 1;
        ir.symbols.push(sym);
        ir.const_values
            .push((idx, ConstValue::String(UString::new(val))));
        idx as i32
    }

    fn add_const_vec3(ir: &mut ShaderIR, x: f32, y: f32, z: f32) -> i32 {
        let idx = ir.symbols.len();
        let name = format!("$cv_{x}_{y}_{z}");
        let mut sym = Symbol::new(
            UString::new(&name),
            TypeSpec::from_simple(TypeDesc::COLOR),
            SymType::Const,
        );
        sym.initializers = 1;
        ir.symbols.push(sym);
        ir.const_values
            .push((idx, ConstValue::Vec3(crate::math::Vec3 { x, y, z })));
        idx as i32
    }

    fn add_sym_str(ir: &mut ShaderIR, name: &str, st: SymType) -> i32 {
        let idx = ir.symbols.len();
        ir.symbols.push(Symbol::new(
            UString::new(name),
            TypeSpec::from_simple(TypeDesc::STRING),
            st,
        ));
        idx as i32
    }

    fn add_sym_vec3(ir: &mut ShaderIR, name: &str, st: SymType) -> i32 {
        let idx = ir.symbols.len();
        ir.symbols.push(Symbol::new(
            UString::new(name),
            TypeSpec::from_simple(TypeDesc::COLOR),
            st,
        ));
        idx as i32
    }

    fn add_const_float_array(ir: &mut ShaderIR, vals: &[f32]) -> i32 {
        let idx = ir.symbols.len();
        let mut sym = Symbol::new(
            UString::new(&format!("$cfa_{}", vals.len())),
            TypeSpec::from_simple(TypeDesc::FLOAT.array(vals.len() as i32)),
            SymType::Const,
        );
        sym.initializers = vals.len() as i32;
        ir.symbols.push(sym);
        ir.const_values
            .push((idx, ConstValue::FloatArray(vals.to_vec())));
        idx as i32
    }

    fn add_const_int_array(ir: &mut ShaderIR, vals: &[i32]) -> i32 {
        let idx = ir.symbols.len();
        let mut sym = Symbol::new(
            UString::new(&format!("$cia_{}", vals.len())),
            TypeSpec::from_simple(TypeDesc::INT.array(vals.len() as i32)),
            SymType::Const,
        );
        sym.initializers = vals.len() as i32;
        ir.symbols.push(sym);
        ir.const_values
            .push((idx, ConstValue::IntArray(vals.to_vec())));
        idx as i32
    }

    fn emit_op_jumps(ir: &mut ShaderIR, name: &str, args: &[i32], jumps: [i32; 4]) {
        let fa = ir.args.len() as i32;
        for &a in args {
            ir.args.push(a);
        }
        let mut op = Opcode::new(
            UString::new(name),
            UString::default(),
            fa,
            args.len() as i32,
        );
        op.jump = jumps;
        if !args.is_empty() {
            op.argread = !0u32;
        }
        ir.opcodes.push(op);
    }

    #[test]
    fn test_cfold_if_true() {
        // Our codegen semantics: jump[0]=true_start, jump[1]=false_start
        // Layout: [0] if(cond=1), [1] assign(then), [2] nop(->end), [3] assign(else)
        let mut ir = make_ir();
        let cond = add_const_i(&mut ir, 1);
        let dst = add_sym(&mut ir, "x", SymType::Local);
        let one = add_const_f(&mut ir, 1.0);
        let two = add_const_f(&mut ir, 2.0);
        // jump[0]=1 (true starts at op 1), jump[1]=3 (false starts at op 3)
        emit_op_jumps(&mut ir, "if", &[cond], [1, 3, -1, -1]);
        emit_op(&mut ir, "assign", &[dst, one]); // opcode 1: then-block
        emit_op_jumps(&mut ir, "nop", &[], [4, -1, -1, -1]); // opcode 2: jump past else
        emit_op(&mut ir, "assign", &[dst, two]); // opcode 3: else-block
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        // TRUE: nop if (op 0), nop false branch [3..4) -> op 3 nop'd
        assert_eq!(ir.opcodes[0].op.as_str(), "nop");
        assert_eq!(ir.opcodes[1].op.as_str(), "assign");
        assert_eq!(ir.opcodes[3].op.as_str(), "nop");
        assert!(stats.constant_folds > 0);
    }

    #[test]
    fn test_cfold_if_false() {
        // Our codegen semantics: jump[0]=true_start, jump[1]=false_start
        // Layout: [0] if(cond=0), [1] assign(then), [2] nop(->end), [3] assign(else)
        let mut ir = make_ir();
        let cond = add_const_i(&mut ir, 0);
        let dst = add_sym(&mut ir, "x", SymType::Local);
        let one = add_const_f(&mut ir, 1.0);
        let two = add_const_f(&mut ir, 2.0);
        // jump[0]=1 (true starts at op 1), jump[1]=3 (false starts at op 3)
        emit_op_jumps(&mut ir, "if", &[cond], [1, 3, -1, -1]);
        emit_op(&mut ir, "assign", &[dst, one]); // opcode 1: then-block
        emit_op_jumps(&mut ir, "nop", &[], [4, -1, -1, -1]); // opcode 2: jump past else
        emit_op(&mut ir, "assign", &[dst, two]); // opcode 3: else-block
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        // FALSE: nop if (op 0), nop true branch [1..3) -> nop [1],[2]; keep else [3]
        assert_eq!(ir.opcodes[0].op.as_str(), "nop");
        assert_eq!(ir.opcodes[1].op.as_str(), "nop");
        assert_eq!(ir.opcodes[2].op.as_str(), "nop");
        assert_eq!(ir.opcodes[3].op.as_str(), "assign");
        assert!(stats.constant_folds > 0);
    }

    #[test]
    fn test_cfold_dot() {
        let mut ir = make_ir();
        let dst = add_sym(&mut ir, "dst", SymType::Local);
        let a = add_const_vec3(&mut ir, 1.0, 2.0, 3.0);
        let b = add_const_vec3(&mut ir, 4.0, 5.0, 6.0);
        emit_op(&mut ir, "dot", &[dst, a, b]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "assign"), 1);
        assert_eq!(count_ops(&ir, "dot"), 0);
        assert!(stats.constant_folds > 0);
    }

    #[test]
    fn test_cfold_compref() {
        let mut ir = make_ir();
        let dst = add_sym(&mut ir, "dst", SymType::Local);
        let v = add_const_vec3(&mut ir, 10.0, 20.0, 30.0);
        let idx = add_const_i(&mut ir, 1);
        emit_op(&mut ir, "compref", &[dst, v, idx]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "compref"), 0);
        assert!(stats.constant_folds > 0);
    }

    #[test]
    fn test_cfold_compassign() {
        let mut ir = make_ir();
        let v = add_sym_vec3(&mut ir, "v", SymType::Local);
        let i0 = add_const_i(&mut ir, 0);
        let i1 = add_const_i(&mut ir, 1);
        let i2 = add_const_i(&mut ir, 2);
        let v0 = add_const_f(&mut ir, 1.0);
        let v1 = add_const_f(&mut ir, 2.0);
        let v2 = add_const_f(&mut ir, 3.0);
        emit_op(&mut ir, "compassign", &[v, i0, v0]);
        emit_op(&mut ir, "compassign", &[v, i1, v1]);
        emit_op(&mut ir, "compassign", &[v, i2, v2]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "assign"), 1);
        assert_eq!(count_ops(&ir, "compassign"), 0);
        assert!(stats.constant_folds > 0);
    }

    #[test]
    fn test_cfold_mxcompassign() {
        let mut ir = make_ir();
        let mx = add_sym(&mut ir, "mx", SymType::Local);
        for row in 0..4i32 {
            for col in 0..4i32 {
                let ri = add_const_i(&mut ir, row);
                let ci = add_const_i(&mut ir, col);
                let cv = add_const_f(&mut ir, (row * 4 + col) as f32);
                emit_op(&mut ir, "mxcompassign", &[mx, ri, ci, cv]);
            }
        }
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "assign"), 1);
        assert_eq!(count_ops(&ir, "mxcompassign"), 0);
        assert!(stats.constant_folds > 0);
    }

    #[test]
    fn test_cfold_aref_float() {
        let mut ir = make_ir();
        let dst = add_sym(&mut ir, "dst", SymType::Local);
        let arr = add_const_float_array(&mut ir, &[10.0, 20.0, 30.0]);
        let idx = add_const_i(&mut ir, 1);
        emit_op(&mut ir, "aref", &[dst, arr, idx]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "aref"), 0);
        assert!(stats.constant_folds > 0);
    }

    #[test]
    fn test_cfold_aref_int() {
        let mut ir = make_ir();
        let dst = add_sym_int(&mut ir, "dst", SymType::Local);
        let arr = add_const_int_array(&mut ir, &[100, 200, 300]);
        let idx = add_const_i(&mut ir, 2);
        emit_op(&mut ir, "aref", &[dst, arr, idx]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "aref"), 0);
        assert!(stats.constant_folds > 0);
    }

    #[test]
    fn test_cfold_arraylength() {
        let mut ir = make_ir();
        let dst = add_sym_int(&mut ir, "dst", SymType::Local);
        let arr = add_const_float_array(&mut ir, &[1.0, 2.0, 3.0, 4.0, 5.0]);
        emit_op(&mut ir, "arraylength", &[dst, arr]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "arraylength"), 0);
        assert!(stats.constant_folds > 0);
    }

    #[test]
    fn test_cfold_aassign() {
        let mut ir = make_ir();
        let ai = ir.symbols.len();
        let mut as_ = Symbol::new(
            UString::new("arr"),
            TypeSpec::from_simple(TypeDesc::FLOAT.array(3)),
            SymType::Local,
        );
        as_.initializers = 3;
        ir.symbols.push(as_);
        let arr = ai as i32;
        let i0 = add_const_i(&mut ir, 0);
        let i1 = add_const_i(&mut ir, 1);
        let i2 = add_const_i(&mut ir, 2);
        let v0 = add_const_f(&mut ir, 10.0);
        let v1 = add_const_f(&mut ir, 20.0);
        let v2 = add_const_f(&mut ir, 30.0);
        emit_op(&mut ir, "aassign", &[arr, i0, v0]);
        emit_op(&mut ir, "aassign", &[arr, i1, v1]);
        emit_op(&mut ir, "aassign", &[arr, i2, v2]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "assign"), 1);
        assert_eq!(count_ops(&ir, "aassign"), 0);
        assert!(stats.constant_folds > 0);
    }

    #[test]
    fn test_cfold_strlen() {
        let mut ir = make_ir();
        let dst = add_sym_int(&mut ir, "dst", SymType::Local);
        let s = add_const_str(&mut ir, "hello");
        emit_op(&mut ir, "strlen", &[dst, s]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "strlen"), 0);
        assert!(stats.constant_folds > 0);
    }

    #[test]
    fn test_cfold_hash() {
        let mut ir = make_ir();
        let dst = add_sym_int(&mut ir, "dst", SymType::Local);
        let s = add_const_str(&mut ir, "test");
        emit_op(&mut ir, "hash", &[dst, s]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "hash"), 0);
        assert!(stats.constant_folds > 0);
    }

    #[test]
    fn test_cfold_getchar() {
        let mut ir = make_ir();
        let dst = add_sym_int(&mut ir, "dst", SymType::Local);
        let s = add_const_str(&mut ir, "ABC");
        let idx = add_const_i(&mut ir, 1);
        emit_op(&mut ir, "getchar", &[dst, s, idx]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "getchar"), 0);
        assert!(stats.constant_folds > 0);
    }

    #[test]
    fn test_cfold_endswith() {
        let mut ir = make_ir();
        let dst = add_sym_int(&mut ir, "dst", SymType::Local);
        let s = add_const_str(&mut ir, "hello_world");
        let suf = add_const_str(&mut ir, "world");
        emit_op(&mut ir, "endswith", &[dst, s, suf]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "endswith"), 0);
        assert!(stats.constant_folds > 0);
    }

    #[test]
    fn test_cfold_startswith() {
        let mut ir = make_ir();
        let dst = add_sym_int(&mut ir, "dst", SymType::Local);
        let s = add_const_str(&mut ir, "hello_world");
        let pre = add_const_str(&mut ir, "hello");
        emit_op(&mut ir, "startswith", &[dst, s, pre]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "startswith"), 0);
        assert!(stats.constant_folds > 0);
    }

    #[test]
    fn test_cfold_stoi() {
        let mut ir = make_ir();
        let dst = add_sym_int(&mut ir, "dst", SymType::Local);
        let s = add_const_str(&mut ir, "42");
        emit_op(&mut ir, "stoi", &[dst, s]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "stoi"), 0);
        assert!(stats.constant_folds > 0);
    }

    #[test]
    fn test_cfold_stof() {
        let mut ir = make_ir();
        let dst = add_sym(&mut ir, "dst", SymType::Local);
        let s = add_const_str(&mut ir, "3.14");
        emit_op(&mut ir, "stof", &[dst, s]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "stof"), 0);
        assert!(stats.constant_folds > 0);
    }

    #[test]
    fn test_cfold_concat() {
        let mut ir = make_ir();
        let dst = add_sym_str(&mut ir, "dst", SymType::Local);
        let a = add_const_str(&mut ir, "hello");
        let b = add_const_str(&mut ir, " world");
        emit_op(&mut ir, "concat", &[dst, a, b]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "concat"), 0);
        assert!(stats.constant_folds > 0);
    }

    #[test]
    fn test_cfold_substr() {
        let mut ir = make_ir();
        let dst = add_sym_str(&mut ir, "dst", SymType::Local);
        let s = add_const_str(&mut ir, "hello world");
        let start = add_const_i(&mut ir, 6);
        let len = add_const_i(&mut ir, 5);
        emit_op(&mut ir, "substr", &[dst, s, start, len]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "substr"), 0);
        assert!(stats.constant_folds > 0);
    }

    #[test]
    fn test_cfold_format_int() {
        let mut ir = make_ir();
        let dst = add_sym_str(&mut ir, "dst", SymType::Local);
        let fmt = add_const_str(&mut ir, "val=%d");
        let val = add_const_i(&mut ir, 42);
        emit_op(&mut ir, "format", &[dst, fmt, val]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "format"), 0);
        assert!(stats.constant_folds > 0);
    }

    #[test]
    fn test_cfold_regex_literal() {
        let mut ir = make_ir();
        let dst = add_sym_int(&mut ir, "dst", SymType::Local);
        let subj = add_const_str(&mut ir, "hello world");
        let pat = add_const_str(&mut ir, "world");
        emit_op(&mut ir, "regex_search", &[dst, subj, pat]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "regex_search"), 0);
        assert!(stats.constant_folds > 0);
    }

    #[test]
    fn test_cfold_regex_complex_nofold() {
        let mut ir = make_ir();
        let dst = add_sym_int(&mut ir, "dst", SymType::Local);
        let subj = add_const_str(&mut ir, "hello");
        let pat = add_const_str(&mut ir, "h.*o");
        emit_op(&mut ir, "regex_search", &[dst, subj, pat]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "regex_search"), 1);
    }

    #[test]
    fn test_cfold_split() {
        let mut ir = make_ir();
        let dst = add_sym_int(&mut ir, "dst", SymType::Local);
        let s = add_const_str(&mut ir, "a b c");
        let res = add_sym_str(&mut ir, "res", SymType::Local);
        emit_op(&mut ir, "split", &[dst, s, res]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "split"), 0);
        assert!(stats.constant_folds > 0);
    }

    #[test]
    fn test_cfold_split_sep() {
        let mut ir = make_ir();
        let dst = add_sym_int(&mut ir, "dst", SymType::Local);
        let s = add_const_str(&mut ir, "a,b,c");
        let res = add_sym_str(&mut ir, "res", SymType::Local);
        let sep = add_const_str(&mut ir, ",");
        emit_op(&mut ir, "split", &[dst, s, res, sep]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "split"), 0);
        assert!(stats.constant_folds > 0);
    }

    // --- constfold_select tests ---

    #[test]
    fn test_cfold_select_const_zero() {
        // select(dst, a, b, 0) => assign(dst, a)
        let mut ir = make_ir();
        let dst = add_sym(&mut ir, "dst", SymType::Local);
        let a = add_sym(&mut ir, "a", SymType::Local);
        let b = add_sym(&mut ir, "b", SymType::Local);
        let cond = add_const_i(&mut ir, 0);
        emit_op(&mut ir, "select", &[dst, a, b, cond]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "select"), 0);
        assert_eq!(count_ops(&ir, "assign"), 1);
        // Result should be a (the first operand when cond==0)
        let fa = ir.opcodes[0].firstarg as usize;
        assert_eq!(ir.args[fa + 1], a);
    }

    #[test]
    fn test_cfold_select_const_nonzero() {
        // select(dst, a, b, 1) => assign(dst, b)
        let mut ir = make_ir();
        let dst = add_sym(&mut ir, "dst", SymType::Local);
        let a = add_sym(&mut ir, "a", SymType::Local);
        let b = add_sym(&mut ir, "b", SymType::Local);
        let cond = add_const_i(&mut ir, 1);
        emit_op(&mut ir, "select", &[dst, a, b, cond]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "select"), 0);
        assert_eq!(count_ops(&ir, "assign"), 1);
        let fa = ir.opcodes[0].firstarg as usize;
        assert_eq!(ir.args[fa + 1], b);
    }

    #[test]
    fn test_cfold_select_same_operands() {
        // select(dst, a, a, cond) => assign(dst, a) regardless of cond
        let mut ir = make_ir();
        let dst = add_sym(&mut ir, "dst", SymType::Local);
        let a = add_sym(&mut ir, "a", SymType::Local);
        let cond = add_sym_int(&mut ir, "cond", SymType::Local); // non-constant!
        emit_op(&mut ir, "select", &[dst, a, a, cond]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "select"), 0);
        assert_eq!(count_ops(&ir, "assign"), 1);
        let fa = ir.opcodes[0].firstarg as usize;
        assert_eq!(ir.args[fa + 1], a);
    }

    // --- constfold_sincos tests ---

    #[test]
    fn test_cfold_sincos() {
        // sincos(0.0, sin_dst, cos_dst) => assign sin_dst 0.0; assign cos_dst 1.0
        let mut ir = make_ir();
        let angle = add_const_f(&mut ir, 0.0);
        let sin_dst = add_sym(&mut ir, "s", SymType::Local);
        let cos_dst = add_sym(&mut ir, "c", SymType::Local);
        emit_op(&mut ir, "sincos", &[angle, sin_dst, cos_dst]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "sincos"), 0);
        assert_eq!(count_ops(&ir, "assign"), 2);
        assert!(stats.constant_folds > 0);
    }

    #[test]
    fn test_cfold_sincos_pi() {
        // sincos(PI, sin_dst, cos_dst) => sin ~ 0, cos ~ -1
        let mut ir = make_ir();
        let angle = add_const_f(&mut ir, std::f32::consts::PI);
        let sin_dst = add_sym(&mut ir, "s", SymType::Local);
        let cos_dst = add_sym(&mut ir, "c", SymType::Local);
        emit_op(&mut ir, "sincos", &[angle, sin_dst, cos_dst]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "sincos"), 0);
        assert_eq!(count_ops(&ir, "assign"), 2);
    }

    // --- constfold_normalize tests ---

    #[test]
    fn test_cfold_normalize() {
        let mut ir = make_ir();
        let dst = add_sym_vec3(&mut ir, "dst", SymType::Local);
        let v = add_const_vec3(&mut ir, 3.0, 0.0, 4.0);
        emit_op(&mut ir, "normalize", &[dst, v]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "normalize"), 0);
        assert_eq!(count_ops(&ir, "assign"), 1);
        assert!(stats.constant_folds > 0);
    }

    #[test]
    fn test_cfold_normalize_zero() {
        // normalize(0,0,0) => (0,0,0)
        let mut ir = make_ir();
        let dst = add_sym_vec3(&mut ir, "dst", SymType::Local);
        let v = add_const_vec3(&mut ir, 0.0, 0.0, 0.0);
        emit_op(&mut ir, "normalize", &[dst, v]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "normalize"), 0);
        assert!(stats.constant_folds > 0);
    }

    // --- constfold_deriv tests ---

    #[test]
    fn test_cfold_dx_const() {
        // Dx(const_float) => 0
        let mut ir = make_ir();
        let dst = add_sym(&mut ir, "dst", SymType::Local);
        let src = add_const_f(&mut ir, 42.0);
        emit_op(&mut ir, "Dx", &[dst, src]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "Dx"), 0);
        assert_eq!(count_ops(&ir, "assign"), 1);
        assert!(stats.constant_folds > 0);
    }

    #[test]
    fn test_cfold_dy_const() {
        let mut ir = make_ir();
        let dst = add_sym(&mut ir, "dst", SymType::Local);
        let src = add_const_f(&mut ir, 1.5);
        emit_op(&mut ir, "Dy", &[dst, src]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "Dy"), 0);
        assert!(stats.constant_folds > 0);
    }

    #[test]
    fn test_cfold_dz_const() {
        let mut ir = make_ir();
        let dst = add_sym(&mut ir, "dst", SymType::Local);
        let src = add_const_f(&mut ir, -3.14);
        emit_op(&mut ir, "Dz", &[dst, src]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "Dz"), 0);
        assert!(stats.constant_folds > 0);
    }

    #[test]
    fn test_cfold_dx_nonconstant_nofold() {
        // Dx(variable) should NOT be folded
        let mut ir = make_ir();
        let dst = add_sym(&mut ir, "dst", SymType::Local);
        let src = add_sym(&mut ir, "x", SymType::Local);
        emit_op(&mut ir, "Dx", &[dst, src]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "Dx"), 1); // not folded
    }

    // --- constfold_isconstant tests ---

    #[test]
    fn test_cfold_isconstant_true() {
        let mut ir = make_ir();
        let dst = add_sym_int(&mut ir, "dst", SymType::Local);
        let src = add_const_f(&mut ir, 1.0);
        emit_op(&mut ir, "isconstant", &[dst, src]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "isconstant"), 0);
        assert_eq!(count_ops(&ir, "assign"), 1);
        assert!(stats.constant_folds > 0);
    }

    #[test]
    fn test_cfold_isconstant_nonconstant_nofold() {
        // Non-constant arg should NOT fold (might become constant later)
        let mut ir = make_ir();
        let dst = add_sym_int(&mut ir, "dst", SymType::Local);
        let src = add_sym(&mut ir, "x", SymType::Local);
        emit_op(&mut ir, "isconstant", &[dst, src]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "isconstant"), 1); // not folded
    }

    // --- constfold_functioncall tests ---

    fn emit_op_with_jump(ir: &mut ShaderIR, name: &str, args: &[i32], jump0: i32) {
        let fa = ir.args.len() as i32;
        for &a in args {
            ir.args.push(a);
        }
        let mut op = crate::symbol::Opcode::new(
            UString::new(name),
            UString::default(),
            fa,
            args.len() as i32,
        );
        if !args.is_empty() {
            op.argwrite = 1;
            op.argread = !1u32;
        }
        op.jump[0] = jump0;
        ir.opcodes.push(op);
    }

    #[test]
    fn test_cfold_functioncall_empty_body() {
        // functioncall with only return => nop everything
        let mut ir = make_ir();
        let dummy = add_sym(&mut ir, "fn", SymType::Local);
        emit_op_with_jump(&mut ir, "functioncall", &[dummy], 3); // jump to op 3
        emit_op(&mut ir, "return", &[]); // op 1
        emit_op(&mut ir, "nop", &[]); // op 2
        emit_op(&mut ir, "assign", &[dummy, dummy]); // op 3 (after fn)
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "functioncall"), 0);
        assert_eq!(count_ops(&ir, "return"), 0);
        assert!(stats.constant_folds > 0);
    }

    #[test]
    fn test_cfold_functioncall_with_body_no_return() {
        // functioncall with real ops but no return => nop just the functioncall
        let mut ir = make_ir();
        let dst = add_sym(&mut ir, "dst", SymType::Local);
        let src = add_sym(&mut ir, "src", SymType::Local);
        emit_op_with_jump(&mut ir, "functioncall", &[dst], 3); // jump to op 3
        emit_op(&mut ir, "add", &[dst, src, src]); // op 1: real work
        emit_op(&mut ir, "nop", &[]); // op 2
        emit_op(&mut ir, "assign", &[dst, dst]); // op 3 (after fn)
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "functioncall"), 0); // noped
        assert_eq!(count_ops(&ir, "add"), 1); // body preserved
    }

    #[test]
    fn test_cfold_functioncall_with_body_and_return() {
        // functioncall with real body + return => no folding
        let mut ir = make_ir();
        let dst = add_sym(&mut ir, "dst", SymType::Local);
        let src = add_sym(&mut ir, "src", SymType::Local);
        emit_op_with_jump(&mut ir, "functioncall", &[dst], 3);
        emit_op(&mut ir, "add", &[dst, src, src]);
        emit_op(&mut ir, "return", &[]);
        emit_op(&mut ir, "assign", &[dst, dst]);
        let mut stats = OptStats::default();
        constant_fold(&mut ir, &mut stats);
        assert_eq!(count_ops(&ir, "functioncall"), 1); // kept
        assert_eq!(count_ops(&ir, "add"), 1); // body kept
    }
}
