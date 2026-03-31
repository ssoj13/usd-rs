//! Symbol and Opcode — compiler/runtime IR types.
//!
//! These mirror the C++ `OSL::pvt::Symbol` and `OSL::pvt::Opcode` from
//! `osl_pvt.h`.

use std::fmt;

use crate::typespec::TypeSpec;
use crate::ustring::UString;

// ---------------------------------------------------------------------------
// ShaderType
// ---------------------------------------------------------------------------

/// The kind of shader (surface, displacement, volume, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum ShaderType {
    #[default]
    Unknown = 0,
    Generic = 1,
    Surface = 2,
    Displacement = 3,
    Volume = 4,
    Light = 5,
}

impl ShaderType {
    pub fn name(self) -> &'static str {
        match self {
            ShaderType::Unknown => "unknown",
            ShaderType::Generic => "shader",
            ShaderType::Surface => "surface",
            ShaderType::Displacement => "displacement",
            ShaderType::Volume => "volume",
            ShaderType::Light => "light",
        }
    }

    pub fn from_name(s: &str) -> Self {
        match s {
            "shader" | "generic" => ShaderType::Generic,
            "surface" => ShaderType::Surface,
            "displacement" => ShaderType::Displacement,
            "volume" => ShaderType::Volume,
            "light" => ShaderType::Light,
            _ => ShaderType::Unknown,
        }
    }
}

impl fmt::Display for ShaderType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

// ---------------------------------------------------------------------------
// SymType
// ---------------------------------------------------------------------------

/// Kind of symbol (parameter, local, global, constant, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SymType {
    Param = 0,
    OutputParam = 1,
    Local = 2,
    Temp = 3,
    Global = 4,
    Const = 5,
    Function = 6,
    Type = 7,
}

impl SymType {
    pub fn short_name(self) -> &'static str {
        match self {
            SymType::Param => "param",
            SymType::OutputParam => "oparam",
            SymType::Local => "local",
            SymType::Temp => "temp",
            SymType::Global => "global",
            SymType::Const => "const",
            SymType::Function => "func",
            SymType::Type => "typename",
        }
    }
}

impl fmt::Display for SymType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.short_name())
    }
}

// ---------------------------------------------------------------------------
// SymArena
// ---------------------------------------------------------------------------

/// Memory arena for a symbol's data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SymArena {
    Unknown = 0,
    Absolute = 1,
    Heap = 2,
    Outputs = 3,
    UserData = 4,
    Interactive = 5,
}

// ---------------------------------------------------------------------------
// ValueSource
// ---------------------------------------------------------------------------

/// Where a symbol's value came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ValueSource {
    Default = 0,
    Instance = 1,
    Geom = 2,
    Connected = 3,
}

// ---------------------------------------------------------------------------
// Symbol
// ---------------------------------------------------------------------------

/// A single symbol (identifier) and all relevant information about it.
///
/// This is a runtime/compiler record used during compilation, optimization,
/// and execution of OSL shaders.
#[derive(Clone)]
pub struct Symbol {
    /// Symbol name (unmangled).
    pub name: UString,
    /// Data type of the symbol.
    pub typespec: TypeSpec,
    /// Size of data in bytes (without derivs).
    pub size: i32,
    /// Kind of symbol (param, local, temp, etc.).
    pub symtype: SymType,
    /// Does this symbol have derivatives?
    pub has_derivs: bool,
    /// Is the initializer a constant expression?
    pub const_initializer: bool,
    /// Connected to a downstream layer?
    pub connected_down: bool,
    /// Has the param been initialized?
    pub initialized: bool,
    /// Is the param overridden by geometry?
    pub interpolated: bool,
    /// May the param change interactively?
    pub interactive: bool,
    /// The param won't be modified interactively.
    pub noninteractive: bool,
    /// Is the param allowed to connect?
    pub allowconnect: bool,
    /// Is this symbol a renderer output?
    pub renderer_output: bool,
    /// Read-only symbol.
    pub readonly: bool,
    /// Uniform under batched execution?
    pub is_uniform: bool,
    /// Forced to be LLVM bool?
    pub forced_llvm_bool: bool,
    /// Memory arena for the symbol's data.
    pub arena: SymArena,
    /// Where did the value come from?
    pub valuesource: ValueSource,
    /// Struct field ID (-1 if not a struct field).
    pub fieldid: i16,
    /// Layer (within the group) this belongs to.
    pub layer: i16,
    /// Scope where this symbol was declared.
    pub scope: i32,
    /// Byte offset of data (-1 for unknown).
    pub dataoffset: i32,
    /// Wide data offset for batched execution.
    pub wide_dataoffset: i32,
    /// Number of default initializers.
    pub initializers: i32,
    /// Range of init ops (begin, end) for params.
    pub initbegin: i32,
    pub initend: i32,
    /// First and last op where the sym is read.
    pub firstread: i32,
    pub lastread: i32,
    /// First and last op where the sym is written.
    pub firstwrite: i32,
    pub lastwrite: i32,
    /// Explicit `[[int lockgeom = N]]` hint from the shader source.
    /// Distinct from the computed `lockgeom()` method which derives the
    /// value from `interpolated` and `interactive` flags. This field
    /// stores the user's explicit override; the optimizer reads it directly.
    pub is_lockgeom: bool,
    /// Metadata from `[[ type name = value ]]` for OSO %meta emission.
    pub metadata: Vec<(String, String, String)>,
}

impl Symbol {
    /// Unknown/uninitialized data offset.
    pub const UNKNOWN_OFFSET: i32 = -1;

    /// Create a new symbol with the given name, type, and kind.
    pub fn new(name: UString, typespec: TypeSpec, symtype: SymType) -> Self {
        let size = if typespec.is_unsized_array() {
            0
        } else {
            typespec.simpletype().size() as i32
        };

        Self {
            name,
            typespec,
            size,
            symtype,
            has_derivs: false,
            const_initializer: false,
            connected_down: false,
            initialized: false,
            interpolated: false,
            interactive: false,
            noninteractive: false,
            allowconnect: true,
            renderer_output: false,
            readonly: false,
            is_uniform: true,
            forced_llvm_bool: false,
            arena: SymArena::Unknown,
            valuesource: ValueSource::Default,
            fieldid: -1,
            layer: -1,
            scope: 0,
            dataoffset: Self::UNKNOWN_OFFSET,
            wide_dataoffset: Self::UNKNOWN_OFFSET,
            initializers: 0,
            initbegin: 0,
            initend: 0,
            firstread: i32::MAX,
            lastread: -1,
            firstwrite: i32::MAX,
            lastwrite: -1,
            is_lockgeom: true,
            metadata: Vec::new(),
        }
    }

    /// Array length (from typespec). 0 = not array, -1 = unsized. Matches C++ `Symbol::arraylen()`.
    #[inline]
    pub fn arraylen(&self) -> i32 {
        self.typespec.arraylength()
    }

    /// Return the mangled name (scope-prefixed).
    pub fn mangled(&self) -> String {
        if self.scope == 0 {
            self.name.as_str().to_string()
        } else {
            format!("___{}_{}", self.scope, self.name)
        }
    }

    /// Size including derivs (3× if has_derivs).
    pub fn derivsize(&self) -> i32 {
        if self.has_derivs {
            3 * self.size
        } else {
            self.size
        }
    }

    /// Is this symbol a function?
    pub fn is_function(&self) -> bool {
        self.symtype == SymType::Function
    }

    /// Is this symbol a struct type?
    pub fn is_structure(&self) -> bool {
        self.symtype == SymType::Type
    }

    /// Is this a constant?
    pub fn is_constant(&self) -> bool {
        self.symtype == SymType::Const
    }

    /// Is this a temporary?
    pub fn is_temp(&self) -> bool {
        self.symtype == SymType::Temp
    }

    /// Is this symbol connected?
    pub fn connected(&self) -> bool {
        self.valuesource == ValueSource::Connected
    }

    /// Is this symbol varying (non-uniform) under batched execution?
    pub fn is_varying(&self) -> bool {
        !self.is_uniform
    }

    /// Mark this symbol as varying.
    pub fn make_varying(&mut self) {
        self.is_uniform = false;
    }

    /// Can we lock the value to a constant?
    pub fn lockgeom(&self) -> bool {
        !self.interpolated && !self.interactive
    }

    /// Mark read/write at a given op.
    pub fn mark_rw(&mut self, op: i32, read: bool, write: bool) {
        if read {
            self.firstread = self.firstread.min(op);
            self.lastread = self.lastread.max(op);
        }
        if write {
            self.firstwrite = self.firstwrite.min(op);
            self.lastwrite = self.lastwrite.max(op);
        }
    }

    /// Clear read/write tracking.
    pub fn clear_rw(&mut self) {
        self.firstread = i32::MAX;
        self.lastread = -1;
        self.firstwrite = i32::MAX;
        self.lastwrite = -1;
    }

    /// Mark as always used.
    pub fn mark_always_used(&mut self, write: bool) {
        self.firstread = 0;
        self.lastread = i32::MAX;
        if write {
            self.firstwrite = 0;
            self.lastwrite = i32::MAX;
        }
    }

    pub fn ever_read(&self) -> bool {
        self.lastread >= 0
    }
    pub fn ever_written(&self) -> bool {
        self.lastwrite >= 0
    }
    pub fn ever_used(&self) -> bool {
        self.ever_read() || self.ever_written()
    }
    pub fn ever_used_in_group(&self) -> bool {
        self.ever_used() || self.connected_down || self.renderer_output
    }

    pub fn has_init_ops(&self) -> bool {
        self.initbegin != self.initend
    }

    /// Set init ops range at once.
    /// Matches C++ `set_initrange(b, e)`.
    pub fn set_initrange(&mut self, begin: i32, end: i32) {
        self.initbegin = begin;
        self.initend = end;
    }

    /// Directly set read range.
    /// Matches C++ `set_read(first, last)`.
    pub fn set_read(&mut self, first: i32, last: i32) {
        self.firstread = first;
        self.lastread = last;
    }

    /// Directly set write range.
    /// Matches C++ `set_write(first, last)`.
    pub fn set_write(&mut self, first: i32, last: i32) {
        self.firstwrite = first;
        self.lastwrite = last;
    }

    /// Merge read/write ranges from another symbol.
    /// Matches C++ `union_rw(fr, lr, fw, lw)`.
    pub fn union_rw(&mut self, fr: i32, lr: i32, fw: i32, lw: i32) {
        self.firstread = self.firstread.min(fr);
        self.lastread = self.lastread.max(lr);
        self.firstwrite = self.firstwrite.min(fw);
        self.lastwrite = self.lastwrite.max(lw);
    }

    /// First op where the symbol is used (read or written).
    /// Matches C++ `firstuse()`.
    pub fn firstuse(&self) -> i32 {
        self.firstread.min(self.firstwrite)
    }

    /// Last op where the symbol is used (read or written).
    /// Matches C++ `lastuse()`.
    pub fn lastuse(&self) -> i32 {
        self.lastread.max(self.lastwrite)
    }

    /// Return an unmangled version of the symbol name.
    /// In the runtime, names are mangled with scope prefix "___<scope>_";
    /// this strips that prefix. Matches C++ `unmangled()`.
    pub fn unmangled(&self) -> &str {
        let s = self.name.as_str();
        // Mangled names start with "___<digits>_"
        if let Some(rest) = s.strip_prefix("___") {
            // Skip digits then underscore
            if let Some(pos) = rest.find('_') {
                return &rest[pos + 1..];
            }
        }
        s
    }

    /// Human-readable string for the value source.
    /// Matches C++ `valuesourcename()`.
    pub fn valuesourcename(&self) -> &'static str {
        match self.valuesource {
            ValueSource::Default => "default",
            ValueSource::Instance => "instance",
            ValueSource::Geom => "geom",
            ValueSource::Connected => "connected",
        }
    }
}

impl fmt::Debug for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Symbol({} {} {:?} scope={})",
            self.symtype, self.name, self.typespec, self.scope
        )
    }
}

// ---------------------------------------------------------------------------
// Opcode
// ---------------------------------------------------------------------------

/// An intermediate representation opcode.
///
/// Each opcode has a name (like "add", "mul", "if", etc.), indices into
/// the argument list, jump targets for control flow, and bit fields
/// tracking which arguments are read, written, and take derivatives.
#[derive(Clone)]
pub struct Opcode {
    /// Name of the opcode operation.
    pub op: UString,
    /// Index of the first argument in the global args list.
    pub firstarg: i32,
    /// Total number of arguments.
    pub nargs: i32,
    /// Which param or method this code belongs to.
    pub method: UString,
    /// Jump target addresses (-1 means no jump). Up to 4 targets.
    pub jump: [i32; 4],
    /// Source filename.
    pub sourcefile: UString,
    /// Source line number.
    pub sourceline: i32,
    /// Bit field: which args are read by this op.
    pub argread: u32,
    /// Bit field: which args are written by this op.
    pub argwrite: u32,
    /// Bit field: which args take derivatives.
    pub argtakesderivs: u32,
    /// Op requires masking under batched execution.
    pub requires_masking: bool,
    /// Analysis-specific flag.
    pub analysis_flag: bool,
}

impl Opcode {
    /// Maximum number of jump targets per opcode.
    pub const MAX_JUMPS: usize = 4;

    pub fn new(op: UString, method: UString, firstarg: i32, nargs: i32) -> Self {
        Self {
            op,
            firstarg,
            nargs,
            method,
            jump: [-1; 4],
            sourcefile: UString::default(),
            sourceline: 0,
            argread: !1u32,    // all args read except first
            argwrite: 1,       // first arg written
            argtakesderivs: 0, // no args take derivs
            requires_masking: false,
            analysis_flag: false,
        }
    }

    /// Reset the opcode to a new operation.
    pub fn reset(&mut self, opname: UString, nargs: i32) {
        self.op = opname;
        self.nargs = nargs;
        self.jump = [-1; 4];
        self.argread = !1u32;
        self.argwrite = 1;
        self.argtakesderivs = 0;
        self.requires_masking = false;
        self.analysis_flag = false;
    }

    /// Set source location.
    pub fn set_source(&mut self, file: UString, line: i32) {
        self.sourcefile = file;
        self.sourceline = line;
    }

    /// Set all jump targets.
    pub fn set_jump(&mut self, j0: i32, j1: i32, j2: i32, j3: i32) {
        self.jump = [j0, j1, j2, j3];
    }

    /// Add a jump target to the first available slot.
    pub fn add_jump(&mut self, target: i32) {
        for j in &mut self.jump {
            if *j < 0 {
                *j = target;
                return;
            }
        }
    }

    /// Farthest jump target address.
    pub fn farthest_jump(&self) -> i32 {
        self.jump.iter().copied().max().unwrap_or(-1)
    }

    /// Is the argument at index `arg` read by this op?
    pub fn is_arg_read(&self, arg: u32) -> bool {
        if arg < 32 {
            (self.argread & (1 << arg)) != 0
        } else {
            true
        }
    }

    /// Is the argument at index `arg` written by this op?
    pub fn is_arg_written(&self, arg: u32) -> bool {
        if arg < 32 {
            (self.argwrite & (1 << arg)) != 0
        } else {
            false
        }
    }

    /// Does the argument at index `arg` take derivatives?
    pub fn arg_takes_derivs(&self, arg: u32) -> bool {
        if arg < 32 {
            (self.argtakesderivs & (1 << arg)) != 0
        } else {
            false
        }
    }

    /// Set whether argument `arg` is read.
    pub fn set_arg_read(&mut self, arg: u32, val: bool) {
        if arg < 32 {
            if val {
                self.argread |= 1 << arg;
            } else {
                self.argread &= !(1 << arg);
            }
        }
    }

    /// Set whether argument `arg` is written.
    pub fn set_arg_written(&mut self, arg: u32, val: bool) {
        if arg < 32 {
            if val {
                self.argwrite |= 1 << arg;
            } else {
                self.argwrite &= !(1 << arg);
            }
        }
    }

    /// Set whether argument `arg` takes derivatives.
    pub fn set_arg_takes_derivs(&mut self, arg: u32, val: bool) {
        if arg < 32 {
            if val {
                self.argtakesderivs |= 1 << arg;
            } else {
                self.argtakesderivs &= !(1 << arg);
            }
        }
    }

    /// Mark an argument as write-only (read=false, write=true).
    pub fn arg_writeonly(&mut self, arg: u32) {
        self.set_arg_read(arg, false);
        self.set_arg_written(arg, true);
    }

    /// Mark an argument as read-only (read=true, write=false).
    pub fn arg_readonly(&mut self, arg: u32) {
        self.set_arg_read(arg, true);
        self.set_arg_written(arg, false);
    }

    /// Set all bit fields at once.
    pub fn set_argbits(&mut self, read: u32, write: u32, derivs: u32) {
        self.argread = read;
        self.argwrite = write;
        self.argtakesderivs = derivs;
    }

    /// Set both firstarg and nargs at once.
    /// Matches C++ `set_args(firstarg, nargs)`.
    pub fn set_args(&mut self, firstarg: i32, nargs: i32) {
        self.firstarg = firstarg;
        self.nargs = nargs;
    }

    /// Change only the opcode name (optimizer transmutation).
    /// Matches C++ `transmute_opname(opname)`.
    pub fn transmute_opname(&mut self, opname: UString) {
        self.op = opname;
    }

    /// Raw bitfield of which args are read.
    /// Matches C++ `argread_bits()`.
    pub fn argread_bits(&self) -> u32 {
        self.argread
    }

    /// Raw bitfield of which args are written.
    /// Matches C++ `argwrite_bits()`.
    pub fn argwrite_bits(&self) -> u32 {
        self.argwrite
    }

    /// Full argtakesderivs bitfield.
    /// Matches C++ `argtakesderivs_all()` getter.
    pub fn argtakesderivs_all(&self) -> u32 {
        self.argtakesderivs
    }

    /// Replace the entire argtakesderivs bitfield.
    /// Matches C++ `argtakesderivs_all(unsigned int)` setter.
    pub fn set_argtakesderivs_all(&mut self, val: u32) {
        self.argtakesderivs = val;
    }

    /// Are two opcodes equivalent enough to merge?
    pub fn equivalent(&self, other: &Opcode) -> bool {
        self.op == other.op
            && self.firstarg == other.firstarg
            && self.nargs == other.nargs
            && self.jump == other.jump
    }
}

impl fmt::Debug for Opcode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Opcode({} args={}..{} jumps={:?}",
            self.op,
            self.firstarg,
            self.firstarg + self.nargs,
            self.jump
        )?;
        if !self.method.is_empty() {
            write!(f, " method={}", self.method)?;
        }
        write!(f, ")")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shader_type() {
        assert_eq!(ShaderType::Surface.name(), "surface");
        assert_eq!(ShaderType::from_name("surface"), ShaderType::Surface);
        assert_eq!(ShaderType::from_name("shader"), ShaderType::Generic);
        assert_eq!(ShaderType::from_name("bogus"), ShaderType::Unknown);
    }

    #[test]
    fn test_symbol_basic() {
        let name = UString::new("my_param");
        let ts = TypeSpec::from_simple(crate::typedesc::TypeDesc::COLOR);
        let sym = Symbol::new(name, ts, SymType::Param);

        assert_eq!(sym.name, name);
        assert!(sym.typespec.is_color());
        assert_eq!(sym.symtype, SymType::Param);
        assert_eq!(sym.size, 12); // color = 3 floats = 12 bytes
        assert!(!sym.ever_used());
    }

    #[test]
    fn test_symbol_rw_tracking() {
        let mut sym = Symbol::new(
            UString::new("x"),
            TypeSpec::from_simple(crate::typedesc::TypeDesc::FLOAT),
            SymType::Local,
        );

        sym.mark_rw(5, true, false);
        sym.mark_rw(10, false, true);
        assert!(sym.ever_read());
        assert!(sym.ever_written());
        assert_eq!(sym.firstread, 5);
        assert_eq!(sym.lastread, 5);
        assert_eq!(sym.firstwrite, 10);
        assert_eq!(sym.lastwrite, 10);

        sym.clear_rw();
        assert!(!sym.ever_read());
        assert!(!sym.ever_written());
    }

    #[test]
    fn test_symbol_varying() {
        let mut sym = Symbol::new(
            UString::new("v"),
            TypeSpec::from_simple(crate::typedesc::TypeDesc::FLOAT),
            SymType::Local,
        );
        assert!(sym.is_uniform);
        assert!(!sym.is_varying());

        sym.make_varying();
        assert!(!sym.is_uniform);
        assert!(sym.is_varying());
    }

    #[test]
    fn test_symbol_arraylen() {
        let sym = Symbol::new(
            UString::new("arr"),
            TypeSpec::from_simple(crate::typedesc::TypeDesc::FLOAT.array(8)),
            SymType::Local,
        );
        assert_eq!(sym.arraylen(), 8);

        let sym2 = Symbol::new(
            UString::new("x"),
            TypeSpec::from_simple(crate::typedesc::TypeDesc::FLOAT),
            SymType::Param,
        );
        assert_eq!(sym2.arraylen(), 0);
    }

    #[test]
    fn test_symbol_lockgeom() {
        let mut sym = Symbol::new(
            UString::new("p"),
            TypeSpec::from_simple(crate::typedesc::TypeDesc::FLOAT),
            SymType::Param,
        );
        // Default: not interpolated, not interactive -> lockgeom = true
        assert!(sym.lockgeom());

        sym.interpolated = true;
        assert!(!sym.lockgeom());

        sym.interpolated = false;
        sym.interactive = true;
        assert!(!sym.lockgeom());
    }

    #[test]
    fn test_symbol_connected() {
        let mut sym = Symbol::new(
            UString::new("c"),
            TypeSpec::from_simple(crate::typedesc::TypeDesc::COLOR),
            SymType::Param,
        );
        assert!(!sym.connected());
        sym.valuesource = ValueSource::Connected;
        assert!(sym.connected());
    }

    #[test]
    fn test_opcode_basic() {
        let op = Opcode::new(UString::new("add"), UString::default(), 0, 3);
        assert_eq!(op.op.as_str(), "add");
        assert_eq!(op.nargs, 3);
        assert!(!op.is_arg_read(0)); // first arg not read (write target)
        assert!(op.is_arg_written(0)); // first arg is written
        assert!(op.is_arg_read(1)); // second arg is read
        assert!(!op.is_arg_written(1));
    }

    #[test]
    fn test_opcode_jumps() {
        let mut op = Opcode::new(UString::new("if"), UString::default(), 0, 1);
        op.add_jump(10);
        op.add_jump(20);
        assert_eq!(op.jump[0], 10);
        assert_eq!(op.jump[1], 20);
        assert_eq!(op.jump[2], -1);
        assert_eq!(op.farthest_jump(), 20);
    }

    // -- New parity tests for Symbol methods --

    #[test]
    fn test_symbol_set_initrange() {
        let mut sym = Symbol::new(
            UString::new("p"),
            TypeSpec::from_simple(crate::typedesc::TypeDesc::FLOAT),
            SymType::Param,
        );
        sym.set_initrange(5, 10);
        assert_eq!(sym.initbegin, 5);
        assert_eq!(sym.initend, 10);
        assert!(sym.has_init_ops());
    }

    #[test]
    fn test_symbol_set_read_write() {
        let mut sym = Symbol::new(
            UString::new("x"),
            TypeSpec::from_simple(crate::typedesc::TypeDesc::FLOAT),
            SymType::Local,
        );
        sym.set_read(3, 7);
        sym.set_write(5, 12);
        assert_eq!(sym.firstread, 3);
        assert_eq!(sym.lastread, 7);
        assert_eq!(sym.firstwrite, 5);
        assert_eq!(sym.lastwrite, 12);
    }

    #[test]
    fn test_symbol_union_rw() {
        let mut sym = Symbol::new(
            UString::new("u"),
            TypeSpec::from_simple(crate::typedesc::TypeDesc::FLOAT),
            SymType::Local,
        );
        sym.mark_rw(10, true, false);
        sym.mark_rw(20, false, true);
        // union with earlier read and later write
        sym.union_rw(5, 15, 18, 30);
        assert_eq!(sym.firstread, 5);
        assert_eq!(sym.lastread, 15);
        assert_eq!(sym.firstwrite, 18);
        assert_eq!(sym.lastwrite, 30);
    }

    #[test]
    fn test_symbol_firstuse_lastuse() {
        let mut sym = Symbol::new(
            UString::new("fl"),
            TypeSpec::from_simple(crate::typedesc::TypeDesc::FLOAT),
            SymType::Local,
        );
        sym.set_read(3, 7);
        sym.set_write(5, 12);
        assert_eq!(sym.firstuse(), 3);
        assert_eq!(sym.lastuse(), 12);
    }

    #[test]
    fn test_symbol_unmangled() {
        // Unmangled name: no scope prefix
        let sym = Symbol::new(
            UString::new("color"),
            TypeSpec::from_simple(crate::typedesc::TypeDesc::COLOR),
            SymType::Local,
        );
        assert_eq!(sym.unmangled(), "color");

        // Mangled name: ___<scope>_<name>
        let sym2 = Symbol::new(
            UString::new("___3_myvar"),
            TypeSpec::from_simple(crate::typedesc::TypeDesc::FLOAT),
            SymType::Local,
        );
        assert_eq!(sym2.unmangled(), "myvar");
    }

    #[test]
    fn test_symbol_valuesourcename() {
        let mut sym = Symbol::new(
            UString::new("v"),
            TypeSpec::from_simple(crate::typedesc::TypeDesc::FLOAT),
            SymType::Param,
        );
        assert_eq!(sym.valuesourcename(), "default");
        sym.valuesource = ValueSource::Instance;
        assert_eq!(sym.valuesourcename(), "instance");
        sym.valuesource = ValueSource::Geom;
        assert_eq!(sym.valuesourcename(), "geom");
        sym.valuesource = ValueSource::Connected;
        assert_eq!(sym.valuesourcename(), "connected");
    }

    // -- New parity tests for Opcode methods --

    #[test]
    fn test_opcode_set_args() {
        let mut op = Opcode::new(UString::new("mul"), UString::default(), 0, 3);
        op.set_args(10, 4);
        assert_eq!(op.firstarg, 10);
        assert_eq!(op.nargs, 4);
    }

    #[test]
    fn test_opcode_transmute_opname() {
        let mut op = Opcode::new(UString::new("add"), UString::default(), 0, 3);
        op.transmute_opname(UString::new("sub"));
        assert_eq!(op.op.as_str(), "sub");
        // other fields unchanged
        assert_eq!(op.nargs, 3);
    }

    #[test]
    fn test_opcode_bits_accessors() {
        let mut op = Opcode::new(UString::new("add"), UString::default(), 0, 3);
        assert_eq!(op.argread_bits(), !1u32);
        assert_eq!(op.argwrite_bits(), 1u32);
        assert_eq!(op.argtakesderivs_all(), 0);

        op.set_argtakesderivs_all(0b111);
        assert_eq!(op.argtakesderivs_all(), 0b111);
    }
}
