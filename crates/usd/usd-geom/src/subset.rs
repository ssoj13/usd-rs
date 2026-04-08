//! UsdGeomSubset - geometry subset schema.
//!
//! Port of pxr/usd/usdGeom/subset.h/cpp
//!
//! Encodes a subset of a piece of geometry as a set of indices.

use super::basis_curves::BasisCurves;
use super::imageable::Imageable;
use super::tet_mesh::TetMesh;
use super::tokens::usd_geom_tokens;
use usd_core::attribute::Variability;
use usd_core::typed::Typed;
use usd_core::{Attribute, Prim, Stage};
use usd_sdf::Path;
use usd_sdf::{TimeCode, ValueTypeRegistry};
use usd_tf::Token;
use usd_vt::Value;

// ============================================================================
// Helpers
// ============================================================================

/// Read a token-typed attribute value, handling both Token and String storage.
/// USDA stores tokens as Token, while runtime-authored values use String.
fn get_token_value(attr: &Attribute, time: TimeCode) -> Token {
    if let Some(v) = attr.get(time) {
        if let Some(t) = v.get::<Token>() {
            return t.clone();
        }
        if let Some(s) = v.get::<String>() {
            return Token::new(s);
        }
    }
    Token::new("")
}

fn get_resolved_attr_value(attr: &Attribute, time: TimeCode) -> Option<Value> {
    if time.is_default() {
        return attr.get(time);
    }

    let samples = attr.get_time_samples();
    if samples.is_empty() {
        return attr.get(TimeCode::default_time());
    }

    if let Some(exact_time) = samples
        .iter()
        .copied()
        .find(|sample| *sample == time.value())
    {
        return attr.get(TimeCode::new(exact_time));
    }

    let held_time = samples
        .iter()
        .copied()
        .filter(|sample| *sample <= time.value())
        .next_back()
        .unwrap_or(samples[0]);

    attr.get(TimeCode::new(held_time))
}

fn value_to_i32(value: &Value) -> Option<i32> {
    value
        .get::<i32>()
        .copied()
        .or_else(|| value.get::<i64>().map(|v| *v as i32))
        .or_else(|| value.get::<usize>().map(|v| *v as i32))
}

fn value_to_vec3i(value: &Value) -> Option<usd_gf::vec3::Vec3i> {
    if let Some(v) = value.get::<usd_gf::vec3::Vec3i>() {
        return Some(*v);
    }
    let values = value.get::<Vec<Value>>()?;
    if values.len() != 3 {
        return None;
    }
    Some(usd_gf::vec3::Vec3i::new(
        value_to_i32(&values[0])?,
        value_to_i32(&values[1])?,
        value_to_i32(&values[2])?,
    ))
}

fn value_to_vec4i(value: &Value) -> Option<usd_gf::vec4::Vec4i> {
    if let Some(v) = value.get::<usd_gf::vec4::Vec4i>() {
        return Some(*v);
    }
    let values = value.get::<Vec<Value>>()?;
    if values.len() != 4 {
        return None;
    }
    Some(usd_gf::vec4::Vec4i::new(
        value_to_i32(&values[0])?,
        value_to_i32(&values[1])?,
        value_to_i32(&values[2])?,
        value_to_i32(&values[3])?,
    ))
}

/// Read an int-array attribute, handling both Vec<i32> and Array<i32> storage.
fn get_int_array(prim: &Prim, attr_name: &str, time: TimeCode) -> Option<Vec<i32>> {
    let attr = prim.get_attribute(attr_name)?;
    let v = get_resolved_attr_value(&attr, time)?;
    if let Some(arr) = v.get::<Vec<i32>>() {
        Some(arr.clone())
    } else if let Some(arr) = v.get::<usd_vt::Array<i32>>() {
        Some(arr.iter().cloned().collect())
    } else {
        None
    }
}

/// Determine element count for a geometry prim given an element type.
///
/// Matches C++ `_GetElementCount()` in subset.cpp.
fn get_element_count(geom: &Imageable, element_type: &Token, time: TimeCode) -> Option<usize> {
    let tokens = usd_geom_tokens();
    let prim = geom.prim();
    let type_name = prim.type_name();

    if *element_type == tokens.face {
        if type_name.as_str() == "TetMesh" {
            if let Some(attr) = prim.get_attribute(tokens.surface_face_vertex_indices.as_str()) {
                if let Some(v) = get_resolved_attr_value(&attr, time) {
                    if let Some(arr) = v.get::<Vec<usd_gf::vec3::Vec3i>>() {
                        return Some(arr.len());
                    }
                    if let Some(arr) = v.get::<usd_vt::Array<usd_gf::vec3::Vec3i>>() {
                        return Some(arr.len());
                    }
                    if let Some(items) = v.get::<Vec<Value>>() {
                        let count = items.iter().filter_map(value_to_vec3i).count();
                        if count == items.len() {
                            return Some(count);
                        }
                    }
                }
            }
            if let Some(attr) = prim.get_attribute(tokens.tet_vertex_indices.as_str()) {
                if let Some(v) = get_resolved_attr_value(&attr, time) {
                    if let Some(arr) = v.get::<Vec<usd_gf::vec4::Vec4i>>() {
                        return Some(arr.len() * 4);
                    }
                    if let Some(arr) = v.get::<usd_vt::Array<usd_gf::vec4::Vec4i>>() {
                        return Some(arr.len() * 4);
                    }
                    if let Some(items) = v.get::<Vec<Value>>() {
                        let count = items.iter().filter_map(value_to_vec4i).count();
                        if count == items.len() {
                            return Some(count * 4);
                        }
                    }
                }
            }
            None
        } else {
            // Mesh: faceVertexCounts.len()
            let counts = get_int_array(prim, tokens.face_vertex_counts.as_str(), time)?;
            Some(counts.len())
        }
    } else if *element_type == tokens.point {
        if let Some(attr) = prim.get_attribute(tokens.points.as_str()) {
            // Try multiple storage formats: Vec<Vec3f>, Array<Vec3f>, typed_vec
            if let Some(vec) = attr.get_typed_vec::<usd_gf::vec3::Vec3f>(time) {
                return Some(vec.len());
            }
            if let Some(v) = get_resolved_attr_value(&attr, time) {
                if let Some(arr) = v.get::<Vec<usd_gf::vec3::Vec3f>>() {
                    return Some(arr.len());
                }
                if let Some(arr) = v.get::<usd_vt::Array<usd_gf::vec3::Vec3f>>() {
                    return Some(arr.len());
                }
            }
        }
        None
    } else if *element_type == tokens.tetrahedron {
        if let Some(attr) = prim.get_attribute(tokens.tet_vertex_indices.as_str()) {
            if let Some(v) = get_resolved_attr_value(&attr, time) {
                if let Some(arr) = v.get::<Vec<usd_gf::vec4::Vec4i>>() {
                    return Some(arr.len());
                }
                if let Some(arr) = v.get::<usd_vt::Array<usd_gf::vec4::Vec4i>>() {
                    return Some(arr.len());
                }
                if let Some(items) = v.get::<Vec<Value>>() {
                    let count = items.iter().filter_map(value_to_vec4i).count();
                    if count == items.len() {
                        return Some(count);
                    }
                }
            }
        }
        TetMesh::new(prim.clone())
            .get_tet_vertex_indices(time)
            .map(|arr| arr.len())
    } else if *element_type == tokens.edge {
        // Build unique edge set from mesh topology
        let face_vertex_counts = get_int_array(prim, tokens.face_vertex_counts.as_str(), time)?;
        let face_vertex_indices = get_int_array(prim, tokens.face_vertex_indices.as_str(), time)?;

        let mut edges = std::collections::BTreeSet::<(i32, i32)>::new();
        let mut offset = 0usize;
        for &count in &face_vertex_counts {
            let n = count as usize;
            for i in 0..n {
                let v0 = face_vertex_indices[offset + i];
                let v1 = face_vertex_indices[offset + (i + 1) % n];
                let edge = if v0 <= v1 { (v0, v1) } else { (v1, v0) };
                edges.insert(edge);
            }
            offset += n;
        }
        Some(edges.len())
    } else if *element_type == tokens.segment {
        // BasisCurves: segments from curveVertexCounts
        let curve_vertex_counts = get_int_array(prim, tokens.curve_vertex_counts.as_str(), time)?;

        let curve_type = prim
            .get_attribute("type")
            .map(|a| get_token_value(&a, TimeCode::default_time()))
            .unwrap_or_else(|| Token::new(""));
        let wrap = prim
            .get_attribute("wrap")
            .map(|a| get_token_value(&a, TimeCode::default_time()))
            .unwrap_or_else(|| Token::new(""));

        let is_periodic = wrap.as_str() == "periodic";
        let is_linear = curve_type.as_str() == "linear";

        let mut total_segments = 0usize;
        for &count in &curve_vertex_counts {
            let n = count as usize;
            let segs = if is_linear {
                if is_periodic { n } else { n.saturating_sub(1) }
            } else if is_periodic {
                n
            } else if wrap.as_str() == "pinned" {
                n.saturating_sub(2)
            } else {
                n.saturating_sub(3)
            };
            total_segments += segs;
        }
        Some(total_segments)
    } else {
        None
    }
}

fn element_count_might_be_time_varying(geom: &Imageable, element_type: &Token) -> bool {
    let tokens = usd_geom_tokens();
    let prim = geom.prim();
    let type_name = prim.type_name();

    if *element_type == tokens.face {
        if type_name.as_str() == "TetMesh" {
            prim.get_attribute(tokens.surface_face_vertex_indices.as_str())
                .map(|attr| attr.value_might_be_time_varying())
                .or_else(|| {
                    prim.get_attribute(tokens.tet_vertex_indices.as_str())
                        .map(|attr| attr.value_might_be_time_varying())
                })
                .unwrap_or(false)
        } else {
            prim.get_attribute(tokens.face_vertex_counts.as_str())
                .map(|attr| attr.value_might_be_time_varying())
                .unwrap_or(false)
        }
    } else if *element_type == tokens.point {
        prim.get_attribute(tokens.points.as_str())
            .map(|attr| attr.value_might_be_time_varying())
            .unwrap_or(false)
    } else if *element_type == tokens.tetrahedron {
        prim.get_attribute(tokens.tet_vertex_indices.as_str())
            .map(|attr| attr.value_might_be_time_varying())
            .unwrap_or(false)
    } else if *element_type == tokens.edge {
        prim.get_attribute(tokens.face_vertex_counts.as_str())
            .map(|attr| attr.value_might_be_time_varying())
            .unwrap_or(false)
            || prim
                .get_attribute(tokens.face_vertex_indices.as_str())
                .map(|attr| attr.value_might_be_time_varying())
                .unwrap_or(false)
    } else if *element_type == tokens.segment {
        prim.get_attribute(tokens.curve_vertex_counts.as_str())
            .map(|attr| attr.value_might_be_time_varying())
            .unwrap_or(false)
    } else {
        false
    }
}

fn validate_geom_type(geom: &Imageable, element_type: &Token) -> bool {
    let type_name = geom.prim().type_name();
    let tokens = usd_geom_tokens();

    match type_name.as_str() {
        "Mesh" => {
            *element_type == tokens.face
                || *element_type == tokens.point
                || *element_type == tokens.edge
        }
        "TetMesh" => *element_type == tokens.face || *element_type == tokens.tetrahedron,
        "BasisCurves" => *element_type == tokens.segment,
        _ => false,
    }
}

fn get_edges_from_prim(
    geom: &Imageable,
    time: TimeCode,
) -> Option<std::collections::BTreeSet<(i32, i32)>> {
    let tokens = usd_geom_tokens();
    let prim = geom.prim();
    let face_vertex_counts = get_int_array(prim, tokens.face_vertex_counts.as_str(), time)?;
    let face_vertex_indices = get_int_array(prim, tokens.face_vertex_indices.as_str(), time)?;

    let mut edges = std::collections::BTreeSet::new();
    let mut offset = 0usize;
    for &count in &face_vertex_counts {
        let n = count as usize;
        if n == 0 {
            continue;
        }
        for i in 0..n {
            let v0 = face_vertex_indices[offset + i];
            let v1 = face_vertex_indices[offset + (i + 1) % n];
            let edge = if v0 <= v1 { (v0, v1) } else { (v1, v0) };
            edges.insert(edge);
        }
        offset += n;
    }
    Some(edges)
}

fn get_all_possible_segments(
    geom: &Imageable,
    time: TimeCode,
) -> Option<std::collections::BTreeSet<(i32, i32)>> {
    let tokens = usd_geom_tokens();
    let prim = geom.prim();
    let curve_vertex_counts = get_int_array(prim, tokens.curve_vertex_counts.as_str(), time)?;
    let basis_curves = BasisCurves::new(prim.clone());
    let segment_counts = basis_curves.compute_segment_counts(time);

    let mut segments = std::collections::BTreeSet::new();
    for (curve_index, _) in curve_vertex_counts.iter().enumerate() {
        let segment_count = segment_counts.get(curve_index).copied().unwrap_or(0).max(0) as usize;
        for segment_index in 0..segment_count {
            segments.insert((curve_index as i32, segment_index as i32));
        }
    }
    Some(segments)
}

fn get_index_pairs(subset: &Subset, time: TimeCode, preserve_order: bool) -> Vec<(i32, i32)> {
    let Some(indices) = subset.get_indices(time) else {
        return Vec::new();
    };

    let mut pairs = Vec::with_capacity(indices.len() / 2);
    for chunk in indices.as_slice().chunks(2) {
        if chunk.len() != 2 {
            break;
        }
        let a = chunk[0];
        let b = chunk[1];
        if preserve_order {
            pairs.push((a, b));
        } else if a <= b {
            pairs.push((a, b));
        } else {
            pairs.push((b, a));
        }
    }
    pairs
}

// ============================================================================
// Subset
// ============================================================================

/// Geometry subset schema.
///
/// Encodes a subset of a piece of geometry as a set of indices.
///
/// Matches C++ `UsdGeomSubset`.
#[derive(Debug, Clone)]
pub struct Subset {
    /// Base typed schema.
    inner: Typed,
}

impl Subset {
    /// Creates a Subset schema from a prim.
    ///
    /// Matches C++ `UsdGeomSubset(const UsdPrim& prim)`.
    pub fn new(prim: Prim) -> Self {
        Self {
            inner: Typed::new(prim),
        }
    }

    /// Creates a Subset schema from a Typed schema.
    ///
    /// Matches C++ `UsdGeomSubset(const UsdSchemaBase& schemaObj)`.
    pub fn from_typed(typed: Typed) -> Self {
        Self { inner: typed }
    }

    /// Creates an invalid Subset schema.
    pub fn invalid() -> Self {
        Self {
            inner: Typed::invalid(),
        }
    }

    /// Returns true if this schema is valid.
    pub fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    /// Returns the wrapped prim.
    pub fn prim(&self) -> &Prim {
        self.inner.prim()
    }

    /// Returns the typed base.
    pub fn typed(&self) -> &Typed {
        &self.inner
    }

    /// Returns the schema type name.
    pub fn schema_type_name() -> Token {
        Token::new("GeomSubset")
    }

    /// Return a Subset holding the prim adhering to this schema at path on stage.
    ///
    /// Matches C++ `UsdGeomSubset::Get(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn get(stage: &Stage, path: &Path) -> Self {
        if let Some(prim) = stage.get_prim_at_path(path) {
            Self::new(prim)
        } else {
            Self::invalid()
        }
    }

    /// Attempt to ensure a prim adhering to this schema at path is defined on this stage.
    ///
    /// Matches C++ `UsdGeomSubset::Define(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn define(stage: &Stage, path: &Path) -> Self {
        match stage.define_prim(path.get_string(), Self::schema_type_name().as_str()) {
            Ok(prim) => Self::new(prim),
            Err(_) => Self::invalid(),
        }
    }

    // ========================================================================
    // ElementType
    // ========================================================================

    /// Returns the elementType attribute, creating it if necessary.
    ///
    /// Matches C++ `GetElementTypeAttr()`.
    pub fn get_element_type_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        if let Some(attr) = prim.get_attribute(usd_geom_tokens().element_type.as_str()) {
            attr
        } else {
            self.create_element_type_attr(None, false)
        }
    }

    /// Creates the elementType attribute.
    ///
    /// Matches C++ `CreateElementTypeAttr()`.
    pub fn create_element_type_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().element_type.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().element_type.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        prim.create_attribute(
            usd_geom_tokens().element_type.as_str(),
            &token_type,
            false,
            Some(Variability::Uniform),
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // Indices
    // ========================================================================

    /// Returns the indices attribute, creating it if necessary.
    ///
    /// Matches C++ `GetIndicesAttr()`.
    pub fn get_indices_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        if let Some(attr) = prim.get_attribute(usd_geom_tokens().indices.as_str()) {
            attr
        } else {
            self.create_indices_attr(None, false)
        }
    }

    /// Creates the indices attribute.
    ///
    /// Matches C++ `CreateIndicesAttr()`.
    pub fn create_indices_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().indices.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().indices.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = ValueTypeRegistry::instance();
        let int_array_type = registry.find_type_by_token(&Token::new("int[]"));

        prim.create_attribute(
            usd_geom_tokens().indices.as_str(),
            &int_array_type,
            false,
            Some(Variability::Varying),
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // FamilyName
    // ========================================================================

    /// Returns the familyName attribute, creating it if necessary.
    ///
    /// Matches C++ `GetFamilyNameAttr()`.
    pub fn get_family_name_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        if let Some(attr) = prim.get_attribute(usd_geom_tokens().family_name.as_str()) {
            attr
        } else {
            self.create_family_name_attr(None, false)
        }
    }

    /// Creates the familyName attribute.
    ///
    /// Matches C++ `CreateFamilyNameAttr()`.
    pub fn create_family_name_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().family_name.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().family_name.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        prim.create_attribute(
            usd_geom_tokens().family_name.as_str(),
            &token_type,
            false,
            Some(Variability::Uniform),
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // Static Helper Methods
    // ========================================================================

    /// Helper to get family type attribute name.
    fn get_family_type_attr_name(family_name: &Token) -> Token {
        let family_name_str = family_name.as_str();
        Token::new(&format!("subsetFamily:{}:familyType", family_name_str))
    }

    /// Creates a new GeomSubset below the given geom with the given name, element type and indices.
    ///
    /// Matches C++ `CreateGeomSubset()`.
    pub fn create_geom_subset(
        geom: &Imageable,
        subset_name: &Token,
        element_type: &Token,
        indices: &[i32],
        family_name: &Token,
        family_type: &Token,
    ) -> Self {
        let parent_path = geom.prim().path();
        let subset_path = parent_path
            .append_child(subset_name.as_str())
            .expect("Failed to create child path");
        let stage = geom.prim().stage().expect("Prim must have a stage");
        // Reuse existing prim if it exists to avoid duplicate children
        let subset = if let Some(prim) = stage.get_prim_at_path(&subset_path) {
            Self::new(prim)
        } else {
            Self::define(&stage, &subset_path)
        };

        let _ = subset
            .create_element_type_attr(None, false)
            .set(element_type.as_str(), TimeCode::default_time());
        use usd_vt::Array;
        let indices_array: Array<i32> = indices.iter().cloned().collect();
        let _ = subset
            .create_indices_attr(None, false)
            .set(indices_array, TimeCode::default_time());
        let _ = subset
            .create_family_name_attr(None, false)
            .set(family_name.as_str(), TimeCode::default_time());

        if !family_name.as_str().is_empty() && !family_type.as_str().is_empty() {
            Self::set_family_type(geom, family_name, family_type);
        }

        subset
    }

    /// Creates a new GeomSubset below the given imageable with a unique name.
    ///
    /// Matches C++ `CreateUniqueGeomSubset()`.
    pub fn create_unique_geom_subset(
        geom: &Imageable,
        subset_name: &Token,
        element_type: &Token,
        indices: &[i32],
        family_name: &Token,
        family_type: &Token,
    ) -> Self {
        let parent_path = geom.prim().path();
        let stage = geom.prim().stage().expect("Prim must have a stage");
        let base_name = subset_name.as_str();
        let mut name = base_name.to_string();
        let mut idx = 0;

        loop {
            let child_path = match parent_path.append_child(&name) {
                Some(path) => path,
                None => {
                    idx += 1;
                    name = format!("{}_{}", base_name, idx);
                    continue;
                }
            };
            if stage.get_prim_at_path(&child_path).is_some() {
                idx += 1;
                name = format!("{}_{}", base_name, idx);
            } else {
                let subset = Self::define(&stage, &child_path);
                let _ = subset
                    .create_element_type_attr(None, false)
                    .set(element_type.as_str(), TimeCode::default_time());
                use usd_vt::Array;
                let indices_array: Array<i32> = indices.iter().cloned().collect();
                let _ = subset
                    .create_indices_attr(None, false)
                    .set(indices_array, TimeCode::default_time());
                let _ = subset
                    .create_family_name_attr(None, false)
                    .set(family_name.as_str(), TimeCode::default_time());

                if !family_name.as_str().is_empty() && !family_type.as_str().is_empty() {
                    Self::set_family_type(geom, family_name, family_type);
                }

                return subset;
            }
        }
    }

    /// Returns all the GeomSubsets defined on the given imageable.
    ///
    /// Matches C++ `GetAllGeomSubsets()`.
    pub fn get_all_geom_subsets(geom: &Imageable) -> Vec<Self> {
        let mut result = Vec::new();

        for child in geom.prim().get_all_children() {
            let flags = child.flags();
            if child.type_name() == Self::schema_type_name()
                && child.is_active()
                && child.is_loaded()
                && flags.contains(usd_core::prim_flags::PrimFlags::HAS_DEFINING_SPECIFIER)
                && !child.is_abstract()
            {
                result.push(Self::new(child));
            }
        }

        result
    }

    /// Returns all the GeomSubsets of the given elementType belonging to the specified family.
    ///
    /// Matches C++ `GetGeomSubsets()`.
    pub fn get_geom_subsets(
        geom: &Imageable,
        element_type: &Token,
        family_name: &Token,
    ) -> Vec<Self> {
        let mut result = Vec::new();

        for subset in Self::get_all_geom_subsets(geom) {
            let subset_element_type =
                get_token_value(&subset.get_element_type_attr(), TimeCode::default_time());
            let subset_family_name =
                get_token_value(&subset.get_family_name_attr(), TimeCode::default_time());

            let element_type_match =
                element_type.as_str().is_empty() || subset_element_type == *element_type;
            let family_name_match =
                family_name.as_str().is_empty() || subset_family_name == *family_name;

            if element_type_match && family_name_match {
                result.push(subset);
            }
        }

        result
    }

    /// Returns the names of all the families of GeomSubsets defined on the given imageable.
    ///
    /// Matches C++ `GetAllGeomSubsetFamilyNames()`.
    pub fn get_all_geom_subset_family_names(geom: &Imageable) -> std::collections::HashSet<Token> {
        let mut family_names = std::collections::HashSet::new();

        for subset in Self::get_all_geom_subsets(geom) {
            let name = get_token_value(&subset.get_family_name_attr(), TimeCode::default_time());
            if !name.as_str().is_empty() {
                family_names.insert(name);
            }
        }

        family_names
    }

    /// Sets the type of family that the GeomSubsets belong to.
    ///
    /// Matches C++ `SetFamilyType()`.
    pub fn set_family_type(geom: &Imageable, family_name: &Token, family_type: &Token) -> bool {
        let attr_name = Self::get_family_type_attr_name(family_name);
        let prim = geom.prim();

        let registry = ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        if let Some(attr) = prim.create_attribute(
            attr_name.as_str(),
            &token_type,
            false,
            Some(Variability::Uniform),
        ) {
            attr.set(family_type.as_str(), TimeCode::default_time())
        } else {
            false
        }
    }

    /// Returns the type of family that the GeomSubsets belong to.
    ///
    /// Matches C++ `GetFamilyType()`.
    pub fn get_family_type(geom: &Imageable, family_name: &Token) -> Token {
        let attr_name = Self::get_family_type_attr_name(family_name);
        let prim = geom.prim();

        if let Some(attr) = prim.get_attribute(attr_name.as_str()) {
            let val = get_token_value(&attr, TimeCode::default_time());
            if !val.as_str().is_empty() {
                return val;
            }
        }

        usd_geom_tokens().unrestricted.clone()
    }

    // ========================================================================
    // Get Methods (Value Retrieval)
    // ========================================================================

    /// Get the indices at the specified time.
    /// Handles both Vec<i32> (from USDA) and Array<i32> (from USDC/runtime) storage.
    ///
    /// Matches C++ `GetIndices(VtIntArray* indices, UsdTimeCode time)`.
    pub fn get_indices(&self, time: TimeCode) -> Option<usd_vt::Array<i32>> {
        let attr = self.get_indices_attr();
        let value = get_resolved_attr_value(&attr, time)?;
        if let Some(arr) = value.get::<usd_vt::Array<i32>>() {
            return Some(arr.clone());
        }
        if let Some(vec) = value.get::<Vec<i32>>() {
            return Some(vec.iter().cloned().collect());
        }
        None
    }

    /// Get the family name at the specified time.
    ///
    /// Matches C++ `GetFamilyName(TfToken* familyName, UsdTimeCode time)`.
    pub fn get_family_name(&self, time: TimeCode) -> Option<Token> {
        self.get_family_name_attr().get_typed::<Token>(time)
    }

    /// Get the element type at the specified time.
    ///
    /// Matches C++ `GetElementType(TfToken* elementType, UsdTimeCode time)`.
    pub fn get_element_type(&self, time: TimeCode) -> Option<Token> {
        self.get_element_type_attr().get_typed::<Token>(time)
    }

    // ========================================================================
    // Validation Methods
    // ========================================================================

    /// Validates the given set of GeomSubsets.
    ///
    /// Checks that all subsets share the same element type and that
    /// indices are valid for the given element count and family type.
    ///
    /// Returns (valid, reason) where reason explains any invalidity.
    ///
    /// Matches C++ `UsdGeomSubset::ValidateSubsets()`.
    pub fn validate_subsets(
        subsets: &[Subset],
        element_count: usize,
        family_type: &Token,
    ) -> (bool, String) {
        if subsets.is_empty() {
            return (true, String::new());
        }

        let mut reason = String::new();
        let unrestricted = usd_geom_tokens().unrestricted.clone();

        let first_element_type = get_token_value(
            &subsets[0].get_element_type_attr(),
            TimeCode::default_time(),
        );

        let mut all_time_samples = std::collections::BTreeSet::<u64>::new();
        for subset in subsets {
            let et = get_token_value(&subset.get_element_type_attr(), TimeCode::default_time());
            if et != first_element_type {
                reason = format!(
                    "Subset at path {} has elementType {}, which does not match '{}'.",
                    subset.prim().path().get_string(),
                    et.as_str(),
                    first_element_type.as_str()
                );
                return (false, reason);
            }

            let samples = subset.get_indices_attr().get_time_samples();
            for t in samples {
                all_time_samples.insert(t.to_bits());
            }
        }

        let mut time_codes = vec![TimeCode::default_time()];
        for bits in &all_time_samples {
            time_codes.push(TimeCode::new(f64::from_bits(*bits)));
        }

        let mut valid = true;
        let partition_token = usd_geom_tokens().partition.clone();

        for t in &time_codes {
            let mut indices_in_family = std::collections::BTreeSet::new();

            for subset in subsets {
                if let Some(indices) = subset.get_indices(*t) {
                    for &index in indices.as_slice() {
                        if !indices_in_family.insert(index) && *family_type != unrestricted {
                            valid = false;
                            reason += "Found duplicate index. ";
                        }
                    }
                }
            }

            if *family_type == partition_token && indices_in_family.len() != element_count {
                valid = false;
                reason += &format!(
                    "Number of unique indices does not match the element count {}. ",
                    element_count
                );
            }

            if element_count > 0 && !indices_in_family.is_empty() {
                if let Some(&max_idx) = indices_in_family.iter().next_back() {
                    if max_idx as usize >= element_count {
                        valid = false;
                        reason +=
                            "Found one or more indices that are greater than the element count. ";
                    }
                }
                if let Some(&min_idx) = indices_in_family.iter().next() {
                    if min_idx < 0 {
                        valid = false;
                        reason += "Found one or more indices that are less than 0. ";
                    }
                }
            }
        }

        (valid, reason)
    }

    /// Validates whether the family of subsets identified by the given
    /// family name and element type on the given imageable contains valid data.
    ///
    /// Matches C++ `UsdGeomSubset::ValidateFamily()`.
    pub fn validate_family(
        geom: &Imageable,
        element_type: &Token,
        family_name: &Token,
    ) -> (bool, String) {
        let tokens = usd_geom_tokens();
        if !validate_geom_type(geom, element_type) {
            return (
                false,
                format!(
                    "Invalid geom type for elementType {}.",
                    element_type.as_str()
                ),
            );
        }

        let subsets = Self::get_geom_subsets(geom, &Token::new(""), family_name);
        let family_type = Self::get_family_type(geom, family_name);
        let family_is_restricted = family_type != tokens.unrestricted;
        let is_edge = *element_type == tokens.edge;
        let is_segment = *element_type == tokens.segment;
        let is_element_count_time_varying = element_count_might_be_time_varying(geom, element_type);
        let earliest_time = if is_element_count_time_varying {
            TimeCode::new(f64::MIN)
        } else {
            TimeCode::default_time()
        };
        let mut earliest_time_element_count =
            get_element_count(geom, element_type, earliest_time).unwrap_or(0);
        if earliest_time_element_count == 0 {
            earliest_time_element_count =
                get_element_count(geom, element_type, TimeCode::default_time()).unwrap_or(0);
        }

        let mut valid = true;
        let mut reason = String::new();
        if !is_element_count_time_varying && earliest_time_element_count == 0 {
            valid = false;
            reason += &format!(
                "Unable to determine element count at earliest time for geom <{}>. ",
                geom.prim().path().get_string()
            );
        }

        let mut all_time_samples = std::collections::BTreeSet::<u64>::new();
        for subset in &subsets {
            let subset_element_type =
                get_token_value(&subset.get_element_type_attr(), TimeCode::default_time());
            if subset_element_type != *element_type {
                return (
                    false,
                    format!(
                        "GeomSubset at path <{}> has elementType '{}', which does not match '{}'.",
                        subset.prim().path().get_string(),
                        subset_element_type.as_str(),
                        element_type.as_str()
                    ),
                );
            }

            for sample in subset.get_indices_attr().get_time_samples() {
                all_time_samples.insert(sample.to_bits());
            }
        }

        let mut time_codes = vec![TimeCode::default_time()];
        for bits in all_time_samples {
            time_codes.push(TimeCode::new(f64::from_bits(bits)));
        }

        let mut has_indices_at_any_time = false;
        for time in time_codes {
            let mut indices_in_family = std::collections::BTreeSet::new();

            for subset in &subsets {
                let indices = subset
                    .get_indices(time)
                    .unwrap_or_else(usd_vt::Array::<i32>::new);

                if (is_edge || is_segment) && indices.as_slice().len() % 2 != 0 {
                    valid = false;
                    reason += &format!(
                        "Indices attribute has an odd number of elements in GeomSubset at path <{}> at time {} with elementType {}. ",
                        subset.prim().path().get_string(),
                        time,
                        element_type.as_str()
                    );
                }

                if !family_is_restricted || is_edge || is_segment {
                    indices_in_family.extend(indices.as_slice().iter().copied());
                } else {
                    for &index in indices.as_slice() {
                        if !indices_in_family.insert(index) {
                            valid = false;
                            reason += &format!(
                                "Found duplicate index {} in GeomSubset at path <{}> at time {}. ",
                                index,
                                subset.prim().path().get_string(),
                                time
                            );
                        }
                    }
                }
            }

            let element_count = if is_element_count_time_varying {
                get_element_count(geom, element_type, time).unwrap_or(0)
            } else {
                earliest_time_element_count
            };

            if !indices_in_family.is_empty() && is_element_count_time_varying && element_count == 0
            {
                valid = false;
                reason += &format!(
                    "Geometry <{}> has no elements at time {}, but the \"{}\" GeomSubset family contains indices. ",
                    geom.prim().path().get_string(),
                    time,
                    family_name.as_str()
                );
            }

            if !is_edge && !is_segment {
                if family_type == tokens.partition && indices_in_family.len() != element_count {
                    valid = false;
                    reason += &format!(
                        "Number of unique indices at time {} does not match the element count {}. ",
                        time, element_count
                    );
                }
            } else {
                let mut pairs_in_family = std::collections::BTreeSet::new();
                for subset in &subsets {
                    let subset_pairs = get_index_pairs(subset, time, is_segment);
                    if !family_is_restricted {
                        pairs_in_family.extend(subset_pairs);
                    } else {
                        for pair in subset_pairs {
                            if !pairs_in_family.insert(pair) {
                                valid = false;
                                let label = if is_edge { "edge" } else { "segment" };
                                reason += &format!(
                                    "Found duplicate {} ({}, {}) in GeomSubset at path <{}> at time {}. ",
                                    label,
                                    pair.0,
                                    pair.1,
                                    subset.prim().path().get_string(),
                                    time
                                );
                            }
                        }
                    }
                }

                if is_edge {
                    if let Some(possible_pairs) = get_edges_from_prim(geom, time) {
                        if !pairs_in_family.is_subset(&possible_pairs) {
                            valid = false;
                            reason += &format!(
                                "At least one edge in family {} at time {} does not exist on the parent prim. ",
                                family_name.as_str(),
                                time
                            );
                        }

                        if family_type == tokens.partition
                            && possible_pairs.difference(&pairs_in_family).next().is_some()
                        {
                            valid = false;
                            reason += &format!(
                                "Number of unique indices at time {} does not match the element count {}. ",
                                time, element_count
                            );
                        }
                    }
                } else if let Some(possible_pairs) = get_all_possible_segments(geom, time) {
                    let prim = geom.prim();
                    let curve_vertex_counts =
                        get_int_array(prim, tokens.curve_vertex_counts.as_str(), time)
                            .unwrap_or_default();
                    let segment_counts =
                        BasisCurves::new(prim.clone()).compute_segment_counts(time);

                    for segment in &pairs_in_family {
                        let curve_index = segment.0;
                        if curve_index < 0 {
                            continue;
                        }

                        let curve_index_usize = curve_index as usize;
                        if curve_index_usize >= curve_vertex_counts.len() {
                            valid = false;
                            reason += &format!(
                                "Found one or more indices that are greater than the curve vertex count {} at time {}. ",
                                curve_vertex_counts.len(),
                                time
                            );
                            continue;
                        }

                        let max_segments = segment_counts
                            .get(curve_index_usize)
                            .copied()
                            .unwrap_or_default();
                        if segment.1 >= max_segments {
                            valid = false;
                            reason += &format!(
                                "Found one or more indices that are greater than the segment count {} at time {}. ",
                                max_segments, time
                            );
                        }
                    }

                    if family_type == tokens.partition
                        && possible_pairs.difference(&pairs_in_family).next().is_some()
                    {
                        valid = false;
                        reason += &format!(
                            "Number of unique indices at time {} does not match the element count {}. ",
                            time, element_count
                        );
                    }
                }
            }

            if indices_in_family.is_empty() {
                continue;
            }

            has_indices_at_any_time = true;

            if !is_segment {
                if let Some(&last_index) = indices_in_family.iter().next_back() {
                    if element_count > 0 && last_index >= 0 && last_index as usize >= element_count
                    {
                        valid = false;
                        reason += &format!(
                            "Found one or more indices that are greater than the element count {} at time {}. ",
                            element_count, time
                        );
                    }
                }
            }

            if let Some(&first_index) = indices_in_family.iter().next() {
                if first_index < 0 {
                    valid = false;
                    reason += &format!(
                        "Found one or more indices that are less than 0 at time {}. ",
                        time
                    );
                }
            }
        }

        if !has_indices_at_any_time {
            valid = false;
            reason += "No indices in family at any time.";
        }

        (valid, reason)
    }

    /// Get indices not assigned to any subset in the given family.
    ///
    /// Returns indices in [0, element_count) that don't appear in any subset.
    ///
    /// Matches C++ `UsdGeomSubset::GetUnassignedIndices()` (deprecated overload).
    pub fn get_unassigned_indices(
        subsets: &[Subset],
        element_count: usize,
        time: TimeCode,
    ) -> Vec<i32> {
        let mut assigned = std::collections::BTreeSet::new();

        for subset in subsets {
            if let Some(indices) = subset.get_indices(time) {
                for &idx in indices.as_slice() {
                    if idx >= 0 {
                        assigned.insert(idx);
                    }
                }
            }
        }

        let mut result = Vec::new();
        for idx in 0..element_count as i32 {
            if !assigned.contains(&idx) {
                result.push(idx);
            }
        }
        result
    }

    /// Get unassigned indices for a family on an imageable.
    ///
    /// For edge/segment element types, returns flat pairs [v0, v1, ...].
    ///
    /// Matches C++ `UsdGeomSubset::GetUnassignedIndices(geom, elementType, familyName, time)`.
    pub fn get_unassigned_indices_for_family(
        geom: &Imageable,
        element_type: &Token,
        family_name: &Token,
        time: TimeCode,
    ) -> Vec<i32> {
        let tokens = usd_geom_tokens();
        let subsets = Self::get_geom_subsets(geom, element_type, family_name);

        if *element_type == tokens.edge {
            let prim = geom.prim();
            let fvc = get_int_array(prim, tokens.face_vertex_counts.as_str(), time);
            let fvi = get_int_array(prim, tokens.face_vertex_indices.as_str(), time);
            if let (Some(counts), Some(indices)) = (fvc, fvi) {
                // Build unique sorted edges from topology
                let mut edge_set = std::collections::BTreeSet::new();
                let mut offset = 0usize;
                for &count in &counts {
                    let n = count as usize;
                    for i in 0..n {
                        let v0 = indices[offset + i];
                        let v1 = indices[offset + (i + 1) % n];
                        let edge = if v0 <= v1 { (v0, v1) } else { (v1, v0) };
                        edge_set.insert(edge);
                    }
                    offset += n;
                }
                let unique_edges: Vec<(i32, i32)> = edge_set.into_iter().collect();

                let mut assigned = std::collections::BTreeSet::<(i32, i32)>::new();
                for subset in &subsets {
                    if let Some(idx_arr) = subset.get_indices(time) {
                        let slice = idx_arr.as_slice();
                        for chunk in slice.chunks(2) {
                            if chunk.len() == 2 && chunk[0] >= 0 && chunk[1] >= 0 {
                                let e = if chunk[0] <= chunk[1] {
                                    (chunk[0], chunk[1])
                                } else {
                                    (chunk[1], chunk[0])
                                };
                                assigned.insert(e);
                            }
                        }
                    }
                }

                let mut result = Vec::new();
                for &(v0, v1) in &unique_edges {
                    if !assigned.contains(&(v0, v1)) {
                        result.push(v0);
                        result.push(v1);
                    }
                }
                return result;
            }
            return Vec::new();
        }

        if *element_type == tokens.segment {
            let prim = geom.prim();
            let cvc = get_int_array(prim, tokens.curve_vertex_counts.as_str(), time);
            if let Some(counts) = cvc {
                let curve_type = prim
                    .get_attribute("type")
                    .map(|a| get_token_value(&a, TimeCode::default_time()))
                    .unwrap_or_else(|| Token::new(""));
                let wrap = prim
                    .get_attribute("wrap")
                    .map(|a| get_token_value(&a, TimeCode::default_time()))
                    .unwrap_or_else(|| Token::new(""));
                let is_periodic = wrap.as_str() == "periodic";
                let is_linear = curve_type.as_str() == "linear";

                let mut all_segments = Vec::new();
                for (ci, &count) in counts.iter().enumerate() {
                    let n = count as usize;
                    let segs = if is_linear {
                        if is_periodic { n } else { n.saturating_sub(1) }
                    } else if is_periodic {
                        n
                    } else if wrap.as_str() == "pinned" {
                        n.saturating_sub(2)
                    } else {
                        n.saturating_sub(3)
                    };
                    for si in 0..segs {
                        all_segments.push((ci as i32, si as i32));
                    }
                }

                let mut assigned = std::collections::BTreeSet::<(i32, i32)>::new();
                for subset in &subsets {
                    if let Some(idx_arr) = subset.get_indices(time) {
                        let slice = idx_arr.as_slice();
                        for chunk in slice.chunks(2) {
                            if chunk.len() == 2 && chunk[0] >= 0 && chunk[1] >= 0 {
                                assigned.insert((chunk[0], chunk[1]));
                            }
                        }
                    }
                }

                let mut result = Vec::new();
                for &(ci, si) in &all_segments {
                    if !assigned.contains(&(ci, si)) {
                        result.push(ci);
                        result.push(si);
                    }
                }
                return result;
            }
            return Vec::new();
        }

        // Standard element types: face, point, tetrahedron
        let element_count = get_element_count(geom, element_type, time).unwrap_or(0);
        Self::get_unassigned_indices(&subsets, element_count, time)
    }

    // ========================================================================
    // Schema Attribute Names
    // ========================================================================

    /// Return a vector of names of all pre-declared attributes for this schema
    /// class and all its ancestor classes.
    ///
    /// Matches C++ `GetSchemaAttributeNames(bool includeInherited)`.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let local_names = vec![
            usd_geom_tokens().element_type.clone(),
            usd_geom_tokens().indices.clone(),
            usd_geom_tokens().family_name.clone(),
        ];

        if include_inherited {
            let mut all_names = Typed::get_schema_attribute_names(true);
            all_names.extend(local_names);
            all_names
        } else {
            local_names
        }
    }
}

impl PartialEq for Subset {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for Subset {}
