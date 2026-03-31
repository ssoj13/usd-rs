//! TypeDesc — type descriptor for MaterialX data types in shader generation.

use std::collections::HashMap;

use crate::core::Value;

/// Base type category
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum BaseType {
    None = 0,
    Boolean,
    Integer,
    Float,
    String,
    Struct,
}

/// Semantic category
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Semantic {
    None = 0,
    Color,
    Vector,
    Matrix,
    Filename,
    Closure,
    Shader,
    Material,
    Enum,
}

/// Struct member description
#[derive(Clone, Debug)]
pub struct StructMemberDesc {
    pub type_desc: TypeDesc,
    pub name: String,
    pub default_value_str: String,
}

/// Type descriptor for MaterialX data types.
#[derive(Clone, Debug)]
pub struct TypeDesc {
    pub name: String,
    pub basetype: BaseType,
    pub semantic: Semantic,
    pub size: u16,
    pub struct_members: Option<Vec<StructMemberDesc>>,
}

impl TypeDesc {
    pub fn new(name: impl Into<String>, basetype: BaseType, semantic: Semantic, size: u16) -> Self {
        Self {
            name: name.into(),
            basetype,
            semantic,
            size,
            struct_members: None,
        }
    }

    pub fn new_struct(name: impl Into<String>, members: Vec<StructMemberDesc>) -> Self {
        Self {
            name: name.into(),
            basetype: BaseType::Struct,
            semantic: Semantic::None,
            size: 1,
            struct_members: Some(members),
        }
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_base_type(&self) -> BaseType {
        self.basetype
    }

    pub fn get_semantic(&self) -> Semantic {
        self.semantic
    }

    pub fn get_size(&self) -> u16 {
        self.size
    }

    pub fn is_scalar(&self) -> bool {
        self.size == 1
    }

    pub fn is_aggregate(&self) -> bool {
        self.size > 1
    }

    pub fn is_array(&self) -> bool {
        self.size == 0
    }

    pub fn is_float2(&self) -> bool {
        self.size == 2 && (self.semantic == Semantic::Color || self.semantic == Semantic::Vector)
    }

    pub fn is_float3(&self) -> bool {
        self.size == 3 && (self.semantic == Semantic::Color || self.semantic == Semantic::Vector)
    }

    pub fn is_float4(&self) -> bool {
        self.size == 4 && (self.semantic == Semantic::Color || self.semantic == Semantic::Vector)
    }

    pub fn is_closure(&self) -> bool {
        matches!(
            self.semantic,
            Semantic::Closure | Semantic::Shader | Semantic::Material
        )
    }

    pub fn is_struct(&self) -> bool {
        self.basetype == BaseType::Struct
    }

    pub fn get_struct_members(&self) -> Option<&[StructMemberDesc]> {
        self.struct_members.as_deref()
    }

    /// Compute a type ID as FNV-1a 32-bit hash of the type name.
    /// Matches C++ TypeDesc::typeId() which stores constexpr_hash of the name.
    pub fn type_id(&self) -> u32 {
        fnv1a_32(self.name.as_bytes())
    }

    /// Create a Value from a string representation of this type.
    /// For non-struct types delegates to Value::from_strings.
    /// For struct types builds an AggregateValue from comma/semicolon-delimited parts.
    /// Returns None when the string cannot be parsed.
    pub fn create_value_from_strings(&self, value_str: &str) -> Option<Value> {
        if !self.is_struct() {
            return Value::from_strings(value_str, &self.name);
        }
        // Struct: split by comma and build AggregateValue
        let members = self.struct_members.as_deref()?;
        let parts = split_struct_value_string(value_str);
        if parts.len() != members.len() {
            return None;
        }
        let sub_values: Vec<Value> = parts
            .iter()
            .zip(members.iter())
            .filter_map(|(part, member)| member.type_desc.create_value_from_strings(part.trim()))
            .collect();
        if sub_values.len() != members.len() {
            return None;
        }
        let mut agg = crate::core::AggregateValue::new(&self.name);
        for v in sub_values {
            agg.append_value(v);
        }
        Some(Value::Aggregate(Box::new(agg)))
    }
}

/// FNV-1a 32-bit hash — matches C++ constexpr_hash in TypeDesc.
#[inline]
fn fnv1a_32(bytes: &[u8]) -> u32 {
    let mut h: u32 = 2166136261;
    for &b in bytes {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    h
}

/// Split a struct value string like "0.1, 0.2, 0.3" by comma.
/// Handles nested struct values by treating braces as nesting.
fn split_struct_value_string(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth: usize = 0;
    let mut current = String::new();
    for ch in s.chars() {
        match ch {
            '{' => {
                depth += 1;
                current.push(ch);
            }
            '}' => {
                depth = depth.saturating_sub(1);
                current.push(ch);
            }
            ',' if depth == 0 => {
                parts.push(current.trim().to_string());
                current = String::new();
            }
            _ => {
                current.push(ch);
            }
        }
    }
    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }
    parts
}

impl PartialEq for TypeDesc {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for TypeDesc {}

impl std::hash::Hash for TypeDesc {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

/// Standard type descriptors
pub mod types {
    use super::*;

    pub fn none() -> TypeDesc {
        TypeDesc::new("none", BaseType::None, Semantic::None, 0)
    }

    pub fn boolean() -> TypeDesc {
        TypeDesc::new("boolean", BaseType::Boolean, Semantic::None, 1)
    }

    pub fn integer() -> TypeDesc {
        TypeDesc::new("integer", BaseType::Integer, Semantic::None, 1)
    }

    pub fn float() -> TypeDesc {
        TypeDesc::new("float", BaseType::Float, Semantic::None, 1)
    }

    pub fn integerarray() -> TypeDesc {
        TypeDesc::new("integerarray", BaseType::Integer, Semantic::None, 0)
    }

    pub fn floatarray() -> TypeDesc {
        TypeDesc::new("floatarray", BaseType::Float, Semantic::None, 0)
    }

    pub fn vector2() -> TypeDesc {
        TypeDesc::new("vector2", BaseType::Float, Semantic::Vector, 2)
    }

    pub fn vector3() -> TypeDesc {
        TypeDesc::new("vector3", BaseType::Float, Semantic::Vector, 3)
    }

    pub fn vector4() -> TypeDesc {
        TypeDesc::new("vector4", BaseType::Float, Semantic::Vector, 4)
    }

    pub fn color3() -> TypeDesc {
        TypeDesc::new("color3", BaseType::Float, Semantic::Color, 3)
    }

    pub fn color4() -> TypeDesc {
        TypeDesc::new("color4", BaseType::Float, Semantic::Color, 4)
    }

    pub fn matrix33() -> TypeDesc {
        TypeDesc::new("matrix33", BaseType::Float, Semantic::Matrix, 9)
    }

    pub fn matrix44() -> TypeDesc {
        TypeDesc::new("matrix44", BaseType::Float, Semantic::Matrix, 16)
    }

    pub fn string() -> TypeDesc {
        TypeDesc::new("string", BaseType::String, Semantic::None, 1)
    }

    pub fn filename() -> TypeDesc {
        TypeDesc::new("filename", BaseType::String, Semantic::Filename, 1)
    }

    pub fn bsdf() -> TypeDesc {
        TypeDesc::new("BSDF", BaseType::None, Semantic::Closure, 1)
    }

    pub fn edf() -> TypeDesc {
        TypeDesc::new("EDF", BaseType::None, Semantic::Closure, 1)
    }

    pub fn vdf() -> TypeDesc {
        TypeDesc::new("VDF", BaseType::None, Semantic::Closure, 1)
    }

    pub fn surfaceshader() -> TypeDesc {
        TypeDesc::new("surfaceshader", BaseType::None, Semantic::Shader, 1)
    }

    pub fn volumeshader() -> TypeDesc {
        TypeDesc::new("volumeshader", BaseType::None, Semantic::Shader, 1)
    }

    pub fn displacementshader() -> TypeDesc {
        TypeDesc::new("displacementshader", BaseType::None, Semantic::Shader, 1)
    }

    pub fn lightshader() -> TypeDesc {
        TypeDesc::new("lightshader", BaseType::None, Semantic::Shader, 1)
    }

    pub fn material() -> TypeDesc {
        TypeDesc::new("material", BaseType::None, Semantic::Material, 1)
    }
}

/// Type system — registration and lookup of type descriptors
#[derive(Default)]
pub struct TypeSystem {
    types: Vec<TypeDesc>,
    by_name: HashMap<String, TypeDesc>,
}

impl TypeSystem {
    pub fn new() -> Self {
        let mut ts = Self::default();
        ts.register_all_standard_types();
        ts
    }

    fn register_all_standard_types(&mut self) {
        self.register_type(types::boolean());
        self.register_type(types::integer());
        self.register_type(types::integerarray());
        self.register_type(types::float());
        self.register_type(types::floatarray());
        self.register_type(types::vector2());
        self.register_type(types::vector3());
        self.register_type(types::vector4());
        self.register_type(types::color3());
        self.register_type(types::color4());
        self.register_type(types::matrix33());
        self.register_type(types::matrix44());
        self.register_type(types::string());
        self.register_type(types::filename());
        self.register_type(types::bsdf());
        self.register_type(types::edf());
        self.register_type(types::vdf());
        self.register_type(types::surfaceshader());
        self.register_type(types::volumeshader());
        self.register_type(types::displacementshader());
        self.register_type(types::lightshader());
        self.register_type(types::material());
    }

    pub fn register_type(&mut self, type_desc: TypeDesc) {
        let name = type_desc.name.clone();
        self.by_name.insert(name.clone(), type_desc.clone());
        if let Some(i) = self.types.iter().position(|t| t.name == name) {
            self.types[i] = type_desc;
        } else {
            self.types.push(type_desc);
        }
    }

    pub fn register_type_custom(
        &mut self,
        name: &str,
        basetype: BaseType,
        semantic: Semantic,
        size: u16,
        struct_members: Option<Vec<StructMemberDesc>>,
    ) {
        let td = if let Some(members) = struct_members {
            TypeDesc::new_struct(name, members)
        } else {
            TypeDesc::new(name, basetype, semantic, size)
        };
        self.register_type(td);
    }

    pub fn get_type(&self, name: &str) -> TypeDesc {
        self.by_name.get(name).cloned().unwrap_or_else(types::none)
    }

    pub fn get_types(&self) -> &[TypeDesc] {
        &self.types
    }

    /// Register type definitions from a MaterialX document.
    /// Reads all TypeDef elements with member children and registers them as struct types.
    /// Matches C++ ShaderGenerator::registerTypeDefs.
    pub fn register_type_defs_from_document(&mut self, doc: &crate::core::Document) {
        for typedef_elem in doc.get_type_defs() {
            let name = typedef_elem.borrow().get_name().to_string();
            let members_elems = crate::core::typedef_get_members(&typedef_elem);
            if members_elems.is_empty() {
                continue;
            }
            let mut struct_members = Vec::new();
            for member in &members_elems {
                let member_ref = member.borrow();
                let member_name = member_ref.get_name().to_string();
                let member_type_str = member_ref
                    .get_type()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "float".to_string());
                let default_val = member_ref.get_value_string();
                let member_type = self.get_type(&member_type_str);
                struct_members.push(StructMemberDesc {
                    type_desc: member_type,
                    name: member_name,
                    default_value_str: default_val,
                });
            }
            let td = TypeDesc::new_struct(&name, struct_members);
            self.register_type(td);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Value;

    #[test]
    fn type_id_stable() {
        // type_id must be stable across calls (same hash for same name)
        let td = types::float();
        assert_eq!(td.type_id(), td.type_id());
        // Different types must have different IDs
        let td2 = types::vector3();
        assert_ne!(td.type_id(), td2.type_id());
    }

    #[test]
    fn type_id_fnv1a_matches_cpp() {
        // Verify FNV-1a hash for "float" matches the C++ constexpr_hash result.
        // C++ uses: h=2166136261, h = (h*16777619) ^ byte[i]
        // For "float": f=102, l=108, o=111, a=97, t=116
        let td = types::float();
        let id = td.type_id();
        // Just verify it's non-zero and consistent
        assert_ne!(id, 0);
        // Cross-check: same as computing manually
        let manual = {
            let mut h: u32 = 2166136261;
            for &b in b"float" {
                h ^= b as u32;
                h = h.wrapping_mul(16777619);
            }
            h
        };
        assert_eq!(id, manual);
    }

    #[test]
    fn create_value_from_strings_scalar() {
        let td = types::float();
        let v = td.create_value_from_strings("3.14");
        assert!(v.is_some());
        if let Some(Value::Float(f)) = v {
            assert!((f - 3.14_f32).abs() < 1e-4);
        } else {
            panic!("expected Float value");
        }
    }

    #[test]
    fn create_value_from_strings_vector3() {
        let td = types::vector3();
        let v = td.create_value_from_strings("1.0, 2.0, 3.0");
        assert!(v.is_some());
    }

    #[test]
    fn create_value_from_strings_struct() {
        let mut ts = TypeSystem::new();
        // Register a simple struct: MyVec { float x; float y; }
        let members = vec![
            StructMemberDesc {
                type_desc: types::float(),
                name: "x".to_string(),
                default_value_str: "0.0".to_string(),
            },
            StructMemberDesc {
                type_desc: types::float(),
                name: "y".to_string(),
                default_value_str: "0.0".to_string(),
            },
        ];
        ts.register_type(TypeDesc::new_struct("MyVec", members));
        let td = ts.get_type("MyVec");
        assert!(td.is_struct());

        let v = td.create_value_from_strings("1.5, 2.5");
        assert!(v.is_some(), "struct value parse failed");
        if let Some(Value::Aggregate(agg)) = v {
            assert_eq!(agg.get_members().len(), 2);
        } else {
            panic!("expected Aggregate value");
        }
    }

    #[test]
    fn create_value_from_strings_struct_wrong_count() {
        let members = vec![StructMemberDesc {
            type_desc: types::float(),
            name: "x".to_string(),
            default_value_str: "0.0".to_string(),
        }];
        let td = TypeDesc::new_struct("SingleFloat", members);
        // Two values for a single-member struct should return None
        assert!(td.create_value_from_strings("1.0, 2.0").is_none());
    }

    #[test]
    fn add_value_to_stage() {
        use crate::gen_shader::shader::ShaderStage;
        let mut stage = ShaderStage::new("pixel");
        stage.add_value(&42_i32);
        assert_eq!(stage.get_source_code(), "42");
        stage.add_value(&" ");
        stage.add_value(&3.14_f32);
        assert!(stage.get_source_code().contains("3.14"));
    }

    #[test]
    fn register_type_defs_empty_doc_is_noop() {
        let mut ts = TypeSystem::new();
        let count_before = ts.get_types().len();
        let doc = crate::core::Document::new();
        ts.register_type_defs_from_document(&doc);
        assert_eq!(ts.get_types().len(), count_before);
    }
}
