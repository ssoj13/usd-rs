//! UsdGeomTetMesh - tetrahedral mesh geometry schema.
//!
//! Port of pxr/usd/usdGeom/tetMesh.h/cpp
//!
//! Encodes a tetrahedral mesh.

use super::point_based::PointBased;
use super::tokens::usd_geom_tokens;
use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, Stage};
use usd_gf::vec3::Vec3f;
use usd_gf::vec3::Vec3i;
use usd_gf::vec4::Vec4i;
use usd_sdf::{TimeCode, ValueTypeRegistry};
use usd_tf::Token;
use usd_vt::Value;

// ============================================================================
// TetMesh
// ============================================================================

/// Tetrahedral mesh geometry schema.
///
/// Encodes a tetrahedral mesh.
///
/// Matches C++ `UsdGeomTetMesh`.
#[derive(Debug, Clone)]
pub struct TetMesh {
    /// Base point-based schema.
    inner: PointBased,
}

impl TetMesh {
    /// Creates a TetMesh schema from a prim.
    ///
    /// Matches C++ `UsdGeomTetMesh(const UsdPrim& prim)`.
    pub fn new(prim: Prim) -> Self {
        Self {
            inner: PointBased::new(prim),
        }
    }

    /// Creates a TetMesh schema from a PointBased schema.
    ///
    /// Matches C++ `UsdGeomTetMesh(const UsdSchemaBase& schemaObj)`.
    pub fn from_point_based(point_based: PointBased) -> Self {
        Self { inner: point_based }
    }

    /// Creates an invalid TetMesh schema.
    pub fn invalid() -> Self {
        Self {
            inner: PointBased::invalid(),
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

    /// Returns the point-based base.
    pub fn point_based(&self) -> &PointBased {
        &self.inner
    }

    /// Returns the schema type name.
    pub fn schema_type_name() -> Token {
        Token::new("TetMesh")
    }

    /// Return a TetMesh holding the prim adhering to this schema at path on stage.
    ///
    /// Matches C++ `UsdGeomTetMesh::Get(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn get(stage: &Stage, path: &usd_sdf::Path) -> Self {
        if let Some(prim) = stage.get_prim_at_path(path) {
            Self::new(prim)
        } else {
            Self::invalid()
        }
    }

    /// Attempt to ensure a prim adhering to this schema at path is defined on this stage.
    ///
    /// Matches C++ `UsdGeomTetMesh::Define(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn define(stage: &Stage, path: &usd_sdf::Path) -> Self {
        match stage.define_prim(path.get_string(), Self::schema_type_name().as_str()) {
            Ok(prim) => Self::new(prim),
            Err(_) => Self::invalid(),
        }
    }

    // ========================================================================
    // TetVertexIndices
    // ========================================================================

    /// Returns the tetVertexIndices attribute.
    ///
    /// Flat list of the index of each vertex of each tetrahedron in the mesh.
    ///
    /// Matches C++ `GetTetVertexIndicesAttr()`.
    pub fn get_tet_vertex_indices_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        if let Some(attr) = prim.get_attribute(usd_geom_tokens().tet_vertex_indices.as_str()) {
            attr
        } else {
            self.create_tet_vertex_indices_attr(None, false)
        }
    }

    /// Creates the tetVertexIndices attribute.
    ///
    /// Matches C++ `CreateTetVertexIndicesAttr()`.
    pub fn create_tet_vertex_indices_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let int4_array_type = registry.find_type_by_token(&Token::new("int4[]"));

        let attr = prim
            .create_attribute(
                usd_geom_tokens().tet_vertex_indices.as_str(),
                &int4_array_type,
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
    // SurfaceFaceVertexIndices
    // ========================================================================

    /// Returns the surfaceFaceVertexIndices attribute.
    ///
    /// Defines the triangle surface faces indices wrt. points of the tetmesh surface.
    ///
    /// Matches C++ `GetSurfaceFaceVertexIndicesAttr()`.
    pub fn get_surface_face_vertex_indices_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        if let Some(attr) =
            prim.get_attribute(usd_geom_tokens().surface_face_vertex_indices.as_str())
        {
            attr
        } else {
            self.create_surface_face_vertex_indices_attr(None, false)
        }
    }

    /// Creates the surfaceFaceVertexIndices attribute.
    ///
    /// Matches C++ `CreateSurfaceFaceVertexIndicesAttr()`.
    pub fn create_surface_face_vertex_indices_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let int3_array_type = registry.find_type_by_token(&Token::new("int3[]"));

        let attr = prim
            .create_attribute(
                usd_geom_tokens().surface_face_vertex_indices.as_str(),
                &int3_array_type,
                false,                      // not custom
                Some(Variability::Varying), // can vary over time
            )
            .unwrap_or_else(Attribute::invalid);
        if let Some(val) = default_value {
            let _ = attr.set(val, TimeCode::default());
        }
        attr
    }

    /// Returns the orientation attribute from the prim.
    ///
    /// Helper to get orientation for FindInvertedElements.
    fn get_orientation_attr(&self) -> Attribute {
        let attr = self.inner.gprim().get_orientation_attr();
        if attr.is_valid() {
            attr
        } else {
            self.inner.gprim().create_orientation_attr(None, false)
        }
    }

    /// Returns the points attribute, creating it if necessary.
    pub fn get_points_attr(&self) -> Attribute {
        let attr = self.inner.get_points_attr();
        if attr.is_valid() {
            attr
        } else {
            self.inner.create_points_attr(None, false)
        }
    }

    /// ComputeSurfaceFaces determines the vertex indices of the surface faces
    /// from tetVertexIndices.
    ///
    /// Matches C++ `ComputeSurfaceFaces()`.
    pub fn compute_surface_faces(&self, time_code: TimeCode) -> Option<Vec<Vec3i>> {
        let tet_vertex_indices_attr = self.get_tet_vertex_indices_attr();
        if !tet_vertex_indices_attr.is_valid() {
            return None;
        }

        let mut tet_vertex_indices: Vec<Vec4i> = Vec::new();
        if let Some(value) = tet_vertex_indices_attr.get(time_code) {
            if let Some(indices) = value.get::<Vec<Vec4i>>() {
                tet_vertex_indices = indices.clone();
            } else if let Some(indices) = value.get::<usd_vt::Array<Vec4i>>() {
                tet_vertex_indices = indices.iter().cloned().collect();
            }
        }

        if tet_vertex_indices.is_empty() {
            return None;
        }

        // Compute surface faces using the algorithm from C++
        // The surface faces are triangles that occur only once
        use std::collections::HashMap;

        // Helper to sort Vec3i for use as hash key
        fn sorted_vec3i(v: Vec3i) -> Vec3i {
            let mut arr = [v.x, v.y, v.z];
            arr.sort();
            Vec3i::new(arr[0], arr[1], arr[2])
        }

        // Hash map: sorted triangle -> (count, original triangle)
        let mut sig_to_count_and_triangle: HashMap<Vec3i, (usize, Vec3i)> = HashMap::new();

        // The four triangles of a tetrahedron
        const TET_FACE_INDICES: [[usize; 3]; 4] = [[1, 2, 3], [0, 3, 2], [0, 1, 3], [0, 2, 1]];

        for tet in &tet_vertex_indices {
            for face_indices in &TET_FACE_INDICES {
                let triangle = Vec3i::new(
                    tet[face_indices[0]],
                    tet[face_indices[1]],
                    tet[face_indices[2]],
                );

                let sig = sorted_vec3i(triangle);
                let entry = sig_to_count_and_triangle
                    .entry(sig)
                    .or_insert((0, triangle));
                entry.0 += 1;
                entry.1 = triangle; // Store original orientation
            }
        }

        // Collect triangles that occur only once
        let mut result: Vec<Vec3i> = sig_to_count_and_triangle
            .into_iter()
            .filter_map(
                |(_, (count, triangle))| {
                    if count == 1 { Some(triangle) } else { None }
                },
            )
            .collect();

        // Sort for deterministic behavior
        result.sort_by(|a, b| {
            if a.x != b.x {
                a.x.cmp(&b.x)
            } else if a.y != b.y {
                a.y.cmp(&b.y)
            } else {
                a.z.cmp(&b.z)
            }
        });

        Some(result)
    }

    /// FindInvertedElements determines if the tetMesh has inverted tetrahedral elements.
    ///
    /// Matches C++ `FindInvertedElements()`.
    pub fn find_inverted_elements(&self, time_code: TimeCode) -> Option<Vec<i32>> {
        // Get points
        let points_attr = self.get_points_attr();
        if !points_attr.is_valid() {
            return None;
        }

        let mut tet_mesh_points: Vec<Vec3f> = Vec::new();
        if let Some(value) = points_attr.get(time_code) {
            if let Some(pts) = value.get::<Vec<Vec3f>>() {
                tet_mesh_points = pts.clone();
            } else if let Some(pts) = value.get::<usd_vt::Array<Vec3f>>() {
                tet_mesh_points = pts.iter().cloned().collect();
            }
        }

        if tet_mesh_points.len() < 4 {
            return None;
        }

        // Get tet vertex indices
        let tet_vertex_indices_attr = self.get_tet_vertex_indices_attr();
        if !tet_vertex_indices_attr.is_valid() {
            return None;
        }

        let mut tet_vertex_indices: Vec<Vec4i> = Vec::new();
        if let Some(value) = tet_vertex_indices_attr.get(time_code) {
            if let Some(indices) = value.get::<Vec<Vec4i>>() {
                tet_vertex_indices = indices.clone();
            } else if let Some(indices) = value.get::<usd_vt::Array<Vec4i>>() {
                tet_vertex_indices = indices.iter().cloned().collect();
            }
        }

        if tet_vertex_indices.is_empty() {
            return None;
        }

        // Get orientation (handles both Token and String storage)
        // Read at default time first since orientation is uniform
        let orientation_attr = self.get_orientation_attr();
        let orientation = if orientation_attr.is_valid() {
            let val = orientation_attr
                .get(TimeCode::default_time())
                .or_else(|| orientation_attr.get(time_code));
            if let Some(value) = val {
                if let Some(t) = value.get::<Token>() {
                    t.clone()
                } else if let Some(s) = value.get::<String>() {
                    Token::new(s)
                } else {
                    usd_geom_tokens().right_handed.clone()
                }
            } else {
                usd_geom_tokens().right_handed.clone()
            }
        } else {
            usd_geom_tokens().right_handed.clone()
        };

        let sign = if orientation == usd_geom_tokens().left_handed {
            1.0f32
        } else {
            -1.0f32
        };

        // Face vertices for right handed/CCW ordering
        const FACE_VERTS: [[usize; 3]; 4] = [[1, 2, 3], [0, 3, 2], [0, 1, 3], [0, 2, 1]];

        let mut inverted_elements = Vec::new();

        for (t, tet) in tet_vertex_indices.iter().enumerate() {
            // Compute element center
            let mut elem_center = Vec3f::new(0.0, 0.0, 0.0);
            let mut elem_points = [Vec3f::new(0.0, 0.0, 0.0); 4];

            for p in 0..4 {
                let idx = tet[p] as usize;
                if idx >= tet_mesh_points.len() {
                    continue;
                }
                elem_points[p] = tet_mesh_points[idx];
                elem_center += elem_points[p];
            }

            elem_center = Vec3f::new(
                elem_center.x * 0.25,
                elem_center.y * 0.25,
                elem_center.z * 0.25,
            );

            // Check each face
            let mut is_inverted = false;
            for f in 0..4 {
                let fv0 = elem_points[FACE_VERTS[f][0]];
                let fv1 = elem_points[FACE_VERTS[f][1]];
                let fv2 = elem_points[FACE_VERTS[f][2]];

                let elem_center_minus_fv0 = elem_center - fv0;
                let fv1_minus_fv0 = fv1 - fv0;
                let fv2_minus_fv0 = fv2 - fv0;

                // Cross product for face normal
                let face_normal = Vec3f::new(
                    fv1_minus_fv0.y * fv2_minus_fv0.z - fv1_minus_fv0.z * fv2_minus_fv0.y,
                    fv1_minus_fv0.z * fv2_minus_fv0.x - fv1_minus_fv0.x * fv2_minus_fv0.z,
                    fv1_minus_fv0.x * fv2_minus_fv0.y - fv1_minus_fv0.y * fv2_minus_fv0.x,
                );

                // Dot product
                let dot = face_normal.x * elem_center_minus_fv0.x
                    + face_normal.y * elem_center_minus_fv0.y
                    + face_normal.z * elem_center_minus_fv0.z;

                if sign * dot < 0.0 {
                    is_inverted = true;
                    break;
                }
            }

            if is_inverted {
                inverted_elements.push(t as i32);
            }
        }

        Some(inverted_elements)
    }

    // ========================================================================
    // Get Methods (Value Retrieval)
    // ========================================================================

    /// Get the points at the specified time.
    ///
    /// Matches C++ `GetPoints(VtVec3fArray* points, UsdTimeCode time)`.
    pub fn get_points(&self, time: TimeCode) -> Option<usd_vt::Array<Vec3f>> {
        let attr = self.get_points_attr();
        if !attr.is_valid() {
            return None;
        }
        attr.get_typed::<usd_vt::Array<Vec3f>>(time)
    }

    /// Get the face vertex indices at the specified time.
    ///
    /// This returns the surfaceFaceVertexIndices as a flattened array.
    /// Matches C++ `GetFaceVertexIndices(VtIntArray* faceVertexIndices, UsdTimeCode time)`.
    pub fn get_face_vertex_indices(&self, time: TimeCode) -> Option<Vec<i32>> {
        let attr = self.get_surface_face_vertex_indices_attr();
        if !attr.is_valid() {
            return None;
        }
        if let Some(value) = attr.get(time) {
            if let Some(indices) = value.get::<Vec<Vec3i>>() {
                Some(indices.iter().flat_map(|v| vec![v.x, v.y, v.z]).collect())
            } else {
                value
                    .get::<usd_vt::Array<Vec3i>>()
                    .map(|indices| indices.iter().flat_map(|v| vec![v.x, v.y, v.z]).collect())
            }
        } else {
            None
        }
    }

    /// Get the face vertex counts at the specified time.
    ///
    /// For TetMesh, all surface faces are triangles, so this returns an array of 3s.
    /// Matches C++ `GetFaceVertexCounts(VtIntArray* faceVertexCounts, UsdTimeCode time)`.
    pub fn get_face_vertex_counts(&self, time: TimeCode) -> Option<Vec<i32>> {
        let attr = self.get_surface_face_vertex_indices_attr();
        if !attr.is_valid() {
            return None;
        }
        if let Some(value) = attr.get(time) {
            let count = if let Some(indices) = value.get::<Vec<Vec3i>>() {
                indices.len()
            } else if let Some(indices) = value.get::<usd_vt::Array<Vec3i>>() {
                indices.len()
            } else {
                return None;
            };
            Some(vec![3; count])
        } else {
            None
        }
    }

    /// Get the tet vertex indices at the specified time.
    ///
    /// Matches C++ `GetTetVertexIndices(VtVec4iArray* tetVertexIndices, UsdTimeCode time)`.
    pub fn get_tet_vertex_indices(&self, time: TimeCode) -> Option<usd_vt::Array<Vec4i>> {
        let attr = self.get_tet_vertex_indices_attr();
        if !attr.is_valid() {
            return None;
        }
        attr.get_typed::<usd_vt::Array<Vec4i>>(time)
    }

    /// Get the orientation at the specified time.
    ///
    /// Matches C++ `GetOrientation(TfToken* orientation, UsdTimeCode time)`.
    pub fn get_orientation(&self, time: TimeCode) -> Option<Token> {
        let attr = self.get_orientation_attr();
        if !attr.is_valid() {
            return None;
        }
        attr.get_typed::<Token>(time)
    }

    // ========================================================================
    // Compute Extent At Time
    // ========================================================================

    /// Compute the extent for the tet mesh at the specified time.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    /// On success, extent will contain the axis-aligned bounding box of the tet mesh.
    ///
    /// Matches C++ `ComputeExtentAtTime(VtVec3fArray* extent, UsdTimeCode time, UsdTimeCode baseTime)`.
    pub fn compute_extent_at_time(
        &self,
        extent: &mut Vec<Vec3f>,
        time: TimeCode,
        _base_time: TimeCode,
    ) -> bool {
        // Get points at the specified time
        let points = match self.get_points(time) {
            Some(p) => p,
            None => return false,
        };

        // Convert to slice for compute_extent
        let points_slice: Vec<Vec3f> = points.iter().cloned().collect();
        if points_slice.is_empty() {
            return false;
        }

        // Compute extent using PointBased static method
        let mut extent_array = [Vec3f::new(0.0, 0.0, 0.0), Vec3f::new(0.0, 0.0, 0.0)];
        if !super::point_based::PointBased::compute_extent(&points_slice, &mut extent_array) {
            return false;
        }

        extent.clear();
        extent.push(extent_array[0]);
        extent.push(extent_array[1]);
        true
    }

    /// Compute the extent for the tet mesh at the specified time with transform.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    ///
    /// Matches C++ `ComputeExtentAtTime(VtVec3fArray* extent, UsdTimeCode time, UsdTimeCode baseTime, const GfMatrix4d& transform)`.
    pub fn compute_extent_at_time_with_transform(
        &self,
        extent: &mut Vec<Vec3f>,
        time: TimeCode,
        _base_time: TimeCode,
        transform: &usd_gf::matrix4::Matrix4d,
    ) -> bool {
        // Get points at the specified time
        let points = match self.get_points(time) {
            Some(p) => p,
            None => return false,
        };

        // Convert to slice for compute_extent_with_transform
        let points_slice: Vec<Vec3f> = points.iter().cloned().collect();
        if points_slice.is_empty() {
            return false;
        }

        // Compute extent using PointBased static method with transform
        let mut extent_array = [Vec3f::new(0.0, 0.0, 0.0), Vec3f::new(0.0, 0.0, 0.0)];
        if !super::point_based::PointBased::compute_extent_with_transform(
            &points_slice,
            transform,
            &mut extent_array,
        ) {
            return false;
        }

        extent.clear();
        extent.push(extent_array[0]);
        extent.push(extent_array[1]);
        true
    }

    /// Compute the extent for the tet mesh at multiple times.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    ///
    /// Matches C++ `ComputeExtentAtTimes(std::vector<VtVec3fArray>* extents, const std::vector<UsdTimeCode>& times, UsdTimeCode baseTime)`.
    pub fn compute_extent_at_times(
        &self,
        extents: &mut Vec<Vec<Vec3f>>,
        times: &[TimeCode],
        base_time: TimeCode,
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

    /// Compute the extent for the tet mesh at multiple times with transform.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    ///
    /// Matches C++ `ComputeExtentAtTimes(std::vector<VtVec3fArray>* extents, const std::vector<UsdTimeCode>& times, UsdTimeCode baseTime, const GfMatrix4d& transform)`.
    pub fn compute_extent_at_times_with_transform(
        &self,
        extents: &mut Vec<Vec<Vec3f>>,
        times: &[TimeCode],
        base_time: TimeCode,
        transform: &usd_gf::matrix4::Matrix4d,
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
            usd_geom_tokens().tet_vertex_indices.clone(),
            usd_geom_tokens().surface_face_vertex_indices.clone(),
        ];

        if include_inherited {
            let mut all_names = PointBased::get_schema_attribute_names(true);
            all_names.extend(local_names);
            all_names
        } else {
            local_names
        }
    }
}

impl PartialEq for TetMesh {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for TetMesh {}

#[cfg(test)]
mod tests {
    use super::TetMesh;
    use usd_core::InitialLoadSet;
    use usd_core::Stage;
    use usd_gf::vec3::Vec3i;
    use usd_gf::vec4::Vec4i;
    use usd_sdf::TimeCode;
    use usd_vt::Value;

    #[test]
    fn create_tet_vertex_indices_attr_writes_default_value() {
        let _ = usd_sdf::init();
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("stage");
        let prim = stage.define_prim("/World/T1", "TetMesh").expect("prim");
        let tm = TetMesh::new(prim);
        let data = vec![Vec4i::new(0, 1, 2, 3)];
        let val = Value::from_no_hash(data.clone());
        let attr = tm.create_tet_vertex_indices_attr(Some(val), false);
        assert!(attr.is_valid());
        let got = attr.get(TimeCode::default()).expect("default sample");
        let roundtrip: Vec<Vec4i> = got
            .get::<Vec<Vec4i>>()
            .cloned()
            .or_else(|| {
                got.get::<usd_vt::Array<Vec4i>>()
                    .map(|a| a.iter().cloned().collect())
            })
            .expect("int4[]");
        assert_eq!(roundtrip, data);
    }

    #[test]
    fn create_surface_face_vertex_indices_attr_writes_default_value() {
        let _ = usd_sdf::init();
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("stage");
        let prim = stage.define_prim("/World/T2", "TetMesh").expect("prim");
        let tm = TetMesh::new(prim);
        let data = vec![Vec3i::new(0, 1, 2)];
        let val = Value::from_no_hash(data.clone());
        let attr = tm.create_surface_face_vertex_indices_attr(Some(val), false);
        assert!(attr.is_valid());
        let got = attr.get(TimeCode::default()).expect("default sample");
        let roundtrip: Vec<Vec3i> = got
            .get::<Vec<Vec3i>>()
            .cloned()
            .or_else(|| {
                got.get::<usd_vt::Array<Vec3i>>()
                    .map(|a| a.iter().cloned().collect())
            })
            .expect("int3[]");
        assert_eq!(roundtrip, data);
    }
}
