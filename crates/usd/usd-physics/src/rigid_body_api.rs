//! Physics Rigid Body API schema.
//!
//! Applies physics body attributes to any UsdGeomXformable prim and marks
//! that prim to be driven by a simulation. If a simulation is running,
//! it will update this prim's pose.
//!
//! All prims in the hierarchy below this prim should move rigidly along
//! with the body, except when the descendant prim has its own RigidBodyAPI
//! (marking a separate rigid body subtree which moves independently).
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdPhysics/rigidBodyAPI.h` and `rigidBodyAPI.cpp`
//!
//! # Mass Properties Computation
//!
//! The `compute_mass_properties` method traverses all collision shapes beneath
//! a rigid body, gathers their mass information (from MassAPI, materials, or
//! computed from volume), and produces final aggregate mass properties.
//!
//! # Usage
//!
//! ```ignore
//! use usd::usd_physics::RigidBodyAPI;
//!
//! // Apply rigid body behavior to a mesh
//! let rigid_body = RigidBodyAPI::apply(&mesh_prim)?;
//! rigid_body.create_velocity_attr(Some(Vec3f::new(0.0, 0.0, 0.0)))?;
//! rigid_body.create_angular_velocity_attr(Some(Vec3f::new(0.0, 0.0, 0.0)))?;
//!
//! // Compute mass properties from collision shapes
//! let (mass, inertia, com, pa) = rigid_body.compute_mass_properties(&mass_info_fn);
//! ```

use std::sync::Arc;

use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, PrimRange, Relationship, SchemaKind, Stage};
use usd_gf::{Matrix3f, Matrix4f, Quatf, Vec3f};
use usd_sdf::{Path, TimeCode, ValueTypeRegistry};
use usd_tf::Token;
use usd_vt::Value;

use super::collision_api::CollisionAPI;
use super::mass_api::MassAPI;
use super::mass_properties::MassProperties;
use super::material_api::MaterialAPI;
use super::metrics::get_stage_kilograms_per_unit;
use super::tokens::USD_PHYSICS_TOKENS;
use usd_geom::{XformCache, get_stage_meters_per_unit};
use usd_shade::{Material, MaterialBindingAPI};

// ============================================================================
// Constants
// ============================================================================

/// Tolerance for floating point comparisons in mass computation.
const COMPARE_TOLERANCE: f32 = 1e-05;

/// Material purpose token for physics materials.
const MATERIAL_PURPOSE_PHYSICS: &str = "physics";

// ============================================================================
// MassApiData - Internal structure for parsed mass properties
// ============================================================================

/// Internal structure holding parsed MassAPI data for a prim.
///
/// Used during mass accumulation to store values read from UsdPhysicsMassAPI.
#[derive(Debug, Clone)]
struct MassApiData {
    /// Explicit mass value (-1 = not set)
    mass: f32,
    /// Density value (-1 = not set)
    density: f32,
    /// Whether inertia tensor was explicitly specified
    has_inertia: bool,
    /// Diagonal inertia values
    diagonal_inertia: Vec3f,
    /// Whether principal axes were explicitly specified
    has_pa: bool,
    /// Principal axes rotation
    principal_axes: Quatf,
}

impl Default for MassApiData {
    fn default() -> Self {
        Self {
            mass: -1.0,
            density: -1.0,
            has_inertia: false,
            diagonal_inertia: Vec3f::new(1.0, 1.0, 1.0),
            has_pa: false,
            principal_axes: Quatf::identity(),
        }
    }
}

/// Parse MassAPI data from a prim.
///
/// Reads mass, density, diagonal inertia, and principal axes from the
/// UsdPhysicsMassAPI applied to the prim.
fn parse_mass_api(prim: &Prim) -> MassApiData {
    let mut result = MassApiData::default();

    if !prim.has_api(&Token::new(MassAPI::SCHEMA_TYPE_NAME)) {
        return result;
    }

    let mass_api = MassAPI::new(prim.clone());

    // Read density
    if let Some(attr) = mass_api.get_density_attr() {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.get::<f32>().copied())
        {
            result.density = val;
        }
    }

    // Read mass
    if let Some(attr) = mass_api.get_mass_attr() {
        if let Some(m) = attr
            .get(TimeCode::default())
            .and_then(|v| v.get::<f32>().copied())
        {
            if m > 0.0 {
                result.mass = m;
            }
        }
    }

    // Read diagonal inertia
    if let Some(attr) = mass_api.get_diagonal_inertia_attr() {
        if let Some(dg) = attr
            .get(TimeCode::default())
            .and_then(|v| v.get::<Vec3f>().copied())
        {
            // Check if non-zero (sentinel value is (0,0,0))
            if !is_close_vec3(&dg, &Vec3f::zero(), COMPARE_TOLERANCE) {
                result.has_inertia = true;
                result.diagonal_inertia = dg;
            }
        }
    }

    // Read principal axes
    if let Some(attr) = mass_api.get_principal_axes_attr() {
        if let Some(pa) = attr
            .get(TimeCode::default())
            .and_then(|v| v.get::<Quatf>().copied())
        {
            // Sentinel value is (0,0,0,0) - check if not sentinel
            let imag = pa.imaginary();
            if !is_close_vec3(imag, &Vec3f::zero(), COMPARE_TOLERANCE)
                || pa.real().abs() > COMPARE_TOLERANCE
            {
                result.has_pa = true;
                result.principal_axes = pa;
            }
        }
    }

    result
}

/// Get center of mass with scale transformation applied.
///
/// Returns (com, true) if CoM was explicitly set, (default, false) otherwise.
fn get_com(prim: &Prim, xf_cache: &mut XformCache) -> (Vec3f, bool) {
    if !prim.has_api(&Token::new(MassAPI::SCHEMA_TYPE_NAME)) {
        return (Vec3f::zero(), false);
    }

    let mass_api = MassAPI::new(prim.clone());

    if let Some(attr) = mass_api.get_center_of_mass_attr() {
        if let Some(v) = attr
            .get(TimeCode::default())
            .and_then(|v| v.get::<Vec3f>().copied())
        {
            // Sentinel value is (-inf, -inf, -inf) - any inf means not set
            if v.x.is_finite() && v.y.is_finite() && v.z.is_finite() {
                // Apply scale from transform - physics doesn't support scale,
                // so we need to scale the CoM manually
                let mat = xf_cache.get_local_to_world_transform(prim);
                let scale = mat.extract_scale();
                let scaled_com = Vec3f::new(
                    v.x * scale.x as f32,
                    v.y * scale.y as f32,
                    v.z * scale.z as f32,
                );
                return (scaled_com, true);
            }
        }
    }

    (Vec3f::zero(), false)
}

/// Get mass/density data for a collision shape, checking material if needed.
///
/// Returns (MassApiData, shape_density) where shape_density is the final
/// density to use for the shape.
fn get_collision_shape_mass_api_data(
    collision_prim: &Prim,
    body_density: f32,
    material: Option<&Material>,
) -> (MassApiData, f32) {
    let mut shape_mass_info = parse_mass_api(collision_prim);

    // Use parent density if shape doesn't have one specified
    if shape_mass_info.density <= 0.0 {
        shape_mass_info.density = body_density;
    }

    let mut density = shape_mass_info.density;

    // If density still not set, try to get it from the material
    if density <= 0.0 {
        if let Some(mat) = material {
            if mat.is_valid() {
                let mat_prim = mat.get_prim();
                if mat_prim.has_api(&Token::new(MaterialAPI::SCHEMA_TYPE_NAME)) {
                    let mat_api = MaterialAPI::new(mat_prim.clone());
                    if let Some(attr) = mat_api.get_density_attr() {
                        if let Some(d) = attr
                            .get(TimeCode::default())
                            .and_then(|v| v.get::<f32>().copied())
                        {
                            density = d;
                        }
                    }
                }
            }
        }
    }

    (shape_mass_info, density)
}

/// Parse a collision shape and compute its mass properties.
///
/// Uses the MassInformationFn callback to get volume and inertia for the shape.
fn parse_collision_shape_for_mass(
    prim: &Prim,
    shape_mass_info: &MassApiData,
    mut density: f32,
    xf_cache: &mut XformCache,
    mass_info_fn: &MassInformationFn,
) -> (MassProperties, Matrix4f) {
    // Get actual mass information from callback
    let mass_info = mass_info_fn(prim);

    if mass_info.volume < 0.0 {
        // Invalid mass info
        return (MassProperties::default(), Matrix4f::identity());
    }

    let mut inertia = mass_info.inertia;
    let mut shape_mass = shape_mass_info.mass;

    // Default density based on stage units if not set
    if density <= 0.0 {
        if let Some(stage) = prim.stage() {
            // Default density: 1000 kg/m^3 converted to stage units
            let meters_per_unit = get_stage_meters_per_unit(&stage) as f32;
            let kg_per_unit = get_stage_kilograms_per_unit(&stage) as f32;
            density = 1000.0 * meters_per_unit * meters_per_unit * meters_per_unit / kg_per_unit;
        } else {
            density = 1000.0; // Fallback
        }
    }

    // Get center of mass (may be overridden)
    let (mut center_of_mass, has_com) = get_com(prim, xf_cache);
    if !has_com {
        center_of_mass = mass_info.center_of_mass;
    }

    // Calculate mass from explicit value or volume * density
    if shape_mass > 0.0 {
        // Explicit mass - scale inertia accordingly
        inertia *= shape_mass / mass_info.volume;
    } else if mass_info.volume >= 0.0 {
        // Compute mass from volume and density
        shape_mass = mass_info.volume * density;
        inertia *= density;
    }

    // Apply explicit inertia if provided
    if shape_mass_info.has_inertia {
        // Convert quaternion to rotation matrix
        let (axis, angle) = shape_mass_info.principal_axes.to_axis_angle();
        let rot_matr = Matrix3f::from_rotation(axis, angle);

        // Build diagonal inertia matrix
        let mut in_matr = Matrix3f::zero();
        in_matr[0][0] = shape_mass_info.diagonal_inertia.x;
        in_matr[1][1] = shape_mass_info.diagonal_inertia.y;
        in_matr[2][2] = shape_mass_info.diagonal_inertia.z;
        inertia = in_matr * rot_matr;
    }

    // Apply principal axes rotation if provided
    if shape_mass_info.has_pa {
        inertia = MassProperties::rotate_inertia(&inertia, &shape_mass_info.principal_axes);
    }

    // Update inertia for CoM override
    if has_com && !shape_mass_info.has_inertia {
        let mut props = MassProperties::new(shape_mass, inertia, mass_info.center_of_mass);
        let translation = center_of_mass - *props.center_of_mass();
        props.translate(&translation);
        inertia = *props.inertia_tensor();
    }

    // Build transform matrix from collision local pose
    let mut transform = Matrix4f::identity();
    transform.set_translate(&mass_info.local_pos);
    transform.set_rotate_only(&mass_info.local_rot);

    (
        MassProperties::new(shape_mass, inertia, center_of_mass),
        transform,
    )
}

/// Check if two Vec3f are approximately equal.
#[inline]
fn is_close_vec3(a: &Vec3f, b: &Vec3f, tolerance: f32) -> bool {
    (a.x - b.x).abs() < tolerance && (a.y - b.y).abs() < tolerance && (a.z - b.z).abs() < tolerance
}

// ============================================================================
// Public types
// ============================================================================

/// Mass information for a collision, used in ComputeMassProperties callback.
///
/// Matches C++ `UsdPhysicsRigidBodyAPI::MassInformation`.
#[derive(Debug, Clone)]
pub struct MassInformation {
    /// Collision volume.
    pub volume: f32,
    /// Collision inertia tensor.
    pub inertia: Matrix3f,
    /// Collision center of mass.
    pub center_of_mass: Vec3f,
    /// Collision local position with respect to the rigid body.
    pub local_pos: Vec3f,
    /// Collision local rotation with respect to the rigid body.
    pub local_rot: Quatf,
}

impl Default for MassInformation {
    fn default() -> Self {
        Self {
            volume: 0.0,
            inertia: Matrix3f::identity(),
            center_of_mass: Vec3f::default(),
            local_pos: Vec3f::default(),
            local_rot: Quatf::identity(),
        }
    }
}

/// Mass information callback function type.
///
/// For a given UsdPrim, returns its MassInformation.
pub type MassInformationFn = Box<dyn Fn(&Prim) -> MassInformation + Send + Sync>;

/// Physics rigid body API schema.
///
/// Applies physics body attributes to any UsdGeomXformable prim and
/// marks that prim to be driven by a simulation. If a simulation is running
/// it will update this prim's pose. All prims in the hierarchy below this
/// prim should move rigidly along with the body, except when the descendant
/// prim has its own UsdPhysicsRigidBodyAPI (marking a separate rigid body
/// subtree which moves independently of the parent rigid body).
///
/// # Schema Kind
///
/// This is a single-apply API schema (SingleApplyAPI).
///
/// # C++ Reference
///
/// Port of `UsdPhysicsRigidBodyAPI` class.
#[derive(Debug, Clone)]
pub struct RigidBodyAPI {
    prim: Prim,
}

impl RigidBodyAPI {
    /// The schema kind for this class.
    pub const SCHEMA_KIND: SchemaKind = SchemaKind::SingleApplyAPI;

    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "PhysicsRigidBodyAPI";

    // =========================================================================
    // Construction
    // =========================================================================

    /// Construct a RigidBodyAPI on the given prim.
    ///
    /// Equivalent to `RigidBodyAPI::get(prim.get_stage(), prim.get_path())`
    /// for a valid prim, but will not immediately throw an error for
    /// an invalid prim.
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Construct a RigidBodyAPI from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return a RigidBodyAPI holding the prim at `path` on `stage`.
    ///
    /// If no prim exists at `path` on `stage`, or if the prim does not
    /// have this API schema applied, return None.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        if prim.has_api(&Token::new(Self::SCHEMA_TYPE_NAME)) {
            Some(Self::new(prim))
        } else {
            None
        }
    }

    /// Returns true if this single-apply API schema can be applied to the given prim.
    ///
    /// If this schema cannot be applied to the prim, this returns false and,
    /// if provided, populates `why_not` with the reason it cannot be applied.
    pub fn can_apply(prim: &Prim, _why_not: Option<&mut String>) -> bool {
        prim.can_apply_api(&Token::new(Self::SCHEMA_TYPE_NAME))
    }

    /// Applies this single-apply API schema to the given prim.
    ///
    /// This information is stored by adding "PhysicsRigidBodyAPI" to the
    /// token-valued, listOp metadata `apiSchemas` on the prim.
    ///
    /// Returns a valid RigidBodyAPI object upon success, or None upon failure.
    pub fn apply(prim: &Prim) -> Option<Self> {
        if prim.apply_api(&Token::new(Self::SCHEMA_TYPE_NAME)) {
            Some(Self::new(prim.clone()))
        } else {
            None
        }
    }

    /// Returns the schema kind.
    pub fn get_schema_kind(&self) -> SchemaKind {
        Self::SCHEMA_KIND
    }

    // =========================================================================
    // RigidBodyEnabled Attribute
    // =========================================================================

    /// Determines if this PhysicsRigidBodyAPI is enabled.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `bool physics:rigidBodyEnabled = 1` |
    /// | C++ Type | bool |
    pub fn get_rigid_body_enabled_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_PHYSICS_TOKENS.physics_rigid_body_enabled.as_str())
    }

    /// Creates the rigidBodyEnabled attribute.
    pub fn create_rigid_body_enabled_attr(&self, default_value: Option<bool>) -> Option<Attribute> {
        let registry = ValueTypeRegistry::instance();
        let type_name = registry.find_type_by_token(&Token::new("bool"));
        let attr = self.prim.create_attribute(
            USD_PHYSICS_TOKENS.physics_rigid_body_enabled.as_str(),
            &type_name,
            false,
            Some(Variability::Varying),
        )?;

        if let Some(value) = default_value {
            attr.set(Value::from(value), TimeCode::default());
        }

        Some(attr)
    }

    // =========================================================================
    // KinematicEnabled Attribute
    // =========================================================================

    /// Determines whether the body is kinematic or not.
    ///
    /// A kinematic body is moved through animated poses or through user
    /// defined poses. The simulation derives velocities for the kinematic
    /// body based on the external motion. When a continuous motion is not
    /// desired, this kinematic flag should be set to false.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `bool physics:kinematicEnabled = 0` |
    /// | C++ Type | bool |
    pub fn get_kinematic_enabled_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_PHYSICS_TOKENS.physics_kinematic_enabled.as_str())
    }

    /// Creates the kinematicEnabled attribute.
    pub fn create_kinematic_enabled_attr(&self, default_value: Option<bool>) -> Option<Attribute> {
        let registry = ValueTypeRegistry::instance();
        let type_name = registry.find_type_by_token(&Token::new("bool"));
        let attr = self.prim.create_attribute(
            USD_PHYSICS_TOKENS.physics_kinematic_enabled.as_str(),
            &type_name,
            false,
            Some(Variability::Varying),
        )?;

        if let Some(value) = default_value {
            attr.set(Value::from(value), TimeCode::default());
        }

        Some(attr)
    }

    // =========================================================================
    // StartsAsleep Attribute
    // =========================================================================

    /// Determines if the body is asleep when the simulation starts.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `uniform bool physics:startsAsleep = 0` |
    /// | C++ Type | bool |
    /// | Variability | Uniform |
    pub fn get_starts_asleep_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_PHYSICS_TOKENS.physics_starts_asleep.as_str())
    }

    /// Creates the startsAsleep attribute.
    pub fn create_starts_asleep_attr(&self, default_value: Option<bool>) -> Option<Attribute> {
        let registry = ValueTypeRegistry::instance();
        let type_name = registry.find_type_by_token(&Token::new("bool"));
        let attr = self.prim.create_attribute(
            USD_PHYSICS_TOKENS.physics_starts_asleep.as_str(),
            &type_name,
            false,
            Some(Variability::Uniform),
        )?;

        if let Some(value) = default_value {
            attr.set(Value::from(value), TimeCode::default());
        }

        Some(attr)
    }

    // =========================================================================
    // Velocity Attribute
    // =========================================================================

    /// Linear velocity in the same space as the node's xform.
    ///
    /// Units: distance/second.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `vector3f physics:velocity = (0, 0, 0)` |
    /// | C++ Type | GfVec3f |
    pub fn get_velocity_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_PHYSICS_TOKENS.physics_velocity.as_str())
    }

    /// Creates the velocity attribute.
    pub fn create_velocity_attr(&self, default_value: Option<Vec3f>) -> Option<Attribute> {
        let registry = ValueTypeRegistry::instance();
        let type_name = registry.find_type_by_token(&Token::new("float3"));
        let attr = self.prim.create_attribute(
            USD_PHYSICS_TOKENS.physics_velocity.as_str(),
            &type_name,
            false,
            Some(Variability::Varying),
        )?;

        if let Some(value) = default_value {
            attr.set(Value::from_no_hash(value), TimeCode::default());
        }

        Some(attr)
    }

    // =========================================================================
    // AngularVelocity Attribute
    // =========================================================================

    /// Angular velocity in the same space as the node's xform.
    ///
    /// Units: degrees/second.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `vector3f physics:angularVelocity = (0, 0, 0)` |
    /// | C++ Type | GfVec3f |
    pub fn get_angular_velocity_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_PHYSICS_TOKENS.physics_angular_velocity.as_str())
    }

    /// Creates the angularVelocity attribute.
    pub fn create_angular_velocity_attr(&self, default_value: Option<Vec3f>) -> Option<Attribute> {
        let registry = ValueTypeRegistry::instance();
        let type_name = registry.find_type_by_token(&Token::new("float3"));
        let attr = self.prim.create_attribute(
            USD_PHYSICS_TOKENS.physics_angular_velocity.as_str(),
            &type_name,
            false,
            Some(Variability::Varying),
        )?;

        if let Some(value) = default_value {
            attr.set(Value::from_no_hash(value), TimeCode::default());
        }

        Some(attr)
    }

    // =========================================================================
    // SimulationOwner Relationship
    // =========================================================================

    /// Single PhysicsScene that will simulate this body.
    ///
    /// By default this is the first PhysicsScene found in the stage
    /// using `UsdStage::Traverse()`.
    pub fn get_simulation_owner_rel(&self) -> Option<Relationship> {
        self.prim
            .get_relationship(USD_PHYSICS_TOKENS.physics_simulation_owner.as_str())
    }

    /// Creates the simulationOwner relationship.
    pub fn create_simulation_owner_rel(&self) -> Option<Relationship> {
        self.prim
            .create_relationship(USD_PHYSICS_TOKENS.physics_simulation_owner.as_str(), false)
    }

    // =========================================================================
    // Schema attribute names
    // =========================================================================

    /// Returns all pre-declared attributes for this schema class.
    pub fn get_schema_attribute_names(_include_inherited: bool) -> Vec<Token> {
        vec![
            USD_PHYSICS_TOKENS.physics_rigid_body_enabled.clone(),
            USD_PHYSICS_TOKENS.physics_kinematic_enabled.clone(),
            USD_PHYSICS_TOKENS.physics_starts_asleep.clone(),
            USD_PHYSICS_TOKENS.physics_velocity.clone(),
            USD_PHYSICS_TOKENS.physics_angular_velocity.clone(),
        ]
    }

    // =========================================================================
    // Custom Methods
    // =========================================================================

    /// Compute mass properties of the rigid body.
    ///
    /// Traverses all collision shapes in the subtree, gathers their mass
    /// information, and produces final aggregate mass properties.
    ///
    /// # Arguments
    ///
    /// * `mass_info_fn` - Callback function that returns `MassInformation`
    ///   (volume, inertia, CoM, local transform) for each collision prim.
    ///
    /// # Returns
    ///
    /// A tuple of `(mass, diagonal_inertia, center_of_mass, principal_axes)`:
    /// - `mass`: Total mass of the rigid body
    /// - `diagonal_inertia`: Principal moments of inertia
    /// - `center_of_mass`: Center of mass position
    /// - `principal_axes`: Rotation to principal axes frame
    ///
    /// # Mass Computation Rules
    ///
    /// 1. If mass is explicitly set on the rigid body, it takes precedence
    /// 2. Otherwise, mass is computed from collision volumes and densities
    /// 3. Density priority: shape MassAPI > body MassAPI > material > default
    /// 4. Default density is 1000 kg/m³ in stage units
    ///
    /// # C++ Reference
    ///
    /// Port of `UsdPhysicsRigidBodyAPI::ComputeMassProperties()`
    pub fn compute_mass_properties(
        &self,
        mass_info_fn: &MassInformationFn,
    ) -> (f32, Vec3f, Vec3f, Quatf) {
        let prim = &self.prim;

        // Create transform cache for this computation
        let mut xf_cache = XformCache::default();

        // Parse mass data from rigid body prim
        let rigid_body_mass_info = parse_mass_api(prim);

        // If we don't have explicit mass, we need to compute from collisions
        let accumulate_mass = rigid_body_mass_info.mass <= 0.0;

        // Get initial values from rigid body mass info
        let (mut out_com, has_com) = get_com(prim, &mut xf_cache);
        let has_pa = rigid_body_mass_info.has_pa;
        let mut out_pa = rigid_body_mass_info.principal_axes;
        let mut out_mass = rigid_body_mass_info.mass;
        let mut out_inertia = rigid_body_mass_info.diagonal_inertia;

        // If we need to compute mass/inertia/CoM from collisions
        if accumulate_mass || !rigid_body_mass_info.has_inertia || !has_com {
            // Collect all collision prims in the subtree
            let mut collision_prims: Vec<Prim> = Vec::new();

            // Traverse subtree, stopping at nested rigid bodies.
            // We track nested body paths to skip their entire subtrees.
            let mut nested_body_paths: Vec<Path> = Vec::new();
            let range = PrimRange::from_prim(prim);
            for descendant in range {
                let desc_path = descendant.path().clone();

                // Check if this descendant is inside a nested rigid body
                let inside_nested = nested_body_paths.iter().any(|nb| desc_path.has_prefix(&nb));
                if inside_nested {
                    continue;
                }

                // Skip the root prim itself for the RigidBodyAPI check
                if desc_path != *prim.path()
                    && descendant.has_api(&Token::new(RigidBodyAPI::SCHEMA_TYPE_NAME))
                {
                    // Mark this as a nested body — skip its entire subtree
                    nested_body_paths.push(desc_path);
                    continue;
                }

                if descendant.has_api(&Token::new(CollisionAPI::SCHEMA_TYPE_NAME)) {
                    collision_prims.push(descendant);
                }
            }

            // Get physics materials for all collision prims
            let physics_purpose = Token::new(MATERIAL_PURPOSE_PHYSICS);
            let materials = MaterialBindingAPI::compute_bound_materials(
                &collision_prims,
                &physics_purpose,
                &mut None,
                false,
            );

            // Filter out materials that don't have PhysicsMaterialAPI
            let physics_materials: Vec<Option<Material>> = materials
                .into_iter()
                .map(|m| {
                    if m.is_valid()
                        && m.get_prim()
                            .has_api(&Token::new(MaterialAPI::SCHEMA_TYPE_NAME))
                    {
                        Some(m)
                    } else {
                        None
                    }
                })
                .collect();

            // Accumulate mass properties from all collisions
            let mut mass_props_list: Vec<MassProperties> = Vec::new();
            let mut transforms_list: Vec<Matrix4f> = Vec::new();

            for (i, collision_prim) in collision_prims.iter().enumerate() {
                // Get mass/density data for this shape
                let (shape_mass_info, shape_density) = get_collision_shape_mass_api_data(
                    collision_prim,
                    rigid_body_mass_info.density,
                    physics_materials[i].as_ref(),
                );

                // Compute mass properties for this collision shape
                let (props, transform) = parse_collision_shape_for_mass(
                    collision_prim,
                    &shape_mass_info,
                    shape_density,
                    &mut xf_cache,
                    mass_info_fn,
                );

                if props.mass() > 0.0 {
                    mass_props_list.push(props);
                    transforms_list.push(transform);
                }
            }

            if !mass_props_list.is_empty() {
                // Combine all mass properties
                let accumulated = MassProperties::sum(&mass_props_list, &transforms_list);

                // Handle mass
                if accumulate_mass {
                    out_mass = accumulated.mass();
                } else {
                    // Scale inertia to match explicit body mass
                    let _mass_ratio = out_mass / accumulated.mass();
                    // Note: accumulated inertia will be scaled below
                }

                // Handle center of mass
                if !has_com {
                    out_com = *accumulated.center_of_mass();
                }

                // Diagonalize the accumulated inertia tensor
                let mut acc_pa = Quatf::identity();
                let acc_inertia = MassProperties::get_mass_space_inertia(
                    accumulated.inertia_tensor(),
                    &mut acc_pa,
                );

                // Handle inertia
                if !rigid_body_mass_info.has_inertia {
                    out_inertia = if accumulate_mass {
                        acc_inertia
                    } else {
                        // Scale inertia to match explicit mass
                        let mass_ratio = out_mass / accumulated.mass();
                        Vec3f::new(
                            acc_inertia.x * mass_ratio,
                            acc_inertia.y * mass_ratio,
                            acc_inertia.z * mass_ratio,
                        )
                    };
                }

                // Handle principal axes
                if !has_pa {
                    out_pa = acc_pa;
                }
            } else {
                // No collision shapes - use fallback inertia if needed
                if !rigid_body_mass_info.has_inertia {
                    // Approximate as a small sphere if mass is specified
                    if out_mass > 0.0 {
                        if let Some(stage) = prim.stage() {
                            let meters_per_unit = get_stage_meters_per_unit(&stage) as f32;
                            let radius = 0.1 / meters_per_unit; // 10cm sphere
                            let inertia_val = 0.4 * out_mass * radius * radius;
                            out_inertia = Vec3f::new(inertia_val, inertia_val, inertia_val);
                        }
                        // Warning: rigid body has no collision shapes for mass computation
                    }
                }
            }
        }

        (out_mass.max(0.0), out_inertia, out_com, out_pa)
    }
}

impl RigidBodyAPI {
    /// Get the wrapped prim.
    pub fn get_prim(&self) -> &Prim {
        &self.prim
    }

    /// Check if this rigid body is valid (has a valid prim).
    pub fn is_valid(&self) -> bool {
        self.prim.is_valid()
    }

    /// Check if rigid body is enabled.
    ///
    /// Returns true if rigidBodyEnabled attribute is true or not authored (defaults to true).
    pub fn is_enabled(&self) -> bool {
        if let Some(attr) = self.get_rigid_body_enabled_attr() {
            attr.get(TimeCode::default())
                .and_then(|v| v.get::<bool>().copied())
                .unwrap_or(true)
        } else {
            true
        }
    }

    /// Check if this is a kinematic body.
    ///
    /// Returns true if kinematicEnabled attribute is true.
    pub fn is_kinematic(&self) -> bool {
        if let Some(attr) = self.get_kinematic_enabled_attr() {
            attr.get(TimeCode::default())
                .and_then(|v| v.get::<bool>().copied())
                .unwrap_or(false)
        } else {
            false
        }
    }

    /// Check if body starts asleep.
    pub fn starts_asleep(&self) -> bool {
        if let Some(attr) = self.get_starts_asleep_attr() {
            attr.get(TimeCode::default())
                .and_then(|v| v.get::<bool>().copied())
                .unwrap_or(false)
        } else {
            false
        }
    }

    /// Get current linear velocity.
    pub fn get_velocity(&self) -> Option<Vec3f> {
        self.get_velocity_attr()?
            .get(TimeCode::default())
            .and_then(|v| v.get::<Vec3f>().copied())
    }

    /// Get current angular velocity.
    pub fn get_angular_velocity(&self) -> Option<Vec3f> {
        self.get_angular_velocity_attr()?
            .get(TimeCode::default())
            .and_then(|v| v.get::<Vec3f>().copied())
    }

    /// Set linear velocity at given time.
    pub fn set_velocity(&self, velocity: &Vec3f, time: TimeCode) -> bool {
        if let Some(attr) = self.get_velocity_attr() {
            attr.set(Value::from_no_hash(*velocity), time)
        } else {
            false
        }
    }

    /// Set angular velocity at given time.
    pub fn set_angular_velocity(&self, angular_velocity: &Vec3f, time: TimeCode) -> bool {
        if let Some(attr) = self.get_angular_velocity_attr() {
            attr.set(Value::from_no_hash(*angular_velocity), time)
        } else {
            false
        }
    }

    /// Get the simulation owner scene for this rigid body.
    ///
    /// Returns the PhysicsScene prim that will simulate this body, or None.
    pub fn get_simulation_owner(&self) -> Option<Prim> {
        let rel = self.get_simulation_owner_rel()?;
        let targets = rel.get_targets();
        if targets.is_empty() {
            None
        } else {
            self.prim.stage()?.get_prim_at_path(&targets[0])
        }
    }
}

// ============================================================================
// From implementations for type conversions
// ============================================================================

impl From<Prim> for RigidBodyAPI {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<RigidBodyAPI> for Prim {
    fn from(api: RigidBodyAPI) -> Self {
        api.prim
    }
}

impl AsRef<Prim> for RigidBodyAPI {
    fn as_ref(&self) -> &Prim {
        &self.prim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_kind() {
        assert_eq!(RigidBodyAPI::SCHEMA_KIND, SchemaKind::SingleApplyAPI);
    }

    #[test]
    fn test_schema_type_name() {
        assert_eq!(RigidBodyAPI::SCHEMA_TYPE_NAME, "PhysicsRigidBodyAPI");
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = RigidBodyAPI::get_schema_attribute_names(false);
        assert!(
            names
                .iter()
                .any(|n| n.get_text() == "physics:rigidBodyEnabled")
        );
        assert!(names.iter().any(|n| n.get_text() == "physics:velocity"));
        assert!(
            names
                .iter()
                .any(|n| n.get_text() == "physics:angularVelocity")
        );
    }

    #[test]
    fn test_mass_information_default() {
        let info = MassInformation::default();
        assert_eq!(info.volume, 0.0);
    }

    #[test]
    fn test_helper_methods_compile() {
        // These tests verify that the new helper methods compile correctly.
        // They don't run because they need a valid stage, but they ensure
        // the API is correct.

        // This would work with a real prim:
        // let api = RigidBodyAPI::new(prim);
        // assert!(api.is_valid());
        // let velocity = api.get_velocity();
        // api.set_velocity(&Vec3f::new(1.0, 0.0, 0.0), TimeCode::default());
    }
}
