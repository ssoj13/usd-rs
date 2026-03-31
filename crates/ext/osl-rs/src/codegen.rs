#![allow(dead_code)]
//! Code generation — AST to IR opcode emission.
//!
//! Port of `codegen.cpp` from the OSL compiler. Transforms the AST
//! into a linear sequence of IR opcodes (Opcode + symbol references).
//!
//! ## P1-9: Non-standard opcodes — DONE for preinc/predec/init_array
//! preinc/predec/postinc/postdec are lowered to add/sub with typed constants.
//! init_array removed — array storage comes from symbol table, elements via aassign.
//!
//! ## P1-9/P1-12: getfield/setfield replaced by struct field expansion
//! Struct variables are expanded into individual per-field sub-symbols at
//! declaration time (matching C++ add_struct_fields). StructSelect resolves
//! directly to the sub-symbol; struct assignment emits per-field assign/arraycopy.
//! Triple component access (color.r) uses compref/compassign as before.

use std::collections::HashMap;

use crate::ast::*;
use crate::symbol::{Opcode, ShaderType, SymType, Symbol};
use crate::typedesc::TypeDesc;
use crate::typespec::{TypeSpec, get_struct};
use crate::ustring::UString;

/// OSO metadata type name from TypeSpec.
fn metadata_type_name(ts: &TypeSpec) -> String {
    use crate::typedesc::{Aggregate, BaseType};
    let sd = ts.simpletype();
    let base = BaseType::from_u8(sd.basetype);
    let agg = sd.aggregate;
    match (base, agg) {
        (BaseType::Int32, _) if agg == Aggregate::Scalar as u8 => "int".into(),
        (BaseType::Float, _) if agg == Aggregate::Scalar as u8 => "float".into(),
        (BaseType::String, _) if agg == Aggregate::Scalar as u8 => "string".into(),
        _ => "float".into(),
    }
}

/// String value for OSO %meta from AST init (Literal).
fn metadata_value_str(init: &Option<Box<ASTNode>>) -> String {
    if let Some(node) = init {
        if let ASTNodeKind::Literal { value } = &node.kind {
            return match value {
                LiteralValue::Int(v) => v.to_string(),
                LiteralValue::Float(v) => v.to_string(),
                LiteralValue::String(s) => s.clone(),
            };
        }
    }
    String::new()
}

/// A constant value stored alongside the IR.
#[derive(Debug, Clone)]
pub enum ConstValue {
    Int(i32),
    Float(f32),
    String(UString),
    Vec3(crate::math::Vec3),
    Matrix(crate::math::Matrix44),
    IntArray(Vec<i32>),
    FloatArray(Vec<f32>),
    StringArray(Vec<UString>),
}

/// Generated IR for a shader.
#[derive(Debug, Clone)]
pub struct ShaderIR {
    /// All symbols (params, locals, temps, constants, globals).
    pub symbols: Vec<Symbol>,
    /// All opcodes.
    pub opcodes: Vec<Opcode>,
    /// Argument indices (flattened; opcodes reference ranges in this vec).
    pub args: Vec<i32>,
    /// Shader type.
    pub shader_type: ShaderType,
    /// Shader name.
    pub shader_name: String,
    /// Constant values: symbol_index -> value (for Const symbols).
    pub const_values: Vec<(usize, ConstValue)>,
    /// Default parameter values: symbol_index -> value (for Param symbols).
    pub param_defaults: Vec<(usize, ConstValue)>,
}

impl Default for ShaderIR {
    fn default() -> Self {
        Self {
            symbols: Vec::new(),
            opcodes: Vec::new(),
            args: Vec::new(),
            shader_type: ShaderType::Unknown,
            shader_name: String::new(),
            const_values: Vec::new(),
            param_defaults: Vec::new(),
        }
    }
}

impl ShaderIR {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Track a loop for break/continue handling.
struct LoopContext {
    /// Opcode indices of nop opcodes that need to be patched to the loop end.
    break_patches: Vec<usize>,
    /// Opcode indices of nop opcodes that need to be patched to the continue target (iterator step).
    continue_patches: Vec<usize>,
}

/// Information about a user-defined function recorded during codegen.
#[derive(Debug, Clone)]
struct UserFunc {
    /// Opcode index where the function body starts (after the skip nop).
    body_start: i32,
    /// Symbol indices of the formal parameters.
    formals: Vec<i32>,
    /// TypeSpecs of the formal parameters (for overload resolution).
    formal_types: Vec<TypeSpec>,
    /// Whether each formal parameter is an output parameter.
    formal_is_output: Vec<bool>,
    /// Symbol index of the return value (if non-void).
    return_sym: i32,
    /// Return type of the function.
    return_type: TypeSpec,
}

/// Code generation context.
pub struct CodeGen {
    ir: ShaderIR,
    /// Next temp ID.
    temp_counter: i32,
    /// Current scope depth.
    scope_depth: i32,
    /// Stack of loop contexts for break/continue.
    loop_stack: Vec<LoopContext>,
    /// Current codegen method name (for opcode method field).
    current_method: String,
    /// Map of user-defined function names to their info.
    user_funcs: HashMap<String, Vec<UserFunc>>,
    /// Return symbol index for the current function being compiled (-1 if in main shader).
    current_return_sym: i32,
    /// Name->index lookup for O(1) symbol resolution (name -> vec of (scope, sym_idx)).
    sym_lookup: HashMap<String, Vec<(i32, usize)>>,
    /// Current source file name for opcode source info.
    current_sourcefile: UString,
    /// Current source line for opcode source info.
    current_line: i32,
}

impl CodeGen {
    pub fn new() -> Self {
        let mut cg = Self {
            ir: ShaderIR::new(),
            temp_counter: 0,
            scope_depth: 0,
            loop_stack: Vec::new(),
            current_method: String::new(),
            user_funcs: HashMap::new(),
            current_return_sym: -1,
            sym_lookup: HashMap::new(),
            current_sourcefile: UString::default(),
            current_line: 0,
        };
        cg.create_globals();
        cg
    }

    /// Pre-create symbols for all OSL shader globals.
    fn create_globals(&mut self) {
        let float_type = TypeSpec::from_simple(TypeDesc::FLOAT);
        let point_type = TypeSpec::from_simple(TypeDesc::POINT);
        let vector_type = TypeSpec::from_simple(TypeDesc::VECTOR);
        let normal_type = TypeSpec::from_simple(TypeDesc::NORMAL);
        let int_type = TypeSpec::from_simple(TypeDesc::INT);

        let globals: &[(&str, TypeSpec)] = &[
            ("P", point_type),
            ("dPdx", vector_type),
            ("dPdy", vector_type),
            ("dPdz", vector_type),
            ("I", vector_type),
            ("dIdx", vector_type),
            ("dIdy", vector_type),
            ("N", normal_type),
            ("Ng", normal_type),
            ("u", float_type),
            ("dudx", float_type),
            ("dudy", float_type),
            ("v", float_type),
            ("dvdx", float_type),
            ("dvdy", float_type),
            ("dPdu", vector_type),
            ("dPdv", vector_type),
            ("time", float_type),
            ("dtime", float_type),
            ("dPdtime", vector_type),
            ("Ps", point_type),
            ("surfacearea", float_type),
            ("raytype", int_type),
            ("flipHandedness", int_type),
            ("backfacing", int_type),
            ("Ci", TypeSpec::closure(TypeDesc::COLOR)), // output closure color
        ];

        for &(name, ts) in globals {
            let idx = self.ir.symbols.len();
            let mut sym = self.make_sym(name, ts, SymType::Global);
            // Mark derivative-carrying globals (P, I, u, v, Ps need derivs)
            match name {
                "P" | "I" | "u" | "v" | "Ps" => sym.has_derivs = true,
                _ => {}
            }
            self.ir.symbols.push(sym);
            self.sym_lookup
                .entry(name.to_string())
                .or_default()
                .push((0, idx));
        }
    }

    /// Returns true for builtin functions that always return void.
    /// Used on the no-typecheck path to avoid emitting a spurious result slot.
    fn is_void_builtin(name: &str) -> bool {
        matches!(
            name,
            "sincos"
                | "printf"
                | "fprintf"
                | "warning"
                | "error"
                | "setmessage"
                | "exit"
                | "useparam"
        )
    }

    /// Generate IR from an AST.
    pub fn generate(&mut self, nodes: &[Box<ASTNode>]) -> ShaderIR {
        for node in nodes {
            self.gen_node(node);
        }
        std::mem::take(&mut self.ir)
    }

    /// Create a symbol with the given name, type, and kind.
    fn make_sym(&self, name: &str, typespec: TypeSpec, symtype: SymType) -> Symbol {
        let mut sym = Symbol::new(UString::new(name), typespec, symtype);
        sym.scope = self.scope_depth;
        sym
    }

    /// Register a named symbol in the lookup table for O(1) resolution.
    fn register_sym(&mut self, name: &str, idx: usize) {
        self.sym_lookup
            .entry(name.to_string())
            .or_default()
            .push((self.scope_depth, idx));
    }

    /// Remove all sym_lookup entries at the given scope depth.
    fn cleanup_scope(&mut self, depth: i32) {
        self.sym_lookup.retain(|_, entries| {
            entries.retain(|&(scope, _)| scope != depth);
            !entries.is_empty()
        });
    }

    /// Find a symbol index by name, returning the innermost-scope match.
    fn find_sym_by_name(&self, name: &str) -> Option<usize> {
        if let Some(entries) = self.sym_lookup.get(name) {
            let mut best: Option<usize> = None;
            let mut best_scope = -1i32;
            for &(scope, idx) in entries {
                if scope <= self.scope_depth && scope >= best_scope {
                    best_scope = scope;
                    best = Some(idx);
                }
            }
            return best;
        }
        None
    }

    /// Expand struct fields into individual sub-symbols (C++ add_struct_fields).
    /// For a struct variable "s" with fields {float a; color b;}, creates:
    ///   - symbol "s.a" (float, fieldid=0)
    ///   - symbol "s.b" (color, fieldid=1)
    /// Nested structs are handled recursively.
    fn add_struct_fields(&mut self, basename: &str, ts: TypeSpec, symtype: SymType, arraylen: i32) {
        let struct_id = ts.structure_id();
        if struct_id <= 0 {
            return;
        }
        let spec = match get_struct(struct_id as i32) {
            Some(s) => s,
            None => return,
        };
        for (i, field) in spec.fields.iter().enumerate() {
            let fieldname = format!("{}.{}", basename, field.name.as_str());
            let mut field_ts = field.type_spec;
            let field_arr = field_ts.simpletype().arraylen;
            // Translate outer array into inner array (C++ codegen.cpp:144-148)
            if arraylen != 0 || field_arr != 0 {
                let arr = (arraylen.max(1)) * (field_arr.max(1));
                field_ts.make_array(arr);
            }
            let idx = self.ir.symbols.len();
            let mut sym = self.make_sym(&fieldname, field_ts, symtype);
            sym.fieldid = i as i16;
            self.ir.symbols.push(sym);
            self.register_sym(&fieldname, idx);
            // Recurse for nested structs
            if field.type_spec.is_structure() || field.type_spec.is_structure_array() {
                let nested_arr = if arraylen != 0 || field_arr != 0 {
                    (arraylen.max(1)) * (field_arr.max(1))
                } else {
                    0
                };
                self.add_struct_fields(&fieldname, field.type_spec, symtype, nested_arr);
            }
        }
    }

    /// Emit per-field assigns for whole-struct assignment (C++ codegen_assign_struct).
    fn emit_struct_assign(&mut self, dst_name: &str, src_name: &str, ts: TypeSpec) {
        let struct_id = ts.structure_id();
        if struct_id <= 0 {
            return;
        }
        let spec = match get_struct(struct_id as i32) {
            Some(s) => s,
            None => return,
        };
        for field in &spec.fields {
            let dst_field = format!("{}.{}", dst_name, field.name.as_str());
            let src_field = format!("{}.{}", src_name, field.name.as_str());
            if field.type_spec.is_structure() || field.type_spec.is_structure_array() {
                // Nested struct: recurse
                self.emit_struct_assign(&dst_field, &src_field, field.type_spec);
            } else {
                let dst_idx = self.find_sym_by_name(&dst_field).unwrap_or(0) as i32;
                let src_idx = self.find_sym_by_name(&src_field).unwrap_or(0) as i32;
                let opname = if field.type_spec.is_array() {
                    "arraycopy"
                } else {
                    "assign"
                };
                self.emit(opname, &[dst_idx, src_idx]);
            }
        }
    }

    /// Init struct fields from a CompoundInitializer element list.
    /// Mirrors C++ codegen_struct_initializers: for each struct field takes
    /// the corresponding init element and emits assign to the field sub-symbol.
    fn emit_struct_init_compound(
        &mut self,
        basename: &str,
        ts: TypeSpec,
        elements: &[Box<ASTNode>],
    ) {
        let struct_id = ts.structure_id();
        if struct_id <= 0 {
            return;
        }
        let spec = match get_struct(struct_id as i32) {
            Some(s) => s,
            None => return,
        };
        let mut elem_idx = 0usize;
        for field in &spec.fields {
            if elem_idx >= elements.len() {
                break;
            }
            let fieldname = format!("{}.{}", basename, field.name.as_str());
            let elem = &elements[elem_idx];
            elem_idx += 1;

            if field.type_spec.is_structure() || field.type_spec.is_structure_array() {
                // Nested struct: element may itself be a CompoundInitializer
                if let ASTNodeKind::CompoundInitializer {
                    elements: sub_elems,
                    canconstruct,
                } = &elem.kind
                {
                    if !canconstruct {
                        let sub: Vec<Box<ASTNode>> = sub_elems.iter().map(|e| e.clone()).collect();
                        self.emit_struct_init_compound(&fieldname, field.type_spec, &sub);
                        continue;
                    }
                }
                // Otherwise gen the elem and do full struct-assign
                let src = self.gen_node(elem);
                if src >= 0 {
                    let src_name = self.ir.symbols[src as usize].name.as_str().to_string();
                    self.emit_struct_assign(&fieldname, &src_name, field.type_spec);
                }
            } else {
                // Scalar/triple/matrix field: gen init element, assign to field.
                // If the element is a CompoundInitializer without typespec (no typecheck),
                // propagate the field's TypeSpec so construct emits the right type.
                let field_idx = self.find_sym_by_name(&fieldname).unwrap_or(0) as i32;
                let val = if elem.typespec.is_unknown() {
                    if let ASTNodeKind::CompoundInitializer { .. } = &elem.kind {
                        let mut elem_clone = elem.as_ref().clone();
                        elem_clone.typespec = field.type_spec;
                        self.gen_node(&elem_clone)
                    } else {
                        self.gen_node(elem)
                    }
                } else {
                    self.gen_node(elem)
                };
                if val >= 0 && field_idx >= 0 {
                    let opname = if field.type_spec.is_array() {
                        "arraycopy"
                    } else {
                        "assign"
                    };
                    self.emit(opname, &[field_idx, val]);
                }
            }
        }
    }

    /// Emit initialization ops for a variable declaration.
    /// Dispatches to per-field struct init or scalar assign as appropriate.
    fn emit_var_init(&mut self, sym_idx: usize, typespec: TypeSpec, init_expr: &ASTNode) {
        if typespec.is_structure_based() {
            if let ASTNodeKind::CompoundInitializer {
                elements,
                canconstruct,
            } = &init_expr.kind
            {
                if !canconstruct {
                    // Struct compound init: assign per-field (C++ codegen_struct_initializers)
                    let dst_name = self.ir.symbols[sym_idx].name.as_str().to_string();
                    let elems: Vec<Box<ASTNode>> = elements.iter().map(|e| e.clone()).collect();
                    self.emit_struct_init_compound(&dst_name, typespec, &elems);
                    return;
                }
            }
            // Whole-struct assignment (VariableRef or canconstruct type constructor)
            let src = self.gen_node(init_expr);
            if src >= 0 {
                let dst_n = self.ir.symbols[sym_idx].name.as_str().to_string();
                let src_n = self.ir.symbols[src as usize].name.as_str().to_string();
                self.emit_struct_assign(&dst_n, &src_n, typespec);
            }
        } else {
            // If this is a CompoundInitializer whose typespec was not set by typecheck
            // (no-typecheck path), propagate the declared typespec so array/triple
            // construction emits the right opcodes (aassign vs construct).
            let src = if init_expr.typespec.is_unknown() {
                if let ASTNodeKind::CompoundInitializer { .. } = &init_expr.kind {
                    let mut patched = init_expr.clone();
                    patched.typespec = typespec;
                    self.gen_node(&patched)
                } else {
                    self.gen_node(init_expr)
                }
            } else {
                self.gen_node(init_expr)
            };
            if src >= 0 {
                self.emit("assign", &[sym_idx as i32, src]);
            }
        }
    }

    /// Allocate a new temporary symbol and return its index.
    fn alloc_temp(&mut self, typespec: TypeSpec) -> i32 {
        let idx = self.ir.symbols.len() as i32;
        let name = format!("$tmp{}", self.temp_counter);
        self.temp_counter += 1;
        self.ir
            .symbols
            .push(self.make_sym(&name, typespec, SymType::Temp));
        // Expand struct temps into per-field sub-symbols
        if typespec.is_structure() || typespec.is_structure_array() {
            let arrlen = typespec.simpletype().arraylen;
            self.add_struct_fields(&name, typespec, SymType::Temp, arrlen);
        }
        idx
    }

    /// Add a constant integer and return its symbol index.
    fn const_int(&mut self, val: i32) -> i32 {
        // Check if we already have this constant
        for &(idx, ref cv) in &self.ir.const_values {
            if let ConstValue::Int(v) = cv {
                if *v == val {
                    return idx as i32;
                }
            }
        }
        let idx = self.ir.symbols.len();
        let name = format!("$const_i{val}");
        let mut sym = self.make_sym(&name, TypeSpec::from_simple(TypeDesc::INT), SymType::Const);
        sym.initializers = 1;
        self.ir.symbols.push(sym);
        self.ir.const_values.push((idx, ConstValue::Int(val)));
        idx as i32
    }

    /// Add a constant float and return its symbol index.
    fn const_float(&mut self, val: f32) -> i32 {
        // Check if we already have this constant (exact bit match)
        for &(idx, ref cv) in &self.ir.const_values {
            if let ConstValue::Float(v) = cv {
                if v.to_bits() == val.to_bits() {
                    return idx as i32;
                }
            }
        }
        let idx = self.ir.symbols.len();
        let name = format!("$const_f{val}");
        let mut sym = self.make_sym(
            &name,
            TypeSpec::from_simple(TypeDesc::FLOAT),
            SymType::Const,
        );
        sym.initializers = 1;
        self.ir.symbols.push(sym);
        self.ir.const_values.push((idx, ConstValue::Float(val)));
        idx as i32
    }

    /// Add a constant Vec3 and return its symbol index.
    fn const_vec3(&mut self, v: crate::math::Vec3, ts: TypeSpec) -> i32 {
        for &(idx, ref cv) in &self.ir.const_values {
            if let ConstValue::Vec3(existing) = cv {
                if (existing.x - v.x).abs() < f32::EPSILON
                    && (existing.y - v.y).abs() < f32::EPSILON
                    && (existing.z - v.z).abs() < f32::EPSILON
                {
                    return idx as i32;
                }
            }
        }
        let idx = self.ir.symbols.len();
        let name = format!("$const_v{}_{}_{}", v.x, v.y, v.z);
        let mut sym = self.make_sym(&name, ts, SymType::Const);
        sym.initializers = 1;
        self.ir.symbols.push(sym);
        self.ir.const_values.push((idx, ConstValue::Vec3(v)));
        idx as i32
    }

    /// Add a constant string and return its symbol index.
    fn const_string(&mut self, s: &str) -> i32 {
        let us = UString::new(s);
        for &(idx, ref cv) in &self.ir.const_values {
            if let ConstValue::String(v) = cv {
                if *v == us {
                    return idx as i32;
                }
            }
        }
        let idx = self.ir.symbols.len();
        let mut sym = self.make_sym(s, TypeSpec::from_simple(TypeDesc::STRING), SymType::Const);
        sym.initializers = 1;
        self.ir.symbols.push(sym);
        self.ir.const_values.push((idx, ConstValue::String(us)));
        idx as i32
    }

    /// Try to extract a constant value from a param's init expression.
    /// Returns `Some(ConstValue)` for literals and simple compound initializers.
    fn const_value_for_param(&self, node: &ASTNode, _ts: TypeSpec) -> Option<ConstValue> {
        match &node.kind {
            ASTNodeKind::Literal { value } => match value {
                LiteralValue::Int(v) => Some(ConstValue::Int(*v)),
                LiteralValue::Float(v) => Some(ConstValue::Float(*v)),
                LiteralValue::String(s) => Some(ConstValue::String(UString::new(s))),
            },
            ASTNodeKind::CompoundInitializer { elements, .. } => {
                // Try to extract all-int or all-float arrays
                let mut ints = Vec::new();
                let mut floats = Vec::new();
                let mut all_int = true;
                let mut all_float = true;
                for e in elements.iter() {
                    if let ASTNodeKind::Literal { value } = &e.kind {
                        match value {
                            LiteralValue::Int(v) => {
                                ints.push(*v);
                                floats.push(*v as f32);
                            }
                            LiteralValue::Float(v) => {
                                all_int = false;
                                floats.push(*v);
                                ints.push(*v as i32);
                            }
                            _ => {
                                all_int = false;
                                all_float = false;
                            }
                        }
                    } else {
                        return None; // Non-literal element
                    }
                }
                if floats.len() == 3 && all_float {
                    Some(ConstValue::Vec3(crate::math::Vec3::new(
                        floats[0], floats[1], floats[2],
                    )))
                } else if all_int && !ints.is_empty() {
                    Some(ConstValue::IntArray(ints))
                } else if !floats.is_empty() {
                    Some(ConstValue::FloatArray(floats))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Emit an opcode.
    fn emit(&mut self, opname: &str, args: &[i32]) {
        let firstarg = self.ir.args.len() as i32;
        let nargs = args.len() as i32;
        self.ir.args.extend_from_slice(args);

        let mut op = Opcode::new(
            UString::new(opname),
            UString::new(&self.current_method),
            firstarg,
            nargs,
        );
        op.sourcefile = self.current_sourcefile;
        op.sourceline = self.current_line;
        self.ir.opcodes.push(op);
    }

    /// Emit an opcode with jump targets.
    fn emit_with_jumps(&mut self, opname: &str, args: &[i32], jumps: [i32; 4]) {
        let firstarg = self.ir.args.len() as i32;
        let nargs = args.len() as i32;
        self.ir.args.extend_from_slice(args);

        let mut op = Opcode::new(
            UString::new(opname),
            UString::new(&self.current_method),
            firstarg,
            nargs,
        );
        op.jump = jumps;
        op.sourcefile = self.current_sourcefile;
        op.sourceline = self.current_line;
        self.ir.opcodes.push(op);
    }

    /// Emit a useparam opcode for a parameter (needed at runtime to trigger
    /// parameter initialization). Matches C++ `emitcode("useparam", sym)`.
    fn emit_useparam(&mut self, sym_idx: i32) {
        self.emit("useparam", &[sym_idx]);
    }

    /// Emit type coercion if needed: convert `src_idx` from `src_type` to `dst_type`.
    /// Returns the symbol index of the (possibly new) value that has `dst_type`.
    /// Matches C++ `ASTNode::coerce()`.
    fn coerce(&mut self, src_idx: i32, src_type: TypeSpec, dst_type: TypeSpec) -> i32 {
        use crate::typedesc::{Aggregate, BaseType};
        // If equivalent, no coercion needed
        if src_type == dst_type {
            return src_idx;
        }
        let sd = src_type.simpletype();
        let dd = dst_type.simpletype();
        // Same basetype + aggregate = equivalent (e.g. point→vector)
        if sd.basetype == dd.basetype && sd.aggregate == dd.aggregate {
            return src_idx; // just reinterpret
        }

        // int → float: emit explicit float() cast
        if sd.basetype == BaseType::Int32 as u8
            && sd.aggregate == Aggregate::Scalar as u8
            && dd.basetype == BaseType::Float as u8
            && dd.aggregate == Aggregate::Scalar as u8
        {
            let tmp = self.alloc_temp(dst_type);
            self.emit("float", &[tmp, src_idx]);
            return tmp;
        }

        // float/int → triple: emit construct(result, val, val, val)
        if dd.basetype == BaseType::Float as u8
            && dd.aggregate == Aggregate::Vec3 as u8
            && sd.aggregate == Aggregate::Scalar as u8
        {
            // First coerce to float if int
            let float_idx = if sd.basetype == BaseType::Int32 as u8 {
                let ftmp = self.alloc_temp(TypeSpec::from_simple(TypeDesc::FLOAT));
                self.emit("float", &[ftmp, src_idx]);
                ftmp
            } else {
                src_idx
            };
            let tmp = self.alloc_temp(dst_type);
            self.emit("construct", &[tmp, float_idx, float_idx, float_idx]);
            return tmp;
        }

        // float/int → matrix: emit matrix(result, val) — diagonal
        if dd.basetype == BaseType::Float as u8
            && dd.aggregate == Aggregate::Matrix44 as u8
            && sd.aggregate == Aggregate::Scalar as u8
        {
            let float_idx = if sd.basetype == BaseType::Int32 as u8 {
                let ftmp = self.alloc_temp(TypeSpec::from_simple(TypeDesc::FLOAT));
                self.emit("float", &[ftmp, src_idx]);
                ftmp
            } else {
                src_idx
            };
            let tmp = self.alloc_temp(dst_type);
            self.emit("matrix", &[tmp, float_idx]);
            return tmp;
        }

        // Generic fallback: emit assign (runtime handles remaining conversions)
        let tmp = self.alloc_temp(dst_type);
        self.emit("assign", &[tmp, src_idx]);
        tmp
    }

    /// Current opcode index (for jump targets).
    fn current_op_index(&self) -> i32 {
        self.ir.opcodes.len() as i32
    }

    /// Generate code for a condition expression, ensuring the result is int.
    /// C++ codegen_int(): if the condition isn't int, emit `neq(tmp, src, 0)`.
    fn gen_cond(&mut self, node: &ASTNode) -> i32 {
        let src = self.gen_node(node);
        if src < 0 {
            return src;
        }
        let ts = self.ir.symbols[src as usize].typespec;
        if ts.is_int() {
            return src;
        }
        // Convert non-int to int via neq with zero
        let tmp = self.alloc_temp(TypeSpec::from_simple(TypeDesc::INT));
        if ts.is_float() {
            let zero = self.const_float(0.0);
            self.emit("neq", &[tmp, src, zero]);
        } else {
            let zero = self.const_int(0);
            self.emit("neq", &[tmp, src, zero]);
        }
        tmp
    }

    /// Generate code for a node, returning the symbol index of the result.
    fn gen_node(&mut self, node: &ASTNode) -> i32 {
        // Track source location for emitted opcodes.
        self.current_line = node.loc.line as i32;
        match &node.kind {
            ASTNodeKind::ShaderDeclaration {
                shader_type,
                name,
                formals,
                statements,
                ..
            } => {
                self.ir.shader_type = match shader_type.as_str() {
                    "surface" => ShaderType::Surface,
                    "displacement" => ShaderType::Displacement,
                    "volume" => ShaderType::Volume,
                    "light" => ShaderType::Light,
                    _ => ShaderType::Generic,
                };
                self.ir.shader_name = name.clone();
                self.current_sourcefile = UString::new(&format!("{name}.osl"));
                self.current_method = "___main___".to_string();

                for formal in formals {
                    self.gen_node(formal);
                }
                for stmt in statements {
                    self.gen_node(stmt);
                }
                // Emit end marker
                self.emit("end", &[]);
                -1
            }

            ASTNodeKind::FunctionDeclaration {
                name,
                return_type,
                formals,
                statements,
                ..
            } => {
                // Emit a nop that jumps past the function body (so linear execution skips it)
                let skip_nop_idx = self.current_op_index() as usize;
                self.emit_with_jumps("nop", &[], [-1, -1, -1, -1]);

                let body_start = self.current_op_index();
                self.scope_depth += 1;

                // Emit formal parameters and record their symbol indices + output flags
                let mut formal_indices = Vec::new();
                let mut formal_is_output = Vec::new();
                for formal in formals {
                    let is_out = matches!(&formal.kind, ASTNodeKind::VariableDeclaration { is_output, .. } if *is_output);
                    let idx = self.gen_node(formal);
                    formal_indices.push(idx);
                    formal_is_output.push(is_out);
                }

                // Create a return value symbol if non-void
                let return_sym = if !return_type.is_void() {
                    let ret_name = format!("$ret_{}", name);
                    let idx = self.ir.symbols.len() as i32;
                    self.ir
                        .symbols
                        .push(self.make_sym(&ret_name, *return_type, SymType::Local));
                    idx
                } else {
                    -1
                };

                let prev_return_sym = self.current_return_sym;
                self.current_return_sym = return_sym;

                for stmt in statements {
                    self.gen_node(stmt);
                }

                self.current_return_sym = prev_return_sym;

                // Emit return at end of function body
                self.emit("return", &[]);

                self.cleanup_scope(self.scope_depth);
                self.scope_depth -= 1;

                // Patch the skip nop to jump past the function body
                let after_body = self.current_op_index();
                self.ir.opcodes[skip_nop_idx].jump[0] = after_body;

                // Collect formal types for overload resolution
                let formal_types: Vec<TypeSpec> = formal_indices
                    .iter()
                    .map(|&idx| {
                        if idx >= 0 {
                            self.ir.symbols[idx as usize].typespec
                        } else {
                            TypeSpec::default()
                        }
                    })
                    .collect();

                // Record this user function
                let uf = UserFunc {
                    body_start,
                    formals: formal_indices,
                    formal_types,
                    formal_is_output,
                    return_sym,
                    return_type: *return_type,
                };
                self.user_funcs.entry(name.clone()).or_default().push(uf);

                -1
            }

            ASTNodeKind::VariableDeclaration {
                name,
                typespec,
                init,
                is_param,
                is_output,
                metadata: ast_metadata,
                ..
            } => {
                let idx = self.ir.symbols.len() as i32;
                let symtype = if *is_param {
                    if *is_output {
                        SymType::OutputParam
                    } else {
                        SymType::Param
                    }
                } else {
                    SymType::Local
                };

                let mut sym = self.make_sym(name, *typespec, symtype);
                sym.initializers = if init.is_some() { 1 } else { 0 };

                // Apply AST metadata [[ type name = value ]] to symbol
                for m in ast_metadata {
                    if let ASTNodeKind::VariableDeclaration {
                        typespec: m_ts,
                        name: m_name,
                        init: m_init,
                        is_metadata: true,
                        ..
                    } = &m.kind
                    {
                        let type_str = metadata_type_name(m_ts);
                        let val_str = metadata_value_str(m_init);
                        if m_name == "lockgeom" && type_str == "int" {
                            sym.is_lockgeom = val_str.parse::<i32>().map_or(true, |v| v != 0);
                        }
                        sym.metadata.push((type_str, m_name.clone(), val_str));
                    }
                }

                self.ir.symbols.push(sym);
                self.register_sym(name, idx as usize);

                // Expand struct fields into individual sub-symbols
                if typespec.is_structure() || typespec.is_structure_array() {
                    let arrlen = typespec.simpletype().arraylen;
                    self.add_struct_fields(name, *typespec, symtype, arrlen);
                }

                // Array storage is allocated by the symbol table entry itself;
                // no init_array opcode needed (not a standard OSO opcode).

                if let Some(init_expr) = init {
                    if *is_param {
                        // Try extracting constant default for the param
                        if let Some(cv) = self.const_value_for_param(init_expr, *typespec) {
                            self.ir.param_defaults.push((idx as usize, cv));
                        }
                        // Non-constant init: emit in __init method block
                        let is_const_init = matches!(
                            &init_expr.kind,
                            ASTNodeKind::Literal { .. } | ASTNodeKind::CompoundInitializer { .. }
                        );
                        if !is_const_init {
                            let prev_method = self.current_method.clone();
                            self.current_method = "___init___".to_string();
                            let init_begin = self.current_op_index();
                            self.emit_var_init(idx as usize, *typespec, init_expr);
                            let init_end = self.current_op_index();
                            self.ir.symbols[idx as usize].initbegin = init_begin;
                            self.ir.symbols[idx as usize].initend = init_end;
                            self.current_method = prev_method;
                        } else {
                            self.emit_var_init(idx as usize, *typespec, init_expr);
                        }
                    } else {
                        self.emit_var_init(idx as usize, *typespec, init_expr);
                    }
                }

                idx
            }

            ASTNodeKind::CompoundStatement { statements } => {
                self.scope_depth += 1;
                let inner_scope = self.scope_depth;
                for stmt in statements {
                    self.gen_node(stmt);
                }
                self.cleanup_scope(inner_scope);
                self.scope_depth -= 1;
                -1
            }

            // Flat statement list (no new scope) - e.g. multi-var declarations
            ASTNodeKind::StatementList { statements } => {
                for stmt in statements {
                    self.gen_node(stmt);
                }
                -1
            }

            ASTNodeKind::Literal { value } => match value {
                LiteralValue::Int(v) => self.const_int(*v),
                LiteralValue::Float(v) => self.const_float(*v),
                LiteralValue::String(s) => self.const_string(s),
            },

            ASTNodeKind::VariableRef { name } => {
                // O(1) lookup via sym_lookup HashMap, pick the innermost scope
                if let Some(entries) = self.sym_lookup.get(name.as_str()) {
                    // Find the entry with the highest scope <= current scope
                    let mut best: Option<usize> = None;
                    let mut best_scope = -1i32;
                    for &(scope, idx) in entries {
                        if scope <= self.scope_depth && scope >= best_scope {
                            best_scope = scope;
                            best = Some(idx);
                        }
                    }
                    if let Some(i) = best {
                        let sym = &self.ir.symbols[i];
                        if sym.symtype == SymType::Param || sym.symtype == SymType::OutputParam {
                            self.emit_useparam(i as i32);
                        }
                        return i as i32;
                    }
                }
                // Fallback: linear scan for symbols not in lookup (e.g., temps)
                for (i, sym) in self.ir.symbols.iter().enumerate().rev() {
                    if sym.name.as_str() == name {
                        if sym.symtype == SymType::Param || sym.symtype == SymType::OutputParam {
                            self.emit_useparam(i as i32);
                        }
                        return i as i32;
                    }
                }
                -1 // Not found
            }

            ASTNodeKind::BinaryExpression { op, left, right } => {
                // Short-circuit &&: if left is false, skip right, result = 0
                if *op == Operator::LogAnd {
                    let l = self.gen_node(left);
                    let result = self.alloc_temp(TypeSpec::from_simple(TypeDesc::INT));
                    let zero = self.const_int(0);
                    self.emit("assign", &[result, zero]);
                    let if_idx = self.current_op_index();
                    self.emit_with_jumps("if", &[l], [-1, -1, -1, -1]);
                    let true_start = self.current_op_index();
                    let r = self.gen_node(right);
                    // Normalize to bool: result = (r != 0)
                    self.emit("neq", &[result, r, zero]);
                    let end = self.current_op_index();
                    self.ir.opcodes[if_idx as usize].jump[0] = true_start;
                    self.ir.opcodes[if_idx as usize].jump[1] = end;
                    return result;
                }
                // Short-circuit ||: if left is true, skip right, result = 1
                if *op == Operator::LogOr {
                    let l = self.gen_node(left);
                    let result = self.alloc_temp(TypeSpec::from_simple(TypeDesc::INT));
                    let one = self.const_int(1);
                    self.emit("assign", &[result, one]);
                    let if_idx = self.current_op_index();
                    self.emit_with_jumps("if", &[l], [-1, -1, -1, -1]);
                    let true_start = self.current_op_index();
                    // True: left was true, skip to end (result stays 1)
                    let jump_end_idx = self.current_op_index();
                    self.emit_with_jumps("nop", &[], [-1, -1, -1, -1]);
                    let false_start = self.current_op_index();
                    // False: left was false, evaluate right
                    let r = self.gen_node(right);
                    // Normalize to bool: result = (r != 0)
                    let zero = self.const_int(0);
                    self.emit("neq", &[result, r, zero]);
                    let end = self.current_op_index();
                    self.ir.opcodes[if_idx as usize].jump[0] = true_start;
                    self.ir.opcodes[if_idx as usize].jump[1] = false_start;
                    self.ir.opcodes[jump_end_idx as usize].jump[0] = end;
                    return result;
                }

                let l = self.gen_node(left);
                let r = self.gen_node(right);
                let result = self.alloc_temp(node.typespec);

                let opname = match op {
                    Operator::Add => "add",
                    Operator::Sub => "sub",
                    Operator::Mul => "mul",
                    Operator::Div => "div",
                    Operator::Mod => "mod",
                    Operator::Eq => "eq",
                    Operator::NotEq => "neq",
                    Operator::Less => "lt",
                    Operator::Greater => "gt",
                    Operator::LessEq => "le",
                    Operator::GreaterEq => "ge",
                    Operator::BitAnd => "bitand",
                    Operator::BitOr => "bitor",
                    Operator::BitXor => "xor",
                    Operator::Shl => "shl",
                    Operator::Shr => "shr",
                    _ => "nop",
                };
                self.emit(opname, &[result, l, r]);
                result
            }

            ASTNodeKind::UnaryExpression { op, expr } => {
                let src = self.gen_node(expr);
                let result = self.alloc_temp(node.typespec);
                let opname = match op {
                    Operator::Neg => "neg",
                    Operator::Not => "not",
                    Operator::BitNot => "compl",
                    _ => "nop",
                };
                self.emit(opname, &[result, src]);
                result
            }

            ASTNodeKind::AssignExpression { op, lvalue, expr } => {
                // Special case: assignment to indexed expression (e.g., C[1] = 8)
                if matches!(op, Operator::Assign) {
                    if let ASTNodeKind::Index {
                        base,
                        index,
                        index2,
                        ..
                    } = &lvalue.kind
                    {
                        // Check for nested double-index: arr[i][j] = val
                        // Parser produces: Index { base: Index { base: arr, index: i }, index: j }
                        if let ASTNodeKind::Index {
                            base: arr_base,
                            index: arr_idx,
                            ..
                        } = &base.kind
                        {
                            if arr_base.typespec.is_array() {
                                let arr_td = arr_base.typespec.simpletype();
                                let is_triple_arr =
                                    arr_td.aggregate == crate::typedesc::Aggregate::Vec3 as u8;
                                let is_matrix_arr =
                                    arr_td.aggregate == crate::typedesc::Aggregate::Matrix44 as u8;
                                let arr_idx_sym = self.gen_node(arr_idx);
                                let comp_idx_sym = self.gen_node(index);
                                let src = self.gen_node(expr);
                                let arr_base_idx = self.gen_node(arr_base);
                                if is_triple_arr {
                                    // arr[i][j] = val for triple arrays
                                    let elem_ts = TypeSpec::new(crate::typedesc::TypeDesc::COLOR);
                                    let tmp = self.alloc_temp(elem_ts);
                                    self.emit("aref", &[tmp, arr_base_idx, arr_idx_sym]);
                                    self.emit_compassign(tmp, comp_idx_sym, src);
                                    self.emit("aassign", &[arr_base_idx, arr_idx_sym, tmp]);
                                } else if is_matrix_arr {
                                    // arr[i][j] for matrix arrays — need third index for mat[r][c]
                                    // For now, treat as mxcompref row access
                                    let elem_ts = TypeSpec::new(crate::typedesc::TypeDesc::MATRIX);
                                    let tmp = self.alloc_temp(elem_ts);
                                    self.emit("aref", &[tmp, arr_base_idx, arr_idx_sym]);
                                    // This sets a row? Actually mat[i][j] for matrix arrays would be mat[i] as row, [j] as col
                                    // For now skip complex matrix array indexing
                                    let zero = self.const_int(0);
                                    self.emit("mxcompassign", &[tmp, comp_idx_sym, zero, src]);
                                    self.emit("aassign", &[arr_base_idx, arr_idx_sym, tmp]);
                                } else {
                                    // Fallback: just aassign
                                    let base_idx = self.gen_node(base);
                                    self.emit("aassign", &[base_idx, comp_idx_sym, src]);
                                }
                                return arr_base_idx;
                            } else if arr_base.typespec.simpletype().is_matrix44() {
                                // M[row][col] = val → mxcompassign(M, row, col, val)
                                let mx_idx = self.gen_node(arr_base);
                                let row = self.gen_node(arr_idx);
                                let col = self.gen_node(index);
                                let src = self.gen_node(expr);
                                self.emit_mxcompassign(mx_idx, row, col, src);
                                return mx_idx;
                            }
                        }

                        let base_idx = self.gen_node(base);
                        let idx1 = self.gen_node(index);
                        let src = self.gen_node(expr);
                        // Use actual symbol typespec: base.typespec may be UNKNOWN
                        // when typecheck wasn't run (no-typecheck path).
                        let base_actual_ts =
                            if base_idx >= 0 && (base_idx as usize) < self.ir.symbols.len() {
                                self.ir.symbols[base_idx as usize].typespec
                            } else {
                                base.typespec
                            };
                        if let Some(idx2_node) = index2 {
                            let idx2 = self.gen_node(idx2_node);
                            self.emit_mxcompassign(base_idx, idx1, idx2, src);
                        } else if base_actual_ts.is_array() {
                            self.emit("aassign", &[base_idx, idx1, src]);
                        } else {
                            self.emit_compassign(base_idx, idx1, src);
                        }
                        return base_idx;
                    }
                    // Assignment to member access (e.g., R.x = val)
                    if let ASTNodeKind::StructSelect { base, field } = &lvalue.kind {
                        let base_ts = base.typespec.simpletype();
                        if base_ts.is_triple()
                            || base_ts.aggregate == crate::typedesc::Aggregate::Vec3 as u8
                        {
                            // Triple member assignment: R.x = val → compassign(R, 0, val)
                            let comp_idx = match field.as_str() {
                                "x" | "r" | "s" => 0,
                                "y" | "g" | "t" => 1,
                                "z" | "b" => 2,
                                _ => 0,
                            };
                            // Array element component: arr[i].x = val
                            // aref tmp,arr,i; compassign tmp,comp,val; aassign arr,i,tmp
                            if let ASTNodeKind::Index {
                                base: arr_base,
                                index: arr_idx,
                                ..
                            } = &base.kind
                            {
                                if arr_base.typespec.is_array() {
                                    let arr_base_idx = self.gen_node(arr_base);
                                    let arr_idx_sym = self.gen_node(arr_idx);
                                    let src = self.gen_node(expr);
                                    let elem_ts = TypeSpec::new(base_ts);
                                    let tmp = self.alloc_temp(elem_ts);
                                    self.emit("aref", &[tmp, arr_base_idx, arr_idx_sym]);
                                    let idx_sym = self.const_int(comp_idx);
                                    self.emit_compassign(tmp, idx_sym, src);
                                    self.emit("aassign", &[arr_base_idx, arr_idx_sym, tmp]);
                                    return arr_base_idx;
                                }
                            }
                            let base_idx = self.gen_node(base);
                            let src = self.gen_node(expr);
                            let idx_sym = self.const_int(comp_idx);
                            self.emit_compassign(base_idx, idx_sym, src);
                            return base_idx;
                        } else if base.typespec.is_structure_based() {
                            // Struct field assignment via sub-symbol
                            let base_idx = self.gen_node(base);
                            let base_name =
                                self.ir.symbols[base_idx as usize].name.as_str().to_string();
                            let field_sym_name = format!("{}.{}", base_name, field);
                            let src = self.gen_node(expr);
                            if let Some(field_idx) = self.find_sym_by_name(&field_sym_name) {
                                let field_ts = self.ir.symbols[field_idx].typespec;
                                if field_ts.is_structure_based() {
                                    // Assigning a struct to a struct field: per-field assign
                                    let src_name =
                                        self.ir.symbols[src as usize].name.as_str().to_string();
                                    self.emit_struct_assign(&field_sym_name, &src_name, field_ts);
                                } else {
                                    let opname = if field_ts.is_array() {
                                        "arraycopy"
                                    } else {
                                        "assign"
                                    };
                                    self.emit(opname, &[field_idx as i32, src]);
                                }
                            } else {
                                // Fallback
                                let field_const = self.const_string(field);
                                self.emit("setfield", &[base_idx, field_const, src]);
                            }
                            return base_idx;
                        } else {
                            // Non-struct, non-triple field access: fallback
                            let base_idx = self.gen_node(base);
                            let src = self.gen_node(expr);
                            let field_const = self.const_string(field);
                            self.emit("setfield", &[base_idx, field_const, src]);
                            return base_idx;
                        }
                    }
                }

                let dst = self.gen_node(lvalue);
                let src = self.gen_node(expr);
                match op {
                    Operator::Assign => {
                        if lvalue.typespec.is_structure_based() {
                            // Whole struct assign: per-field copy
                            let dst_name = self.ir.symbols[dst as usize].name.as_str().to_string();
                            let src_name = self.ir.symbols[src as usize].name.as_str().to_string();
                            self.emit_struct_assign(&dst_name, &src_name, lvalue.typespec);
                        } else {
                            // C++ codegen.cpp:534: use "arraycopy" for array-to-array assign
                            let opname = if lvalue.typespec.is_array() {
                                "arraycopy"
                            } else {
                                "assign"
                            };
                            self.emit(opname, &[dst, src]);
                        }
                    }
                    Operator::AddAssign => self.emit("add", &[dst, dst, src]),
                    Operator::SubAssign => self.emit("sub", &[dst, dst, src]),
                    Operator::MulAssign => self.emit("mul", &[dst, dst, src]),
                    Operator::DivAssign => self.emit("div", &[dst, dst, src]),
                    Operator::BitAndAssign => self.emit("bitand", &[dst, dst, src]),
                    Operator::BitOrAssign => self.emit("bitor", &[dst, dst, src]),
                    Operator::BitXorAssign => self.emit("xor", &[dst, dst, src]),
                    Operator::ShlAssign => self.emit("shl", &[dst, dst, src]),
                    Operator::ShrAssign => self.emit("shr", &[dst, dst, src]),
                    _ => {}
                }
                dst
            }

            ASTNodeKind::FunctionCall { name, args, .. } => {
                // Check if this is a user-defined function
                if let Some(overloads) = self.user_funcs.get(name).cloned() {
                    // Collect actual argument types for overload resolution
                    let call_arg_types: Vec<TypeSpec> = args.iter().map(|a| a.typespec).collect();

                    // Score-based overload resolution (matches C++ CandidateFunctions)
                    // Scoring constants per C++ reference:
                    const EXACT: i32 = 100;
                    const INT_TO_FP: i32 = 77;
                    const ARRAY_MATCH: i32 = 44;
                    const SPATIAL_COERCE: i32 = 32; // vectriple <-> vectriple
                    const TRIPLE_COERCE: i32 = 27; // triple <-> triple (incl color)
                    const COERCABLE: i32 = 23; // float -> triple etc
                    // FP_TO_INT = 0 (not allowed)

                    let score_type = |formal: &TypeSpec, actual: &TypeSpec| -> i32 {
                        if formal.simpletype() == actual.simpletype() {
                            return EXACT;
                        }
                        // int <-> float
                        if !actual.is_closure()
                            && actual.is_scalarnum()
                            && !formal.is_closure()
                            && formal.is_scalarnum()
                        {
                            return if formal.simpletype().is_int() {
                                0
                            } else {
                                INT_TO_FP
                            };
                        }
                        // Sized array -> unsized array
                        if formal.simpletype().is_unsized_array()
                            && actual.simpletype().is_sized_array()
                            && formal.simpletype().elementtype()
                                == actual.simpletype().elementtype()
                        {
                            return ARRAY_MATCH;
                        }
                        // Check assignability
                        if formal.assignable_from(actual) {
                            // Spatial triple <-> spatial triple
                            if actual.is_vectriple_based() && formal.is_vectriple_based() {
                                return SPATIAL_COERCE;
                            }
                            // Triple <-> triple (color included)
                            if !actual.is_closure()
                                && actual.is_triple()
                                && !formal.is_closure()
                                && formal.is_triple()
                            {
                                return TRIPLE_COERCE;
                            }
                            return COERCABLE;
                        }
                        0 // no match
                    };

                    // Return type rank for tiebreaking (C++ precedence)
                    let ret_rank = |rt: &TypeSpec| -> i32 {
                        // Check struct/closure/void BEFORE simpletype checks
                        if rt.is_structure_based() {
                            return 9;
                        }
                        if rt.is_closure() {
                            return 8;
                        }
                        if rt.is_void() {
                            return 10;
                        }
                        let td = rt.simpletype();
                        if td == TypeDesc::FLOAT {
                            return 0;
                        }
                        if td == TypeDesc::INT {
                            return 1;
                        }
                        if td == TypeDesc::COLOR {
                            return 2;
                        }
                        if td == TypeDesc::VECTOR {
                            return 3;
                        }
                        if td == TypeDesc::POINT {
                            return 4;
                        }
                        if td == TypeDesc::NORMAL {
                            return 5;
                        }
                        if td == TypeDesc::MATRIX {
                            return 6;
                        }
                        if td == TypeDesc::STRING {
                            return 7;
                        }
                        11 // unknown
                    };

                    // Collect all candidates with their scores
                    let mut candidates: Vec<(usize, i32, i32)> = Vec::new(); // (idx, ascore, rscore)
                    let expected_ret = node.typespec;
                    for (idx, uf) in overloads.iter().enumerate() {
                        if uf.formals.len() != args.len() {
                            continue;
                        }
                        let mut total = 0i32;
                        let mut ok = true;
                        for (ft, at) in uf.formal_types.iter().zip(call_arg_types.iter()) {
                            let s = score_type(ft, at);
                            if s == 0 && ft.simpletype() != at.simpletype() {
                                ok = false;
                                break;
                            }
                            total += s;
                        }
                        if !ok {
                            continue;
                        }
                        let rscore = score_type(&expected_ret, &uf.return_type);
                        // Keep only candidates with highest arg score
                        if let Some(best) = candidates.first() {
                            if total < best.1 {
                                continue;
                            }
                            if total > best.1 {
                                candidates.clear();
                            }
                        }
                        candidates.push((idx, total, rscore));
                    }

                    let best_idx = if candidates.len() <= 1 {
                        candidates.first().map(|c| c.0).unwrap_or(0)
                    } else {
                        // C++ always uses return type rank for tiebreaker
                        // (the rscore path is disabled in reference: `if (true)`)
                        candidates
                            .iter()
                            .min_by_key(|c| ret_rank(&overloads[c.0].return_type))
                            .map(|c| c.0)
                            .unwrap_or(0)
                    };
                    let uf = overloads[best_idx].clone();

                    // Generate actual argument values and collect writeback info
                    // for output params that target array elements or vector components
                    let mut actual_indices = Vec::new();
                    // Writeback info: (base_idx, index_idx, kind) for output params
                    // kind: 0=simple assign, 1=aassign, 2=compassign
                    let mut writeback_info: Vec<(i32, i32, u8)> = Vec::new();
                    for (i, arg) in args.iter().enumerate() {
                        let is_output = i < uf.formal_is_output.len() && uf.formal_is_output[i];
                        let mut wb = (0i32, 0i32, 0u8);
                        if is_output {
                            // Check if arg is Index (array subscript or component access)
                            if let ASTNodeKind::Index { base, index, .. } = &arg.kind {
                                let base_idx = self.gen_node(base);
                                let idx_sym = self.gen_node(index);
                                if base.typespec.is_array() {
                                    wb = (base_idx, idx_sym, 1); // aassign
                                } else {
                                    wb = (base_idx, idx_sym, 2); // compassign
                                }
                            }
                            // Check if arg is StructSelect on triple (R.x)
                            if let ASTNodeKind::StructSelect { base, field } = &arg.kind {
                                let base_ts = base.typespec.simpletype();
                                if base_ts.is_triple()
                                    || base_ts.aggregate == crate::typedesc::Aggregate::Vec3 as u8
                                {
                                    let comp = match field.as_str() {
                                        "x" | "r" | "s" => 0,
                                        "y" | "g" | "t" => 1,
                                        "z" | "b" => 2,
                                        _ => 0,
                                    };
                                    let base_idx = self.gen_node(base);
                                    let idx_sym = self.const_int(comp);
                                    wb = (base_idx, idx_sym, 2); // compassign
                                }
                            }
                        }
                        writeback_info.push(wb);
                        let mut idx = self.gen_node(arg);
                        if i < uf.formal_types.len() {
                            let actual_type = arg.typespec;
                            let formal_type = uf.formal_types[i];
                            idx = self.coerce(idx, actual_type, formal_type);
                        }
                        actual_indices.push(idx);
                    }

                    // Assign actual args to formal params (C++ struct_pair_all_fields).
                    // For struct types: emit per-field assigns; scalars use plain assign.
                    for (fi, (formal_idx, actual_idx)) in
                        uf.formals.iter().zip(actual_indices.iter()).enumerate()
                    {
                        if *formal_idx >= 0 && *actual_idx >= 0 {
                            let ftype = if fi < uf.formal_types.len() {
                                uf.formal_types[fi]
                            } else {
                                TypeSpec::default()
                            };
                            if ftype.is_structure() || ftype.is_structure_array() {
                                // Per-field copy: formal.field <- actual.field
                                let fname = self.ir.symbols[*formal_idx as usize]
                                    .name
                                    .as_str()
                                    .to_string();
                                let aname = self.ir.symbols[*actual_idx as usize]
                                    .name
                                    .as_str()
                                    .to_string();
                                self.emit_struct_assign(&fname, &aname, ftype);
                            } else {
                                self.emit("assign", &[*formal_idx, *actual_idx]);
                            }
                        }
                    }

                    // Emit functioncall opcode: jump[0] = body_start
                    self.emit_with_jumps("functioncall", &[], [uf.body_start, -1, -1, -1]);

                    // Copy output formals back to actual args.
                    // For struct output params: emit per-field reverse copy.
                    for (i, (formal_idx, actual_idx)) in
                        uf.formals.iter().zip(actual_indices.iter()).enumerate()
                    {
                        if i < uf.formal_is_output.len() && uf.formal_is_output[i] {
                            if *formal_idx >= 0 && *actual_idx >= 0 {
                                let ftype = if i < uf.formal_types.len() {
                                    uf.formal_types[i]
                                } else {
                                    TypeSpec::default()
                                };
                                let wb = &writeback_info[i];
                                if ftype.is_structure() || ftype.is_structure_array() {
                                    // Struct output: per-field copy back actual.field <- formal.field
                                    let aname = self.ir.symbols[*actual_idx as usize]
                                        .name
                                        .as_str()
                                        .to_string();
                                    let fname = self.ir.symbols[*formal_idx as usize]
                                        .name
                                        .as_str()
                                        .to_string();
                                    self.emit_struct_assign(&aname, &fname, ftype);
                                } else {
                                    match wb.2 {
                                        1 => {
                                            // Array element: aassign(base, idx, formal)
                                            self.emit("assign", &[*actual_idx, *formal_idx]);
                                            self.emit("aassign", &[wb.0, wb.1, *actual_idx]);
                                        }
                                        2 => {
                                            // Component: compassign(base, idx, formal)
                                            self.emit("assign", &[*actual_idx, *formal_idx]);
                                            self.emit_compassign(wb.0, wb.1, *actual_idx);
                                        }
                                        _ => {
                                            // Simple assign back
                                            self.emit("assign", &[*actual_idx, *formal_idx]);
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Copy return value to a fresh temp so subsequent calls don't overwrite it
                    if uf.return_sym >= 0 {
                        let ret_copy = self.alloc_temp(uf.return_type);
                        self.emit("assign", &[ret_copy, uf.return_sym]);
                        ret_copy
                    } else {
                        -1
                    }
                } else if {
                    // Struct type constructor: MyStruct(field1, field2, ...)
                    // Either typecheck resolved it to a struct TypeSpec, or we detect it
                    // directly by looking up the name in the struct registry.
                    let by_typespec = node.typespec.is_structure_based();
                    let sid_by_name = if !by_typespec {
                        crate::typespec::find_struct_by_name(crate::ustring::UString::new(name))
                    } else {
                        0
                    };
                    by_typespec || sid_by_name > 0
                } {
                    // Resolve the struct TypeSpec: either from typecheck or by name lookup.
                    let struct_ts = if node.typespec.is_structure_based() {
                        node.typespec
                    } else {
                        let sid = crate::typespec::find_struct_by_name(
                            crate::ustring::UString::new(name),
                        );
                        crate::typespec::TypeSpec::structure(sid as i16, 0)
                    };
                    // Allocate temp struct symbol (also creates sub-symbols via add_struct_fields)
                    let result = self.alloc_temp(struct_ts);
                    let tmp_name = self.ir.symbols[result as usize].name.as_str().to_string();
                    // Assign each argument directly to the corresponding field sub-symbol.
                    // This mirrors emit_struct_init_compound: field[i] <- args[i].
                    let struct_id = struct_ts.structure_id();
                    if let Some(spec) = get_struct(struct_id as i32) {
                        let fields: Vec<_> = spec.fields.clone();
                        let args_cloned: Vec<Box<ASTNode>> =
                            args.iter().map(|a| a.clone()).collect();
                        for (fi, field) in fields.iter().enumerate() {
                            if fi >= args_cloned.len() {
                                break;
                            }
                            let fieldname = format!("{}.{}", tmp_name, field.name.as_str());
                            let val = self.gen_node(&args_cloned[fi]);
                            if val >= 0 {
                                if field.type_spec.is_structure()
                                    || field.type_spec.is_structure_array()
                                {
                                    let src_name =
                                        self.ir.symbols[val as usize].name.as_str().to_string();
                                    self.emit_struct_assign(&fieldname, &src_name, field.type_spec);
                                } else {
                                    let field_idx =
                                        self.find_sym_by_name(&fieldname).unwrap_or(0) as i32;
                                    let opname = if field.type_spec.is_array() {
                                        "arraycopy"
                                    } else {
                                        "assign"
                                    };
                                    self.emit(opname, &[field_idx, val]);
                                }
                            }
                        }
                    }
                    result
                } else if node.typespec.is_void() || Self::is_void_builtin(name) {
                    // void-return builtins: sincos, printf, setmessage, exit, etc.
                    // Also handles no-typecheck path where node.typespec is UNKNOWN
                    // but the function is known to return void.
                    let op_idx = self.ir.opcodes.len();
                    let mut arg_indices = Vec::with_capacity(args.len());
                    for arg in args {
                        let idx = self.gen_node(arg);
                        arg_indices.push(idx);
                    }
                    let opname = self.resolve_polymorphic_name(name, args);
                    self.emit(&opname, &arg_indices);
                    if let Some(op) = self.ir.opcodes.get_mut(op_idx) {
                        op.argwrite = 0;
                        op.argread = !0u32;
                    }
                    -1
                } else {
                    let result = self.alloc_temp(node.typespec);
                    let mut arg_indices = vec![result];
                    for arg in args {
                        let idx = self.gen_node(arg);
                        arg_indices.push(idx);
                    }

                    // Check for polymorphic builtins that need name mangling
                    let opname = self.resolve_polymorphic_name(name, args);
                    self.emit(&opname, &arg_indices);
                    result
                }
            }

            ASTNodeKind::ConditionalStatement {
                cond,
                true_stmt,
                false_stmt,
            } => {
                let cond_val = self.gen_cond(cond);
                let if_op_idx = self.current_op_index();
                self.emit_with_jumps("if", &[cond_val], [-1, -1, -1, -1]);

                let true_start = self.current_op_index();
                self.gen_node(true_stmt);

                if let Some(false_stmt) = false_stmt {
                    let jump_end_idx = self.current_op_index();
                    self.emit_with_jumps("nop", &[], [-1, -1, -1, -1]);

                    let false_start = self.current_op_index();
                    self.gen_node(false_stmt);
                    let end = self.current_op_index();

                    self.ir.opcodes[if_op_idx as usize].jump[0] = true_start;
                    self.ir.opcodes[if_op_idx as usize].jump[1] = false_start;
                    self.ir.opcodes[jump_end_idx as usize].jump[0] = end;
                } else {
                    let end = self.current_op_index();
                    self.ir.opcodes[if_op_idx as usize].jump[0] = true_start;
                    self.ir.opcodes[if_op_idx as usize].jump[1] = end;
                }
                -1
            }

            ASTNodeKind::LoopStatement {
                loop_type,
                init,
                cond,
                iter,
                body,
            } => {
                let has_init = init.is_some();
                if has_init {
                    self.scope_depth += 1;
                }
                if let Some(init) = init {
                    self.gen_node(init);
                }

                let loop_start = self.current_op_index();

                // Push a loop context for break/continue
                self.loop_stack.push(LoopContext {
                    break_patches: Vec::new(),
                    continue_patches: Vec::new(),
                });

                if *loop_type == LoopType::DoWhile {
                    // Do-while: body first, then condition check (C++ "dowhile" opcode)
                    self.gen_node(body);
                    let continue_target = self.current_op_index();

                    if let Some(cond) = cond {
                        let cond_val = self.gen_cond(cond);
                        let check_idx = self.current_op_index();
                        self.emit_with_jumps("if", &[cond_val], [-1, -1, -1, -1]);
                        // True: jump back to loop_start
                        self.ir.opcodes[check_idx as usize].jump[0] = self.current_op_index();
                        self.emit_with_jumps("nop", &[], [loop_start, -1, -1, -1]);
                        // False: fall through to end
                        let end = self.current_op_index();
                        self.ir.opcodes[check_idx as usize].jump[1] = end;

                        if let Some(lctx) = self.loop_stack.last() {
                            for &patch_idx in &lctx.break_patches {
                                self.ir.opcodes[patch_idx].jump[0] = end;
                            }
                            for &patch_idx in &lctx.continue_patches {
                                self.ir.opcodes[patch_idx].jump[0] = continue_target;
                            }
                        }
                    } else {
                        // do {} while() with no cond -> infinite loop
                        self.emit_with_jumps("nop", &[], [loop_start, -1, -1, -1]);
                        let end = self.current_op_index();

                        if let Some(lctx) = self.loop_stack.last() {
                            for &patch_idx in &lctx.break_patches {
                                self.ir.opcodes[patch_idx].jump[0] = end;
                            }
                            for &patch_idx in &lctx.continue_patches {
                                self.ir.opcodes[patch_idx].jump[0] = continue_target;
                            }
                        }
                    }
                } else if let Some(cond) = cond {
                    // While/for loop: condition first
                    let cond_val = self.gen_cond(cond);
                    let check_idx = self.current_op_index();
                    self.emit_with_jumps("if", &[cond_val], [-1, -1, -1, -1]);

                    self.gen_node(body);

                    let continue_target = self.current_op_index();

                    if let Some(iter) = iter {
                        self.gen_node(iter);
                    }

                    self.emit_with_jumps("nop", &[], [loop_start, -1, -1, -1]);
                    let end = self.current_op_index();
                    self.ir.opcodes[check_idx as usize].jump[1] = end;

                    if let Some(lctx) = self.loop_stack.last() {
                        for &patch_idx in &lctx.break_patches {
                            self.ir.opcodes[patch_idx].jump[0] = end;
                        }
                        for &patch_idx in &lctx.continue_patches {
                            self.ir.opcodes[patch_idx].jump[0] = continue_target;
                        }
                    }
                } else {
                    // Infinite loop (while(true))
                    self.gen_node(body);

                    let continue_target = self.current_op_index();
                    self.emit_with_jumps("nop", &[], [loop_start, -1, -1, -1]);
                    let end = self.current_op_index();

                    if let Some(lctx) = self.loop_stack.last() {
                        for &patch_idx in &lctx.break_patches {
                            self.ir.opcodes[patch_idx].jump[0] = end;
                        }
                        for &patch_idx in &lctx.continue_patches {
                            self.ir.opcodes[patch_idx].jump[0] = continue_target;
                        }
                    }
                }

                self.loop_stack.pop();
                if has_init {
                    self.cleanup_scope(self.scope_depth);
                    self.scope_depth -= 1;
                }
                -1
            }

            ASTNodeKind::LoopModStatement { mod_type } => {
                // C++ emits real "break"/"continue" opcodes (codegen.cpp:1450-1454)
                match mod_type {
                    LoopMod::Break => {
                        let idx = self.current_op_index() as usize;
                        self.emit_with_jumps("break", &[], [-1, -1, -1, -1]);
                        if let Some(lctx) = self.loop_stack.last_mut() {
                            lctx.break_patches.push(idx);
                        }
                    }
                    LoopMod::Continue => {
                        let idx = self.current_op_index() as usize;
                        self.emit_with_jumps("continue", &[], [-1, -1, -1, -1]);
                        if let Some(lctx) = self.loop_stack.last_mut() {
                            lctx.continue_patches.push(idx);
                        }
                    }
                }
                -1
            }

            ASTNodeKind::ReturnStatement { expr } => {
                // C++ emits "return" inside user functions, "exit" for shader body
                let opname = if self.current_return_sym >= 0 {
                    "return"
                } else {
                    "exit"
                };
                if let Some(expr) = expr {
                    let val = self.gen_node(expr);
                    if self.current_return_sym >= 0 {
                        self.emit("assign", &[self.current_return_sym, val]);
                    }
                    self.emit(opname, &[]);
                } else {
                    self.emit(opname, &[]);
                }
                -1
            }

            ASTNodeKind::TernaryExpression {
                cond,
                true_expr,
                false_expr,
            } => {
                // Conditional evaluation: only evaluate the taken branch
                let cond_val = self.gen_cond(cond);
                let result = self.alloc_temp(node.typespec);
                let if_op_idx = self.current_op_index();
                self.emit_with_jumps("if", &[cond_val], [-1, -1, -1, -1]);

                let true_start = self.current_op_index();
                let t = self.gen_node(true_expr);
                self.emit("assign", &[result, t]);
                let jump_end_idx = self.current_op_index();
                self.emit_with_jumps("nop", &[], [-1, -1, -1, -1]);

                let false_start = self.current_op_index();
                let f = self.gen_node(false_expr);
                self.emit("assign", &[result, f]);
                let end = self.current_op_index();

                self.ir.opcodes[if_op_idx as usize].jump[0] = true_start;
                self.ir.opcodes[if_op_idx as usize].jump[1] = false_start;
                self.ir.opcodes[jump_end_idx as usize].jump[0] = end;
                result
            }

            ASTNodeKind::TypeConstructor { typespec, args } => {
                let result = self.alloc_temp(*typespec);
                let mut arg_indices = vec![result];
                for arg in args {
                    arg_indices.push(self.gen_node(arg));
                }
                self.emit("construct", &arg_indices);
                result
            }

            ASTNodeKind::Index {
                base,
                index,
                index2,
                ..
            } => {
                // Detect nested matrix double-index: M[row][col] → mxcompref
                // Parser produces: Index { base: Index { base: M, index: row }, index: col }
                if index2.is_none() {
                    if let ASTNodeKind::Index {
                        base: inner_base,
                        index: inner_idx,
                        index2: None,
                        ..
                    } = &base.kind
                    {
                        if inner_base.typespec.simpletype().is_matrix44() {
                            let mx_idx = self.gen_node(inner_base);
                            let row = self.gen_node(inner_idx);
                            let col = self.gen_node(index);
                            let result = self.alloc_temp(node.typespec);
                            self.emit("mxcompref", &[result, mx_idx, row, col]);
                            return result;
                        }
                    }
                }
                let base_idx = self.gen_node(base);
                let idx = self.gen_node(index);
                if let Some(idx2_node) = index2 {
                    // Double-index: matrix[i][j] → mxcompref
                    let idx2 = self.gen_node(idx2_node);
                    let result = self.alloc_temp(node.typespec);
                    self.emit("mxcompref", &[result, base_idx, idx, idx2]);
                    result
                } else {
                    // Use actual symbol typespec when base.typespec is UNKNOWN
                    // (no-typecheck path: base.typespec defaults to UNKNOWN).
                    let bt = if base.typespec.is_unknown()
                        && base_idx >= 0
                        && (base_idx as usize) < self.ir.symbols.len()
                    {
                        self.ir.symbols[base_idx as usize].typespec
                    } else {
                        base.typespec
                    };
                    let result = self.alloc_temp(node.typespec);
                    if bt.is_array() {
                        self.emit("aref", &[result, base_idx, idx]);
                    } else {
                        // Component access: vec[i] or matrix[i] → compref
                        self.emit("compref", &[result, base_idx, idx]);
                    }
                    result
                }
            }

            ASTNodeKind::PreIncDec { op, expr } => {
                let sym = self.gen_node(expr);
                // C++ codegen.cpp:1148 — use int 1 or float 1.0 based on type
                let one = if node.typespec.is_int() {
                    self.const_int(1)
                } else {
                    self.const_float(1.0)
                };
                match op {
                    Operator::PreInc => self.emit("add", &[sym, sym, one]),
                    Operator::PreDec => self.emit("sub", &[sym, sym, one]),
                    _ => {}
                }
                sym
            }

            ASTNodeKind::PostIncDec { op, expr } => {
                let sym = self.gen_node(expr);
                let result = self.alloc_temp(node.typespec);
                self.emit("assign", &[result, sym]);
                // C++ codegen.cpp:1161 — use int 1 or float 1.0 based on type
                let one = if node.typespec.is_int() {
                    self.const_int(1)
                } else {
                    self.const_float(1.0)
                };
                match op {
                    Operator::PostInc => self.emit("add", &[sym, sym, one]),
                    Operator::PostDec => self.emit("sub", &[sym, sym, one]),
                    _ => {}
                }
                result
            }

            ASTNodeKind::StructDeclaration { .. } => {
                // Struct declarations define types; no opcodes emitted.
                // The struct layout is handled by the type system.
                -1
            }

            ASTNodeKind::StructSelect { base, field } => {
                let mut base_ts = base.typespec;
                // If typespec wasn't set by typecheck (UNKNOWN), try to resolve from
                // the already-registered symbol's TypeSpec (codegen-only path, no typecheck).
                if !base_ts.is_structure_based() {
                    if let ASTNodeKind::VariableRef { name: vname } = &base.kind {
                        if let Some(idx) = self.find_sym_by_name(vname) {
                            let sym_ts = self.ir.symbols[idx].typespec;
                            if sym_ts.is_structure_based() {
                                base_ts = sym_ts;
                            }
                        }
                    }
                }
                if base_ts.is_structure_based() {
                    // Struct field access: resolve to the pre-expanded sub-symbol
                    let base_idx = self.gen_node(base);
                    let base_name = self.ir.symbols[base_idx as usize].name.as_str().to_string();
                    let field_sym_name = format!("{}.{}", base_name, field);
                    if let Some(field_idx) = self.find_sym_by_name(&field_sym_name) {
                        field_idx as i32
                    } else {
                        // Fallback: emit getfield if sub-symbol not found
                        let result = self.alloc_temp(node.typespec);
                        let field_const = self.const_string(field);
                        self.emit("getfield", &[result, base_idx, field_const]);
                        result
                    }
                } else {
                    // Triple/matrix component access (e.g., color.r → compref)
                    let base_idx = self.gen_node(base);
                    let base_td = base_ts.simpletype();
                    if base_td.is_triple()
                        || base_td.aggregate == crate::typedesc::Aggregate::Vec3 as u8
                    {
                        let comp = match field.as_str() {
                            "x" | "r" | "s" => 0,
                            "y" | "g" | "t" => 1,
                            "z" | "b" => 2,
                            _ => 0,
                        };
                        let result = self.alloc_temp(node.typespec);
                        let idx_sym = self.const_int(comp);
                        self.emit("compref", &[result, base_idx, idx_sym]);
                        result
                    } else {
                        // Unknown field access: fallback to getfield
                        let result = self.alloc_temp(node.typespec);
                        let field_const = self.const_string(field);
                        self.emit("getfield", &[result, base_idx, field_const]);
                        result
                    }
                }
            }

            ASTNodeKind::CompoundInitializer { elements, .. } => {
                if elements.is_empty() {
                    return -1;
                }
                let ts = node.typespec;
                let result = self.alloc_temp(ts);

                if ts.simpletype().is_array() || ts.simpletype().arraylen > 0 {
                    // Array initialization: emit per-element aassign (no init_array needed)
                    for (i, elem) in elements.iter().enumerate() {
                        let val = self.gen_node(elem);
                        let idx = self.const_int(i as i32);
                        self.emit("aassign", &[result, idx, val]);
                    }
                } else if ts.simpletype().is_triple() && elements.len() == 3 {
                    // Triple construction (color/vector/point/normal)
                    let mut arg_indices = vec![result];
                    for elem in elements {
                        arg_indices.push(self.gen_node(elem));
                    }
                    self.emit("construct", &arg_indices);
                } else {
                    // Generic construct
                    let mut arg_indices = vec![result];
                    for elem in elements {
                        arg_indices.push(self.gen_node(elem));
                    }
                    self.emit("construct", &arg_indices);
                }
                result
            }

            ASTNodeKind::TypecastExpression { expr, .. } => {
                let src = self.gen_node(expr);
                let result = self.alloc_temp(node.typespec);
                self.emit("assign", &[result, src]);
                result
            }

            ASTNodeKind::CommaOperator { exprs } => {
                let mut last = -1i32;
                for expr in exprs {
                    last = self.gen_node(expr);
                }
                last
            }

            ASTNodeKind::EmptyStatement => -1,
        }
    }

    /// Resolve polymorphic function names — match C++ polymorphic dispatch.
    /// For most builtins, just return the name as-is. The interpreter handles
    /// polymorphism dynamically. For specific cases, mangle the name.
    fn resolve_polymorphic_name(&self, name: &str, _args: &[Box<ASTNode>]) -> String {
        // The interpreter handles most polymorphism dynamically.
        // For OSO compatibility, we emit the base name.
        // C++ oslc emits polymorphic names like "noise_fff" only for LLVM;
        // the OSO format uses the base name.
        name.to_string()
    }

    /// Emit code for a component assignment: vec[i] = value.
    fn emit_compassign(&mut self, vec_idx: i32, comp_idx: i32, value_idx: i32) {
        self.emit("compassign", &[vec_idx, comp_idx, value_idx]);
    }

    /// Emit code for a matrix component assignment: mat[r][c] = value.
    fn emit_mxcompassign(&mut self, mat_idx: i32, row_idx: i32, col_idx: i32, value_idx: i32) {
        self.emit("mxcompassign", &[mat_idx, row_idx, col_idx, value_idx]);
    }
}

/// Convenience function: generate IR from an AST.
pub fn generate(nodes: &[Box<ASTNode>]) -> ShaderIR {
    let mut codegen = CodeGen::new();
    codegen.generate(nodes)
}

// ---------------------------------------------------------------------------
// Post-codegen optimization passes
// ---------------------------------------------------------------------------

/// Try to evaluate a binary op on two ConstValues at compile time.
fn eval_const_binary(op: &str, a: &ConstValue, b: &ConstValue) -> Option<ConstValue> {
    match (op, a, b) {
        // int op int
        ("add", ConstValue::Int(a), ConstValue::Int(b)) => {
            Some(ConstValue::Int(a.wrapping_add(*b)))
        }
        ("sub", ConstValue::Int(a), ConstValue::Int(b)) => {
            Some(ConstValue::Int(a.wrapping_sub(*b)))
        }
        ("mul", ConstValue::Int(a), ConstValue::Int(b)) => {
            Some(ConstValue::Int(a.wrapping_mul(*b)))
        }
        ("div", ConstValue::Int(a), ConstValue::Int(b)) if *b != 0 => Some(ConstValue::Int(a / b)),
        ("mod", ConstValue::Int(a), ConstValue::Int(b)) if *b != 0 => Some(ConstValue::Int(a % b)),
        // float op float
        ("add", ConstValue::Float(a), ConstValue::Float(b)) => Some(ConstValue::Float(a + b)),
        ("sub", ConstValue::Float(a), ConstValue::Float(b)) => Some(ConstValue::Float(a - b)),
        ("mul", ConstValue::Float(a), ConstValue::Float(b)) => Some(ConstValue::Float(a * b)),
        ("div", ConstValue::Float(a), ConstValue::Float(b)) if *b != 0.0 => {
            Some(ConstValue::Float(a / b))
        }
        _ => None,
    }
}

/// Try to evaluate a unary op on a ConstValue at compile time.
fn eval_const_unary(op: &str, a: &ConstValue) -> Option<ConstValue> {
    match (op, a) {
        ("neg", ConstValue::Int(v)) => Some(ConstValue::Int(-v)),
        ("neg", ConstValue::Float(v)) => Some(ConstValue::Float(-v)),
        ("abs", ConstValue::Int(v)) => Some(ConstValue::Int(v.abs())),
        ("abs", ConstValue::Float(v)) => Some(ConstValue::Float(v.abs())),
        ("sqrt", ConstValue::Float(v)) if *v >= 0.0 => Some(ConstValue::Float(v.sqrt())),
        _ => None,
    }
}

/// Look up a symbol's constant value if it's a Const symbol.
fn get_const_val<'a>(ir: &'a ShaderIR, sym_idx: usize) -> Option<&'a ConstValue> {
    if sym_idx >= ir.symbols.len() || ir.symbols[sym_idx].symtype != SymType::Const {
        return None;
    }
    ir.const_values
        .iter()
        .find(|(i, _)| *i == sym_idx)
        .map(|(_, v)| v)
}

/// Constant folding pass: replace opcodes with all-constant inputs by assignments.
/// Handles add, sub, mul, div, mod, neg, abs, sqrt for int/float.
pub fn fold_constants(ir: &mut ShaderIR) {
    let num_ops = ir.opcodes.len();
    for op_idx in 0..num_ops {
        let opname = ir.opcodes[op_idx].op.as_str().to_string();
        let nargs = ir.opcodes[op_idx].nargs as usize;
        let firstarg = ir.opcodes[op_idx].firstarg as usize;

        // Binary ops: result = op(a, b) — 3 args
        if nargs == 3 && matches!(opname.as_str(), "add" | "sub" | "mul" | "div" | "mod") {
            if firstarg + 2 >= ir.args.len() {
                continue;
            }
            let a_idx = ir.args[firstarg + 1] as usize;
            let b_idx = ir.args[firstarg + 2] as usize;
            let dst_idx = ir.args[firstarg] as usize;

            let (a_val, b_val) = {
                let a = get_const_val(ir, a_idx);
                let b = get_const_val(ir, b_idx);
                match (a, b) {
                    (Some(a), Some(b)) => (a.clone(), b.clone()),
                    _ => continue,
                }
            };

            if let Some(result_cv) = eval_const_binary(&opname, &a_val, &b_val) {
                // Create a new constant symbol for the result
                let new_const_idx = ir.symbols.len();
                let (ts, name) = match &result_cv {
                    ConstValue::Int(v) => {
                        (TypeSpec::from_simple(TypeDesc::INT), format!("$fold_i{v}"))
                    }
                    ConstValue::Float(v) => (
                        TypeSpec::from_simple(TypeDesc::FLOAT),
                        format!("$fold_f{v}"),
                    ),
                    _ => continue,
                };
                let mut sym = Symbol::new(UString::new(&name), ts, SymType::Const);
                sym.initializers = 1;
                ir.symbols.push(sym);
                ir.const_values.push((new_const_idx, result_cv));

                // Replace opcode: assign dst = new_const
                // Reuse arg slots: args[firstarg] = dst, args[firstarg+1] = new_const
                ir.args[firstarg + 1] = new_const_idx as i32;
                ir.opcodes[op_idx].op = UString::new("assign");
                ir.opcodes[op_idx].nargs = 2;
                // Fix arg rw: arg0=write, arg1=read
                ir.opcodes[op_idx].argwrite = 1;
                ir.opcodes[op_idx].argread = 0b10;
                let _ = dst_idx; // used via args[firstarg]
            }
        }

        // Unary ops: result = op(a) — 2 args
        if nargs == 2 && matches!(opname.as_str(), "neg" | "abs" | "sqrt") {
            if firstarg + 1 >= ir.args.len() {
                continue;
            }
            let a_idx = ir.args[firstarg + 1] as usize;

            let a_val = match get_const_val(ir, a_idx) {
                Some(v) => v.clone(),
                None => continue,
            };

            if let Some(result_cv) = eval_const_unary(&opname, &a_val) {
                let new_const_idx = ir.symbols.len();
                let (ts, name) = match &result_cv {
                    ConstValue::Int(v) => {
                        (TypeSpec::from_simple(TypeDesc::INT), format!("$fold_i{v}"))
                    }
                    ConstValue::Float(v) => (
                        TypeSpec::from_simple(TypeDesc::FLOAT),
                        format!("$fold_f{v}"),
                    ),
                    _ => continue,
                };
                let mut sym = Symbol::new(UString::new(&name), ts, SymType::Const);
                sym.initializers = 1;
                ir.symbols.push(sym);
                ir.const_values.push((new_const_idx, result_cv));

                ir.args[firstarg + 1] = new_const_idx as i32;
                ir.opcodes[op_idx].op = UString::new("assign");
                ir.opcodes[op_idx].nargs = 2;
                ir.opcodes[op_idx].argwrite = 1;
                ir.opcodes[op_idx].argread = 0b10;
            }
        }
    }
}

/// Dead code elimination: remove opcodes whose output is never read.
/// Skips control flow ops (if, nop with jumps, return, end, functioncall, useparam).
pub fn eliminate_dead_code(ir: &mut ShaderIR) {
    // Build a set of symbols that are actually read by any opcode
    let mut read_syms = std::collections::HashSet::new();
    for op in &ir.opcodes {
        if op.op.as_str().is_empty() {
            continue;
        }
        let nargs = op.nargs as usize;
        let firstarg = op.firstarg as usize;
        for j in 0..nargs {
            if firstarg + j >= ir.args.len() {
                break;
            }
            if op.is_arg_read(j as u32) {
                read_syms.insert(ir.args[firstarg + j] as usize);
            }
        }
    }

    // Also mark params/output params/globals as always needed
    for (i, sym) in ir.symbols.iter().enumerate() {
        match sym.symtype {
            SymType::Param | SymType::OutputParam | SymType::Global => {
                read_syms.insert(i);
            }
            _ => {}
        }
    }

    // Blank out opcodes that write to a symbol nobody reads
    for op in &mut ir.opcodes {
        let opname = op.op.as_str();
        // Skip control flow and side-effect ops
        if matches!(
            opname,
            "" | "if"
                | "nop"
                | "return"
                | "end"
                | "functioncall"
                | "useparam"
                | "printf"
                | "warning"
                | "error"
                | "fprintf"
                | "exit"
        ) {
            continue;
        }
        // Skip ops with jump targets (control flow)
        if op.jump.iter().any(|&j| j >= 0) {
            continue;
        }
        let nargs = op.nargs as usize;
        let firstarg = op.firstarg as usize;
        if nargs == 0 {
            continue;
        }
        // Check if any written arg is actually read somewhere
        let mut any_output_read = false;
        for j in 0..nargs {
            if firstarg + j >= ir.args.len() {
                break;
            }
            if op.is_arg_written(j as u32) {
                let sym_idx = ir.args[firstarg + j] as usize;
                if read_syms.contains(&sym_idx) {
                    any_output_read = true;
                    break;
                }
            }
        }
        if !any_output_read {
            op.op = UString::new(""); // blank out dead op
        }
    }
}

/// Propagate has_derivs flag: any symbol assigned from a derivs-carrying source
/// also gets has_derivs set.
pub fn propagate_derivs(ir: &mut ShaderIR) {
    let mut changed = true;
    while changed {
        changed = false;
        for op_idx in 0..ir.opcodes.len() {
            let opname = ir.opcodes[op_idx].op.as_str().to_string();
            let nargs = ir.opcodes[op_idx].nargs as usize;
            let firstarg = ir.opcodes[op_idx].firstarg as usize;
            if opname.is_empty() || nargs < 2 {
                continue;
            }

            // Check if any read arg has derivs
            let mut src_has_derivs = false;
            for j in 1..nargs {
                if firstarg + j >= ir.args.len() {
                    break;
                }
                if ir.opcodes[op_idx].is_arg_read(j as u32) {
                    let sym_idx = ir.args[firstarg + j] as usize;
                    if sym_idx < ir.symbols.len() && ir.symbols[sym_idx].has_derivs {
                        src_has_derivs = true;
                        break;
                    }
                }
            }

            if !src_has_derivs {
                continue;
            }

            // Propagate to written args (for ops that carry derivs through)
            if matches!(
                opname.as_str(),
                "assign" | "add" | "sub" | "mul" | "div" | "neg" | "aref" | "compref" | "construct"
            ) {
                for j in 0..nargs {
                    if firstarg + j >= ir.args.len() {
                        break;
                    }
                    if ir.opcodes[op_idx].is_arg_written(j as u32) {
                        let sym_idx = ir.args[firstarg + j] as usize;
                        if sym_idx < ir.symbols.len()
                            && !ir.symbols[sym_idx].has_derivs
                            && ir.symbols[sym_idx].typespec.simpletype().basetype
                                == crate::typedesc::BaseType::Float as u8
                        {
                            ir.symbols[sym_idx].has_derivs = true;
                            changed = true;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;

    #[test]
    fn test_simple_codegen() {
        let src = r#"
surface test(float Kd = 0.5) {
    float x = Kd + 1.0;
}
"#;
        let ast = parser::parse(src).unwrap().ast;
        let ir = generate(&ast);
        assert_eq!(ir.shader_type, ShaderType::Surface);
        assert_eq!(ir.shader_name, "test");
        assert!(!ir.symbols.is_empty());
        assert!(!ir.opcodes.is_empty());
    }

    #[test]
    fn test_assignment_codegen() {
        let src = r#"
shader test(float a = 1.0) {
    float b = a;
    b = b + 1.0;
}
"#;
        let ast = parser::parse(src).unwrap().ast;
        let ir = generate(&ast);
        assert!(ir.symbols.len() >= 2);
        assert!(ir.opcodes.len() >= 2);
    }

    #[test]
    fn test_conditional_codegen() {
        let src = r#"
shader test(float x = 0.0) {
    if (x > 0.0) {
        x = 1.0;
    }
}
"#;
        let ast = parser::parse(src).unwrap().ast;
        let ir = generate(&ast);
        assert!(
            ir.opcodes
                .iter()
                .any(|op| op.op == "if" || op.op == "gt")
        );
    }

    #[test]
    fn test_struct_type_constructor_ir() {
        // Verify that struct type constructor `Pair(3.14, 7)` compiles correctly.
        let src = r#"
struct Pair { float val; int count; };
shader test() {
    Pair p = Pair(3.14, 7);
    float v = p.val;
}
"#;
        let ast = parser::parse(src).expect("parse").ast;
        let ir = generate(&ast);
        // Check that symbol 'p' has is_structure_based == true
        let p_sym = ir.symbols.iter().find(|s| s.name == "p");
        assert!(p_sym.is_some(), "symbol 'p' not found");
        let p_ts = p_sym.unwrap().typespec;
        assert!(
            p_ts.is_structure_based(),
            "p.typespec should be struct-based, got structure_id={}",
            p_ts.structure_id()
        );
        // Check that 'p.val' sub-symbol exists
        let pval_sym = ir.symbols.iter().find(|s| s.name == "p.val");
        assert!(
            pval_sym.is_some(),
            "sub-symbol 'p.val' not found: {:?}",
            ir.symbols
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>()
        );
        // Struct type constructor now emits direct field assigns (not construct opcode).
        // Check that assign opcodes are emitted for the temp sub-symbols.
        let ops: Vec<&str> = ir.opcodes.iter().map(|op| op.op.as_str()).collect();
        let assign_count = ir
            .opcodes
            .iter()
            .filter(|op| op.op == "assign")
            .count();
        assert!(
            assign_count >= 2,
            "expected at least 2 assign ops for p.val and p.count, got ops={:?}",
            ops
        );
    }

    #[test]
    fn test_struct_compound_init_ops() {
        // Struct with compound initializer {1.5, 2} should emit assign to s.val and s.count,
        // NOT getfield. This is the core compound-init feature.
        let src = r#"
struct MyData { float val; int count; };
shader test() { MyData s = {1.5, 2}; float v = s.val; }
"#;
        let ast = parser::parse(src).expect("parse").ast;
        let ir = generate(&ast);
        let _ops: Vec<&str> = ir.opcodes.iter().map(|op| op.op.as_str()).collect();
        // Should not use getfield for struct field access after compound init
        // (we should directly use the sub-symbol)
        let s_sym = ir.symbols.iter().find(|s| s.name == "s");
        assert!(
            s_sym
                .map(|s| s.typespec.is_structure_based())
                .unwrap_or(false),
            "s should be struct-based"
        );
        let sval_sym = ir.symbols.iter().find(|s| s.name == "s.val");
        assert!(sval_sym.is_some(), "s.val sub-symbol should exist");
        // The first assign op should assign to s.val
        let first_assign = ir.opcodes.iter().find(|op| op.op == "assign");
        assert!(first_assign.is_some(), "expected at least one assign op");
        let first_arg = ir.args[first_assign.unwrap().firstarg as usize];
        let assigned_sym = ir.symbols[first_arg as usize].name.as_str();
        assert_eq!(
            assigned_sym, "s.val",
            "first assign should target s.val, got {}",
            assigned_sym
        );
    }
}
