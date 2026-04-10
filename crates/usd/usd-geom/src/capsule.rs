//! UsdGeomCapsule - capsule geometry schema.
//!
//! Port of pxr/usd/usdGeom/capsule.h/cpp
//!
//! Defines a primitive capsule, i.e. a cylinder capped by two half spheres.

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
// Capsule
// ============================================================================

/// Capsule geometry schema.
///
/// Defines a primitive capsule, i.e. a cylinder capped by two half spheres.
///
/// Matches C++ `UsdGeomCapsule`.
#[derive(Debug, Clone)]
pub struct Capsule {
    /// Base gprim schema.
    inner: Gprim,
}

impl Capsule {
    /// Creates a Capsule schema from a prim.
    ///
    /// Matches C++ `UsdGeomCapsule(const UsdPrim& prim)`.
    pub fn new(prim: Prim) -> Self {
        Self {
            inner: Gprim::new(prim),
        }
    }

    /// Creates a Capsule schema from a Gprim schema.
    ///
    /// Matches C++ `UsdGeomCapsule(const UsdSchemaBase& schemaObj)`.
    pub fn from_gprim(gprim: Gprim) -> Self {
        Self { inner: gprim }
    }

    /// Creates an invalid Capsule schema.
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
        Token::new("Capsule")
    }

    /// Return a Capsule holding the prim adhering to this schema at path on stage.
    ///
    /// Matches C++ `UsdGeomCapsule::Get(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn get(stage: &Stage, path: &usd_sdf::Path) -> Self {
        if let Some(prim) = stage.get_prim_at_path(path) {
            Self::new(prim)
        } else {
            Self::invalid()
        }
    }

    /// Attempt to ensure a prim adhering to this schema at path is defined on this stage.
    ///
    /// Matches C++ `UsdGeomCapsule::Define(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn define(stage: &Stage, path: &usd_sdf::Path) -> Self {
        match stage.define_prim(path.get_string(), Self::schema_type_name().as_str()) {
            Ok(prim) => Self::new(prim),
            Err(_) => Self::invalid(),
        }
    }

    // ========================================================================
    // Height
    // ========================================================================

    /// Returns the height attribute.
    ///
    /// The length of the capsule's spine along the specified axis excluding the size of the two half spheres.
    ///
    /// Matches C++ `GetHeightAttr()`.
    pub fn get_height_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().height.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the height attribute.
    ///
    /// Matches C++ `CreateHeightAttr()`.
    pub fn create_height_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().height.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().height.as_str())
                .unwrap_or_else(Attribute::invalid);
        }

        let registry = ValueTypeRegistry::instance();
        let double_type = registry.find_type_by_token(&Token::new("double"));

        let attr = prim
            .create_attribute(
                usd_geom_tokens().height.as_str(),
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
    // Radius
    // ========================================================================

    /// Returns the radius attribute.
    ///
    /// The radius of the capsule.
    ///
    /// Matches C++ `GetRadiusAttr()`.
    pub fn get_radius_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().radius.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the radius attribute.
    ///
    /// Matches C++ `CreateRadiusAttr()`.
    pub fn create_radius_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().radius.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().radius.as_str())
                .unwrap_or_else(Attribute::invalid);
        }

        let registry = ValueTypeRegistry::instance();
        let double_type = registry.find_type_by_token(&Token::new("double"));

        let attr = prim
            .create_attribute(
                usd_geom_tokens().radius.as_str(),
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
    // RadiusTop (Capsule_1)
    // ========================================================================

    /// Returns the radiusTop attribute (Capsule_1 schema).
    ///
    /// The radius of the capping sphere at the top of the capsule,
    /// i.e. the sphere in the direction of the positive axis.
    ///
    /// Matches C++ `UsdGeomCapsule_1::GetRadiusTopAttr()`.
    pub fn get_radius_top_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().radius_top.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the radiusTop attribute (Capsule_1 schema).
    ///
    /// Matches C++ `UsdGeomCapsule_1::CreateRadiusTopAttr()`.
    pub fn create_radius_top_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().radius_top.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().radius_top.as_str())
                .unwrap_or_else(Attribute::invalid);
        }

        let registry = ValueTypeRegistry::instance();
        let double_type = registry.find_type_by_token(&Token::new("double"));

        let attr = prim
            .create_attribute(
                usd_geom_tokens().radius_top.as_str(),
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
    // RadiusBottom (Capsule_1)
    // ========================================================================

    /// Returns the radiusBottom attribute (Capsule_1 schema).
    ///
    /// The radius of the capping sphere at the bottom of the capsule,
    /// i.e. the sphere in the direction of the negative axis.
    ///
    /// Matches C++ `UsdGeomCapsule_1::GetRadiusBottomAttr()`.
    pub fn get_radius_bottom_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().radius_bottom.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the radiusBottom attribute (Capsule_1 schema).
    ///
    /// Matches C++ `UsdGeomCapsule_1::CreateRadiusBottomAttr()`.
    pub fn create_radius_bottom_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().radius_bottom.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().radius_bottom.as_str())
                .unwrap_or_else(Attribute::invalid);
        }

        let registry = ValueTypeRegistry::instance();
        let double_type = registry.find_type_by_token(&Token::new("double"));

        let attr = prim
            .create_attribute(
                usd_geom_tokens().radius_bottom.as_str(),
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
    // Axis
    // ========================================================================

    /// Returns the axis attribute.
    ///
    /// The axis along which the spine of the capsule is aligned.
    ///
    /// Matches C++ `GetAxisAttr()`.
    pub fn get_axis_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().axis.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the axis attribute.
    ///
    /// Matches C++ `CreateAxisAttr()`.
    pub fn create_axis_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().axis.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().axis.as_str())
                .unwrap_or_else(Attribute::invalid);
        }

        let registry = ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        let attr = prim
            .create_attribute(
                usd_geom_tokens().axis.as_str(),
                &token_type,
                false,                      // not custom
                Some(Variability::Uniform), // uniform
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
    /// Extent is re-defined on Capsule only to provide a fallback value.
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

    /// Compute the extent for the capsule defined by height, radius, and axis.
    ///
    /// Returns true upon success, false if unable to calculate extent.
    ///
    /// Matches C++ `ComputeExtent(double height, double radius, const TfToken& axis, VtVec3fArray* extent)`.
    pub fn compute_extent(height: f64, radius: f64, axis: &Token, extent: &mut [Vec3f; 2]) -> bool {
        let max = match Self::compute_extent_max(height, radius, axis) {
            Some(m) => m,
            None => return false,
        };
        extent[0] = Vec3f::new(-max.x, -max.y, -max.z);
        extent[1] = max;
        true
    }

    /// Compute the extent as if the matrix transform was first applied.
    ///
    /// Matches C++ `ComputeExtent(double height, double radius, const TfToken& axis, const GfMatrix4d& transform, VtVec3fArray* extent)`.
    pub fn compute_extent_with_transform(
        height: f64,
        radius: f64,
        axis: &Token,
        transform: &Matrix4d,
        extent: &mut [Vec3f; 2],
    ) -> bool {
        let max = match Self::compute_extent_max(height, radius, axis) {
            Some(m) => m,
            None => return false,
        };

        // Create bounding box from capsule range
        let range = Range3d::new(
            Vec3d::new(-max.x as f64, -max.y as f64, -max.z as f64),
            Vec3d::new(max.x as f64, max.y as f64, max.z as f64),
        );
        let bbox = BBox3d::from_range_matrix(range, *transform);
        let aligned_range = bbox.compute_aligned_range();

        let min_vec = aligned_range.min();
        let max_vec = aligned_range.max();
        extent[0] = Vec3f::new(min_vec.x as f32, min_vec.y as f32, min_vec.z as f32);
        extent[1] = Vec3f::new(max_vec.x as f32, max_vec.y as f32, max_vec.z as f32);
        true
    }

    /// Compute the extent for a Capsule_1 (with different top/bottom radii).
    ///
    /// Matches C++ `UsdGeomCapsule_1::ComputeExtent(height, radiusTop, radiusBottom, axis, extent)`.
    pub fn compute_extent_capsule1(
        height: f64,
        radius_top: f64,
        radius_bottom: f64,
        axis: &Token,
        extent: &mut [Vec3f; 2],
    ) -> bool {
        let max_radius = radius_top.max(radius_bottom);
        let max = match Self::compute_extent_max(height, max_radius, axis) {
            Some(m) => m,
            None => return false,
        };
        extent[0] = Vec3f::new(-max.x, -max.y, -max.z);
        extent[1] = max;
        true
    }

    /// Compute the extent for a Capsule_1 with transform applied.
    ///
    /// Matches C++ `UsdGeomCapsule_1::ComputeExtent(height, radiusTop, radiusBottom, axis, transform, extent)`.
    pub fn compute_extent_capsule1_with_transform(
        height: f64,
        radius_top: f64,
        radius_bottom: f64,
        axis: &Token,
        transform: &Matrix4d,
        extent: &mut [Vec3f; 2],
    ) -> bool {
        let max_radius = radius_top.max(radius_bottom);
        Self::compute_extent_with_transform(height, max_radius, axis, transform, extent)
    }

    /// Helper function to compute extent max based on axis.
    /// The height is increased by the capsule's radius from the hemispheres on either side.
    fn compute_extent_max(height: f64, radius: f64, axis: &Token) -> Option<Vec3f> {
        let half_height_with_cap = (height * 0.5 + radius) as f32;
        let radius_f = radius as f32;

        let axis_str = axis.as_str();
        if axis_str == usd_geom_tokens().x.as_str() {
            Some(Vec3f::new(half_height_with_cap, radius_f, radius_f))
        } else if axis_str == usd_geom_tokens().y.as_str() {
            Some(Vec3f::new(radius_f, half_height_with_cap, radius_f))
        } else if axis_str == usd_geom_tokens().z.as_str() {
            Some(Vec3f::new(radius_f, radius_f, half_height_with_cap))
        } else {
            None // invalid axis
        }
    }

    // ========================================================================
    // Get Methods (Value Retrieval)
    // ========================================================================

    /// Get the height at the specified time.
    ///
    /// Matches C++ `GetHeight(double* height, UsdTimeCode time)`.
    pub fn get_height(&self, time: usd_sdf::TimeCode) -> Option<f64> {
        self.get_height_attr().get_typed::<f64>(time)
    }

    /// Get the radius at the specified time.
    ///
    /// Matches C++ `GetRadius(double* radius, UsdTimeCode time)`.
    pub fn get_radius(&self, time: usd_sdf::TimeCode) -> Option<f64> {
        self.get_radius_attr().get_typed::<f64>(time)
    }

    /// Get the axis at the specified time.
    ///
    /// Matches C++ `GetAxis(TfToken* axis, UsdTimeCode time)`.
    pub fn get_axis(&self, time: usd_sdf::TimeCode) -> Option<Token> {
        self.get_axis_attr().get_typed::<Token>(time)
    }

    /// Get the top radius at the specified time (Capsule_1 schema).
    ///
    /// Matches C++ `UsdGeomCapsule_1::GetRadiusTop`.
    pub fn get_radius_top(&self, time: usd_sdf::TimeCode) -> Option<f64> {
        self.get_radius_top_attr().get_typed::<f64>(time)
    }

    /// Get the bottom radius at the specified time (Capsule_1 schema).
    ///
    /// Matches C++ `UsdGeomCapsule_1::GetRadiusBottom`.
    pub fn get_radius_bottom(&self, time: usd_sdf::TimeCode) -> Option<f64> {
        self.get_radius_bottom_attr().get_typed::<f64>(time)
    }

    // ========================================================================
    // Compute Extent At Time
    // ========================================================================

    /// Compute the extent for the capsule at the specified time.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    /// On success, extent will contain the axis-aligned bounding box of the capsule.
    ///
    /// Matches C++ `ComputeExtentAtTime(VtVec3fArray* extent, UsdTimeCode time, UsdTimeCode baseTime)`.
    pub fn compute_extent_at_time(
        &self,
        extent: &mut Vec<Vec3f>,
        time: usd_sdf::TimeCode,
        _base_time: usd_sdf::TimeCode,
    ) -> bool {
        // Get height, radius, and axis at the specified time
        let height = self.get_height(time).unwrap_or(1.0);
        let radius = self.get_radius(time).unwrap_or(0.5);
        let axis = match self.get_axis(time) {
            Some(a) => a,
            None => usd_geom_tokens().z.clone(), // default axis
        };

        // Compute extent
        let mut extent_array = [Vec3f::new(0.0, 0.0, 0.0), Vec3f::new(0.0, 0.0, 0.0)];
        if !Self::compute_extent(height, radius, &axis, &mut extent_array) {
            return false;
        }

        extent.clear();
        extent.push(extent_array[0]);
        extent.push(extent_array[1]);
        true
    }

    /// Compute the extent for the capsule at the specified time with transform.
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
        // Get height, radius, and axis at the specified time
        let height = self.get_height(time).unwrap_or(1.0);
        let radius = self.get_radius(time).unwrap_or(0.5);
        let axis = match self.get_axis(time) {
            Some(a) => a,
            None => usd_geom_tokens().z.clone(), // default axis
        };

        // Compute extent with transform
        let mut extent_array = [Vec3f::new(0.0, 0.0, 0.0), Vec3f::new(0.0, 0.0, 0.0)];
        if !Self::compute_extent_with_transform(height, radius, &axis, transform, &mut extent_array)
        {
            return false;
        }

        extent.clear();
        extent.push(extent_array[0]);
        extent.push(extent_array[1]);
        true
    }

    /// Compute the extent for the capsule at multiple times.
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

    /// Compute the extent for the capsule at multiple times with transform.
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
            usd_geom_tokens().height.clone(),
            usd_geom_tokens().radius.clone(),
            usd_geom_tokens().radius_top.clone(),
            usd_geom_tokens().radius_bottom.clone(),
            usd_geom_tokens().axis.clone(),
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

impl PartialEq for Capsule {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for Capsule {}

/// Schema for `UsdGeomCapsule_1` — capsule with potentially different top/bottom radii.
///
/// C++ registers this as a separate prim type `"Capsule_1"` distinct from `"Capsule"`.
/// In the Rust port the attribute set is identical to `Capsule`, but the schema type
/// name and `define()` type must use `"Capsule_1"`.
///
/// Matches C++ `UsdGeomCapsule_1`.
#[derive(Debug, Clone)]
pub struct Capsule1 {
    inner: Capsule,
}

impl Capsule1 {
    /// Creates a Capsule1 schema from a prim.
    pub fn new(prim: Prim) -> Self {
        Self {
            inner: Capsule::new(prim),
        }
    }

    /// Returns the schema type name, which is `"Capsule_1"`.
    ///
    /// Matches C++ `TfType::AddAlias<UsdSchemaBase, UsdGeomCapsule_1>("Capsule_1")`.
    pub fn schema_type_name() -> Token {
        Token::new("Capsule_1")
    }

    /// Return a Capsule1 holding the prim adhering to this schema at path on stage.
    pub fn get(stage: &Stage, path: &usd_sdf::Path) -> Self {
        if let Some(prim) = stage.get_prim_at_path(path) {
            Self::new(prim)
        } else {
            Self {
                inner: Capsule::invalid(),
            }
        }
    }

    /// Ensure a prim of type `"Capsule_1"` is defined at path.
    pub fn define(stage: &Stage, path: &usd_sdf::Path) -> Self {
        match stage.define_prim(path.get_string(), Self::schema_type_name().as_str()) {
            Ok(prim) => Self::new(prim),
            Err(_) => Self {
                inner: Capsule::invalid(),
            },
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

    /// Access underlying Capsule schema.
    pub fn as_capsule(&self) -> &Capsule {
        &self.inner
    }

    /// Matches C++ `UsdGeomCapsule_1::GetSchemaAttributeNames`.
    ///
    /// Local names are those of **Capsule_1** (top/bottom radii), not the legacy single `radius`.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let local_names = vec![
            usd_geom_tokens().height.clone(),
            usd_geom_tokens().radius_top.clone(),
            usd_geom_tokens().radius_bottom.clone(),
            usd_geom_tokens().axis.clone(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule1_schema_type_name() {
        // Capsule1 must report "Capsule_1", not "Capsule".
        assert_eq!(Capsule1::schema_type_name().as_str(), "Capsule_1");
        // Original Capsule is still "Capsule".
        assert_eq!(Capsule::schema_type_name().as_str(), "Capsule");
    }

    #[test]
    fn test_capsule1_radius_top_bottom_attrs() {
        // Verify Capsule includes Capsule_1 attributes in schema names
        let names = Capsule::get_schema_attribute_names(false);
        let name_strs: Vec<&str> = names.iter().map(|t| t.as_str()).collect();
        assert!(name_strs.contains(&"radiusTop"), "missing radiusTop");
        assert!(name_strs.contains(&"radiusBottom"), "missing radiusBottom");
        assert!(name_strs.contains(&"radius"), "missing radius");
    }

    #[test]
    fn test_capsule1_compute_extent() {
        let axis = Token::new("Z");
        let mut extent = [Vec3f::new(0.0, 0.0, 0.0); 2];

        // With equal radii, same as normal capsule extent
        assert!(Capsule::compute_extent_capsule1(
            1.0,
            0.5,
            0.5,
            &axis,
            &mut extent
        ));
        assert_eq!(extent[0], Vec3f::new(-0.5, -0.5, -1.0));
        assert_eq!(extent[1], Vec3f::new(0.5, 0.5, 1.0));

        // With different radii, uses max radius for bounding
        assert!(Capsule::compute_extent_capsule1(
            1.0,
            0.3,
            0.7,
            &axis,
            &mut extent
        ));
        // max_radius = 0.7, half_height_with_cap = 0.5 + 0.7 = 1.2
        assert_eq!(extent[1].x, 0.7_f32);
        assert_eq!(extent[1].z, 1.2_f32);
    }
}
