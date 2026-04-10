//! UsdGeomCone - cone geometry schema.
//!
//! Port of pxr/usd/usdGeom/cone.h/cpp
//!
//! Defines a primitive cone, centered at the origin.

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
// Cone
// ============================================================================

/// Cone geometry schema.
///
/// Defines a primitive cone, centered at the origin.
///
/// Matches C++ `UsdGeomCone`.
#[derive(Debug, Clone)]
pub struct Cone {
    /// Base gprim schema.
    inner: Gprim,
}

impl Cone {
    /// Creates a Cone schema from a prim.
    ///
    /// Matches C++ `UsdGeomCone(const UsdPrim& prim)`.
    pub fn new(prim: Prim) -> Self {
        Self {
            inner: Gprim::new(prim),
        }
    }

    /// Creates a Cone schema from a Gprim schema.
    ///
    /// Matches C++ `UsdGeomCone(const UsdSchemaBase& schemaObj)`.
    pub fn from_gprim(gprim: Gprim) -> Self {
        Self { inner: gprim }
    }

    /// Creates an invalid Cone schema.
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
        Token::new("Cone")
    }

    /// Return a Cone holding the prim adhering to this schema at path on stage.
    ///
    /// Matches C++ `UsdGeomCone::Get(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn get(stage: &Stage, path: &usd_sdf::Path) -> Self {
        if let Some(prim) = stage.get_prim_at_path(path) {
            Self::new(prim)
        } else {
            Self::invalid()
        }
    }

    /// Attempt to ensure a prim adhering to this schema at path is defined on this stage.
    ///
    /// Matches C++ `UsdGeomCone::Define(const UsdStagePtr &stage, const SdfPath &path)`.
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
    /// The length of the cone's spine along the specified axis.
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
    /// The radius of the cone.
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
    // Axis
    // ========================================================================

    /// Returns the axis attribute.
    ///
    /// The axis along which the spine of the cone is aligned.
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
    /// Extent is re-defined on Cone only to provide a fallback value.
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

    /// Compute the extent for the cone defined by height, radius, and axis.
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

        // Create bounding box from cone range
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

    /// Helper function to compute extent max based on axis.
    fn compute_extent_max(height: f64, radius: f64, axis: &Token) -> Option<Vec3f> {
        let axis_str = axis.as_str();
        if axis_str == usd_geom_tokens().x.as_str() {
            Some(Vec3f::new(
                (height * 0.5) as f32,
                radius as f32,
                radius as f32,
            ))
        } else if axis_str == usd_geom_tokens().y.as_str() {
            Some(Vec3f::new(
                radius as f32,
                (height * 0.5) as f32,
                radius as f32,
            ))
        } else if axis_str == usd_geom_tokens().z.as_str() {
            Some(Vec3f::new(
                radius as f32,
                radius as f32,
                (height * 0.5) as f32,
            ))
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

    // ========================================================================
    // Compute Extent At Time
    // ========================================================================

    /// Compute the extent for the cone at the specified time.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    /// On success, extent will contain the axis-aligned bounding box of the cone.
    ///
    /// Matches C++ `ComputeExtentAtTime(VtVec3fArray* extent, UsdTimeCode time, UsdTimeCode baseTime)`.
    pub fn compute_extent_at_time(
        &self,
        extent: &mut Vec<Vec3f>,
        time: usd_sdf::TimeCode,
        _base_time: usd_sdf::TimeCode,
    ) -> bool {
        // Get height, radius, and axis at the specified time
        let height = self.get_height(time).unwrap_or(2.0);
        let radius = self.get_radius(time).unwrap_or(1.0);
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

    /// Compute the extent for the cone at the specified time with transform.
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
        let height = self.get_height(time).unwrap_or(2.0);
        let radius = self.get_radius(time).unwrap_or(1.0);
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

    /// Compute the extent for the cone at multiple times.
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

    /// Compute the extent for the cone at multiple times with transform.
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

impl PartialEq for Cone {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for Cone {}

#[cfg(test)]
mod tests {
    use super::Cone;
    use usd_core::InitialLoadSet;
    use usd_core::Stage;
    use usd_sdf::TimeCode;
    use usd_vt::Value;

    #[test]
    fn create_height_attr_writes_default_value() {
        let _ = usd_sdf::init();
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("stage");
        let prim = stage.define_prim("/World/Cone", "Cone").expect("prim");
        let cone = Cone::new(prim);
        let attr = cone.create_height_attr(Some(Value::from(3.25_f64)), false);
        assert!(attr.is_valid());
        let got = attr.get(TimeCode::default()).expect("default sample");
        let h = *got.get::<f64>().expect("double");
        assert!((h - 3.25).abs() < 1e-9);
    }
}
