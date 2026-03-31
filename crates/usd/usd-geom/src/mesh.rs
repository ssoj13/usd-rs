//! UsdGeomMesh - mesh geometry schema.
//!
//! Port of pxr/usd/usdGeom/mesh.h/cpp
//!
//! Encodes a mesh with optional subdivision properties and features.

use super::point_based::PointBased;
use super::tokens::usd_geom_tokens;
use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, Stage};
use usd_gf::vec3::Vec3f;
use usd_sdf::{TimeCode, ValueTypeRegistry};
use usd_tf::Token;
use usd_vt::Value;

// ============================================================================
// Mesh
// ============================================================================

/// Constant for infinite sharpness in crease and corner sharpness arrays.
///
/// Matches C++ `UsdGeomMesh::SHARPNESS_INFINITE`.
pub const SHARPNESS_INFINITE: f32 = 10.0;

/// Mesh geometry schema.
///
/// Encodes a mesh with optional subdivision properties and features.
/// As a point-based primitive, meshes are defined in terms of points that
/// are connected into edges and faces.
///
/// Matches C++ `UsdGeomMesh`.
#[derive(Debug, Clone)]
pub struct Mesh {
    /// Base point-based schema.
    inner: PointBased,
}

impl Mesh {
    /// Creates a Mesh schema from a prim.
    ///
    /// Matches C++ `UsdGeomMesh(const UsdPrim& prim)`.
    pub fn new(prim: Prim) -> Self {
        Self {
            inner: PointBased::new(prim),
        }
    }

    /// Creates a Mesh schema from a PointBased schema.
    ///
    /// Matches C++ `UsdGeomMesh(const UsdSchemaBase& schemaObj)`.
    pub fn from_point_based(point_based: PointBased) -> Self {
        Self { inner: point_based }
    }

    /// Creates an invalid Mesh schema.
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
        Token::new("Mesh")
    }

    /// Return a Mesh holding the prim adhering to this schema at path on stage.
    ///
    /// Matches C++ `UsdGeomMesh::Get(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn get(stage: &Stage, path: &usd_sdf::Path) -> Self {
        if let Some(prim) = stage.get_prim_at_path(path) {
            Self::new(prim)
        } else {
            Self::invalid()
        }
    }

    /// Attempt to ensure a prim adhering to this schema at path is defined on this stage.
    ///
    /// Matches C++ `UsdGeomMesh::Define(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn define(stage: &Stage, path: &usd_sdf::Path) -> Self {
        match stage.define_prim(path.get_string(), Self::schema_type_name().as_str()) {
            Ok(prim) => Self::new(prim),
            Err(_) => Self::invalid(),
        }
    }

    // ========================================================================
    // FaceVertexIndices
    // ========================================================================

    /// Returns the faceVertexIndices attribute.
    ///
    /// Flat list of the index (into the points attribute) of each
    /// vertex of each face in the mesh.
    ///
    /// Matches C++ `GetFaceVertexIndicesAttr()`.
    pub fn get_face_vertex_indices_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().face_vertex_indices.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the faceVertexIndices attribute.
    ///
    /// Matches C++ `CreateFaceVertexIndicesAttr()`.
    pub fn create_face_vertex_indices_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let int_array_type = registry.find_type_by_token(&Token::new("int[]"));

        // Always call create_attribute — idempotent for the spec but ensures
        // property_names is updated so flatten() copies this attribute.
        prim.create_attribute(
            usd_geom_tokens().face_vertex_indices.as_str(),
            &int_array_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // FaceVertexCounts
    // ========================================================================

    /// Returns the faceVertexCounts attribute.
    ///
    /// Provides the number of vertices in each face of the mesh.
    ///
    /// Matches C++ `GetFaceVertexCountsAttr()`.
    pub fn get_face_vertex_counts_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().face_vertex_counts.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the faceVertexCounts attribute.
    ///
    /// Matches C++ `CreateFaceVertexCountsAttr()`.
    pub fn create_face_vertex_counts_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let int_array_type = registry.find_type_by_token(&Token::new("int[]"));

        // Always call create_attribute — it's idempotent for the spec but
        // ensures property_names is populated so flatten() copies the attr.
        prim.create_attribute(
            usd_geom_tokens().face_vertex_counts.as_str(),
            &int_array_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // SubdivisionScheme
    // ========================================================================

    /// Returns the subdivisionScheme attribute.
    ///
    /// The subdivision scheme to be applied to the surface.
    ///
    /// Matches C++ `GetSubdivisionSchemeAttr()`.
    pub fn get_subdivision_scheme_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().subdivision_scheme.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the subdivisionScheme attribute.
    ///
    /// Matches C++ `CreateSubdivisionSchemeAttr()`.
    pub fn create_subdivision_scheme_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        prim.create_attribute(
            usd_geom_tokens().subdivision_scheme.as_str(),
            &token_type,
            false,                      // not custom
            Some(Variability::Uniform), // uniform (doesn't vary over time)
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // InterpolateBoundary
    // ========================================================================

    /// Returns the interpolateBoundary attribute.
    ///
    /// Specifies how subdivision is applied for faces adjacent to boundary edges.
    ///
    /// Matches C++ `GetInterpolateBoundaryAttr()`.
    pub fn get_interpolate_boundary_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().interpolate_boundary.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the interpolateBoundary attribute.
    ///
    /// Matches C++ `CreateInterpolateBoundaryAttr()`.
    pub fn create_interpolate_boundary_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        prim.create_attribute(
            usd_geom_tokens().interpolate_boundary.as_str(),
            &token_type,
            false,                      // not custom
            Some(Variability::Varying), // C++ mesh.cpp:152 SdfVariabilityVarying
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // FaceVaryingLinearInterpolation
    // ========================================================================

    /// Returns the faceVaryingLinearInterpolation attribute.
    ///
    /// Specifies how elements of a primvar of interpolation type "faceVarying"
    /// are interpolated for subdivision surfaces.
    ///
    /// Matches C++ `GetFaceVaryingLinearInterpolationAttr()`.
    pub fn get_face_varying_linear_interpolation_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().face_varying_linear_interpolation.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the faceVaryingLinearInterpolation attribute.
    ///
    /// Matches C++ `CreateFaceVaryingLinearInterpolationAttr()`.
    pub fn create_face_varying_linear_interpolation_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        prim.create_attribute(
            usd_geom_tokens().face_varying_linear_interpolation.as_str(),
            &token_type,
            false,                      // not custom
            Some(Variability::Varying), // C++ mesh.cpp:169 SdfVariabilityVarying
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // TriangleSubdivisionRule
    // ========================================================================

    /// Returns the triangleSubdivisionRule attribute.
    ///
    /// Specifies an option to the subdivision rules for the Catmull-Clark scheme.
    ///
    /// Matches C++ `GetTriangleSubdivisionRuleAttr()`.
    pub fn get_triangle_subdivision_rule_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().triangle_subdivision_rule.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the triangleSubdivisionRule attribute.
    ///
    /// Matches C++ `CreateTriangleSubdivisionRuleAttr()`.
    pub fn create_triangle_subdivision_rule_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        prim.create_attribute(
            usd_geom_tokens().triangle_subdivision_rule.as_str(),
            &token_type,
            false,                      // not custom
            Some(Variability::Varying), // C++ mesh.cpp:186 SdfVariabilityVarying
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // HoleIndices
    // ========================================================================

    /// Returns the holeIndices attribute.
    ///
    /// The indices of all faces that should be treated as holes.
    ///
    /// Matches C++ `GetHoleIndicesAttr()`.
    pub fn get_hole_indices_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().hole_indices.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the holeIndices attribute.
    ///
    /// Matches C++ `CreateHoleIndicesAttr()`.
    pub fn create_hole_indices_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let int_array_type = registry.find_type_by_token(&Token::new("int[]"));

        prim.create_attribute(
            usd_geom_tokens().hole_indices.as_str(),
            &int_array_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // CornerIndices
    // ========================================================================

    /// Returns the cornerIndices attribute.
    ///
    /// The indices of points for which a corresponding sharpness value is specified.
    ///
    /// Matches C++ `GetCornerIndicesAttr()`.
    pub fn get_corner_indices_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().corner_indices.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the cornerIndices attribute.
    ///
    /// Matches C++ `CreateCornerIndicesAttr()`.
    pub fn create_corner_indices_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let int_array_type = registry.find_type_by_token(&Token::new("int[]"));

        prim.create_attribute(
            usd_geom_tokens().corner_indices.as_str(),
            &int_array_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // CornerSharpnesses
    // ========================================================================

    /// Returns the cornerSharpnesses attribute.
    ///
    /// The sharpness values associated with a corresponding set of points.
    ///
    /// Matches C++ `GetCornerSharpnessesAttr()`.
    pub fn get_corner_sharpnesses_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().corner_sharpnesses.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the cornerSharpnesses attribute.
    ///
    /// Matches C++ `CreateCornerSharpnessesAttr()`.
    pub fn create_corner_sharpnesses_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let float_array_type = registry.find_type_by_token(&Token::new("float[]"));

        prim.create_attribute(
            usd_geom_tokens().corner_sharpnesses.as_str(),
            &float_array_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // CreaseIndices
    // ========================================================================

    /// Returns the creaseIndices attribute.
    ///
    /// The indices of points grouped into sets of successive pairs that identify edges to be creased.
    ///
    /// Matches C++ `GetCreaseIndicesAttr()`.
    pub fn get_crease_indices_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().crease_indices.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the creaseIndices attribute.
    ///
    /// Matches C++ `CreateCreaseIndicesAttr()`.
    pub fn create_crease_indices_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let int_array_type = registry.find_type_by_token(&Token::new("int[]"));

        prim.create_attribute(
            usd_geom_tokens().crease_indices.as_str(),
            &int_array_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // CreaseLengths
    // ========================================================================

    /// Returns the creaseLengths attribute.
    ///
    /// The length of this array specifies the number of creases on the mesh.
    ///
    /// Matches C++ `GetCreaseLengthsAttr()`.
    pub fn get_crease_lengths_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().crease_lengths.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the creaseLengths attribute.
    ///
    /// Matches C++ `CreateCreaseLengthsAttr()`.
    pub fn create_crease_lengths_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let int_array_type = registry.find_type_by_token(&Token::new("int[]"));

        prim.create_attribute(
            usd_geom_tokens().crease_lengths.as_str(),
            &int_array_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // CreaseSharpnesses
    // ========================================================================

    /// Returns the creaseSharpnesses attribute.
    ///
    /// The per-crease or per-edge sharpness values for all creases.
    ///
    /// Matches C++ `GetCreaseSharpnessesAttr()`.
    pub fn get_crease_sharpnesses_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().crease_sharpnesses.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the creaseSharpnesses attribute.
    ///
    /// Matches C++ `CreateCreaseSharpnessesAttr()`.
    pub fn create_crease_sharpnesses_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let float_array_type = registry.find_type_by_token(&Token::new("float[]"));

        prim.create_attribute(
            usd_geom_tokens().crease_sharpnesses.as_str(),
            &float_array_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // Validation and Utility Methods
    // ========================================================================

    /// Validate the topology of a mesh.
    ///
    /// This validates that the sum of face_vertex_counts is equal to the size
    /// of the face_vertex_indices array, and that all face vertex indices in
    /// the face_vertex_indices array are in the range [0, num_points).
    ///
    /// Returns true if the topology is valid, or false otherwise.
    /// If the topology is invalid and reason is Some, an error message
    /// describing the validation error will be set.
    ///
    /// Matches C++ `ValidateTopology()`.
    pub fn validate_topology(
        face_vertex_indices: &[i32],
        face_vertex_counts: &[i32],
        num_points: usize,
        reason: Option<&mut String>,
    ) -> bool {
        // Sum of the vertex counts should be equal to the number of vertex indices.
        let vert_counts_sum: i32 = face_vertex_counts.iter().sum();

        if vert_counts_sum as usize != face_vertex_indices.len() {
            if let Some(reason) = reason {
                *reason = format!(
                    "Sum of faceVertexCounts [{}] != size of faceVertexIndices [{}].",
                    vert_counts_sum,
                    face_vertex_indices.len()
                );
            }
            return false;
        }

        // Make sure all verts are within the range of the point count.
        for &vertex_index in face_vertex_indices {
            if vertex_index < 0 || vertex_index as usize >= num_points {
                if let Some(reason) = reason {
                    *reason = format!(
                        "Out of range face vertex index {}: Vertex must be in the range [0,{}).",
                        vertex_index, num_points
                    );
                }
                return false;
            }
        }
        true
    }

    /// Returns whether or not sharpness is considered infinite.
    ///
    /// The sharpness value is usually intended for 'creaseSharpness' or
    /// 'cornerSharpness' arrays and a return value of true indicates that
    /// the crease or corner is perfectly sharp.
    ///
    /// Matches C++ `IsSharpnessInfinite()`.
    pub fn is_sharpness_infinite(sharpness: f32) -> bool {
        sharpness >= SHARPNESS_INFINITE
    }

    /// Returns the number of faces as defined by the size of the
    /// faceVertexCounts array at time_code.
    ///
    /// Matches C++ `GetFaceCount()`.
    pub fn get_face_count(&self, time_code: TimeCode) -> usize {
        let vertex_counts_attr = self.get_face_vertex_counts_attr();
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

    // ========================================================================
    // Get Methods (Value Retrieval)
    // ========================================================================

    /// Get the face vertex indices at the specified time.
    ///
    /// Matches C++ `GetFaceVertexIndices(VtIntArray* indices, UsdTimeCode time)`.
    pub fn get_face_vertex_indices(&self, time: TimeCode) -> Option<usd_vt::Array<i32>> {
        self.get_face_vertex_indices_attr()
            .get_typed::<usd_vt::Array<i32>>(time)
    }

    /// Get the face vertex counts at the specified time.
    ///
    /// Matches C++ `GetFaceVertexCounts(VtIntArray* counts, UsdTimeCode time)`.
    pub fn get_face_vertex_counts(&self, time: TimeCode) -> Option<usd_vt::Array<i32>> {
        self.get_face_vertex_counts_attr()
            .get_typed::<usd_vt::Array<i32>>(time)
    }

    /// Get the subdivision scheme at the specified time.
    ///
    /// Matches C++ `GetSubdivisionScheme(TfToken* scheme, UsdTimeCode time)`.
    pub fn get_subdivision_scheme(&self, time: TimeCode) -> Option<Token> {
        self.get_subdivision_scheme_attr().get_typed::<Token>(time)
    }

    /// Set the subdivision scheme at the specified time.
    ///
    /// Matches C++ `SetSubdivisionScheme(const TfToken& scheme, UsdTimeCode time)`.
    pub fn set_subdivision_scheme(&self, scheme: &Token, time: TimeCode) -> bool {
        let attr = self.get_subdivision_scheme_attr();
        if !attr.is_valid() {
            return false;
        }
        use usd_vt::Value;
        attr.set(Value::from_no_hash(scheme.clone()), time)
    }

    /// Get the interpolate boundary value at the specified time.
    ///
    /// Matches C++ `GetInterpolateBoundary(TfToken* boundary, UsdTimeCode time)`.
    pub fn get_interpolate_boundary(&self, time: TimeCode) -> Option<Token> {
        self.get_interpolate_boundary_attr()
            .get_typed::<Token>(time)
    }

    /// Get the face varying linear interpolation value at the specified time.
    ///
    /// Matches C++ `GetFaceVaryingLinearInterpolation(TfToken* interpolation, UsdTimeCode time)`.
    pub fn get_face_varying_linear_interpolation(&self, time: TimeCode) -> Option<Token> {
        self.get_face_varying_linear_interpolation_attr()
            .get_typed::<Token>(time)
    }

    /// Get the triangle subdivision rule at the specified time.
    ///
    /// Matches C++ `GetTriangleSubdivisionRule(TfToken* rule, UsdTimeCode time)`.
    pub fn get_triangle_subdivision_rule(&self, time: TimeCode) -> Option<Token> {
        self.get_triangle_subdivision_rule_attr()
            .get_typed::<Token>(time)
    }

    /// Get the hole indices at the specified time.
    ///
    /// Matches C++ `GetHoleIndices(VtIntArray* indices, UsdTimeCode time)`.
    pub fn get_hole_indices(&self, time: TimeCode) -> Option<usd_vt::Array<i32>> {
        self.get_hole_indices_attr()
            .get_typed::<usd_vt::Array<i32>>(time)
    }

    /// Get the corner indices at the specified time.
    ///
    /// Matches C++ `GetCornerIndices(VtIntArray* indices, UsdTimeCode time)`.
    pub fn get_corner_indices(&self, time: TimeCode) -> Option<usd_vt::Array<i32>> {
        self.get_corner_indices_attr()
            .get_typed::<usd_vt::Array<i32>>(time)
    }

    /// Get the corner sharpnesses at the specified time.
    ///
    /// Matches C++ `GetCornerSharpnesses(VtFloatArray* sharpnesses, UsdTimeCode time)`.
    pub fn get_corner_sharpnesses(&self, time: TimeCode) -> Option<usd_vt::Array<f32>> {
        self.get_corner_sharpnesses_attr()
            .get_typed::<usd_vt::Array<f32>>(time)
    }

    /// Get the crease indices at the specified time.
    ///
    /// Matches C++ `GetCreaseIndices(VtIntArray* indices, UsdTimeCode time)`.
    pub fn get_crease_indices(&self, time: TimeCode) -> Option<usd_vt::Array<i32>> {
        self.get_crease_indices_attr()
            .get_typed::<usd_vt::Array<i32>>(time)
    }

    /// Get the crease lengths at the specified time.
    ///
    /// Matches C++ `GetCreaseLengths(VtIntArray* lengths, UsdTimeCode time)`.
    pub fn get_crease_lengths(&self, time: TimeCode) -> Option<usd_vt::Array<i32>> {
        self.get_crease_lengths_attr()
            .get_typed::<usd_vt::Array<i32>>(time)
    }

    /// Get the crease sharpnesses at the specified time.
    ///
    /// Matches C++ `GetCreaseSharpnesses(VtFloatArray* sharpnesses, UsdTimeCode time)`.
    pub fn get_crease_sharpnesses(&self, time: TimeCode) -> Option<usd_vt::Array<f32>> {
        self.get_crease_sharpnesses_attr()
            .get_typed::<usd_vt::Array<f32>>(time)
    }

    // ========================================================================
    // Compute Extent
    // ========================================================================

    /// Compute the extent for the mesh at the specified time.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    /// On success, extent will contain the axis-aligned bounding box of the mesh.
    ///
    /// Matches C++ `ComputeExtentAtTime(VtVec3fArray* extent, UsdTimeCode time, UsdTimeCode baseTime)`.
    pub fn compute_extent_at_time(
        &self,
        extent: &mut Vec<Vec3f>,
        time: TimeCode,
        base_time: TimeCode,
    ) -> bool {
        // Get points at the specified time
        let mut points = Vec::new();
        if !self
            .inner
            .compute_points_at_time(&mut points, time, base_time)
        {
            return false;
        }

        if points.is_empty() {
            return false;
        }

        // Compute extent from points
        let mut extent_array = [Vec3f::new(0.0, 0.0, 0.0), Vec3f::new(0.0, 0.0, 0.0)];
        if !PointBased::compute_extent(&points, &mut extent_array) {
            return false;
        }

        extent.clear();
        extent.push(extent_array[0]);
        extent.push(extent_array[1]);
        true
    }

    /// Compute the extent for the mesh at the specified time with transform.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    ///
    /// Matches C++ `ComputeExtentAtTime(VtVec3fArray* extent, UsdTimeCode time, UsdTimeCode baseTime, const GfMatrix4d& transform)`.
    pub fn compute_extent_at_time_with_transform(
        &self,
        extent: &mut Vec<Vec3f>,
        time: TimeCode,
        base_time: TimeCode,
        transform: &usd_gf::matrix4::Matrix4d,
    ) -> bool {
        // Get points at the specified time
        let mut points = Vec::new();
        if !self
            .inner
            .compute_points_at_time(&mut points, time, base_time)
        {
            return false;
        }

        if points.is_empty() {
            return false;
        }

        // Compute extent from points with transform
        let mut extent_array = [Vec3f::new(0.0, 0.0, 0.0), Vec3f::new(0.0, 0.0, 0.0)];
        if !PointBased::compute_extent_with_transform(&points, transform, &mut extent_array) {
            return false;
        }

        extent.clear();
        extent.push(extent_array[0]);
        extent.push(extent_array[1]);
        true
    }

    /// Compute the extent for the mesh at multiple times.
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

    /// Compute the extent for the mesh at multiple times with transform.
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
            usd_geom_tokens().face_vertex_indices.clone(),
            usd_geom_tokens().face_vertex_counts.clone(),
            usd_geom_tokens().subdivision_scheme.clone(),
            usd_geom_tokens().interpolate_boundary.clone(),
            usd_geom_tokens().face_varying_linear_interpolation.clone(),
            usd_geom_tokens().triangle_subdivision_rule.clone(),
            usd_geom_tokens().hole_indices.clone(),
            usd_geom_tokens().corner_indices.clone(),
            usd_geom_tokens().corner_sharpnesses.clone(),
            usd_geom_tokens().crease_indices.clone(),
            usd_geom_tokens().crease_lengths.clone(),
            usd_geom_tokens().crease_sharpnesses.clone(),
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

impl PartialEq for Mesh {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for Mesh {}
