//! UsdGeomPrimvar - schema wrapper for primvar attributes.
//!
//! Port of pxr/usd/usdGeom/primvar.h and primvar.cpp
//!
//! A Primvar wraps a UsdAttribute for authoring and introspecting
//! primvar-specific data: interpolation, elementSize, and indexed values.

use super::tokens::usd_geom_tokens;
use usd_core::Attribute;
use usd_sdf::{TimeCode, ValueTypeName};
use usd_tf::Token;
use usd_vt::Value;

/// Namespace prefix for primvar attributes.
const PRIMVARS_PREFIX: &str = "primvars:";

/// Suffix for the indices sub-attribute of an indexed primvar.
const INDICES_SUFFIX: &str = ":indices";

/// Reserved keywords that cannot be used as primvar base names.
const RESERVED_KEYWORDS: &[&str] = &["indices"];

// ============================================================================
// Primvar
// ============================================================================

/// A wrapper around UsdAttribute for primvar-specific operations.
///
/// Matches C++ `UsdGeomPrimvar`. Provides access to interpolation,
/// elementSize, and indexed primvar data on top of a regular attribute.
#[derive(Debug, Clone)]
pub struct Primvar {
    /// The underlying attribute.
    attr: Attribute,
}

impl Primvar {
    /// Construct a Primvar wrapping an existing attribute.
    pub fn new(attr: Attribute) -> Self {
        Self { attr }
    }

    /// Returns a reference to the underlying attribute.
    pub fn get_attr(&self) -> &Attribute {
        &self.attr
    }

    /// Returns true if the underlying attribute is defined and is a valid primvar.
    pub fn is_defined(&self) -> bool {
        self.attr.is_valid() && self.attr.name().as_str().starts_with(PRIMVARS_PREFIX)
    }

    /// Bool conversion - valid for querying if the primvar is defined.
    pub fn is_valid(&self) -> bool {
        self.is_defined()
    }

    /// Returns true if the primvar has a value (authored or fallback).
    pub fn has_value(&self) -> bool {
        self.attr.has_value()
    }

    // ========================================================================
    // Interpolation
    // ========================================================================

    /// Return the primvar's interpolation, defaulting to "constant" if not authored.
    ///
    /// Matches C++ `GetInterpolation()`: reads `interpolation` metadata as a Token.
    pub fn get_interpolation(&self) -> Token {
        let tokens = usd_geom_tokens();
        if let Some(val) = self.attr.get_metadata(&tokens.interpolation) {
            // Try Token first (standard USD storage), then String as fallback
            if let Some(t) = val.downcast_clone::<Token>() {
                return t;
            }
            if let Some(s) = val.downcast_clone::<String>() {
                return Token::new(&s);
            }
        }
        // Default interpolation is "constant"
        tokens.constant.clone()
    }

    /// Set the primvar's interpolation.
    ///
    /// Matches C++ `SetInterpolation()`: stores as Token metadata.
    pub fn set_interpolation(&self, interp: &Token) -> bool {
        if !Self::is_valid_interpolation(interp) {
            return false;
        }
        // Store as Token, matching C++ `_attr.SetMetadata(UsdGeomTokens->interpolation, interp)`
        self.attr.set_metadata(
            &usd_geom_tokens().interpolation,
            Value::from(interp.clone()),
        )
    }

    /// Returns true if interpolation has been explicitly authored.
    pub fn has_authored_interpolation(&self) -> bool {
        self.attr
            .has_authored_metadata(&usd_geom_tokens().interpolation)
    }

    // ========================================================================
    // ElementSize
    // ========================================================================

    /// Return the element size, defaulting to 1 if not authored.
    ///
    /// Matches C++ `GetElementSize()`.
    pub fn get_element_size(&self) -> i32 {
        if let Some(val) = self.attr.get_metadata(&usd_geom_tokens().element_size) {
            if let Some(&n) = val.get::<i32>() {
                return n;
            }
            // Try i64 in case it was stored as i64
            if let Some(&n) = val.get::<i64>() {
                return n as i32;
            }
        }
        1 // default
    }

    /// Set the element size. Returns false if elt_size < 1.
    pub fn set_element_size(&self, elt_size: i32) -> bool {
        if elt_size < 1 {
            return false;
        }
        self.attr
            .set_metadata(&usd_geom_tokens().element_size, elt_size)
    }

    /// Returns true if elementSize has been explicitly authored.
    pub fn has_authored_element_size(&self) -> bool {
        self.attr
            .has_authored_metadata(&usd_geom_tokens().element_size)
    }

    // ========================================================================
    // Indexed Primvars
    // ========================================================================

    /// Returns the indices attribute for this primvar, if it exists.
    ///
    /// The indices attribute is named `<primvar_name>:indices`.
    pub fn get_indices_attr(&self) -> Option<Attribute> {
        let indices_name = format!("{}{}", self.attr.name().as_str(), INDICES_SUFFIX);
        let prim_path = self.attr.prim_path();
        let stage = self.attr.stage()?;

        // Check composed stage layers (not just root layer)
        let prim = stage.get_prim_at_path(&prim_path)?;
        let attr = prim.get_attribute(&indices_name)?;
        // Only return if the attribute actually exists in any layer
        if attr.is_valid() { Some(attr) } else { None }
    }

    /// Read indices values at the given time.
    ///
    /// Returns `None` if no indices attribute exists or has no value.
    pub fn get_indices(&self, time: TimeCode) -> Option<Vec<i32>> {
        let indices_attr = self.get_indices_attr()?;
        let val = indices_attr.get(time)?;
        // Try Vec<i32> first, then Vec<i64>
        if let Some(arr) = val.get::<Vec<i32>>() {
            return Some(arr.clone());
        }
        if let Some(arr) = val.get::<Vec<i64>>() {
            return Some(arr.iter().map(|&v| v as i32).collect());
        }
        None
    }

    /// Set the indices for this indexed primvar at the given time.
    ///
    /// Creates the indices attribute if it does not exist.
    pub fn set_indices(&self, indices: &[i32], time: TimeCode) -> bool {
        let indices_name = format!("{}{}", self.attr.name().as_str(), INDICES_SUFFIX);
        let prim_path = self.attr.prim_path();
        let Some(stage) = self.attr.stage() else {
            return false;
        };
        let Some(prim) = stage.get_prim_at_path(&prim_path) else {
            return false;
        };

        // Check if the indices attribute spec already exists in the layer
        let layer = stage.root_layer();
        let indices_path = match prim_path.append_property(&indices_name) {
            Some(p) => p,
            None => return false,
        };

        let indices_attr = if layer.get_attribute_at_path(&indices_path).is_some() {
            // Attribute exists, get handle
            match prim.get_attribute(&indices_name) {
                Some(a) => a,
                None => return false,
            }
        } else {
            // Create new attribute
            let int_type = usd_sdf::ValueTypeRegistry::instance().find_type("int[]");
            match prim.create_attribute(&indices_name, &int_type, false, None) {
                Some(attr) => attr,
                None => return false,
            }
        };

        indices_attr.set(Value::new(indices.to_vec()), time)
    }

    /// Block the indices attribute, making this primvar non-indexed.
    pub fn block_indices(&self) {
        if let Some(indices_attr) = self.get_indices_attr() {
            let _ = indices_attr.block();
        }
    }

    /// Returns true if this primvar is indexed (has an indices attribute with values).
    pub fn is_indexed(&self) -> bool {
        if let Some(indices_attr) = self.get_indices_attr() {
            return indices_attr.has_value();
        }
        false
    }

    // ========================================================================
    // UnauthoredValuesIndex
    // ========================================================================

    /// Returns the index into the primvar's values that represents "unauthorized" values.
    ///
    /// Defaults to -1 (not authored) if not set.
    ///
    /// Matches C++ `UsdGeomPrimvar::GetUnauthoredValuesIndex()`.
    pub fn get_unauthored_values_index(&self) -> i32 {
        if let Some(val) = self
            .attr
            .get_metadata(&usd_geom_tokens().unauthored_values_index)
        {
            if let Some(&n) = val.get::<i32>() {
                return n;
            }
            if let Some(&n) = val.get::<i64>() {
                return n as i32;
            }
        }
        -1 // default: not authored
    }

    /// Sets the index representing "unauthorized" values in this primvar.
    ///
    /// Matches C++ `UsdGeomPrimvar::SetUnauthoredValuesIndex(int)`.
    pub fn set_unauthored_values_index(&self, index: i32) -> bool {
        self.attr
            .set_metadata(&usd_geom_tokens().unauthored_values_index, index)
    }

    // ========================================================================
    // ComputeFlattened
    // ========================================================================

    /// Computes the flattened value by expanding indexed data.
    ///
    /// If the primvar is not indexed, returns the authored value as-is.
    pub fn compute_flattened(&self, time: TimeCode) -> Option<Value> {
        let value = self.attr.get(time)?;

        // C++ check: if value is not an array or primvar is not indexed, return directly.
        // Non-array values (scalars) pass through as-is — matches C++ `IsArrayValued()`.
        if !value.is_array_valued() || !self.is_indexed() {
            return Some(value);
        }

        // Get indices; error out if not found for indexed primvar
        let indices = self.get_indices(time)?;

        // Flatten with the authored element_size (C++ uses GetElementSize() here)
        let element_size = self.get_element_size() as usize;
        Self::flatten_with_indices_es(&value, &indices, element_size).ok()
    }

    /// Computes the flattened value of `attr_val` given `indices`, assuming
    /// an element size of 1.
    ///
    /// Static convenience function matching C++
    /// `UsdGeomPrimvar::ComputeFlattened(VtValue*, VtValue, VtIntArray, string*)`.
    ///
    /// Returns `Ok(value)` on success, `Err(message)` if unsupported type or
    /// invalid indices.
    pub fn compute_flattened_static(attr_val: &Value, indices: &[i32]) -> Result<Value, String> {
        Self::compute_flattened_static_with_element_size(attr_val, indices, 1)
    }

    /// Computes the flattened value of `attr_val` given `indices` and
    /// `element_size`.
    ///
    /// Static convenience function matching C++
    /// `UsdGeomPrimvar::ComputeFlattened(VtValue*, VtValue, VtIntArray, int, string*)`.
    ///
    /// Returns `Ok(value)` on success, `Err(message)` if unsupported type or
    /// invalid indices.
    pub fn compute_flattened_static_with_element_size(
        attr_val: &Value,
        indices: &[i32],
        element_size: usize,
    ) -> Result<Value, String> {
        if element_size < 1 {
            return Err("element_size must be >= 1".into());
        }
        Self::flatten_with_indices_es(attr_val, indices, element_size)
    }

    /// Internal helper: flatten a value array using an index array with
    /// the given element_size. Each index maps to `element_size` consecutive
    /// elements in the authored array.
    fn flatten_with_indices_es(
        value: &Value,
        indices: &[i32],
        element_size: usize,
    ) -> Result<Value, String> {
        // Macro to reduce boilerplate for each array type
        macro_rules! try_flatten {
            ($ty:ty, $conv:expr) => {
                if let Some(arr) = value.get::<Vec<$ty>>() {
                    let mut result = Vec::with_capacity(indices.len() * element_size);
                    for (pos, &idx) in indices.iter().enumerate() {
                        // Guard negative indices BEFORE casting to avoid wrapping
                        if idx < 0 {
                            return Err(format!(
                                "Index {} at position {} out of range (array len {}, elem_size {})",
                                idx,
                                pos,
                                arr.len(),
                                element_size
                            ));
                        }
                        let base = idx as usize * element_size;
                        if base + element_size > arr.len() {
                            return Err(format!(
                                "Index {} at position {} out of range (array len {}, elem_size {})",
                                idx,
                                pos,
                                arr.len(),
                                element_size
                            ));
                        }
                        result.extend_from_slice(&arr[base..base + element_size]);
                    }
                    return Ok($conv(result));
                }
            };
        }

        try_flatten!(f64, Value::from_no_hash);
        try_flatten!(f32, Value::from_no_hash);
        try_flatten!(i32, |v| Value::new(v));
        try_flatten!(i64, |v| Value::new(v));
        try_flatten!([f32; 3], Value::from_no_hash);
        try_flatten!([f64; 3], Value::from_no_hash);

        if let Some(arr) = value.get::<Vec<String>>() {
            let mut result = Vec::with_capacity(indices.len() * element_size);
            for (pos, &idx) in indices.iter().enumerate() {
                // Guard negative indices BEFORE casting to avoid wrapping
                if idx < 0 {
                    return Err(format!(
                        "Index {} at position {} out of range (array len {}, elem_size {})",
                        idx,
                        pos,
                        arr.len(),
                        element_size
                    ));
                }
                let base = idx as usize * element_size;
                if base + element_size > arr.len() {
                    return Err(format!(
                        "Index {} at position {} out of range (array len {}, elem_size {})",
                        idx,
                        pos,
                        arr.len(),
                        element_size
                    ));
                }
                result.extend_from_slice(&arr[base..base + element_size]);
            }
            return Ok(Value::new(result));
        }

        // Non-array type: return as-is (scalar primvars aren't indexed)
        Ok(value.clone())
    }

    // ========================================================================
    // Value queries
    // ========================================================================

    /// Returns true if the attribute has an authored value.
    pub fn has_authored_value(&self) -> bool {
        self.attr.has_authored_value()
    }

    /// Returns true if the primvar's value might be time-varying.
    ///
    /// Also considers the indices attribute if the primvar is indexed.
    pub fn value_might_be_time_varying(&self) -> bool {
        if self.attr.value_might_be_time_varying() {
            return true;
        }
        // Also check indices attribute
        if let Some(indices_attr) = self.get_indices_attr() {
            if indices_attr.value_might_be_time_varying() {
                return true;
            }
        }
        false
    }

    // ========================================================================
    // Name utilities
    // ========================================================================

    /// Returns the primvar name without the "primvars:" prefix.
    ///
    /// Matches C++ `GetPrimvarName()`.
    pub fn get_primvar_name(&self) -> Token {
        Self::strip_primvars_name(&self.attr.name())
    }

    /// Returns the full attribute name.
    pub fn get_name(&self) -> Token {
        self.attr.name()
    }

    /// Returns the type name of the underlying attribute.
    pub fn get_type_name(&self) -> ValueTypeName {
        let type_tok = self.attr.type_name();
        usd_sdf::ValueTypeRegistry::instance().find_type_by_token(&type_tok)
    }

    // ========================================================================
    // Static utilities
    // ========================================================================

    /// Validate that a name is a valid primvar name.
    ///
    /// A valid primvar name starts with "primvars:" and the base name
    /// is not a reserved keyword.
    pub fn is_valid_primvar_name(name: &str) -> bool {
        if !name.starts_with(PRIMVARS_PREFIX) {
            return false;
        }
        let base = &name[PRIMVARS_PREFIX.len()..];
        if base.is_empty() {
            return false;
        }
        // Check reserved keywords
        !RESERVED_KEYWORDS.contains(&base)
    }

    /// Validate that an interpolation token is one of the 5 valid values.
    pub fn is_valid_interpolation(interp: &Token) -> bool {
        let s = interp.as_str();
        let tokens = usd_geom_tokens();
        s == tokens.constant.as_str()
            || s == tokens.uniform.as_str()
            || s == tokens.varying.as_str()
            || s == tokens.vertex.as_str()
            || s == tokens.face_varying.as_str()
    }

    /// Strip the "primvars:" prefix from a token, if present.
    pub fn strip_primvars_name(name: &Token) -> Token {
        let s = name.as_str();
        if let Some(stripped) = s.strip_prefix(PRIMVARS_PREFIX) {
            Token::new(stripped)
        } else {
            name.clone()
        }
    }

    /// Returns declaration info for this primvar.
    ///
    /// Matches C++ `GetDeclarationInfo()`.
    pub fn get_declaration_info(&self) -> (Token, ValueTypeName, Token, i32) {
        let name = self.get_primvar_name();
        let type_name = self.get_type_name();
        let interpolation = self.get_interpolation();
        let element_size = self.get_element_size();
        (name, type_name, interpolation, element_size)
    }

    // ========================================================================
    // Static: IsPrimvar / MakeNamespaced / NamespacePrefix
    // ========================================================================

    /// Returns true if `attr` is a valid primvar attribute.
    ///
    /// Matches C++ `UsdGeomPrimvar::IsPrimvar()`.
    pub fn is_primvar(attr: &Attribute) -> bool {
        if !attr.is_valid() {
            return false;
        }
        Self::is_valid_primvar_name(attr.name().as_str())
    }

    /// Add "primvars:" namespace prefix if not already present.
    /// Returns empty token if the resulting name is invalid (e.g. reserved).
    ///
    /// Matches C++ `UsdGeomPrimvar::_MakeNamespaced()`.
    pub fn make_namespaced(name: &Token) -> Token {
        Self::make_namespaced_quiet(name, false)
    }

    /// Like `make_namespaced` but optionally suppresses error logging.
    pub fn make_namespaced_quiet(name: &Token, quiet: bool) -> Token {
        let s = name.as_str();
        let result = if s.starts_with(PRIMVARS_PREFIX) {
            name.clone()
        } else {
            Token::new(&format!("{}{}", PRIMVARS_PREFIX, s))
        };
        if !Self::is_valid_primvar_name(result.as_str()) {
            if !quiet {
                log::error!(
                    "'{}' is not a valid primvar name (contains reserved keyword 'indices')",
                    name.as_str()
                );
            }
            return Token::new("");
        }
        result
    }

    /// Returns the namespace prefix token "primvars:".
    ///
    /// Matches C++ `UsdGeomPrimvar::_GetNamespacePrefix()`.
    pub fn namespace_prefix() -> Token {
        Token::new(PRIMVARS_PREFIX)
    }

    /// Returns true if the primvar base name contains additional namespaces.
    ///
    /// Matches C++ `NameContainsNamespaces()`.
    pub fn name_contains_namespaces(&self) -> bool {
        let full = self.attr.name();
        let s = full.as_str();
        // Look for ':' after the "primvars:" prefix
        s.get(PRIMVARS_PREFIX.len()..)
            .map_or(false, |rest| rest.contains(':'))
    }

    // ========================================================================
    // Time samples (merges value + indices time samples)
    // ========================================================================

    /// Returns all time samples, merging value and indices attributes.
    ///
    /// Matches C++ `UsdGeomPrimvar::GetTimeSamples()`.
    pub fn get_time_samples(&self) -> Vec<f64> {
        let mut times = self.attr.get_time_samples();
        // Merge indices time samples
        if let Some(idx_attr) = self.get_indices_attr() {
            for t in idx_attr.get_time_samples() {
                if times
                    .binary_search_by(|a| a.partial_cmp(&t).unwrap_or(std::cmp::Ordering::Equal))
                    .is_err()
                {
                    times.push(t);
                }
            }
            times.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        }
        times
    }

    /// Returns time samples within [start, end], merging value and indices.
    ///
    /// Matches C++ `UsdGeomPrimvar::GetTimeSamplesInInterval()`.
    pub fn get_time_samples_in_interval(&self, start: f64, end: f64) -> Vec<f64> {
        self.get_time_samples()
            .into_iter()
            .filter(|&t| t >= start && t <= end)
            .collect()
    }

    // ========================================================================
    // Id Target
    // ========================================================================

    /// Returns true if this primvar is an "Id" primvar (has an idFrom relationship).
    ///
    /// Matches C++ `UsdGeomPrimvar::IsIdTarget()`.
    pub fn is_id_target(&self) -> bool {
        // Id primvars must be string or string[] typed
        let tn = self.attr.type_name();
        let tn_str = tn.as_str();
        if tn_str != "string" && tn_str != "token" && tn_str != "string[]" && tn_str != "token[]" {
            return false;
        }
        // Check for the idFrom relationship
        let rel_name = format!("{}:idFrom", self.attr.name().as_str());
        let prim_path = self.attr.prim_path();
        if let Some(stage) = self.attr.stage() {
            if let Some(prim) = stage.get_prim_at_path(&prim_path) {
                return prim.get_relationship(&rel_name).is_some();
            }
        }
        false
    }

    /// Sets the Id target for this primvar.
    ///
    /// Matches C++ `UsdGeomPrimvar::SetIdTarget()`.
    pub fn set_id_target(&self, path: &usd_sdf::Path) -> bool {
        // Id primvars must be string or string[] typed
        let tn = self.attr.type_name();
        let tn_str = tn.as_str();
        if tn_str != "string" && tn_str != "token" && tn_str != "string[]" && tn_str != "token[]" {
            log::error!(
                "SetIdTarget requires string/token typed primvar, got '{}'",
                tn_str
            );
            return false;
        }
        let rel_name = format!("{}:idFrom", self.attr.name().as_str());
        let prim_path = self.attr.prim_path();
        if let Some(stage) = self.attr.stage() {
            if let Some(prim) = stage.get_prim_at_path(&prim_path) {
                if let Some(rel) = prim.create_relationship(&rel_name, false) {
                    return rel.set_targets(&[path.clone()]);
                }
            }
        }
        false
    }
}

impl std::ops::Deref for Primvar {
    type Target = Attribute;

    fn deref(&self) -> &Self::Target {
        &self.attr
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use usd_core::common::InitialLoadSet;
    use usd_core::stage::Stage;
    use usd_sdf::{TimeCode, ValueTypeRegistry};

    /// Helper to create an in-memory stage with a mesh prim and a primvar attribute.
    fn setup_stage_with_primvar() -> (std::sync::Arc<usd_core::stage::Stage>, Primvar) {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("stage");
        let prim = stage.define_prim("/Test", "Mesh").expect("define prim");

        let float_type = ValueTypeRegistry::instance().find_type("float[]");
        let attr = prim
            .create_attribute("primvars:testPV", &float_type, false, None)
            .expect("create attr");

        let primvar = Primvar::new(attr);
        (stage, primvar)
    }

    #[test]
    fn test_primvar_is_defined() {
        let (_stage, pv) = setup_stage_with_primvar();
        assert!(pv.is_defined());
        assert!(pv.is_valid());
    }

    #[test]
    fn test_primvar_name() {
        let (_stage, pv) = setup_stage_with_primvar();
        assert_eq!(pv.get_primvar_name().as_str(), "testPV");
        assert_eq!(pv.get_name().as_str(), "primvars:testPV");
    }

    #[test]
    fn test_interpolation_default() {
        let (_stage, pv) = setup_stage_with_primvar();
        // Default interpolation is "constant"
        assert_eq!(pv.get_interpolation().as_str(), "constant");
        assert!(!pv.has_authored_interpolation());
    }

    #[test]
    fn test_set_interpolation() {
        let (_stage, pv) = setup_stage_with_primvar();
        let tokens = usd_geom_tokens();

        assert!(pv.set_interpolation(&tokens.vertex));
        assert_eq!(pv.get_interpolation().as_str(), "vertex");
        assert!(pv.has_authored_interpolation());
    }

    #[test]
    fn test_invalid_interpolation() {
        let (_stage, pv) = setup_stage_with_primvar();
        let bad = Token::new("bogus");
        assert!(!pv.set_interpolation(&bad));
    }

    #[test]
    fn test_element_size_default() {
        let (_stage, pv) = setup_stage_with_primvar();
        assert_eq!(pv.get_element_size(), 1);
        assert!(!pv.has_authored_element_size());
    }

    #[test]
    fn test_set_element_size() {
        let (_stage, pv) = setup_stage_with_primvar();
        assert!(pv.set_element_size(9));
        assert_eq!(pv.get_element_size(), 9);
        assert!(pv.has_authored_element_size());
    }

    #[test]
    fn test_set_element_size_invalid() {
        let (_stage, pv) = setup_stage_with_primvar();
        assert!(!pv.set_element_size(0));
        assert!(!pv.set_element_size(-1));
    }

    #[test]
    fn test_strip_primvars_name() {
        let name = Token::new("primvars:displayColor");
        assert_eq!(Primvar::strip_primvars_name(&name).as_str(), "displayColor");

        let plain = Token::new("points");
        assert_eq!(Primvar::strip_primvars_name(&plain).as_str(), "points");
    }

    #[test]
    fn test_is_valid_primvar_name() {
        assert!(Primvar::is_valid_primvar_name("primvars:foo"));
        assert!(Primvar::is_valid_primvar_name("primvars:ns:bar"));
        assert!(!Primvar::is_valid_primvar_name("foo"));
        assert!(!Primvar::is_valid_primvar_name("primvars:"));
        assert!(!Primvar::is_valid_primvar_name("primvars:indices"));
    }

    #[test]
    fn test_is_valid_interpolation() {
        let tokens = usd_geom_tokens();
        assert!(Primvar::is_valid_interpolation(&tokens.constant));
        assert!(Primvar::is_valid_interpolation(&tokens.uniform));
        assert!(Primvar::is_valid_interpolation(&tokens.varying));
        assert!(Primvar::is_valid_interpolation(&tokens.vertex));
        assert!(Primvar::is_valid_interpolation(&tokens.face_varying));
        assert!(!Primvar::is_valid_interpolation(&Token::new("invalid")));
    }

    #[test]
    fn test_not_indexed_by_default() {
        let (_stage, pv) = setup_stage_with_primvar();
        assert!(!pv.is_indexed());
        assert!(pv.get_indices(TimeCode::default_time()).is_none());
        assert!(pv.get_indices_attr().is_none());
    }

    #[test]
    fn test_set_and_get_indices() {
        let (_stage, pv) = setup_stage_with_primvar();
        let indices = vec![0, 1, 2, 0, 2, 3];
        assert!(pv.set_indices(&indices, TimeCode::default_time()));
        // After setting, the indices attribute exists
        assert!(pv.get_indices_attr().is_some());
        // is_indexed checks has_value on the indices attr
        assert!(pv.is_indexed());
    }

    #[test]
    fn test_block_indices() {
        let (_stage, pv) = setup_stage_with_primvar();
        let indices = vec![0, 1, 2];
        assert!(pv.set_indices(&indices, TimeCode::default_time()));
        assert!(pv.is_indexed());

        pv.block_indices();
        // After blocking, the indices attr still exists but value is blocked
        assert!(pv.get_indices_attr().is_some());
    }

    #[test]
    fn test_has_authored_value() {
        let (_stage, pv) = setup_stage_with_primvar();
        assert!(!pv.has_authored_value());

        // Set a string value (string round-trips cleanly through layer)
        pv.attr
            .set(Value::new("test_val".to_string()), TimeCode::default_time());
        assert!(pv.has_authored_value());
    }

    #[test]
    fn test_value_might_be_time_varying() {
        let (_stage, pv) = setup_stage_with_primvar();
        assert!(!pv.value_might_be_time_varying());
    }

    #[test]
    fn test_declaration_info() {
        let (_stage, pv) = setup_stage_with_primvar();
        let tokens = usd_geom_tokens();
        pv.set_interpolation(&tokens.vertex);
        pv.set_element_size(3);

        let (name, _type_name, interp, elem_size) = pv.get_declaration_info();
        assert_eq!(name.as_str(), "testPV");
        assert_eq!(interp.as_str(), "vertex");
        assert_eq!(elem_size, 3);
    }

    #[test]
    fn test_compute_flattened_non_indexed() {
        let (_stage, pv) = setup_stage_with_primvar();

        // Set a string value (strings round-trip cleanly through layer)
        pv.attr
            .set(Value::new("hello".to_string()), TimeCode::default_time());

        // Non-indexed primvar returns the authored value as-is
        let result = pv.compute_flattened(TimeCode::default_time());
        assert!(result.is_some());
    }

    #[test]
    fn test_get_unauthored_values_index_default() {
        let (_stage, pv) = setup_stage_with_primvar();
        // Default when not authored is -1, matching C++ GetUnauthoredValuesIndex()
        assert_eq!(pv.get_unauthored_values_index(), -1);
    }

    #[test]
    fn test_set_and_get_unauthored_values_index() {
        let (_stage, pv) = setup_stage_with_primvar();
        // Set an index and read it back
        assert!(pv.set_unauthored_values_index(42));
        assert_eq!(pv.get_unauthored_values_index(), 42);

        // Setting to 0 should also work
        assert!(pv.set_unauthored_values_index(0));
        assert_eq!(pv.get_unauthored_values_index(), 0);
    }
}
