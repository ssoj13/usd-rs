//! Adapter registry for mapping USD types to adapters.

use super::prim_adapter::{NoOpAdapter, PrimAdapterHandle};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use usd_core::{Prim, SchemaRegistry};
use usd_tf::Token;

/// Registry mapping USD prim types to their corresponding adapters.
///
/// The registry maintains a mapping from USD schema type tokens
/// (e.g., "Mesh", "Camera") to adapter instances that know how to
/// convert those prims to Hydra data.
///
/// # Thread Safety
///
/// The registry is thread-safe and can be shared across threads.
/// Adapters are registered once during initialization and then
/// accessed read-only during scene traversal.
#[derive(Clone)]
pub struct AdapterRegistry {
    /// Map from USD type token to adapter
    adapters: Arc<RwLock<HashMap<Token, PrimAdapterHandle>>>,
}

impl AdapterRegistry {
    fn schema_depth(type_name: &Token) -> usize {
        SchemaRegistry::find_schema_info(type_name)
            .map(|info| info.base_type_names.len())
            .unwrap_or(0)
    }

    /// Create new empty adapter registry.
    pub fn new() -> Self {
        Self {
            adapters: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create registry with all standard USD imaging adapters pre-registered.
    ///
    /// Registers adapters for all known USD schema types: geometry, cameras,
    /// lights, materials, instancers, implicit surfaces, volumes, skeletons,
    /// and render settings.
    pub fn new_with_defaults() -> Self {
        let reg = Self::new();

        // ---- Geometry ----
        reg.register(
            Token::new("Mesh"),
            Arc::new(super::mesh_adapter::MeshAdapter::new()),
        );
        reg.register(
            Token::new("BasisCurves"),
            Arc::new(super::curves_adapter::BasisCurvesAdapter::new()),
        );
        reg.register(
            Token::new("HermiteCurves"),
            Arc::new(super::curves_adapter::HermiteCurvesAdapter::new()),
        );
        reg.register(
            Token::new("NurbsCurves"),
            Arc::new(super::curves_adapter::NurbsCurvesAdapter::new()),
        );
        reg.register(
            Token::new("NurbsPatch"),
            Arc::new(super::curves_adapter::NurbsPatchAdapter::new()),
        );
        reg.register(
            Token::new("Points"),
            Arc::new(super::points_adapter::PointsAdapter::new()),
        );
        reg.register(
            Token::new("GeomSubset"),
            Arc::new(super::geom_subset_adapter::GeomSubsetAdapter::new()),
        );
        reg.register(
            Token::new("TetMesh"),
            Arc::new(super::tet_mesh_adapter::TetMeshAdapter::new()),
        );

        // ---- Implicit surfaces ----
        reg.register(
            Token::new("Cube"),
            Arc::new(super::implicit_surface_adapter::CubeAdapter::new()),
        );
        reg.register(
            Token::new("Sphere"),
            Arc::new(super::implicit_surface_adapter::SphereAdapter::new()),
        );
        reg.register(
            Token::new("Cylinder"),
            Arc::new(super::implicit_surface_adapter::CylinderAdapter::new()),
        );
        reg.register(
            Token::new("Cone"),
            Arc::new(super::implicit_surface_adapter::ConeAdapter::new()),
        );
        reg.register(
            Token::new("Capsule"),
            Arc::new(super::implicit_surface_adapter::CapsuleAdapter::new()),
        );
        reg.register(
            Token::new("Plane"),
            Arc::new(super::implicit_surface_adapter::PlaneAdapter::new()),
        );

        // ---- Camera ----
        reg.register(
            Token::new("Camera"),
            Arc::new(super::camera_adapter::CameraAdapter::new()),
        );

        // ---- Lights ----
        reg.register(
            Token::new("DomeLight"),
            Arc::new(super::light_adapter::DomeLightAdapter::new()),
        );
        reg.register(
            Token::new("DomeLight_1"),
            Arc::new(super::light_adapter::DomeLight1Adapter::new()),
        );
        reg.register(
            Token::new("SphereLight"),
            Arc::new(super::light_adapter::SphereLightAdapter::new()),
        );
        reg.register(
            Token::new("RectLight"),
            Arc::new(super::light_adapter::RectLightAdapter::new()),
        );
        reg.register(
            Token::new("DiskLight"),
            Arc::new(super::light_adapter::DiskLightAdapter::new()),
        );
        reg.register(
            Token::new("CylinderLight"),
            Arc::new(super::light_adapter::CylinderLightAdapter::new()),
        );
        reg.register(
            Token::new("DistantLight"),
            Arc::new(super::light_adapter::DistantLightAdapter::new()),
        );
        reg.register(
            Token::new("GeometryLight"),
            Arc::new(super::light_adapter::GeometryLightAdapter::new()),
        );
        reg.register(
            Token::new("PluginLight"),
            Arc::new(super::light_adapter::PluginLightAdapter::new()),
        );
        reg.register(
            Token::new("LightFilter"),
            Arc::new(super::light_adapter::LightFilterAdapter::new()),
        );
        reg.register(
            Token::new("PluginLightFilter"),
            Arc::new(super::light_adapter::PluginLightFilterAdapter::new()),
        );

        // ---- Materials ----
        reg.register(
            Token::new("Material"),
            Arc::new(super::material_adapter::MaterialAdapter::new()),
        );
        reg.register(
            Token::new("Shader"),
            Arc::new(super::material_adapter::ShaderAdapter::new()),
        );
        reg.register(
            Token::new("NodeGraph"),
            Arc::new(super::material_adapter::NodeGraphAdapter::new()),
        );

        // ---- Instancing ----
        reg.register(
            Token::new("PointInstancer"),
            Arc::new(super::instancer_adapter::PointInstancerAdapter::new()),
        );

        // ---- Volume ----
        reg.register(
            Token::new("Volume"),
            Arc::new(super::volume_adapter::VolumeAdapter::new()),
        );

        // ---- Render settings ----
        reg.register(
            Token::new("RenderSettings"),
            Arc::new(super::render_settings_adapter::RenderSettingsAdapter::new()),
        );
        reg.register(
            Token::new("RenderProduct"),
            Arc::new(super::render_settings_adapter::RenderProductAdapter::new()),
        );
        reg.register(
            Token::new("RenderVar"),
            Arc::new(super::render_settings_adapter::RenderVarAdapter::new()),
        );
        reg.register(
            Token::new("RenderPass"),
            Arc::new(super::render_settings_adapter::RenderPassAdapter::new()),
        );

        // ---- Skeleton ----
        reg.register(
            Token::new("SkelRoot"),
            Arc::new(super::skel::skel_root_adapter::SkelRootAdapter::new()),
        );
        reg.register(
            Token::new("Skeleton"),
            Arc::new(super::skel::skeleton_adapter::SkeletonAdapter::new()),
        );
        reg.register(
            Token::new("SkelAnimation"),
            Arc::new(super::skel::animation_adapter::AnimationAdapter::new()),
        );
        reg.register(
            Token::new("BlendShape"),
            Arc::new(super::skel::blend_shape_adapter::BlendShapeAdapter::new()),
        );

        // ---- Coordinate systems ----
        reg.register(
            Token::new("CoordSys"),
            Arc::new(super::coord_sys_adapter::CoordSysAdapter::new()),
        );

        log::info!(
            "[AdapterRegistry] registered {} default adapters",
            reg.adapter_count()
        );
        reg
    }

    /// Register an adapter for a USD prim type.
    ///
    /// # Arguments
    ///
    /// * `prim_type` - USD schema type token (e.g., "Mesh")
    /// * `adapter` - The adapter instance to handle this type
    ///
    /// # Example
    ///
    /// ```ignore
    /// registry.register(Token::new("Mesh"), Arc::new(MeshAdapter::new()));
    /// ```
    pub fn register(&self, prim_type: Token, adapter: PrimAdapterHandle) {
        let mut adapters = self.adapters.write();
        adapters.insert(prim_type, adapter);
    }

    /// Find adapter for a USD prim type token.
    ///
    /// # Arguments
    ///
    /// * `prim_type` - USD schema type token
    ///
    /// # Returns
    ///
    /// The registered adapter, or None if not found
    pub fn find(&self, prim_type: &Token) -> Option<PrimAdapterHandle> {
        let adapters = self.adapters.read();
        adapters.get(prim_type).cloned()
    }

    /// Find adapter for a USD prim.
    ///
    /// This looks up the adapter based on the prim's type name.
    ///
    /// # Arguments
    ///
    /// * `prim` - The USD prim
    ///
    /// # Returns
    ///
    /// The registered adapter, or a default no-op adapter if not found
    pub fn find_for_prim(&self, prim: &Prim) -> PrimAdapterHandle {
        let type_name = prim.get_type_name();
        let adapters = self.adapters.read();

        if let Some(adapter) = adapters.get(&type_name) {
            return adapter.clone();
        }

        let best = adapters
            .iter()
            .filter(|(registered_type, _)| prim.is_a(registered_type))
            .max_by_key(|(registered_type, _)| Self::schema_depth(registered_type))
            .map(|(_, adapter)| adapter.clone());

        best.unwrap_or_else(|| Arc::new(NoOpAdapter::new(type_name.clone())))
    }

    /// Returns whether an adapter is registered for the given type.
    pub fn has_adapter(&self, prim_type: &Token) -> bool {
        let adapters = self.adapters.read();
        adapters.contains_key(prim_type)
    }

    /// Returns number of registered adapters.
    pub fn adapter_count(&self) -> usize {
        let adapters = self.adapters.read();
        adapters.len()
    }

    /// Clear all registered adapters.
    pub fn clear(&self) {
        let mut adapters = self.adapters.write();
        adapters.clear();
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_core::Stage;

    #[test]
    fn test_registry_new() {
        let registry = AdapterRegistry::new();
        assert_eq!(registry.adapter_count(), 0);
    }

    #[test]
    fn test_register_and_find() {
        let registry = AdapterRegistry::new();
        let mesh_type = Token::new("Mesh");
        let adapter = Arc::new(NoOpAdapter::new(mesh_type.clone()));

        registry.register(mesh_type.clone(), adapter.clone());
        assert_eq!(registry.adapter_count(), 1);
        assert!(registry.has_adapter(&mesh_type));

        let found = registry.find(&mesh_type);
        assert!(found.is_some());
    }

    #[test]
    fn test_find_missing() {
        let registry = AdapterRegistry::new();
        let mesh_type = Token::new("Mesh");

        let found = registry.find(&mesh_type);
        assert!(found.is_none());
        assert!(!registry.has_adapter(&mesh_type));
    }

    #[test]
    fn test_find_for_prim() {
        let registry = AdapterRegistry::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");

        // Define a prim (will have Xform type)
        let prim_path = usd_sdf::Path::from_string("/test").expect("Valid path");
        stage.define_prim("/test", "Xform").expect("define prim");
        let prim = stage.get_prim_at_path(&prim_path).expect("Prim exists");

        // Should return no-op adapter for unregistered type
        let adapter = registry.find_for_prim(&prim);
        let subprims = adapter.get_imaging_subprims(&prim);
        assert_eq!(subprims.len(), 1);
    }

    #[test]
    fn test_clear() {
        let registry = AdapterRegistry::new();
        let mesh_type = Token::new("Mesh");
        let adapter = Arc::new(NoOpAdapter::new(mesh_type.clone()));

        registry.register(mesh_type.clone(), adapter);
        assert_eq!(registry.adapter_count(), 1);

        registry.clear();
        assert_eq!(registry.adapter_count(), 0);
        assert!(!registry.has_adapter(&mesh_type));
    }

    #[test]
    fn test_thread_safety() {
        use std::thread;

        let registry = AdapterRegistry::new();
        let registry_clone = registry.clone();

        let handle = thread::spawn(move || {
            let adapter = Arc::new(NoOpAdapter::new(Token::new("Mesh")));
            registry_clone.register(Token::new("Mesh"), adapter);
        });

        handle.join().expect("Thread panicked");
        assert_eq!(registry.adapter_count(), 1);
    }
}
