//! Value parsing context for the USDA text parser.
//!
//! This module provides `ValueContext`, which manages the state for parsing
//! nested arrays and tuples of atomic values. It tracks dimensions, validates
//! that arrays are "square", and produces the final `VtValue`.
//!
//! # C++ Parity
//!
//! This is a direct port of `Sdf_ParserValueContext` from `parserValueContext.h`.
//! It handles:
//! - Nested list/tuple parsing with dimension tracking
//! - Shape validation (arrays must be rectangular)
//! - Value accumulation and production
//! - String recording for unknown types
//!
//! # Usage
//!
//! ```rust,ignore
//! let mut ctx = ValueContext::new();
//! ctx.setup_factory("float3[]")?;
//! ctx.begin_list();
//!     ctx.begin_tuple();
//!         ctx.append_value(Value::Float(1.0));
//!         ctx.append_value(Value::Float(2.0));
//!         ctx.append_value(Value::Float(3.0));
//!     ctx.end_tuple();
//! ctx.end_list();
//! let result = ctx.produce_value()?;
//! ```

use usd_tf::Token;

// ============================================================================
// Array Edit (forward declaration)
// ============================================================================

/// Array edit operation type.
///
/// Full implementation is in `values::typed::ArrayEditOp`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrayEditOp {
    /// Prepend to the beginning.
    Prepend,
    /// Append to the end.
    Append,
    /// Delete matching items.
    Delete,
    /// Add items (set union semantics).
    Add,
    /// Reorder items.
    Reorder,
    /// Write to a specific index.
    Write,
    /// Insert at a specific index.
    Insert,
    /// Erase at a specific index.
    Erase,
}

/// An array edit operation with its value.
///
/// Full implementation is in `values::typed::ArrayEdit`.
#[derive(Debug, Clone, PartialEq)]
pub struct ArrayEdit {
    /// The operation type.
    pub op: ArrayEditOp,
    /// The value to apply (stored as boxed Value to break recursion).
    pub value: Box<Value>,
    /// Target index (for write/insert/erase).
    pub index: Option<i64>,
}

// ============================================================================
// Parser Value
// ============================================================================

/// A value parsed from the text file.
///
/// This is the fundamental value type used during parsing. It can hold
/// atomic values that the lexer produces, matching the C++ `_Variant` type:
/// - `uint64_t` / `int64_t` / `double`
/// - `std::string` / `TfToken` / `SdfAssetPath`
///
/// Extended with compound types for the recursive descent parser:
/// - `Tuple` for `(a, b, c)`
/// - `List` for `[a, b, c]`
/// - `Dictionary` for `{ type key = value; }`
///
/// The `Get<T>()` functionality from C++ is provided via conversion methods.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Boolean value.
    Bool(bool),
    /// Unsigned 64-bit integer.
    UInt64(u64),
    /// Signed 64-bit integer.
    Int64(i64),
    /// Double-precision float.
    Double(f64),
    /// String value.
    String(String),
    /// Token value.
    Token(Token),
    /// Asset path value.
    AssetPath(String),
    /// SDF path reference value: `<path>`.
    Path(String),
    /// Tuple value: `(a, b, c)`.
    Tuple(Vec<Value>),
    /// List value: `[a, b, c]`.
    List(Vec<Value>),
    /// Dictionary value: `{ type key = value; }`.
    /// Each entry is (type_name, key, value).
    Dictionary(Vec<(String, String, Value)>),
    /// Array edit operation (boxed to avoid recursive size issues).
    ArrayEdit(Box<ArrayEdit>),
    /// Parsed reference list: `references = @asset@</path> (offset=N; scale=N; customData={...})`.
    /// Each entry is (asset_path, prim_path, layer_offset, layer_scale, custom_data).
    /// asset_path is empty string for internal references, prim_path is empty for default prim.
    ReferenceList(Vec<(String, String, f64, f64)>),
    /// Parsed payload list: `payload = @asset@</path> (offset=N; scale=N)`.
    /// Same layout as ReferenceList but no customData.
    PayloadList(Vec<(String, String, f64, f64)>),
    /// Path list for inherits/specializes arcs: `inherits = </Base>` or `[</A>, </B>]`.
    PathList(Vec<String>),
    /// Relocates map: `relocates = { </Old>: </New>, ... }`.
    /// Vec of (source_path, target_path) pairs.
    RelocatesMap(Vec<(String, String)>),
    /// Sublayer list with optional LayerOffset per entry.
    /// Each entry is (asset_path, offset, scale). Offset defaults to 0.0, scale to 1.0.
    SubLayerList(Vec<(String, f64, f64)>),
    /// Animation block marker for attribute default values.
    AnimationBlock,
    /// Explicit None value (e.g. `key = None`). Distinct from missing or empty.
    None,
}

impl Value {
    /// Attempts to get the value as a bool.
    ///
    /// Conversion rules (matching C++ `_GetImpl<bool>`):
    /// - Numbers: nonzero = true, zero = false
    /// - Strings: "yes", "true", "on", "1" = true; "no", "false", "off", "0" = false
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            Self::UInt64(n) => Some(*n != 0),
            Self::Int64(n) => Some(*n != 0),
            Self::Double(n) => Some(*n != 0.0),
            Self::String(s) | Self::AssetPath(s) | Self::Path(s) => bool_from_string(s),
            Self::Token(t) => bool_from_string(t.as_str()),
            Self::Tuple(_)
            | Self::List(_)
            | Self::Dictionary(_)
            | Self::ArrayEdit(_)
            | Self::ReferenceList(_)
            | Self::PayloadList(_)
            | Self::PathList(_)
            | Self::RelocatesMap(_)
            | Self::SubLayerList(_)
            | Self::AnimationBlock
            | Self::None => None,
        }
    }

    /// Attempts to get the value as an i64.
    ///
    /// Conversion rules (matching C++ `_GetImpl<T>` for integral types):
    /// - Unsigned integers are cast if in range
    /// - Doubles are cast if finite and in range
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Bool(b) => Some(if *b { 1 } else { 0 }),
            Self::Int64(n) => Some(*n),
            Self::UInt64(n) => {
                if *n <= i64::MAX as u64 {
                    Some(*n as i64)
                } else {
                    None
                }
            }
            Self::Double(n) => {
                if n.is_finite() && *n >= i64::MIN as f64 && *n <= i64::MAX as f64 {
                    Some(*n as i64)
                } else {
                    None
                }
            }
            Self::String(_)
            | Self::AssetPath(_)
            | Self::Path(_)
            | Self::Token(_)
            | Self::Tuple(_)
            | Self::List(_)
            | Self::Dictionary(_)
            | Self::ArrayEdit(_)
            | Self::ReferenceList(_)
            | Self::PayloadList(_)
            | Self::PathList(_)
            | Self::RelocatesMap(_)
            | Self::SubLayerList(_)
            | Self::AnimationBlock
            | Self::None => None,
        }
    }

    /// Attempts to get the value as a u64.
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Self::Bool(b) => Some(if *b { 1 } else { 0 }),
            Self::UInt64(n) => Some(*n),
            Self::Int64(n) => {
                if *n >= 0 {
                    Some(*n as u64)
                } else {
                    None
                }
            }
            Self::Double(n) => {
                if n.is_finite() && *n >= 0.0 && *n <= u64::MAX as f64 {
                    Some(*n as u64)
                } else {
                    None
                }
            }
            Self::String(_)
            | Self::AssetPath(_)
            | Self::Path(_)
            | Self::Token(_)
            | Self::Tuple(_)
            | Self::List(_)
            | Self::Dictionary(_)
            | Self::ArrayEdit(_)
            | Self::ReferenceList(_)
            | Self::PayloadList(_)
            | Self::PathList(_)
            | Self::RelocatesMap(_)
            | Self::SubLayerList(_)
            | Self::AnimationBlock
            | Self::None => None,
        }
    }

    /// Attempts to get the value as an f64.
    ///
    /// Conversion rules (matching C++ `_GetImpl<T>` for floating point):
    /// - Integers are cast to double
    /// - Strings "inf", "-inf", "nan" are converted to special values
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            Self::Double(n) => Some(*n),
            Self::Int64(n) => Some(*n as f64),
            Self::UInt64(n) => Some(*n as f64),
            Self::String(s) | Self::AssetPath(s) | Self::Path(s) => float_from_string(s),
            Self::Token(t) => float_from_string(t.as_str()),
            Self::Tuple(_)
            | Self::List(_)
            | Self::Dictionary(_)
            | Self::ArrayEdit(_)
            | Self::ReferenceList(_)
            | Self::PayloadList(_)
            | Self::PathList(_)
            | Self::RelocatesMap(_)
            | Self::SubLayerList(_)
            | Self::AnimationBlock
            | Self::None => None,
        }
    }

    /// Attempts to get the value as a string.
    pub fn as_string(&self) -> Option<&str> {
        match self {
            Self::String(s) | Self::AssetPath(s) | Self::Path(s) => Some(s),
            Self::Token(t) => Some(t.as_str()),
            Self::Bool(_)
            | Self::UInt64(_)
            | Self::Int64(_)
            | Self::Double(_)
            | Self::Tuple(_)
            | Self::List(_)
            | Self::Dictionary(_)
            | Self::ArrayEdit(_)
            | Self::ReferenceList(_)
            | Self::PayloadList(_)
            | Self::PathList(_)
            | Self::RelocatesMap(_)
            | Self::SubLayerList(_)
            | Self::AnimationBlock
            | Self::None => None,
        }
    }

    /// Returns the value as a string, converting if necessary.
    pub fn to_display_string(&self) -> String {
        match self {
            Self::Bool(b) => b.to_string(),
            Self::UInt64(n) => n.to_string(),
            Self::Int64(n) => n.to_string(),
            Self::Double(n) => {
                if n.is_infinite() {
                    if *n > 0.0 {
                        "inf".to_string()
                    } else {
                        "-inf".to_string()
                    }
                } else if n.is_nan() {
                    "nan".to_string()
                } else {
                    n.to_string()
                }
            }
            Self::String(s) | Self::AssetPath(s) => s.clone(),
            Self::Path(s) => format!("<{}>", s),
            Self::Token(t) => t.as_str().to_string(),
            Self::Tuple(elems) => {
                let inner: Vec<_> = elems.iter().map(|v| v.to_display_string()).collect();
                format!("({})", inner.join(", "))
            }
            Self::List(elems) => {
                let inner: Vec<_> = elems.iter().map(|v| v.to_display_string()).collect();
                format!("[{}]", inner.join(", "))
            }
            Self::Dictionary(entries) => {
                let inner: Vec<_> = entries
                    .iter()
                    .map(|(t, k, v)| format!("{} {} = {}", t, k, v.to_display_string()))
                    .collect();
                format!("{{ {} }}", inner.join("; "))
            }
            Self::ArrayEdit(edit) => {
                format!("{:?} {:?}", edit.op, edit.value)
            }
            Self::ReferenceList(refs) => {
                let items: Vec<_> = refs
                    .iter()
                    .map(|(asset, path, off, scale)| {
                        format!("@{}@<{}> (offset={}; scale={})", asset, path, off, scale)
                    })
                    .collect();
                format!("[{}]", items.join(", "))
            }
            Self::PayloadList(payloads) => {
                let items: Vec<_> = payloads
                    .iter()
                    .map(|(asset, path, off, scale)| {
                        format!("@{}@<{}> (offset={}; scale={})", asset, path, off, scale)
                    })
                    .collect();
                format!("[{}]", items.join(", "))
            }
            Self::PathList(paths) => {
                let items: Vec<_> = paths.iter().map(|p| format!("<{}>", p)).collect();
                format!("[{}]", items.join(", "))
            }
            Self::RelocatesMap(pairs) => {
                let items: Vec<_> = pairs
                    .iter()
                    .map(|(src, dst)| format!("<{}>: <{}>", src, dst))
                    .collect();
                format!("{{ {} }}", items.join(", "))
            }
            Self::SubLayerList(items) => {
                let parts: Vec<_> = items
                    .iter()
                    .map(|(path, off, scale)| {
                        if *off == 0.0 && *scale == 1.0 {
                            format!("@{}@", path)
                        } else {
                            format!("@{}@ (offset = {}; scale = {})", path, off, scale)
                        }
                    })
                    .collect();
                format!("[{}]", parts.join(", "))
            }
            Self::AnimationBlock => "AnimationBlock".to_string(),
            Self::None => "None".to_string(),
        }
    }
}

/// Converts a string to bool (matching C++ `Sdf_BoolFromString`).
///
/// Accepts case insensitive: "yes", "no", "false", "true", "0", "1", "on", "off".
fn bool_from_string(s: &str) -> Option<bool> {
    match s.to_lowercase().as_str() {
        "true" | "yes" | "on" | "1" => Some(true),
        "false" | "no" | "off" | "0" => Some(false),
        _ => None,
    }
}

/// Converts special float strings to f64.
fn float_from_string(s: &str) -> Option<f64> {
    match s {
        "inf" => Some(f64::INFINITY),
        "-inf" => Some(f64::NEG_INFINITY),
        "nan" => Some(f64::NAN),
        _ => None,
    }
}

// ============================================================================
// Tuple Dimensions
// ============================================================================

/// Dimensions of a tuple type (e.g., Vec3 = 3, Matrix4 = [4,4]).
///
/// Matches C++ `SdfTupleDimensions`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TupleDimensions {
    /// Dimension sizes. Up to 2 dimensions supported.
    pub d: [usize; 2],
    /// Number of dimensions (0, 1, or 2).
    pub size: usize,
}

impl TupleDimensions {
    /// Creates scalar dimensions (no tuple).
    #[inline]
    pub const fn scalar() -> Self {
        Self { d: [0, 0], size: 0 }
    }

    /// Creates 1D tuple dimensions (e.g., Vec3 = 3).
    #[inline]
    pub const fn d1(n: usize) -> Self {
        Self { d: [n, 0], size: 1 }
    }

    /// Creates 2D tuple dimensions (e.g., Matrix4 = [4,4]).
    #[inline]
    pub const fn d2(n: usize, m: usize) -> Self {
        Self { d: [n, m], size: 2 }
    }

    /// Returns the total number of elements.
    #[inline]
    pub fn total(&self) -> usize {
        match self.size {
            0 => 1,
            1 => self.d[0],
            2 => self.d[0] * self.d[1],
            _ => 0,
        }
    }
}

// ============================================================================
// Value Factory
// ============================================================================

/// Factory function type for producing final values from parsed data.
///
/// In the full implementation, this would produce VtValue. For now,
/// we return a Vec<Value> representing the shaped data.
pub type ValueFactoryFunc =
    fn(shape: &[usize], values: &[Value], index: &mut usize) -> Result<Vec<Value>, String>;

/// Value factory information for a type.
///
/// Matches C++ `Sdf_ParserHelpers::ValueFactory`.
#[derive(Clone)]
pub struct ValueFactory {
    /// Type name (e.g., "float3", "double[]").
    pub type_name: String,
    /// Tuple dimensions for this type.
    pub dimensions: TupleDimensions,
    /// Whether this is a shaped (array) type.
    pub is_shaped: bool,
    /// Factory function to produce VtValues.
    pub func: Option<ValueFactoryFunc>,
}

impl Default for ValueFactory {
    fn default() -> Self {
        Self {
            type_name: String::new(),
            dimensions: TupleDimensions::scalar(),
            is_shaped: false,
            func: None,
        }
    }
}

// ============================================================================
// Produced Value
// ============================================================================

/// The result of parsing a value.
///
/// Contains all the information needed to construct a final VtValue:
/// - Type name for factory lookup
/// - Shape for arrays
/// - Raw parsed values
/// - Tuple dimensions for composite types
#[derive(Debug, Clone)]
pub struct ProducedValue {
    /// Type name (e.g., "float3", "int[]").
    pub type_name: String,
    /// Shape of the value (for arrays).
    pub shape: Vec<usize>,
    /// Accumulated atomic values.
    pub values: Vec<Value>,
    /// Whether this is a shaped (array) type.
    pub is_shaped: bool,
    /// Tuple dimensions for composite types.
    pub tuple_dimensions: TupleDimensions,
}

impl ProducedValue {
    /// Returns true if no values were parsed.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Returns the number of values.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns the single value if exactly one was parsed.
    #[must_use]
    pub fn single_value(&self) -> Option<&Value> {
        if self.values.len() == 1 {
            self.values.first()
        } else {
            None
        }
    }

    /// Converts to a single i64 if possible.
    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        self.single_value()?.as_i64()
    }

    /// Converts to a single f64 if possible.
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        self.single_value()?.as_f64()
    }

    /// Converts to a single string if possible.
    #[must_use]
    pub fn as_string(&self) -> Option<&str> {
        self.single_value()?.as_string()
    }

    /// Converts to a single bool if possible.
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        self.single_value()?.as_bool()
    }
}

// ============================================================================
// Value Context
// ============================================================================

/// Context for parsing nested values (arrays and tuples).
///
/// This is a direct port of `Sdf_ParserValueContext` from C++.
/// It manages state during value parsing:
/// - Tracks nesting depth (dim) and shape
/// - Accumulates atomic values
/// - Validates array rectangularity
/// - Produces final VtValues
#[derive(Debug)]
pub struct ValueContext {
    // ========================================================================
    // Nesting State
    // ========================================================================
    /// Current nesting dimension (0 = top level).
    pub dim: i32,

    /// Shape of the value being built.
    pub shape: Vec<usize>,

    /// Current tuple depth.
    pub tuple_depth: i32,

    /// Tuple dimensions for the type being parsed.
    pub tuple_dimensions: TupleDimensions,

    /// Accumulated atomic values.
    pub values: Vec<Value>,

    /// Working shape during parsing.
    pub working_shape: Vec<usize>,

    /// The dim at which we got our first AppendValue.
    /// If we get subsequent pushes where dim != push_dim, it's an error.
    /// Initially -1 to indicate we have never appended anything.
    pub push_dim: i32,

    // ========================================================================
    // Factory State
    // ========================================================================
    /// Type name being parsed.
    pub value_type_name: String,

    /// Whether the type is valid/recognized.
    pub value_type_is_valid: bool,

    /// Last type name (for caching).
    pub last_type_name: String,

    /// Factory function for the type.
    pub value_func: Option<ValueFactoryFunc>,

    /// Whether the type is shaped (array).
    pub value_is_shaped: bool,

    /// Tuple dimensions for the type.
    pub value_tuple_dimensions: TupleDimensions,

    // ========================================================================
    // String Recording
    // ========================================================================
    /// Whether we need a comma in recorded string.
    need_comma: bool,

    /// Whether we're recording string representation.
    is_recording_string: bool,

    /// Recorded string representation.
    recorded_string: String,
}

impl ValueContext {
    /// Creates a new empty value context.
    #[must_use]
    pub fn new() -> Self {
        Self {
            dim: 0,
            shape: Vec::new(),
            tuple_depth: 0,
            tuple_dimensions: TupleDimensions::scalar(),
            values: Vec::new(),
            working_shape: Vec::new(),
            push_dim: -1,
            value_type_name: String::new(),
            value_type_is_valid: false,
            last_type_name: String::new(),
            value_func: None,
            value_is_shaped: false,
            value_tuple_dimensions: TupleDimensions::scalar(),
            need_comma: false,
            is_recording_string: false,
            recorded_string: String::new(),
        }
    }

    /// Clears all state for reuse.
    pub fn clear(&mut self) {
        self.dim = 0;
        self.shape.clear();
        self.tuple_depth = 0;
        self.tuple_dimensions = TupleDimensions::scalar();
        self.values.clear();
        self.working_shape.clear();
        self.push_dim = -1;
        self.value_type_name.clear();
        self.value_type_is_valid = false;
        self.value_func = None;
        self.value_is_shaped = false;
        self.value_tuple_dimensions = TupleDimensions::scalar();
        self.need_comma = false;
        self.is_recording_string = false;
        self.recorded_string.clear();
    }

    /// Sets up the factory for a given type name.
    ///
    /// Returns true if the type is valid and recognized.
    /// Uses ValueTypeRegistry for type lookup.
    pub fn setup_factory(&mut self, type_name: &str) -> bool {
        self.value_type_name = type_name.to_string();

        // Look up type in registry
        let registry = crate::ValueTypeRegistry::instance();
        let value_type = registry.find_type(type_name);

        if value_type.is_valid() {
            // Use registry data for dimensions and array status
            self.value_type_is_valid = true;
            self.value_is_shaped = value_type.is_array();

            // Get dimensions from registry
            let dims = value_type.dimensions();
            self.value_tuple_dimensions = TupleDimensions {
                size: dims.size,
                d: dims.d,
            };
        } else {
            // Fallback for types not in registry
            self.value_type_is_valid = !type_name.is_empty();
            self.value_is_shaped = type_name.ends_with("[]");

            // Determine tuple dimensions from type name pattern
            self.value_tuple_dimensions = match type_name {
                s if s.starts_with("float2")
                    || s.starts_with("double2")
                    || s.starts_with("int2")
                    || s.starts_with("half2") =>
                {
                    TupleDimensions::d1(2)
                }
                s if s.starts_with("float3")
                    || s.starts_with("double3")
                    || s.starts_with("int3")
                    || s.starts_with("half3")
                    || s.starts_with("color3")
                    || s.starts_with("point3")
                    || s.starts_with("normal3")
                    || s.starts_with("vector3") =>
                {
                    TupleDimensions::d1(3)
                }
                s if s.starts_with("float4")
                    || s.starts_with("double4")
                    || s.starts_with("int4")
                    || s.starts_with("half4")
                    || s.starts_with("color4")
                    || s.starts_with("quath")
                    || s.starts_with("quatf")
                    || s.starts_with("quatd") =>
                {
                    TupleDimensions::d1(4)
                }
                s if s.starts_with("matrix2") => TupleDimensions::d2(2, 2),
                s if s.starts_with("matrix3") => TupleDimensions::d2(3, 3),
                s if s.starts_with("matrix4") => TupleDimensions::d2(4, 4),
                _ => TupleDimensions::scalar(),
            };
        }

        self.value_type_is_valid
    }

    /// Appends an atomic value.
    ///
    /// This is called for each number, string, or other atomic value.
    pub fn append_value(&mut self, value: Value) {
        // Check dimension consistency
        if self.push_dim == -1 {
            self.push_dim = self.dim;
        }

        // Record if needed
        if self.is_recording_string {
            if self.need_comma {
                self.recorded_string.push_str(", ");
            }
            self.recorded_string.push_str(&value.to_display_string());
            self.need_comma = true;
        }

        self.values.push(value);
    }

    /// Called before each list (corresponds to '[' token).
    pub fn begin_list(&mut self) {
        self.dim += 1;

        if self.is_recording_string {
            if self.need_comma {
                self.recorded_string.push_str(", ");
            }
            self.recorded_string.push('[');
            self.need_comma = false;
        }

        // Grow working shape if needed
        while self.working_shape.len() < self.dim as usize {
            self.working_shape.push(0);
        }
    }

    /// Called after each list (corresponds to ']' token).
    pub fn end_list(&mut self) {
        if self.is_recording_string {
            self.recorded_string.push(']');
            self.need_comma = true;
        }

        // Update shape
        let dim_idx = (self.dim - 1) as usize;
        if dim_idx < self.working_shape.len() {
            let count = self.working_shape[dim_idx];
            if self.shape.len() <= dim_idx {
                self.shape.push(count);
            }
            self.working_shape[dim_idx] = 0;
        }

        self.dim -= 1;

        // Increment parent dimension count
        if self.dim > 0 {
            let parent_idx = (self.dim - 1) as usize;
            if parent_idx < self.working_shape.len() {
                self.working_shape[parent_idx] += 1;
            }
        }
    }

    /// Called before each tuple (corresponds to '(' token).
    pub fn begin_tuple(&mut self) {
        self.tuple_depth += 1;

        if self.is_recording_string {
            if self.need_comma {
                self.recorded_string.push_str(", ");
            }
            self.recorded_string.push('(');
            self.need_comma = false;
        }
    }

    /// Called after each tuple (corresponds to ')' token).
    pub fn end_tuple(&mut self) {
        if self.is_recording_string {
            self.recorded_string.push(')');
            self.need_comma = true;
        }

        self.tuple_depth -= 1;

        // Update working shape
        if self.dim > 0 {
            let dim_idx = (self.dim - 1) as usize;
            if dim_idx < self.working_shape.len() {
                self.working_shape[dim_idx] += 1;
            }
        }
    }

    /// Produces the final value from accumulated data.
    ///
    /// Returns the accumulated values. In the full implementation,
    /// this would use the factory function to produce a properly typed VtValue.
    pub fn produce_value(&mut self) -> Result<ProducedValue, String> {
        let result = ProducedValue {
            type_name: self.value_type_name.clone(),
            shape: self.shape.clone(),
            values: std::mem::take(&mut self.values),
            is_shaped: self.value_is_shaped,
            tuple_dimensions: self.value_tuple_dimensions,
        };

        self.clear();
        Ok(result)
    }

    // ========================================================================
    // String Recording
    // ========================================================================

    /// Starts recording a string representation of parsed values.
    pub fn start_recording_string(&mut self) {
        self.is_recording_string = true;
        self.recorded_string.clear();
        self.need_comma = false;
    }

    /// Stops recording the string representation.
    pub fn stop_recording_string(&mut self) {
        self.is_recording_string = false;
    }

    /// Returns whether we're currently recording.
    #[must_use]
    pub fn is_recording_string(&self) -> bool {
        self.is_recording_string
    }

    /// Returns the recorded string.
    #[must_use]
    pub fn get_recorded_string(&self) -> &str {
        &self.recorded_string
    }

    /// Sets the recorded string (override hook).
    pub fn set_recorded_string(&mut self, text: impl Into<String>) {
        self.recorded_string = text.into();
    }
}

impl Default for ValueContext {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_as_bool() {
        assert_eq!(Value::Int64(0).as_bool(), Some(false));
        assert_eq!(Value::Int64(1).as_bool(), Some(true));
        assert_eq!(Value::Int64(-1).as_bool(), Some(true));
        assert_eq!(Value::Double(0.0).as_bool(), Some(false));
        assert_eq!(Value::Double(1.5).as_bool(), Some(true));
        assert_eq!(Value::String("true".into()).as_bool(), Some(true));
        assert_eq!(Value::String("false".into()).as_bool(), Some(false));
        assert_eq!(Value::String("yes".into()).as_bool(), Some(true));
        assert_eq!(Value::String("no".into()).as_bool(), Some(false));
    }

    #[test]
    fn test_value_as_f64() {
        assert_eq!(Value::Double(3.14).as_f64(), Some(3.14));
        assert_eq!(Value::Int64(42).as_f64(), Some(42.0));
        assert_eq!(Value::String("inf".into()).as_f64(), Some(f64::INFINITY));
        assert_eq!(
            Value::String("-inf".into()).as_f64(),
            Some(f64::NEG_INFINITY)
        );
        assert!(Value::String("nan".into()).as_f64().unwrap().is_nan());
    }

    #[test]
    fn test_tuple_dimensions() {
        let scalar = TupleDimensions::scalar();
        assert_eq!(scalar.total(), 1);

        let vec3 = TupleDimensions::d1(3);
        assert_eq!(vec3.total(), 3);

        let mat4 = TupleDimensions::d2(4, 4);
        assert_eq!(mat4.total(), 16);
    }

    #[test]
    fn test_value_context_basic() {
        let mut ctx = ValueContext::new();
        ctx.setup_factory("float");

        ctx.append_value(Value::Double(3.14));

        let result = ctx.produce_value().unwrap();
        assert!(!result.is_empty());
        assert_eq!(result.len(), 1);
        assert!((result.as_f64().unwrap() - 3.14).abs() < 0.001);
    }

    #[test]
    fn test_value_context_list() {
        let mut ctx = ValueContext::new();
        ctx.setup_factory("int[]");

        ctx.begin_list();
        ctx.append_value(Value::Int64(1));
        ctx.append_value(Value::Int64(2));
        ctx.append_value(Value::Int64(3));
        ctx.end_list();

        assert_eq!(ctx.values.len(), 3);
    }

    #[test]
    fn test_value_context_recording() {
        let mut ctx = ValueContext::new();
        ctx.start_recording_string();

        ctx.begin_list();
        ctx.append_value(Value::Int64(1));
        ctx.append_value(Value::Int64(2));
        ctx.end_list();

        ctx.stop_recording_string();

        let recorded = ctx.get_recorded_string();
        assert!(recorded.contains('['));
        assert!(recorded.contains('1'));
        assert!(recorded.contains('2'));
        assert!(recorded.contains(']'));
    }
}
