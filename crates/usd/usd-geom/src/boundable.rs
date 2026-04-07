//! UsdGeomBoundable - base class for prims that can cache extent.
//!
//! Port of pxr/usd/usdGeom/boundable.h/cpp
//!
//! Boundable introduces the ability for a prim to persistently
//! cache a rectilinear, local-space, extent.

use super::tokens::usd_geom_tokens;
use super::xformable::Xformable;
use usd_core::{Attribute, Prim};
use usd_gf::vec3::Vec3f;
use usd_tf::Token;

// ============================================================================
// Boundable
// ============================================================================

/// Base class for prims that can cache extent.
///
/// Boundable introduces the ability for a prim to persistently
/// cache a rectilinear, local-space, extent.
///
/// Matches C++ `UsdGeomBoundable`.
#[derive(Debug, Clone)]
pub struct Boundable {
    /// Base xformable schema.
    inner: Xformable,
}

impl Boundable {
    /// Creates a Boundable schema from a prim.
    ///
    /// Matches C++ `UsdGeomBoundable(const UsdPrim& prim)`.
    pub fn new(prim: Prim) -> Self {
        Self {
            inner: Xformable::new(prim),
        }
    }

    /// Creates a Boundable schema from a Xformable schema.
    ///
    /// Matches C++ `UsdGeomBoundable(const UsdSchemaBase& schemaObj)`.
    pub fn from_xformable(xformable: Xformable) -> Self {
        Self { inner: xformable }
    }

    /// Creates an invalid Boundable schema.
    pub fn invalid() -> Self {
        Self {
            inner: Xformable::invalid(),
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

    /// Returns the xformable base.
    pub fn xformable(&self) -> &Xformable {
        &self.inner
    }

    /// Returns the schema type name.
    pub fn schema_type_name() -> Token {
        Token::new("Boundable")
    }

    // ========================================================================
    // Extent
    // ========================================================================

    /// Returns the extent attribute.
    ///
    /// Extent is a three dimensional range measuring the geometric
    /// extent of the authored gprim in its own local space.
    ///
    /// Matches C++ `GetExtentAttr()`.
    pub fn get_extent_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().extent.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the extent attribute.
    ///
    /// Matches C++ `CreateExtentAttr()`.
    pub fn create_extent_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        // Get or create the attribute with proper type (Vec3fArray)
        if prim.has_authored_attribute(usd_geom_tokens().extent.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().extent.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        // Find Vec3fArray type from registry
        let registry = usd_sdf::ValueTypeRegistry::instance();
        let extent_type = registry.find_type_by_token(&Token::new("float3[]"));
        // If not found, try alternative names
        let extent_type = if !extent_type.is_valid() {
            registry.find_type_by_token(&Token::new("Vec3f[]"))
        } else {
            extent_type
        };

        // Create attribute with proper type and variability
        prim.create_attribute(
            usd_geom_tokens().extent.as_str(),
            &extent_type,
            false,                                           // not custom
            Some(usd_core::attribute::Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    /// Compute the extent for the boundable prim at the specified time.
    ///
    /// Matches C++ `ComputeExtent()`.
    ///
    /// First checks if extent is authored, otherwise computes from plugins.
    pub fn compute_extent(&self, time: usd_sdf::TimeCode) -> Option<Vec<Vec3f>> {
        // First check if extent is authored
        let extent_attr = self.get_extent_attr();
        if extent_attr.is_valid() {
            if let Some(extent_value) = extent_attr.get(time) {
                if let Some(extent_array) = extent_value.as_vec_clone::<Vec3f>() {
                    // Validate extent has exactly 2 elements (min and max)
                    if extent_array.len() == 2 {
                        return Some(extent_array);
                    }
                }
            }
        }

        // If no authored extent, try to compute from integrated implementations
        Self::compute_extent_from_integrated(self, time)
    }

    /// Compute extent from integrated implementations with transform.
    ///
    /// Matches C++ `ComputeExtentFromPlugins(const UsdGeomBoundable &, const UsdTimeCode &, const GfMatrix4d &, VtVec3fArray *)`
    pub fn compute_extent_from_plugins_with_transform(
        boundable: &Boundable,
        time: usd_sdf::TimeCode,
        transform: &usd_gf::Matrix4d,
    ) -> Option<Vec<Vec3f>> {
        // First compute untransformed extent
        let mut extent = Self::compute_extent_from_integrated(boundable, time)?;

        // Transform the extent bounds
        use usd_gf::range::Range3d;
        use usd_gf::vec3::Vec3d;

        // Convert extent to Range3d for transformation
        if extent.len() != 2 {
            return None;
        }

        let min = Vec3d::new(extent[0].x as f64, extent[0].y as f64, extent[0].z as f64);
        let max = Vec3d::new(extent[1].x as f64, extent[1].y as f64, extent[1].z as f64);

        // Transform all 8 corners of the bounding box
        let corners = [
            Vec3d::new(min.x, min.y, min.z),
            Vec3d::new(max.x, min.y, min.z),
            Vec3d::new(min.x, max.y, min.z),
            Vec3d::new(max.x, max.y, min.z),
            Vec3d::new(min.x, min.y, max.z),
            Vec3d::new(max.x, min.y, max.z),
            Vec3d::new(min.x, max.y, max.z),
            Vec3d::new(max.x, max.y, max.z),
        ];

        let mut transformed_range = Range3d::empty();
        for corner in &corners {
            let transformed_corner = transform.transform_point(corner);
            transformed_range.union_with_point(&transformed_corner);
        }

        // Convert back to Vec3f array
        let transformed_min = transformed_range.min();
        let transformed_max = transformed_range.max();

        extent[0] = Vec3f::new(
            transformed_min.x as f32,
            transformed_min.y as f32,
            transformed_min.z as f32,
        );
        extent[1] = Vec3f::new(
            transformed_max.x as f32,
            transformed_max.y as f32,
            transformed_max.z as f32,
        );

        Some(extent)
    }

    /// Compute extent from integrated implementations (no plugin system).
    ///
    /// Matches C++ `ComputeExtentFromPlugins()` but uses integrated implementations.
    fn compute_extent_from_integrated(
        boundable: &Boundable,
        time: usd_sdf::TimeCode,
    ) -> Option<Vec<Vec3f>> {
        // Try PointInstancer first
        use super::point_instancer::PointInstancer;
        let point_instancer = PointInstancer::from_boundable_ref(boundable);
        if point_instancer.is_valid() {
            let mut extent = usd_vt::Array::new();
            if point_instancer.compute_extent_at_time(&mut extent, time, time) {
                return Some(extent.iter().cloned().collect());
            }
        }

        let prim = boundable.inner.prim();
        if !prim.is_valid() {
            return None;
        }

        // Get prim type name to dispatch to appropriate compute function
        let type_name = prim.type_name();
        let type_str = type_name.as_str();

        // Try Mesh
        if type_str == "Mesh" {
            use super::mesh::Mesh;
            let mesh = Mesh::new(prim.clone());
            if mesh.is_valid() {
                let mut extent = Vec::new();
                if mesh.compute_extent_at_time(&mut extent, time, time) {
                    return Some(extent);
                }
            }
        }

        // Try Points
        if type_str == "Points" {
            use super::points::Points;
            let points = Points::new(prim.clone());
            if points.is_valid() {
                let mut extent = Vec::new();
                if points.compute_extent_at_time(&mut extent, time, time) {
                    return Some(extent);
                }
            }
        }

        // Try Curves (abstract base, but we can still compute extent)
        if type_str == "Curves"
            || type_str == "BasisCurves"
            || type_str == "NurbsCurves"
            || type_str == "HermiteCurves"
        {
            use super::curves::Curves;
            let curves = Curves::new(prim.clone());
            if curves.is_valid() {
                let mut extent = Vec::new();
                if curves.compute_extent_at_time(&mut extent, time, time) {
                    return Some(extent);
                }
            }
        }

        // Try Sphere
        if type_str == "Sphere" {
            use super::sphere::Sphere;
            let sphere = Sphere::new(prim.clone());
            if sphere.is_valid() {
                let mut extent = Vec::new();
                if sphere.compute_extent_at_time(&mut extent, time, time) {
                    return Some(extent);
                }
            }
        }

        // Try Cube
        if type_str == "Cube" {
            use super::cube::Cube;
            let cube = Cube::new(prim.clone());
            if cube.is_valid() {
                let mut extent = Vec::new();
                if cube.compute_extent_at_time(&mut extent, time, time) {
                    return Some(extent);
                }
            }
        }

        // Try Cylinder
        if type_str == "Cylinder" {
            use super::cylinder::Cylinder;
            let cylinder = Cylinder::new(prim.clone());
            if cylinder.is_valid() {
                let mut extent = Vec::new();
                if cylinder.compute_extent_at_time(&mut extent, time, time) {
                    return Some(extent);
                }
            }
        }

        // Try Cone
        if type_str == "Cone" {
            use super::cone::Cone;
            let cone = Cone::new(prim.clone());
            if cone.is_valid() {
                let mut extent = Vec::new();
                if cone.compute_extent_at_time(&mut extent, time, time) {
                    return Some(extent);
                }
            }
        }

        // Try Capsule
        if type_str == "Capsule" {
            use super::capsule::Capsule;
            let capsule = Capsule::new(prim.clone());
            if capsule.is_valid() {
                let mut extent = Vec::new();
                if capsule.compute_extent_at_time(&mut extent, time, time) {
                    return Some(extent);
                }
            }
        }

        // Try Plane
        if type_str == "Plane" {
            use super::plane::Plane;
            let plane = Plane::new(prim.clone());
            if plane.is_valid() {
                let mut extent = Vec::new();
                if plane.compute_extent_at_time(&mut extent, time, time) {
                    return Some(extent);
                }
            }
        }

        // Fall back to the plugin-based extent registry (handles UsdLux light
        // types and any externally-registered extent functions).  Matches C++
        // ComputeExtentFromPlugins which walks a TfType-keyed function table.
        if let Some(extent) =
            super::boundable_compute_extent::compute_extent_from_plugins(boundable, time, None)
        {
            return Some(vec![extent[0], extent[1]]);
        }

        None
    }

    /// Return a vector of names of all pre-declared attributes for this schema
    /// class and all its ancestor classes.
    ///
    /// Matches C++ `GetSchemaAttributeNames()`.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let local_names = vec![usd_geom_tokens().extent.clone()];

        if include_inherited {
            let mut all_names = Xformable::get_schema_attribute_names(true);
            all_names.extend(local_names);
            all_names
        } else {
            local_names
        }
    }

    /// Return a Boundable holding the prim adhering to this schema at path on stage.
    ///
    /// Matches C++ `Get(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn get(stage: &usd_core::Stage, path: &usd_sdf::Path) -> Self {
        if let Some(prim) = stage.get_prim_at_path(path) {
            Self::new(prim)
        } else {
            Self::invalid()
        }
    }
}

impl PartialEq for Boundable {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for Boundable {}
