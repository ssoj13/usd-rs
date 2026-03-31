
//! HDSI tokens - Scene index utilities tokens.

use once_cell::sync::Lazy;
use usd_tf::Token as TfToken;

/// Tokens for configuring [`HdsiImplicitSurfaceSceneIndex`](crate::HdsiImplicitSurfaceSceneIndex).
///
/// Scene index that converts implicit surface prims (spheres, cubes, cones, cylinders, capsules)
/// into explicit mesh representations for rendering.
///
/// # Example
/// ```ignore
/// use usd_hdsi::IMPLICIT_SURFACE_SCENE_INDEX_TOKENS;
///
/// let tokens = &*IMPLICIT_SURFACE_SCENE_INDEX_TOKENS;
/// // Configure conversion behavior
/// scene_index.set_arg(tokens.to_mesh, value);
/// ```
pub static IMPLICIT_SURFACE_SCENE_INDEX_TOKENS: Lazy<ImplicitSurfaceSceneIndexTokens> =
    Lazy::new(ImplicitSurfaceSceneIndexTokens::new);

/// Token collection for implicit surface to mesh conversion.
///
/// Used by scene indices that tessellate implicit primitives into polygonal meshes.
#[derive(Debug, Clone)]
pub struct ImplicitSurfaceSceneIndexTokens {
    /// Convert implicit surface to mesh representation.
    ///
    /// Controls whether implicit primitives (sphere, cube, cone, etc.) should be
    /// converted to explicit mesh geometry.
    pub to_mesh: TfToken,

    /// Transform the implicit surface axis system.
    ///
    /// Controls axis alignment transformation when converting implicit surfaces.
    /// Different DCCs may use different axis conventions (Y-up vs Z-up).
    pub axis_to_transform: TfToken,
}

impl ImplicitSurfaceSceneIndexTokens {
    fn new() -> Self {
        Self {
            to_mesh: TfToken::new("toMesh"),
            axis_to_transform: TfToken::new("axisToTransform"),
        }
    }
}

/// Tokens for configuring [`HdsiPrimTypePruningSceneIndex`](crate::HdsiPrimTypePruningSceneIndex).
///
/// Scene index that prunes (removes) primitives of specified types from the scene graph.
/// Useful for filtering out unwanted geometry types during rendering or viewport display.
///
/// # Example
/// ```ignore
/// use usd_hdsi::PRIM_TYPE_PRUNING_SCENE_INDEX_TOKENS;
///
/// let tokens = &*PRIM_TYPE_PRUNING_SCENE_INDEX_TOKENS;
/// // Specify types to prune (e.g., cameras, lights)
/// scene_index.set_arg(tokens.prim_types, types_to_remove);
/// ```
pub static PRIM_TYPE_PRUNING_SCENE_INDEX_TOKENS: Lazy<PrimTypePruningSceneIndexTokens> =
    Lazy::new(PrimTypePruningSceneIndexTokens::new);

/// Token collection for prim type-based pruning operations.
///
/// Controls which primitive types should be removed from scene graph traversal.
#[derive(Debug, Clone)]
pub struct PrimTypePruningSceneIndexTokens {
    /// List of primitive types to prune from the scene.
    ///
    /// Specifies which prim types (e.g., "camera", "light", "material") should be
    /// removed from the scene index. Accepts an array of type names.
    pub prim_types: TfToken,

    /// Token used for material binding relationships.
    ///
    /// When pruning materials, this specifies the binding relationship name to consider
    /// (typically "material:binding").
    pub binding_token: TfToken,

    /// Preserve non-prim paths during pruning.
    ///
    /// When true, paths that don't correspond to actual prims (e.g., relationship targets,
    /// attribute connections) won't be pruned even if their type matches.
    pub do_not_prune_non_prim_paths: TfToken,
}

impl PrimTypePruningSceneIndexTokens {
    fn new() -> Self {
        Self {
            prim_types: TfToken::new("primTypes"),
            binding_token: TfToken::new("bindingToken"),
            do_not_prune_non_prim_paths: TfToken::new("doNotPruneNonPrimPaths"),
        }
    }
}

/// Tokens for configuring [`HdsiPrimTypeAndPathPruningSceneIndex`](crate::HdsiPrimTypeAndPathPruningSceneIndex).
///
/// Scene index that prunes primitives based on both type and path predicates.
/// Combines type-based filtering with custom path matching for fine-grained control.
///
/// # Example
/// ```ignore
/// use usd_hdsi::PRIM_TYPE_AND_PATH_PRUNING_SCENE_INDEX_TOKENS;
///
/// let tokens = &*PRIM_TYPE_AND_PATH_PRUNING_SCENE_INDEX_TOKENS;
/// // Prune cameras under specific path prefixes
/// scene_index.set_arg(tokens.path_predicate, predicate_fn);
/// ```
pub static PRIM_TYPE_AND_PATH_PRUNING_SCENE_INDEX_TOKENS: Lazy<
    PrimTypeAndPathPruningSceneIndexTokens,
> = Lazy::new(PrimTypeAndPathPruningSceneIndexTokens::new);

/// Token collection for combined type and path-based pruning.
///
/// Extends type pruning with custom path filtering logic.
/// P1-4 fix: only primTypes is retained; bindingToken/pathPredicate removed.
#[derive(Debug, Clone)]
pub struct PrimTypeAndPathPruningSceneIndexTokens {
    /// List of primitive types to prune from the scene.
    ///
    /// Specifies which prim types should be candidates for removal.
    pub prim_types: TfToken,
}

impl PrimTypeAndPathPruningSceneIndexTokens {
    fn new() -> Self {
        Self {
            prim_types: TfToken::new("primTypes"),
        }
    }
}

/// Tokens for configuring [`HdsiPrimTypeNoticeBatchingSceneIndex`](crate::HdsiPrimTypeNoticeBatchingSceneIndex).
///
/// Scene index that batches change notifications for specified prim types.
/// Improves performance by coalescing multiple individual change notices into
/// a single batch update, reducing overhead in the scene graph update pipeline.
///
/// # Example
/// ```ignore
/// use usd_hdsi::PRIM_TYPE_NOTICE_BATCHING_SCENE_INDEX_TOKENS;
///
/// let tokens = &*PRIM_TYPE_NOTICE_BATCHING_SCENE_INDEX_TOKENS;
/// // Batch updates for instance prims for better performance
/// scene_index.set_arg(tokens.prim_types, vec!["instance"]);
/// ```
pub static PRIM_TYPE_NOTICE_BATCHING_SCENE_INDEX_TOKENS: Lazy<
    PrimTypeNoticeBatchingSceneIndexTokens,
> = Lazy::new(PrimTypeNoticeBatchingSceneIndexTokens::new);

/// Token collection for batching change notifications.
///
/// Controls which prim types should have their change notices batched for efficiency.
/// P1-3 fix: token is primTypePriorityFunctor, not primTypes.
#[derive(Debug, Clone)]
pub struct PrimTypeNoticeBatchingSceneIndexTokens {
    /// Priority functor for ordering prim type batching.
    ///
    /// A callable that determines the processing priority of prim types,
    /// used to order batch notifications correctly.
    pub prim_type_priority_functor: TfToken,
}

impl PrimTypeNoticeBatchingSceneIndexTokens {
    fn new() -> Self {
        Self {
            prim_type_priority_functor: TfToken::new("primTypePriorityFunctor"),
        }
    }
}

/// Tokens for configuring [`HdsiDomeLightCameraVisibilitySceneIndex`](crate::HdsiDomeLightCameraVisibilitySceneIndex).
///
/// Scene index that manages dome light visibility relative to a specific camera.
/// Controls whether environment/dome lights are visible in camera views or only
/// contribute to lighting calculations.
///
/// # Example
/// ```ignore
/// use usd_hdsi::DOME_LIGHT_CAMERA_VISIBILITY_SCENE_INDEX_TOKENS;
///
/// let tokens = &*DOME_LIGHT_CAMERA_VISIBILITY_SCENE_INDEX_TOKENS;
/// // Set which camera determines dome light visibility
/// scene_index.set_arg(tokens.camera_path, camera_prim_path);
/// ```
pub static DOME_LIGHT_CAMERA_VISIBILITY_SCENE_INDEX_TOKENS: Lazy<
    DomeLightCameraVisibilitySceneIndexTokens,
> = Lazy::new(DomeLightCameraVisibilitySceneIndexTokens::new);

/// Token collection for dome light camera visibility control.
///
/// Determines how dome lights interact with specific camera views.
/// P1-2 fix: token is cameraVisibility, not cameraPath.
#[derive(Debug, Clone)]
pub struct DomeLightCameraVisibilitySceneIndexTokens {
    /// Whether dome light is visible to the camera.
    ///
    /// Boolean token controlling dome light camera visibility.
    pub camera_visibility: TfToken,
}

impl DomeLightCameraVisibilitySceneIndexTokens {
    fn new() -> Self {
        Self {
            camera_visibility: TfToken::new("cameraVisibility"),
        }
    }
}

/// Tokens for configuring [`HdsiLightLinkingSceneIndex`](crate::HdsiLightLinkingSceneIndex).
///
/// Scene index that processes light linking relationships, controlling which lights
/// affect which geometry. Supports both illumination linking and shadow linking.
///
/// Light linking allows artists to create specific light-geometry relationships,
/// overriding default "lights affect all geometry" behavior.
///
/// # Example
/// ```ignore
/// use usd_hdsi::LIGHT_LINKING_SCENE_INDEX_TOKENS;
///
/// let tokens = &*LIGHT_LINKING_SCENE_INDEX_TOKENS;
/// // Configure light-geometry relationships
/// scene_index.set_arg(tokens.light_link, link_collection);
/// scene_index.set_arg(tokens.shadow_link, shadow_collection);
/// ```
pub static LIGHT_LINKING_SCENE_INDEX_TOKENS: Lazy<LightLinkingSceneIndexTokens> =
    Lazy::new(LightLinkingSceneIndexTokens::new);

/// Token collection for light and shadow linking relationships.
///
/// Controls explicit light-to-geometry associations for rendering.
/// P1-1 fix: tokens match C++ HDSI_LIGHT_LINKING_SCENE_INDEX_TOKENS.
#[derive(Debug, Clone)]
pub struct LightLinkingSceneIndexTokens {
    /// Prim types that are lights (used for light linking collections).
    pub light_prim_types: TfToken,

    /// Prim types that are light filters.
    pub light_filter_prim_types: TfToken,

    /// Prim types that are geometry (geometry linking collections).
    pub geometry_prim_types: TfToken,
}

impl LightLinkingSceneIndexTokens {
    fn new() -> Self {
        Self {
            light_prim_types: TfToken::new("lightPrimTypes"),
            light_filter_prim_types: TfToken::new("lightFilterPrimTypes"),
            geometry_prim_types: TfToken::new("geometryPrimTypes"),
        }
    }
}

/// Tokens for configuring [`HdsiPrefixPathPruningSceneIndex`](crate::HdsiPrefixPathPruningSceneIndex).
///
/// Scene index that prunes entire scene graph branches at or below the listed paths.
/// Useful for hiding or excluding specific scene hierarchy sections.
///
/// # Example
/// ```ignore
/// use usd_hdsi::PREFIX_PATH_PRUNING_SCENE_INDEX_TOKENS;
///
/// let tokens = &*PREFIX_PATH_PRUNING_SCENE_INDEX_TOKENS;
/// // Prune everything at or under "/World/Hidden"
/// scene_index.set_exclude_path_prefixes(vec!["/World/Hidden".into()]);
/// ```
pub static PREFIX_PATH_PRUNING_SCENE_INDEX_TOKENS: Lazy<PrefixPathPruningSceneIndexTokens> =
    Lazy::new(PrefixPathPruningSceneIndexTokens::new);

/// Token collection for path prefix-based pruning.
///
/// Controls which scene graph branches should be removed based on path matching.
/// Matches C++ HdsiPrefixPathPruningSceneIndexTokens.
#[derive(Debug, Clone)]
pub struct PrefixPathPruningSceneIndexTokens {
    /// List of path prefixes to exclude from the scene.
    ///
    /// Prims at or below any of these paths are pruned (removed).
    /// Provided via input args or SetExcludePathPrefixes.
    pub exclude_path_prefixes: TfToken,
}

impl PrefixPathPruningSceneIndexTokens {
    fn new() -> Self {
        Self {
            exclude_path_prefixes: TfToken::new("excludePathPrefixes"),
        }
    }
}

/// Tokens for configuring [`HdsiPrimManagingSceneIndexObserver`](crate::HdsiPrimManagingSceneIndexObserver).
///
/// Observer that manages prim lifecycle and change notifications from a scene index.
/// Automatically creates, updates, and destroys managed prim objects in response to
/// scene graph changes.
///
/// This is a core component for maintaining derived representations of scene data,
/// such as render delegates or accelerator structures.
///
/// # Example
/// ```ignore
/// use usd_hdsi::PRIM_MANAGING_SCENE_INDEX_OBSERVER_TOKENS;
///
/// let tokens = &*PRIM_MANAGING_SCENE_INDEX_OBSERVER_TOKENS;
/// // Attach observer to a scene index
/// observer.set_arg(tokens.scene_index, target_scene_index);
/// ```
pub static PRIM_MANAGING_SCENE_INDEX_OBSERVER_TOKENS: Lazy<PrimManagingSceneIndexObserverTokens> =
    Lazy::new(PrimManagingSceneIndexObserverTokens::new);

/// Token collection for scene index observation and prim management.
///
/// Controls the prim factory used to create managed prims.
/// P1-5 fix: token is primFactory, not sceneIndex.
#[derive(Debug, Clone)]
pub struct PrimManagingSceneIndexObserverTokens {
    /// Prim factory for creating managed prim instances.
    ///
    /// A PrimFactoryBaseHandle typed data source passed via input_args.
    /// The factory is called for each prim in the observed scene index
    /// to create an associated managed prim object.
    pub prim_factory: TfToken,
}

impl PrimManagingSceneIndexObserverTokens {
    fn new() -> Self {
        Self {
            prim_factory: TfToken::new("primFactory"),
        }
    }
}

/// Tokens for configuring [`HdsiRenderSettingsFilteringSceneIndex`](crate::HdsiRenderSettingsFilteringSceneIndex).
///
/// Scene index that filters and applies render settings from a specific settings prim.
/// Render settings control resolution, sampling, output format, and other global
/// rendering parameters.
///
/// # Example
/// ```ignore
/// use usd_hdsi::RENDER_SETTINGS_FILTERING_SCENE_INDEX_TOKENS;
///
/// let tokens = &*RENDER_SETTINGS_FILTERING_SCENE_INDEX_TOKENS;
/// // Use render settings from specific prim
/// scene_index.set_arg(tokens.render_settings_prim_path, "/Render/Settings");
/// ```
pub static RENDER_SETTINGS_FILTERING_SCENE_INDEX_TOKENS: Lazy<
    RenderSettingsFilteringSceneIndexTokens,
> = Lazy::new(RenderSettingsFilteringSceneIndexTokens::new);

/// Token collection for render settings filtering.
///
/// Identifies which render settings prim should be used and configures filtering.
/// P1-8 fix: renderSettingsPrimPath removed per C++ header.
#[derive(Debug, Clone)]
pub struct RenderSettingsFilteringSceneIndexTokens {
    /// Token array of namespace prefixes for filtering namespacedSettings.
    ///
    /// Only settings whose names start with one of these prefixes are included.
    /// Empty array means no filtering.
    pub namespace_prefixes: TfToken,

    /// Fallback prim data source for /Render/__HdsiRenderSettingsFilteringSceneIndex__FallbackSettings.
    pub fallback_prim_ds: TfToken,
}

impl RenderSettingsFilteringSceneIndexTokens {
    fn new() -> Self {
        Self {
            namespace_prefixes: TfToken::new("namespacePrefixes"),
            fallback_prim_ds: TfToken::new("fallbackPrimDs"),
        }
    }
}

/// Tokens for configuring [`HdsiUnboundMaterialPruningSceneIndex`](crate::HdsiUnboundMaterialPruningSceneIndex).
///
/// Scene index that removes material prims that aren't bound to any geometry.
/// Optimizes scene graph by eliminating unused materials, reducing memory overhead
/// and processing time.
///
/// # Example
/// ```ignore
/// use usd_hdsi::UNBOUND_MATERIAL_PRUNING_SCENE_INDEX_TOKENS;
///
/// let tokens = &*UNBOUND_MATERIAL_PRUNING_SCENE_INDEX_TOKENS;
/// // Enable pruning of unused materials
/// scene_index.set_arg(tokens.prune_unbound_materials, true);
/// ```
pub static UNBOUND_MATERIAL_PRUNING_SCENE_INDEX_TOKENS: Lazy<
    UnboundMaterialPruningSceneIndexTokens,
> = Lazy::new(UnboundMaterialPruningSceneIndexTokens::new);

/// Token collection for unbound material pruning.
///
/// Controls automatic cleanup of unused material definitions.
/// P1-6 fix: pruneUnboundMaterials token removed; only materialBindingPurposes retained.
#[derive(Debug, Clone)]
pub struct UnboundMaterialPruningSceneIndexTokens {
    /// Material binding purposes to consider (e.g. "allPurpose").
    /// HdTokenArrayDataSource in input_args.
    pub material_binding_purposes: TfToken,
}

impl UnboundMaterialPruningSceneIndexTokens {
    fn new() -> Self {
        Self {
            material_binding_purposes: TfToken::new("materialBindingPurposes"),
        }
    }
}

/// Tokens for configuring [`HdsiVelocityMotionResolvingSceneIndex`](crate::HdsiVelocityMotionResolvingSceneIndex).
///
/// Scene index that computes motion blur from velocity and acceleration attributes.
/// Converts per-point velocity data into position time samples for motion blur rendering,
/// avoiding the need to store explicit position samples.
///
/// This is an optimization for dynamic simulations where velocities are readily available
/// but position samples would be expensive to store.
///
/// # Example
/// ```ignore
/// use usd_hdsi::VELOCITY_MOTION_RESOLVING_SCENE_INDEX_TOKENS;
///
/// let tokens = &*VELOCITY_MOTION_RESOLVING_SCENE_INDEX_TOKENS;
/// // Configure motion blur time window
/// scene_index.set_arg(tokens.time, current_frame_time);
/// scene_index.set_arg(tokens.time_samples_span, shutter_interval);
/// ```
pub static VELOCITY_MOTION_RESOLVING_SCENE_INDEX_TOKENS: Lazy<
    VelocityMotionResolvingSceneIndexTokens,
> = Lazy::new(VelocityMotionResolvingSceneIndexTokens::new);

/// Token collection for velocity-based motion blur.
///
/// Controls time codes per second and velocity-to-position conversion for motion blur.
/// P1-7 fix: removed time/timeSamplesSpan, added timeCodesPerSecond.
#[derive(Debug, Clone)]
pub struct VelocityMotionResolvingSceneIndexTokens {
    /// Time codes per second for velocity integration.
    pub time_codes_per_second: TfToken,
    /// Velocity motion mode: disable - freeze to authored; enable - resolve; ignore - defer to source; noAcceleration - ignore accelerations.
    pub velocity_motion_mode: TfToken,
    /// Mode value: disable.
    pub disable: TfToken,
    /// Mode value: enable.
    pub enable: TfToken,
    /// Mode value: ignore (defer to source).
    pub ignore: TfToken,
    /// Mode value: noAcceleration (ignore accelerations).
    pub no_acceleration: TfToken,
}

impl VelocityMotionResolvingSceneIndexTokens {
    fn new() -> Self {
        Self {
            time_codes_per_second: TfToken::new("timeCodesPerSecond"),
            velocity_motion_mode: TfToken::new("__velocityMotionMode"),
            disable: TfToken::new("disable"),
            enable: TfToken::new("enable"),
            ignore: TfToken::new("ignore"),
            no_acceleration: TfToken::new("noAcceleration"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_implicit_surface_tokens() {
        let tokens = &*IMPLICIT_SURFACE_SCENE_INDEX_TOKENS;
        assert_eq!(tokens.to_mesh.as_str(), "toMesh");
        assert_eq!(tokens.axis_to_transform.as_str(), "axisToTransform");
    }

    #[test]
    fn test_prim_type_pruning_tokens() {
        let tokens = &*PRIM_TYPE_PRUNING_SCENE_INDEX_TOKENS;
        assert_eq!(tokens.prim_types.as_str(), "primTypes");
        assert_eq!(tokens.binding_token.as_str(), "bindingToken");
        assert_eq!(
            tokens.do_not_prune_non_prim_paths.as_str(),
            "doNotPruneNonPrimPaths"
        );
    }

    // P1-1: LightLinking tokens
    #[test]
    fn test_light_linking_tokens() {
        let tokens = &*LIGHT_LINKING_SCENE_INDEX_TOKENS;
        assert_eq!(tokens.light_prim_types.as_str(), "lightPrimTypes");
        assert_eq!(
            tokens.light_filter_prim_types.as_str(),
            "lightFilterPrimTypes"
        );
        assert_eq!(tokens.geometry_prim_types.as_str(), "geometryPrimTypes");
    }

    // P1-2: DomeLightCameraVisibility token
    #[test]
    fn test_dome_light_camera_visibility_tokens() {
        let tokens = &*DOME_LIGHT_CAMERA_VISIBILITY_SCENE_INDEX_TOKENS;
        assert_eq!(tokens.camera_visibility.as_str(), "cameraVisibility");
    }

    // P1-3: PrimTypeNoticeBatching token
    #[test]
    fn test_prim_type_notice_batching_tokens() {
        let tokens = &*PRIM_TYPE_NOTICE_BATCHING_SCENE_INDEX_TOKENS;
        assert_eq!(
            tokens.prim_type_priority_functor.as_str(),
            "primTypePriorityFunctor"
        );
    }

    // P1-4: PrimTypeAndPathPruning — only primTypes
    #[test]
    fn test_prim_type_and_path_pruning_tokens() {
        let tokens = &*PRIM_TYPE_AND_PATH_PRUNING_SCENE_INDEX_TOKENS;
        assert_eq!(tokens.prim_types.as_str(), "primTypes");
    }

    // P1-5: PrimManagingSceneIndexObserver — primFactory
    #[test]
    fn test_prim_managing_observer_tokens() {
        let tokens = &*PRIM_MANAGING_SCENE_INDEX_OBSERVER_TOKENS;
        assert_eq!(tokens.prim_factory.as_str(), "primFactory");
    }

    // P1-6: UnboundMaterialPruning — no pruneUnboundMaterials
    #[test]
    fn test_unbound_material_pruning_tokens() {
        let tokens = &*UNBOUND_MATERIAL_PRUNING_SCENE_INDEX_TOKENS;
        assert_eq!(
            tokens.material_binding_purposes.as_str(),
            "materialBindingPurposes"
        );
    }

    // P1-7: VelocityMotionResolving — timeCodesPerSecond
    #[test]
    fn test_velocity_motion_resolving_tokens() {
        let tokens = &*VELOCITY_MOTION_RESOLVING_SCENE_INDEX_TOKENS;
        assert_eq!(tokens.time_codes_per_second.as_str(), "timeCodesPerSecond");
        assert_eq!(tokens.velocity_motion_mode.as_str(), "__velocityMotionMode");
        assert_eq!(tokens.disable.as_str(), "disable");
        assert_eq!(tokens.enable.as_str(), "enable");
    }

    // P1-8: RenderSettingsFiltering — no renderSettingsPrimPath
    #[test]
    fn test_render_settings_filtering_tokens() {
        let tokens = &*RENDER_SETTINGS_FILTERING_SCENE_INDEX_TOKENS;
        assert_eq!(tokens.namespace_prefixes.as_str(), "namespacePrefixes");
        assert_eq!(tokens.fallback_prim_ds.as_str(), "fallbackPrimDs");
    }
}
