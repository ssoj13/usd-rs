//! TypeSpec — compiler's extended type representation.
//!
//! `TypeSpec` wraps a `TypeDesc` (OIIO simple type) and adds:
//! - A structure ID (0 = not a struct, >=1 for registered structs)
//! - A closure flag (is this a closure type?)
//!
//! This mirrors the C++ `OSL::pvt::TypeSpec` from `osl_pvt.h`.

use std::fmt;
use std::sync::Mutex;

use crate::typedesc::TypeDesc;
use crate::ustring::UString;

/// Description of a single structure field.
#[derive(Debug, Clone)]
pub struct FieldSpec {
    pub type_spec: TypeSpec,
    pub name: UString,
}

/// Describes the layout of an OSL `struct`.
#[derive(Debug, Clone)]
pub struct StructSpec {
    /// Name of the struct (may not be unique across scopes).
    pub name: UString,
    /// Scope ID where the struct was defined.
    pub scope: i32,
    /// Ordered list of fields.
    pub fields: Vec<FieldSpec>,
}

impl StructSpec {
    pub fn new(name: UString, scope: i32) -> Self {
        Self {
            name,
            scope,
            fields: Vec::new(),
        }
    }

    pub fn add_field(&mut self, type_spec: TypeSpec, name: UString) {
        self.fields.push(FieldSpec { type_spec, name });
    }

    pub fn num_fields(&self) -> usize {
        self.fields.len()
    }

    pub fn field(&self, index: usize) -> &FieldSpec {
        &self.fields[index]
    }

    /// Look up a field by name, returns its index or `None`.
    pub fn lookup_field(&self, name: UString) -> Option<usize> {
        self.fields.iter().position(|f| f.name == name)
    }

    /// Compute the byte offset and size of a field within the struct.
    /// Returns `(byte_offset, field_size_bytes)`.
    /// Fields are laid out contiguously in declaration order with 4-byte alignment.
    pub fn field_byte_offset(&self, field_index: usize) -> (usize, usize) {
        let mut offset = 0usize;
        for (i, f) in self.fields.iter().enumerate() {
            let sz = f.type_spec.simpletype().size();
            // Align to 4 bytes
            if offset % 4 != 0 {
                offset += 4 - (offset % 4);
            }
            if i == field_index {
                return (offset, sz);
            }
            offset += sz;
        }
        (0, 0)
    }

    /// Total byte size of the struct (all fields, with alignment).
    pub fn total_byte_size(&self) -> usize {
        let mut offset = 0usize;
        for f in &self.fields {
            if offset % 4 != 0 {
                offset += 4 - (offset % 4);
            }
            offset += f.type_spec.simpletype().size();
        }
        offset
    }

    /// Return the mangled name (scope-prefixed).
    pub fn mangled(&self) -> String {
        format!("___struct_{}_s{}", self.name, self.scope)
    }
}

// ---------------------------------------------------------------------------
// Global struct registry
// ---------------------------------------------------------------------------

// TODO: This registry is per-process (a single static), not per-ShadingSystem.
// C++ OSL has the same limitation. If multiple ShadingSystems run in one
// process they share the same struct table, which can cause name collisions.
static STRUCT_LIST: Mutex<Vec<Option<StructSpec>>> = Mutex::new(Vec::new());

fn with_struct_list<F, R>(f: F) -> R
where
    F: FnOnce(&mut Vec<Option<StructSpec>>) -> R,
{
    let mut list = STRUCT_LIST.lock().unwrap();
    f(&mut list)
}

/// Register a new struct and return its ID (>= 1).
pub fn register_struct(spec: StructSpec) -> i32 {
    with_struct_list(|list| {
        // ID 0 is reserved for "not a struct".
        if list.is_empty() {
            list.push(None); // slot 0
        }
        let id = list.len() as i32;
        list.push(Some(spec));
        id
    })
}

/// Look up a struct by ID. Returns a cloned StructSpec.
pub fn get_struct(id: i32) -> Option<StructSpec> {
    with_struct_list(|list| list.get(id as usize).and_then(|s| s.clone()))
}

/// Find a struct by name. Returns its ID, or 0 if not found.
pub fn find_struct_by_name(name: UString) -> i32 {
    with_struct_list(|list| {
        for (i, entry) in list.iter().enumerate() {
            if let Some(s) = entry {
                if s.name == name {
                    return i as i32;
                }
            }
        }
        0
    })
}

// ---------------------------------------------------------------------------
// TypeSpec
// ---------------------------------------------------------------------------

/// The compiler's extended type representation.
///
/// Wraps [`TypeDesc`] with additional struct ID and closure flag.
#[derive(Clone, Copy)]
pub struct TypeSpec {
    /// The underlying simple type descriptor.
    simple: TypeDesc,
    /// Structure ID (0 = not a struct, >= 1 for struct types).
    structure: i16,
    /// Is this a closure type?
    closure: bool,
}

impl TypeSpec {
    /// Unknown / uninitialized type.
    pub const UNKNOWN: Self = Self {
        simple: TypeDesc::UNKNOWN,
        structure: 0,
        closure: false,
    };

    /// Void type.
    pub const VOID: Self = Self {
        simple: TypeDesc::NONE,
        structure: 0,
        closure: false,
    };

    /// Construct a wildcard sentinel (discriminant in the structure field < 0).
    /// Only used by WILDCARD_* constants in typecheck.rs; never appears in real types.
    pub const fn sentinel(tag: i16) -> Self {
        Self {
            simple: TypeDesc::UNKNOWN,
            structure: tag,
            closure: false,
        }
    }

    /// Construct from a simple TypeDesc.
    pub const fn from_simple(simple: TypeDesc) -> Self {
        Self {
            simple,
            structure: 0,
            closure: false,
        }
    }

    /// Construct from a simple TypeDesc (alias for `from_simple`).
    pub const fn new(simple: TypeDesc) -> Self {
        Self::from_simple(simple)
    }

    /// Construct an array type from a base TypeDesc and array length.
    /// Use -1 for unsized arrays.
    pub const fn new_array(base: TypeDesc, arraylen: i32) -> Self {
        Self {
            simple: TypeDesc {
                basetype: base.basetype,
                aggregate: base.aggregate,
                vecsemantics: base.vecsemantics,
                reserved: base.reserved,
                arraylen,
            },
            structure: 0,
            closure: false,
        }
    }

    /// Construct a closure type (alias for `closure`).
    pub const fn new_closure(simple: TypeDesc) -> Self {
        Self::closure(simple)
    }

    /// Construct a closure type.
    pub const fn closure(_simple: TypeDesc) -> Self {
        Self {
            // Closures are represented as pointers.
            simple: TypeDesc::PTR,
            structure: 0,
            closure: true,
        }
    }

    /// Construct an array-of-closures type with the given length.
    /// Use -1 for unsized arrays. Builds on TypeDesc::PTR (same as scalar closure)
    /// but sets arraylen so is_closure_array() returns true.
    pub const fn closure_array(arraylen: i32) -> Self {
        Self {
            simple: TypeDesc {
                basetype: TypeDesc::PTR.basetype,
                aggregate: TypeDesc::PTR.aggregate,
                vecsemantics: TypeDesc::PTR.vecsemantics,
                reserved: TypeDesc::PTR.reserved,
                arraylen,
            },
            structure: 0,
            closure: true,
        }
    }

    /// Construct a struct type.
    pub const fn structure(struct_id: i16, arraylen: i32) -> Self {
        Self {
            simple: TypeDesc {
                basetype: 0, // UNKNOWN for structs
                aggregate: 1,
                vecsemantics: 0,
                reserved: 0,
                arraylen,
            },
            structure: struct_id,
            closure: false,
        }
    }

    // -- Accessors --

    /// Get the underlying simple type.
    #[inline]
    pub const fn simpletype(&self) -> TypeDesc {
        self.simple
    }

    /// Is this type unknown/uninitialized?
    #[inline]
    pub fn is_unknown(&self) -> bool {
        self.simple == TypeDesc::UNKNOWN && self.structure == 0 && !self.closure
    }

    /// Is this a closure? (scalar closure, not array of closures)
    #[inline]
    pub fn is_closure(&self) -> bool {
        self.closure && !self.is_array()
    }

    /// Is this an array of closures?
    #[inline]
    pub fn is_closure_array(&self) -> bool {
        self.closure && self.is_array()
    }

    /// Is this closure-based (scalar or array)?
    #[inline]
    pub fn is_closure_based(&self) -> bool {
        self.closure
    }

    /// Is this a single struct (not an array of structs)?
    #[inline]
    pub fn is_structure(&self) -> bool {
        self.structure > 0 && !self.is_array()
    }

    /// Is this an array of structs?
    #[inline]
    pub fn is_structure_array(&self) -> bool {
        self.structure > 0 && self.is_array()
    }

    /// Is this struct-based (scalar or array)?
    #[inline]
    pub fn is_structure_based(&self) -> bool {
        self.structure > 0
    }

    /// Get the structure ID (0 if not a struct).
    #[inline]
    pub fn structure_id(&self) -> i16 {
        self.structure
    }

    /// Is this an array (simple or struct)?
    #[inline]
    pub fn is_array(&self) -> bool {
        self.simple.arraylen != 0
    }

    /// Is this an unsized array?
    #[inline]
    pub fn is_unsized_array(&self) -> bool {
        self.simple.arraylen < 0
    }

    /// Is this a sized array?
    #[inline]
    pub fn is_sized_array(&self) -> bool {
        self.simple.arraylen > 0
    }

    /// Array length (0 if not an array).
    #[inline]
    pub fn arraylength(&self) -> i32 {
        self.simple.arraylen
    }

    /// Number of elements (max(1, arraylength)).
    #[inline]
    pub fn numelements(&self) -> i32 {
        std::cmp::max(1, self.simple.arraylen)
    }

    /// Make this into an array of the given length.
    pub fn make_array(&mut self, len: i32) {
        self.simple.arraylen = len;
    }

    /// Return the element type (strip array).
    pub fn elementtype(&self) -> Self {
        let mut t = *self;
        t.make_array(0);
        t
    }

    /// Encoded type code for polymorphic function signatures.
    /// C++ typespec.cpp:215-244 TypeSpec::code_from_type().
    pub fn code_from_type(&self) -> String {
        if self.is_structure() || self.is_structure_array() {
            return format!("S{}", self.structure);
        }
        if self.is_closure() || self.is_closure_array() {
            return "C".to_string();
        }
        let elem = self.elementtype().simpletype();
        let ch = if elem == TypeDesc::INT {
            'i'
        } else if elem == TypeDesc::FLOAT {
            'f'
        } else if elem == TypeDesc::COLOR {
            'c'
        } else if elem == TypeDesc::POINT {
            'p'
        } else if elem == TypeDesc::VECTOR {
            'v'
        } else if elem == TypeDesc::NORMAL {
            'n'
        } else if elem == TypeDesc::MATRIX {
            'm'
        } else if elem == TypeDesc::STRING {
            's'
        } else if elem == TypeDesc::NONE {
            'x'
        } else {
            'x'
        };
        ch.to_string()
    }

    // -- Scalar predicates --

    #[inline]
    pub fn is_int(&self) -> bool {
        self.simple == TypeDesc::INT && !self.closure
    }

    #[inline]
    pub fn is_float(&self) -> bool {
        self.simple == TypeDesc::FLOAT && !self.closure
    }

    #[inline]
    pub fn is_color(&self) -> bool {
        self.simple == TypeDesc::COLOR && !self.closure
    }

    #[inline]
    pub fn is_point(&self) -> bool {
        self.simple == TypeDesc::POINT && !self.closure
    }

    #[inline]
    pub fn is_vector(&self) -> bool {
        self.simple == TypeDesc::VECTOR && !self.closure
    }

    #[inline]
    pub fn is_normal(&self) -> bool {
        self.simple == TypeDesc::NORMAL && !self.closure
    }

    #[inline]
    pub fn is_string(&self) -> bool {
        self.simple == TypeDesc::STRING && !self.closure
    }

    #[inline]
    pub fn is_string_based(&self) -> bool {
        self.simple.basetype == crate::typedesc::BaseType::String as u8
    }

    #[inline]
    pub fn is_int_based(&self) -> bool {
        self.simple.basetype == crate::typedesc::BaseType::Int32 as u8
    }

    #[inline]
    pub fn is_float_based(&self) -> bool {
        self.simple.basetype == crate::typedesc::BaseType::Float as u8 && !self.closure
    }

    #[inline]
    pub fn is_void(&self) -> bool {
        self.simple == TypeDesc::NONE
    }

    /// Is this a triple (color, point, vector, or normal) — non-array?
    #[inline]
    pub fn is_triple(&self) -> bool {
        !self.closure
            && self.simple.aggregate == crate::typedesc::Aggregate::Vec3 as u8
            && self.simple.basetype == crate::typedesc::BaseType::Float as u8
            && !self.is_array()
    }

    /// Is this based on a triple (ok for array/closure)?
    #[inline]
    pub fn is_triple_based(&self) -> bool {
        !self.closure
            && self.simple.aggregate == crate::typedesc::Aggregate::Vec3 as u8
            && self.simple.basetype == crate::typedesc::BaseType::Float as u8
    }

    /// Is this a triple or float (non-array)?
    #[inline]
    pub fn is_triple_or_float(&self) -> bool {
        !self.closure
            && (self.simple.aggregate == crate::typedesc::Aggregate::Vec3 as u8
                || self.simple.aggregate == crate::typedesc::Aggregate::Scalar as u8)
            && self.simple.basetype == crate::typedesc::BaseType::Float as u8
            && !self.is_array()
    }

    /// Is this a simple numeric type (float or int-based, non-closure, non-array)?
    #[inline]
    pub fn is_numeric(&self) -> bool {
        !self.closure
            && !self.is_array()
            && (self.simple.basetype == crate::typedesc::BaseType::Float as u8
                || self.simple.basetype == crate::typedesc::BaseType::Int32 as u8)
    }

    #[inline]
    pub fn is_scalarnum(&self) -> bool {
        self.is_numeric() && self.simple.aggregate == crate::typedesc::Aggregate::Scalar as u8
    }

    /// Is it a simple matrix (non-array, non-closure)?
    #[inline]
    pub fn is_matrix(&self) -> bool {
        self.simple == TypeDesc::MATRIX && !self.closure
    }

    /// Is it a color closure?
    #[inline]
    pub fn is_color_closure(&self) -> bool {
        self.is_closure()
    }

    /// Return the aggregate of the underlying simple type.
    #[inline]
    pub fn aggregate(&self) -> crate::typedesc::Aggregate {
        crate::typedesc::Aggregate::from_u8(self.simple.aggregate)
    }

    /// Is it a simple int or float (scalar, non-array, non-closure)?
    #[inline]
    pub fn is_int_or_float(&self) -> bool {
        self.is_scalarnum()
    }

    /// Is it a vector-like triple (point, vector, or normal, but NOT color)?
    #[inline]
    pub fn is_vectriple(&self) -> bool {
        !self.closure
            && (self.simple == TypeDesc::POINT
                || self.simple == TypeDesc::VECTOR
                || self.simple == TypeDesc::NORMAL)
    }

    /// Is it based on a vector-like triple (point, vector, or normal)?
    /// Array-ok version.
    #[inline]
    pub fn is_vectriple_based(&self) -> bool {
        let elem = self.simple.elementtype();
        elem == TypeDesc::POINT || elem == TypeDesc::VECTOR || elem == TypeDesc::NORMAL
    }

    /// Are two TypeSpecs equivalent for type-checking purposes?
    /// Vector-like triples (point, vector, normal) are equivalent to each other.
    pub fn equivalent(&self, other: &Self) -> bool {
        if self == other {
            return true;
        }
        // Closures: only equivalent to other closures
        if self.closure || other.closure {
            return self.closure == other.closure;
        }
        // Structs must have same ID
        if self.structure != 0 || other.structure != 0 {
            return self.structure == other.structure
                && self.simple.arraylen == other.simple.arraylen;
        }
        // Vector-like triples are equivalent to each other
        if self.is_triple() && other.is_triple() {
            return true;
        }
        false
    }

    /// Is `src` assignable to `self` (as dst)?
    pub fn assignable_from(&self, src: &Self) -> bool {
        if self.closure || src.closure {
            return self.closure && src.closure;
        }
        self.equivalent(src)
            || (self.is_float_based() && !self.is_array() && (src.is_float() || src.is_int()))
    }
}

impl Default for TypeSpec {
    fn default() -> Self {
        Self::UNKNOWN
    }
}

impl PartialEq for TypeSpec {
    fn eq(&self, other: &Self) -> bool {
        self.simple == other.simple
            && self.structure == other.structure
            && self.closure == other.closure
    }
}

impl Eq for TypeSpec {}

impl From<TypeDesc> for TypeSpec {
    fn from(td: TypeDesc) -> Self {
        Self::from_simple(td)
    }
}

impl fmt::Debug for TypeSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.closure {
            write!(f, "closure {}", self.simple)
        } else if self.structure > 0 {
            write!(f, "struct#{}", self.structure)
        } else {
            write!(f, "{}", self.simple)
        }
    }
}

impl fmt::Display for TypeSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.closure {
            write!(f, "closure color")
        } else if self.structure > 0 {
            if let Some(s) = get_struct(self.structure as i32) {
                write!(f, "{}", s.name)?;
            } else {
                write!(f, "struct#{}", self.structure)?;
            }
            if self.is_array() {
                write!(f, "[{}]", self.simple.arraylen)?;
            }
            Ok(())
        } else {
            write!(f, "{}", self.simple)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typedesc::TypeDesc;

    #[test]
    fn test_basic() {
        let f = TypeSpec::from_simple(TypeDesc::FLOAT);
        assert!(f.is_float());
        assert!(!f.is_closure());
        assert!(!f.is_structure());

        let c = TypeSpec::closure(TypeDesc::COLOR);
        assert!(c.is_closure());
        assert!(!c.is_float());
    }

    #[test]
    fn test_triple_equivalence() {
        let color = TypeSpec::from_simple(TypeDesc::COLOR);
        let point = TypeSpec::from_simple(TypeDesc::POINT);
        let vector = TypeSpec::from_simple(TypeDesc::VECTOR);
        let normal = TypeSpec::from_simple(TypeDesc::NORMAL);

        assert!(color.equivalent(&point));
        assert!(color.equivalent(&vector));
        assert!(color.equivalent(&normal));
        assert!(point.equivalent(&vector));
        assert!(!color.equivalent(&TypeSpec::from_simple(TypeDesc::FLOAT)));
    }

    #[test]
    fn test_struct() {
        let name = UString::new("MyStruct");
        let mut spec = StructSpec::new(name, 0);
        spec.add_field(TypeSpec::from_simple(TypeDesc::FLOAT), UString::new("x"));
        spec.add_field(TypeSpec::from_simple(TypeDesc::COLOR), UString::new("c"));

        let id = register_struct(spec);
        assert!(id >= 1);

        let ts = TypeSpec::structure(id as i16, 0);
        assert!(ts.is_structure());
        assert!(!ts.is_closure());

        let retrieved = get_struct(id).unwrap();
        assert_eq!(retrieved.num_fields(), 2);
    }

    #[test]
    fn test_vectriple() {
        let point = TypeSpec::from_simple(TypeDesc::POINT);
        let vector = TypeSpec::from_simple(TypeDesc::VECTOR);
        let normal = TypeSpec::from_simple(TypeDesc::NORMAL);
        let color = TypeSpec::from_simple(TypeDesc::COLOR);

        // vectriple = point/vector/normal but NOT color
        assert!(point.is_vectriple());
        assert!(vector.is_vectriple());
        assert!(normal.is_vectriple());
        assert!(!color.is_vectriple());

        // vectriple_based works with arrays too
        let point_arr = TypeSpec::new_array(TypeDesc::POINT, 3);
        assert!(!point_arr.is_vectriple()); // array -> false for non-based
        assert!(point_arr.is_vectriple_based()); // array -> true for based
    }

    #[test]
    fn test_aggregate() {
        use crate::typedesc::Aggregate;
        let color = TypeSpec::from_simple(TypeDesc::COLOR);
        assert_eq!(color.aggregate() as u8, Aggregate::Vec3 as u8);

        let flt = TypeSpec::from_simple(TypeDesc::FLOAT);
        assert_eq!(flt.aggregate() as u8, Aggregate::Scalar as u8);

        let mat = TypeSpec::from_simple(TypeDesc::MATRIX);
        assert_eq!(mat.aggregate() as u8, Aggregate::Matrix44 as u8);
    }

    #[test]
    fn test_int_or_float() {
        let flt = TypeSpec::from_simple(TypeDesc::FLOAT);
        let int = TypeSpec::from_simple(TypeDesc::INT);
        let color = TypeSpec::from_simple(TypeDesc::COLOR);
        let str = TypeSpec::from_simple(TypeDesc::STRING);

        assert!(flt.is_int_or_float());
        assert!(int.is_int_or_float());
        assert!(!color.is_int_or_float()); // aggregate, not scalar
        assert!(!str.is_int_or_float());
    }

    #[test]
    fn test_assignable() {
        let flt = TypeSpec::from_simple(TypeDesc::FLOAT);
        let int = TypeSpec::from_simple(TypeDesc::INT);
        let color = TypeSpec::from_simple(TypeDesc::COLOR);

        assert!(flt.assignable_from(&int)); // float <- int is OK
        assert!(color.assignable_from(&flt)); // color <- float is OK
        assert!(!int.assignable_from(&flt)); // int <- float is NOT OK
    }
}
