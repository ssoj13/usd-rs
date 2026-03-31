//! UsdGeomBasisCurves - basis curves geometry schema.
//!
//! Port of pxr/usd/usdGeom/basisCurves.h/cpp
//!
//! BasisCurves are a batched curve representation analogous to the classic RIB
//! definition via Basis and Curves statements.

use super::curves::Curves;
use super::tokens::usd_geom_tokens;
use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, Stage};
use usd_sdf::{TimeCode, ValueTypeRegistry};
use usd_tf::Token;
use usd_vt::Value;

/// Extract a Token from a Value that may be stored as String or Token.
fn token_from_value(v: &Value) -> Option<Token> {
    if let Some(t) = v.get::<Token>() {
        Some(t.clone())
    } else if let Some(s) = v.get::<String>() {
        Some(Token::new(s))
    } else {
        None
    }
}

// ============================================================================
// BasisCurves
// ============================================================================

/// Basis curves geometry schema.
///
/// BasisCurves are a batched curve representation analogous to the classic RIB
/// definition via Basis and Curves statements.
///
/// Matches C++ `UsdGeomBasisCurves`.
#[derive(Debug, Clone)]
pub struct BasisCurves {
    /// Base curves schema.
    inner: Curves,
}

impl BasisCurves {
    /// Creates a BasisCurves schema from a prim.
    ///
    /// Matches C++ `UsdGeomBasisCurves(const UsdPrim& prim)`.
    pub fn new(prim: Prim) -> Self {
        Self {
            inner: Curves::new(prim),
        }
    }

    /// Creates a BasisCurves schema from a Curves schema.
    ///
    /// Matches C++ `UsdGeomBasisCurves(const UsdSchemaBase& schemaObj)`.
    pub fn from_curves(curves: Curves) -> Self {
        Self { inner: curves }
    }

    /// Creates an invalid BasisCurves schema.
    pub fn invalid() -> Self {
        Self {
            inner: Curves::invalid(),
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

    /// Returns the curves base.
    pub fn curves(&self) -> &Curves {
        &self.inner
    }

    /// Returns the schema type name.
    pub fn schema_type_name() -> Token {
        Token::new("BasisCurves")
    }

    /// Return a BasisCurves holding the prim adhering to this schema at path on stage.
    ///
    /// Matches C++ `UsdGeomBasisCurves::Get(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn get(stage: &Stage, path: &usd_sdf::Path) -> Self {
        if let Some(prim) = stage.get_prim_at_path(path) {
            Self::new(prim)
        } else {
            Self::invalid()
        }
    }

    /// Attempt to ensure a prim adhering to this schema at path is defined on this stage.
    ///
    /// Matches C++ `UsdGeomBasisCurves::Define(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn define(stage: &Stage, path: &usd_sdf::Path) -> Self {
        match stage.define_prim(path.get_string(), Self::schema_type_name().as_str()) {
            Ok(prim) => Self::new(prim),
            Err(_) => Self::invalid(),
        }
    }

    // ========================================================================
    // Type
    // ========================================================================

    /// Returns the type attribute.
    ///
    /// Linear curves interpolate linearly between two vertices.
    /// Cubic curves use a basis matrix with four vertices to interpolate a segment.
    ///
    /// Matches C++ `GetTypeAttr()`.
    pub fn get_type_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().type_.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the type attribute.
    ///
    /// Matches C++ `CreateTypeAttr()`.
    pub fn create_type_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().type_.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().type_.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        prim.create_attribute(
            usd_geom_tokens().type_.as_str(),
            &token_type,
            false,                      // not custom
            Some(Variability::Uniform), // uniform
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // Basis
    // ========================================================================

    /// Returns the basis attribute.
    ///
    /// The basis specifies the vstep and matrix used for cubic interpolation.
    ///
    /// Matches C++ `GetBasisAttr()`.
    pub fn get_basis_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().basis.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the basis attribute.
    ///
    /// Matches C++ `CreateBasisAttr()`.
    pub fn create_basis_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().basis.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().basis.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        prim.create_attribute(
            usd_geom_tokens().basis.as_str(),
            &token_type,
            false,                      // not custom
            Some(Variability::Uniform), // uniform
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // Wrap
    // ========================================================================

    /// Returns the wrap attribute.
    ///
    /// If wrap is set to periodic, the curve when rendered will repeat the initial vertices.
    /// If wrap is set to 'pinned', phantom points may be created.
    ///
    /// Matches C++ `GetWrapAttr()`.
    pub fn get_wrap_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().wrap.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the wrap attribute.
    ///
    /// Matches C++ `CreateWrapAttr()`.
    pub fn create_wrap_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().wrap.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().wrap.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        prim.create_attribute(
            usd_geom_tokens().wrap.as_str(),
            &token_type,
            false,                      // not custom
            Some(Variability::Uniform), // uniform
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // Helper Methods
    // ========================================================================

    /// Get vstep for a given basis.
    ///
    /// Returns the vstep value: bezier=3, bspline=1, catmullRom=1.
    fn get_vstep_for_basis(basis: &Token) -> usize {
        let basis_str = basis.as_str();
        if basis_str == usd_geom_tokens().bezier.as_str() {
            3
        } else if basis_str == usd_geom_tokens().bspline.as_str() {
            1
        } else if basis_str == usd_geom_tokens().catmull_rom.as_str() {
            1
        } else {
            0 // unknown basis
        }
    }

    /// Computes the expected size for data with "uniform" interpolation.
    ///
    /// Matches C++ `ComputeUniformDataSize()`.
    pub fn compute_uniform_data_size(&self, time_code: TimeCode) -> usize {
        let vertex_counts_attr = self.inner.get_curve_vertex_counts_attr();
        if !vertex_counts_attr.is_valid() {
            return 0;
        }

        if let Some(value) = vertex_counts_attr.get(time_code) {
            if let Some(counts) = value.get::<Vec<i32>>() {
                return counts.len();
            } else if let Some(counts) = value.get::<usd_vt::Array<i32>>() {
                return counts.len();
            }
        }
        0
    }

    /// Computes the expected size for data with "varying" interpolation.
    ///
    /// Matches C++ `ComputeVaryingDataSize()`.
    pub fn compute_varying_data_size(&self, time_code: TimeCode) -> usize {
        let vertex_counts_attr = self.inner.get_curve_vertex_counts_attr();
        if !vertex_counts_attr.is_valid() {
            return 0;
        }

        let mut curve_vertex_counts: Vec<i32> = Vec::new();
        if let Some(value) = vertex_counts_attr.get(time_code) {
            if let Some(counts) = value.get::<Vec<i32>>() {
                curve_vertex_counts = counts.clone();
            } else if let Some(counts) = value.get::<usd_vt::Array<i32>>() {
                curve_vertex_counts = counts.iter().cloned().collect();
            }
        }

        if curve_vertex_counts.is_empty() {
            return 0;
        }

        // Get type and wrap
        let curves_type = self
            .get_type_attr()
            .get(time_code)
            .and_then(|v| token_from_value(&v))
            .unwrap_or_else(|| usd_geom_tokens().cubic.clone());

        let wrap = self
            .get_wrap_attr()
            .get(time_code)
            .and_then(|v| token_from_value(&v))
            .unwrap_or_else(|| usd_geom_tokens().nonperiodic.clone());

        let mut result = 0;

        if curves_type == usd_geom_tokens().linear {
            if wrap == usd_geom_tokens().nonperiodic || wrap == usd_geom_tokens().pinned {
                for count in &curve_vertex_counts {
                    result += (*count).max(0) as usize;
                }
            } else {
                // periodic
                for count in &curve_vertex_counts {
                    result += ((*count).max(0) as usize) + 1;
                }
            }
            return result;
        }

        // Cubic curves
        let basis = self
            .get_basis_attr()
            .get(time_code)
            .and_then(|v| token_from_value(&v))
            .unwrap_or_else(|| usd_geom_tokens().bezier.clone());

        let vstep = Self::get_vstep_for_basis(&basis);

        if wrap == usd_geom_tokens().nonperiodic || wrap == usd_geom_tokens().pinned {
            for count in &curve_vertex_counts {
                let count_usize = (*count).max(0) as usize;
                result += ((count_usize.saturating_sub(4)) / vstep.max(1)).max(0) + 2;
            }
        } else {
            // periodic
            for count in &curve_vertex_counts {
                result += ((*count).max(0) as usize) / vstep.max(1);
            }
        }

        result
    }

    /// Computes the expected size for data with "vertex" interpolation.
    ///
    /// Matches C++ `ComputeVertexDataSize()`.
    pub fn compute_vertex_data_size(&self, time_code: TimeCode) -> usize {
        let vertex_counts_attr = self.inner.get_curve_vertex_counts_attr();
        if !vertex_counts_attr.is_valid() {
            return 0;
        }

        let mut result = 0;
        if let Some(value) = vertex_counts_attr.get(time_code) {
            if let Some(counts) = value.get::<Vec<i32>>() {
                for count in counts {
                    result += (*count).max(0) as usize;
                }
            } else if let Some(counts) = value.get::<usd_vt::Array<i32>>() {
                for count in counts.iter() {
                    result += (*count).max(0) as usize;
                }
            }
        }
        result
    }

    /// Computes the segment counts of the curves based on their vertex counts.
    ///
    /// Matches C++ `ComputeSegmentCounts()`.
    pub fn compute_segment_counts(&self, time_code: TimeCode) -> Vec<i32> {
        let vertex_counts_attr = self.inner.get_curve_vertex_counts_attr();
        if !vertex_counts_attr.is_valid() {
            return Vec::new();
        }

        let mut curve_vertex_counts: Vec<i32> = Vec::new();
        if let Some(value) = vertex_counts_attr.get(time_code) {
            if let Some(counts) = value.get::<Vec<i32>>() {
                curve_vertex_counts = counts.clone();
            } else if let Some(counts) = value.get::<usd_vt::Array<i32>>() {
                curve_vertex_counts = counts.iter().cloned().collect();
            }
        }

        if curve_vertex_counts.is_empty() {
            return Vec::new();
        }

        // Get type, wrap, and basis
        let curves_type = self
            .get_type_attr()
            .get(time_code)
            .and_then(|v| token_from_value(&v))
            .unwrap_or_else(|| usd_geom_tokens().cubic.clone());

        let wrap = self
            .get_wrap_attr()
            .get(time_code)
            .and_then(|v| token_from_value(&v))
            .unwrap_or_else(|| usd_geom_tokens().nonperiodic.clone());

        let basis = self
            .get_basis_attr()
            .get(time_code)
            .and_then(|v| token_from_value(&v))
            .unwrap_or_else(|| usd_geom_tokens().bezier.clone());

        let mut segment_counts = vec![0i32; curve_vertex_counts.len()];
        let mut is_valid = false;

        if curves_type == usd_geom_tokens().linear {
            if wrap == usd_geom_tokens().periodic {
                segment_counts = curve_vertex_counts.clone();
                is_valid = true;
            } else if wrap == usd_geom_tokens().nonperiodic || wrap == usd_geom_tokens().pinned {
                for (i, count) in curve_vertex_counts.iter().enumerate() {
                    segment_counts[i] = *count - 1;
                }
                is_valid = true;
            }
        } else if curves_type == usd_geom_tokens().cubic {
            if basis == usd_geom_tokens().bezier {
                const VSTEP: i32 = 3;
                if wrap == usd_geom_tokens().periodic {
                    for (i, count) in curve_vertex_counts.iter().enumerate() {
                        segment_counts[i] = *count / VSTEP;
                    }
                    is_valid = true;
                } else if wrap == usd_geom_tokens().nonperiodic || wrap == usd_geom_tokens().pinned
                {
                    for (i, count) in curve_vertex_counts.iter().enumerate() {
                        segment_counts[i] = (*count - 4) / VSTEP + 1;
                    }
                    is_valid = true;
                }
            } else if basis == usd_geom_tokens().bspline || basis == usd_geom_tokens().catmull_rom {
                if wrap == usd_geom_tokens().periodic {
                    segment_counts = curve_vertex_counts.clone();
                    is_valid = true;
                } else if wrap == usd_geom_tokens().nonperiodic {
                    for (i, count) in curve_vertex_counts.iter().enumerate() {
                        segment_counts[i] = *count - 3;
                    }
                    is_valid = true;
                } else if wrap == usd_geom_tokens().pinned {
                    for (i, count) in curve_vertex_counts.iter().enumerate() {
                        segment_counts[i] = *count - 1;
                    }
                    is_valid = true;
                }
            }
        }

        if !is_valid {
            return Vec::new();
        }

        segment_counts
    }

    // ========================================================================
    // Get Methods (Value Retrieval)
    // ========================================================================

    /// Get the type at the specified time.
    ///
    /// Matches C++ `GetType(TfToken* type, UsdTimeCode time)`.
    pub fn get_type(&self, time: TimeCode) -> Option<Token> {
        self.get_type_attr().get_typed::<Token>(time)
    }

    /// Get the basis at the specified time.
    ///
    /// Matches C++ `GetBasis(TfToken* basis, UsdTimeCode time)`.
    pub fn get_basis(&self, time: TimeCode) -> Option<Token> {
        self.get_basis_attr().get_typed::<Token>(time)
    }

    /// Get the wrap at the specified time.
    ///
    /// Matches C++ `GetWrap(TfToken* wrap, UsdTimeCode time)`.
    pub fn get_wrap(&self, time: TimeCode) -> Option<Token> {
        self.get_wrap_attr().get_typed::<Token>(time)
    }

    // ========================================================================
    // Compute Interpolation For Size
    // ========================================================================

    /// Computes interpolation token for the given size.
    ///
    /// If this returns an empty token and info was Some, it'll contain
    /// the expected value for each token.
    ///
    /// The topology is determined using time_code.
    ///
    /// Matches C++ `ComputeInterpolationForSize(size_t n, const UsdTimeCode& timeCode, ComputeInterpolationInfo* info)`.
    pub fn compute_interpolation_for_size(
        &self,
        n: usize,
        time_code: TimeCode,
        mut info: Option<&mut Vec<(Token, usize)>>,
    ) -> Token {
        if let Some(ref mut info_vec) = info {
            info_vec.clear();
        }

        // Check constant (size 1)
        if n == 1 {
            return usd_geom_tokens().constant.clone();
        }
        if let Some(ref mut info_vec) = info {
            info_vec.push((usd_geom_tokens().constant.clone(), 1));
        }

        // Get curve vertex counts
        let curve_vertex_counts_attr = self.inner.get_curve_vertex_counts_attr();
        let mut curve_vertex_counts: Vec<i32> = Vec::new();
        if curve_vertex_counts_attr.is_valid() {
            if let Some(value) = curve_vertex_counts_attr.get(time_code) {
                if let Some(counts) = value.get::<Vec<i32>>() {
                    curve_vertex_counts = counts.clone();
                } else if let Some(counts) = value.get::<usd_vt::Array<i32>>() {
                    curve_vertex_counts = counts.iter().cloned().collect();
                }
            }
        }

        // Check uniform (size = number of curves)
        let num_uniform = curve_vertex_counts.len();
        if n == num_uniform {
            return usd_geom_tokens().uniform.clone();
        }
        if let Some(ref mut info_vec) = info {
            info_vec.push((usd_geom_tokens().uniform.clone(), num_uniform));
        }

        // Check varying
        let num_varying = self.compute_varying_data_size(time_code);
        if n == num_varying {
            return usd_geom_tokens().varying.clone();
        }
        if let Some(ref mut info_vec) = info {
            info_vec.push((usd_geom_tokens().varying.clone(), num_varying));
        }

        // Check vertex
        let num_vertex = self.compute_vertex_data_size(time_code);
        if n == num_vertex {
            return usd_geom_tokens().vertex.clone();
        }
        if let Some(ref mut info_vec) = info {
            info_vec.push((usd_geom_tokens().vertex.clone(), num_vertex));
        }

        // No match found
        Token::new("")
    }

    // ========================================================================
    // Compute Extent At Time
    // ========================================================================

    /// Compute the extent for the basis curves at the specified time.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    /// On success, extent will contain the axis-aligned bounding box of the curves.
    ///
    /// Matches C++ `ComputeExtentAtTime(VtVec3fArray* extent, UsdTimeCode time, UsdTimeCode baseTime)`.
    pub fn compute_extent_at_time(
        &self,
        extent: &mut Vec<usd_gf::vec3::Vec3f>,
        time: TimeCode,
        base_time: TimeCode,
    ) -> bool {
        // Use Curves base implementation
        self.inner.compute_extent_at_time(extent, time, base_time)
    }

    /// Compute the extent for the basis curves at the specified time with transform.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    ///
    /// Matches C++ `ComputeExtentAtTime(VtVec3fArray* extent, UsdTimeCode time, UsdTimeCode baseTime, const GfMatrix4d& transform)`.
    pub fn compute_extent_at_time_with_transform(
        &self,
        extent: &mut Vec<usd_gf::vec3::Vec3f>,
        time: TimeCode,
        base_time: TimeCode,
        transform: &usd_gf::matrix4::Matrix4d,
    ) -> bool {
        // Use Curves base implementation
        self.inner
            .compute_extent_at_time_with_transform(extent, time, base_time, transform)
    }

    /// Compute the extent for the basis curves at multiple times.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    ///
    /// Matches C++ `ComputeExtentAtTimes(std::vector<VtVec3fArray>* extents, const std::vector<UsdTimeCode>& times, UsdTimeCode baseTime)`.
    pub fn compute_extent_at_times(
        &self,
        extents: &mut Vec<Vec<usd_gf::vec3::Vec3f>>,
        times: &[TimeCode],
        base_time: TimeCode,
    ) -> bool {
        // Use Curves base implementation
        self.inner
            .compute_extent_at_times(extents, times, base_time)
    }

    /// Compute the extent for the basis curves at multiple times with transform.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    ///
    /// Matches C++ `ComputeExtentAtTimes(std::vector<VtVec3fArray>* extents, const std::vector<UsdTimeCode>& times, UsdTimeCode baseTime, const GfMatrix4d& transform)`.
    pub fn compute_extent_at_times_with_transform(
        &self,
        extents: &mut Vec<Vec<usd_gf::vec3::Vec3f>>,
        times: &[TimeCode],
        base_time: TimeCode,
        transform: &usd_gf::matrix4::Matrix4d,
    ) -> bool {
        // Use Curves base implementation
        self.inner
            .compute_extent_at_times_with_transform(extents, times, base_time, transform)
    }

    /// Return a vector of names of all pre-declared attributes for this schema
    /// class and all its ancestor classes.
    ///
    /// Matches C++ `GetSchemaAttributeNames(bool includeInherited)`.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let local_names = vec![
            usd_geom_tokens().type_.clone(),
            usd_geom_tokens().basis.clone(),
            usd_geom_tokens().wrap.clone(),
        ];

        if include_inherited {
            let mut all_names = Curves::get_schema_attribute_names(true);
            all_names.extend(local_names);
            all_names
        } else {
            local_names
        }
    }
}

impl PartialEq for BasisCurves {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for BasisCurves {}
