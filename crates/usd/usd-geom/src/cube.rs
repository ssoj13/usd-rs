//! UsdGeomCube - cube geometry schema.
//!
//! Port of pxr/usd/usdGeom/cube.h/cpp
//!
//! Defines a primitive rectilinear cube centered at the origin.

use super::gprim::Gprim;
use super::tokens::usd_geom_tokens;
use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, Stage};
use usd_gf::bbox3d::BBox3d;
use usd_gf::matrix4::Matrix4d;
use usd_gf::range::Range3d;
use usd_gf::vec3::Vec3d;
use usd_gf::vec3::Vec3f;
use usd_sdf::TimeCode;
use usd_sdf::ValueTypeRegistry;
use usd_tf::Token;
use usd_vt::Value;

// ============================================================================
// Cube
// ============================================================================

/// Cube geometry schema.
///
/// Defines a primitive rectilinear cube centered at the origin.
///
/// Matches C++ `UsdGeomCube`.
#[derive(Debug, Clone)]
pub struct Cube {
    /// Base gprim schema.
    inner: Gprim,
}

impl Cube {
    /// Creates a Cube schema from a prim.
    ///
    /// Matches C++ `UsdGeomCube(const UsdPrim& prim)`.
    pub fn new(prim: Prim) -> Self {
        Self {
            inner: Gprim::new(prim),
        }
    }

    /// Creates a Cube schema from a Gprim schema.
    ///
    /// Matches C++ `UsdGeomCube(const UsdSchemaBase& schemaObj)`.
    pub fn from_gprim(gprim: Gprim) -> Self {
        Self { inner: gprim }
    }

    /// Creates an invalid Cube schema.
    pub fn invalid() -> Self {
        Self {
            inner: Gprim::invalid(),
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

    /// Returns the gprim base.
    pub fn gprim(&self) -> &Gprim {
        &self.inner
    }

    /// Returns the schema type name.
    pub fn schema_type_name() -> Token {
        Token::new("Cube")
    }

    /// Return a Cube holding the prim adhering to this schema at path on stage.
    ///
    /// Matches C++ `UsdGeomCube::Get(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn get(stage: &Stage, path: &usd_sdf::Path) -> Self {
        if let Some(prim) = stage.get_prim_at_path(path) {
            Self::new(prim)
        } else {
            Self::invalid()
        }
    }

    /// Attempt to ensure a prim adhering to this schema at path is defined on this stage.
    ///
    /// Matches C++ `UsdGeomCube::Define(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn define(stage: &Stage, path: &usd_sdf::Path) -> Self {
        match stage.define_prim(path.get_string(), Self::schema_type_name().as_str()) {
            Ok(prim) => Self::new(prim),
            Err(_) => Self::invalid(),
        }
    }

    // ========================================================================
    // Size
    // ========================================================================

    /// Returns the size attribute.
    ///
    /// Indicates the length of each edge of the cube. If you author size you must also author extent.
    ///
    /// Matches C++ `GetSizeAttr()`.
    pub fn get_size_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().size.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the size attribute.
    ///
    /// Matches C++ `CreateSizeAttr()`.
    pub fn create_size_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().size.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().size.as_str())
                .unwrap_or_else(Attribute::invalid);
        }

        let registry = ValueTypeRegistry::instance();
        let double_type = registry.find_type_by_token(&Token::new("double"));

        let attr = prim
            .create_attribute(
                usd_geom_tokens().size.as_str(),
                &double_type,
                false,                      // not custom
                Some(Variability::Varying), // can vary over time
            )
            .unwrap_or_else(Attribute::invalid);
        if let Some(val) = default_value {
            let _ = attr.set(val, TimeCode::default());
        }
        attr
    }

    // ========================================================================
    // Extent
    // ========================================================================

    /// Returns the extent attribute.
    ///
    /// Extent is re-defined on Cube only to provide a fallback value.
    ///
    /// Matches C++ `GetExtentAttr()`.
    pub fn get_extent_attr(&self) -> Attribute {
        self.inner.boundable().get_extent_attr()
    }

    /// Creates the extent attribute.
    ///
    /// Matches C++ `CreateExtentAttr()`.
    pub fn create_extent_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        self.inner
            .boundable()
            .create_extent_attr(default_value, write_sparsely)
    }

    // ========================================================================
    // Compute Extent
    // ========================================================================

    /// Compute the extent for the cube defined by the size.
    ///
    /// Returns true upon success, false if unable to calculate extent.
    ///
    /// On success, extent will contain an approximate axis-aligned bounding
    /// box of the cube defined by the size.
    ///
    /// Matches C++ `ComputeExtent(double size, VtVec3fArray* extent)`.
    pub fn compute_extent(size: f64, extent: &mut [Vec3f; 2]) -> bool {
        let half_size = size * 0.5;
        extent[0] = Vec3f::new(-half_size as f32, -half_size as f32, -half_size as f32);
        extent[1] = Vec3f::new(half_size as f32, half_size as f32, half_size as f32);
        true
    }

    /// Compute the extent as if the matrix transform was first applied.
    ///
    /// Matches C++ `ComputeExtent(double size, const GfMatrix4d& transform, VtVec3fArray* extent)`.
    pub fn compute_extent_with_transform(
        size: f64,
        transform: &Matrix4d,
        extent: &mut [Vec3f; 2],
    ) -> bool {
        let half_size = size * 0.5;
        // Create bounding box from cube range
        let range = Range3d::new(
            Vec3d::new(-half_size, -half_size, -half_size),
            Vec3d::new(half_size, half_size, half_size),
        );
        let bbox = BBox3d::from_range_matrix(range, *transform);
        let aligned_range = bbox.compute_aligned_range();

        let min_vec = aligned_range.min();
        let max_vec = aligned_range.max();
        extent[0] = Vec3f::new(min_vec.x as f32, min_vec.y as f32, min_vec.z as f32);
        extent[1] = Vec3f::new(max_vec.x as f32, max_vec.y as f32, max_vec.z as f32);
        true
    }

    // ========================================================================
    // Get Methods (Value Retrieval)
    // ========================================================================

    /// Get the size at the specified time.
    ///
    /// Matches C++ `GetSize(double* size, UsdTimeCode time)`.
    pub fn get_size(&self, time: usd_sdf::TimeCode) -> Option<f64> {
        self.get_size_attr().get_typed::<f64>(time)
    }

    // ========================================================================
    // Compute Extent At Time
    // ========================================================================

    /// Compute the extent for the cube at the specified time.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    /// On success, extent will contain the axis-aligned bounding box of the cube.
    ///
    /// Matches C++ `ComputeExtentAtTime(VtVec3fArray* extent, UsdTimeCode time, UsdTimeCode baseTime)`.
    pub fn compute_extent_at_time(
        &self,
        extent: &mut Vec<Vec3f>,
        time: usd_sdf::TimeCode,
        _base_time: usd_sdf::TimeCode,
    ) -> bool {
        // Get size at the specified time
        let size = self.get_size(time).unwrap_or(2.0);

        // Compute extent
        let mut extent_array = [Vec3f::new(0.0, 0.0, 0.0), Vec3f::new(0.0, 0.0, 0.0)];
        if !Self::compute_extent(size, &mut extent_array) {
            return false;
        }

        extent.clear();
        extent.push(extent_array[0]);
        extent.push(extent_array[1]);
        true
    }

    /// Compute the extent for the cube at the specified time with transform.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    ///
    /// Matches C++ `ComputeExtentAtTime(VtVec3fArray* extent, UsdTimeCode time, UsdTimeCode baseTime, const GfMatrix4d& transform)`.
    pub fn compute_extent_at_time_with_transform(
        &self,
        extent: &mut Vec<Vec3f>,
        time: usd_sdf::TimeCode,
        _base_time: usd_sdf::TimeCode,
        transform: &Matrix4d,
    ) -> bool {
        // Get size at the specified time
        let size = self.get_size(time).unwrap_or(2.0);

        // Compute extent with transform
        let mut extent_array = [Vec3f::new(0.0, 0.0, 0.0), Vec3f::new(0.0, 0.0, 0.0)];
        if !Self::compute_extent_with_transform(size, transform, &mut extent_array) {
            return false;
        }

        extent.clear();
        extent.push(extent_array[0]);
        extent.push(extent_array[1]);
        true
    }

    /// Compute the extent for the cube at multiple times.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    ///
    /// Matches C++ `ComputeExtentAtTimes(std::vector<VtVec3fArray>* extents, const std::vector<UsdTimeCode>& times, UsdTimeCode baseTime)`.
    pub fn compute_extent_at_times(
        &self,
        extents: &mut Vec<Vec<Vec3f>>,
        times: &[usd_sdf::TimeCode],
        base_time: usd_sdf::TimeCode,
    ) -> bool {
        let num_samples = times.len();
        extents.clear();
        extents.reserve(num_samples);

        for &time in times {
            let mut extent = Vec::new();
            if !self.compute_extent_at_time(&mut extent, time, base_time) {
                return false;
            }
            extents.push(extent);
        }

        true
    }

    /// Compute the extent for the cube at multiple times with transform.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    ///
    /// Matches C++ `ComputeExtentAtTimes(std::vector<VtVec3fArray>* extents, const std::vector<UsdTimeCode>& times, UsdTimeCode baseTime, const GfMatrix4d& transform)`.
    pub fn compute_extent_at_times_with_transform(
        &self,
        extents: &mut Vec<Vec<Vec3f>>,
        times: &[usd_sdf::TimeCode],
        base_time: usd_sdf::TimeCode,
        transform: &Matrix4d,
    ) -> bool {
        let num_samples = times.len();
        extents.clear();
        extents.reserve(num_samples);

        for &time in times {
            let mut extent = Vec::new();
            if !self.compute_extent_at_time_with_transform(&mut extent, time, base_time, transform)
            {
                return false;
            }
            extents.push(extent);
        }

        true
    }

    /// Return a vector of names of all pre-declared attributes for this schema
    /// class and all its ancestor classes.
    ///
    /// Matches C++ `GetSchemaAttributeNames(bool includeInherited)`.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let local_names = vec![
            usd_geom_tokens().size.clone(),
            usd_geom_tokens().extent.clone(),
        ];

        if include_inherited {
            let mut all_names = Gprim::get_schema_attribute_names(true);
            all_names.extend(local_names);
            all_names
        } else {
            local_names
        }
    }
}

impl PartialEq for Cube {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for Cube {}
