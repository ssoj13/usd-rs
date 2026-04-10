//! Light List API schema.
//!
//! API schema to support discovery and publishing of lights in a scene.
//!
//! # Light Discovery
//!
//! `ComputeLightList` performs a traversal to find all lights in the scene.
//! For efficiency, computed light lists can be cached using `StoreLightList`.
//!
//! # Cache Behavior
//!
//! The `lightList:cacheBehavior` attribute controls how cached lists are used:
//! - `ignore` - Disregard the cache (default)
//! - `consumeAndContinue` - Use cache but continue traversing descendants
//! - `consumeAndHalt` - Use cache and stop traversal at this prim
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdLux/lightListAPI.h` and `lightListAPI.cpp`

use std::collections::HashSet;
use std::sync::Arc;

use usd_core::attribute::Variability;
use usd_core::prim_flags::{
    PrimFlagsConjunction, USD_PRIM_IS_ABSTRACT, USD_PRIM_IS_ACTIVE, USD_PRIM_IS_DEFINED,
    USD_PRIM_IS_MODEL,
};
use usd_core::{Attribute, Prim, Relationship, SchemaKind, Stage};
use usd_sdf::{Path, TimeCode};
use usd_tf::Token;
use usd_vt::Value;

use super::tokens::tokens;
use crate::schema_create_attr::create_lux_schema_attr;

/// Compute mode for light list computation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComputeMode {
    /// Consult any caches found on the model hierarchy.
    /// Do not traverse beneath the model hierarchy.
    ConsultModelHierarchyCache,
    /// Ignore any caches found, and do a full prim traversal.
    IgnoreCache,
}

/// API schema for light list discovery and caching.
///
/// LightListAPI provides methods to discover lights in a scene and
/// optionally cache the results for efficient retrieval.
///
/// # Schema Kind
///
/// This is a SingleApplyAPI schema.
#[derive(Clone)]
pub struct LightListAPI {
    prim: Prim,
}

impl LightListAPI {
    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "LightListAPI";

    /// The schema kind.
    pub const SCHEMA_KIND: SchemaKind = SchemaKind::SingleApplyAPI;

    // =========================================================================
    // Construction
    // =========================================================================

    /// Construct a LightListAPI on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Construct from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return a LightListAPI holding the prim at `path` on `stage`.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        if prim.has_api(&tokens().light_list_api) {
            Some(Self::new(prim))
        } else {
            None
        }
    }

    /// Returns true if this API schema can be applied to the given prim.
    pub fn can_apply(prim: &Prim, _why_not: Option<&mut String>) -> bool {
        prim.is_valid()
    }

    /// Applies this single-apply API schema to the given prim.
    pub fn apply(prim: &Prim) -> Option<Self> {
        if !prim.is_valid() {
            return None;
        }

        if prim.apply_api(&tokens().light_list_api) {
            Some(Self::new(prim.clone()))
        } else {
            None
        }
    }

    /// Returns the schema kind.
    pub fn get_schema_kind(&self) -> SchemaKind {
        Self::SCHEMA_KIND
    }

    /// Returns true if this schema is valid.
    pub fn is_valid(&self) -> bool {
        self.prim.is_valid()
    }

    /// Get the wrapped prim.
    pub fn get_prim(&self) -> &Prim {
        &self.prim
    }

    // =========================================================================
    // LightList:cacheBehavior Attribute
    // =========================================================================

    /// Get the lightList:cacheBehavior attribute.
    ///
    /// Controls how the lightList should be interpreted.
    /// Valid values: consumeAndHalt, consumeAndContinue, ignore
    pub fn get_light_list_cache_behavior_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(tokens().light_list_cache_behavior.as_str())
    }

    /// Create the lightList:cacheBehavior attribute.
    ///
    /// Matches C++ `UsdLuxLightListAPI::CreateLightListCacheBehaviorAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_light_list_cache_behavior_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        create_lux_schema_attr(
            &self.prim,
            tokens().light_list_cache_behavior.as_str(),
            "token",
            Variability::Varying,
            default_value,
            write_sparsely,
        )
    }

    // =========================================================================
    // LightList Relationship
    // =========================================================================

    /// Get the lightList relationship.
    ///
    /// Relationship to lights in the scene.
    pub fn get_light_list_rel(&self) -> Option<Relationship> {
        self.prim.get_relationship(tokens().light_list.as_str())
    }

    /// Create the lightList relationship.
    pub fn create_light_list_rel(&self) -> Option<Relationship> {
        self.prim
            .create_relationship(tokens().light_list.as_str(), false)
    }

    // =========================================================================
    // Light List Computation
    // =========================================================================

    /// Computes and returns the list of lights and light filters in the stage.
    ///
    /// In `IgnoreCache` mode, caching is ignored and a full prim traversal
    /// is performed looking for prims that have LightAPI or are LightFilters.
    ///
    /// In `ConsultModelHierarchyCache` mode, traverses the model hierarchy,
    /// consulting cached light lists based on `lightList:cacheBehavior`.
    pub fn compute_light_list(&self, mode: ComputeMode) -> HashSet<Path> {
        let mut lights = HashSet::new();

        match mode {
            ComputeMode::IgnoreCache => {
                // Full traversal ignoring cache
                self.traverse_for_lights(&self.prim, &mut lights);
            }
            ComputeMode::ConsultModelHierarchyCache => {
                // Traverse model hierarchy consulting caches
                self.traverse_model_hierarchy(&self.prim, &mut lights);
            }
        }

        lights
    }

    /// Store the given paths as the lightlist for this prim.
    ///
    /// Matches C++ `StoreLightList`: absolute paths that do NOT have this
    /// prim's path as a prefix are skipped. Relative paths are always kept.
    /// Sets `lightList:cacheBehavior` to "consumeAndContinue".
    pub fn store_light_list(&self, lights: &HashSet<Path>) {
        let prim_path = self.prim.path();

        // C++ condition to SKIP: p.IsAbsolutePath() && !p.HasPrefix(GetPath())
        let valid_paths: Vec<Path> = lights
            .iter()
            .filter(|p| !(p.is_absolute_path() && !p.has_prefix(prim_path)))
            .cloned()
            .collect();

        // Create or get the lightList relationship
        if let Some(rel) = self.create_light_list_rel() {
            rel.set_targets(&valid_paths);
        }

        // Set cache behavior to consumeAndContinue
        self.create_light_list_cache_behavior_attr(
            Some(Value::from(tokens().consume_and_continue.clone())),
            false,
        );
    }

    /// Mark any stored lightlist as invalid.
    ///
    /// Sets the `lightList:cacheBehavior` attribute to "ignore".
    pub fn invalidate_light_list(&self) {
        self.create_light_list_cache_behavior_attr(Some(Value::from(tokens().ignore.clone())), false);
    }

    // =========================================================================
    // Private helpers
    // =========================================================================

    /// Check if a prim is a light or light filter.
    ///
    /// Matches C++ `_Traverse` check: prim.HasAPI<UsdLuxLightAPI>() or is a LightFilter.
    fn is_light_prim(prim: &Prim) -> bool {
        if prim.has_api(&tokens().light_api) {
            return true;
        }
        prim.is_a(&tokens().boundable_light_base)
            || prim.is_a(&tokens().nonboundable_light_base)
            || prim.is_a(&tokens().light_filter)
    }

    /// Build the traversal predicate for IgnoreCache mode.
    ///
    /// C++: `UsdTraverseInstanceProxies(UsdPrimIsActive && !UsdPrimIsAbstract && UsdPrimIsDefined)`
    fn base_flags() -> usd_core::prim_flags::PrimFlagsPredicate {
        PrimFlagsConjunction::from_term(USD_PRIM_IS_ACTIVE)
            .and(USD_PRIM_IS_ABSTRACT.not())
            .and(USD_PRIM_IS_DEFINED)
            .traverse_instance_proxies(true)
            .into_predicate()
    }

    /// Build the traversal predicate for ConsultModelHierarchyCache mode.
    ///
    /// C++: `UsdTraverseInstanceProxies(flags && UsdPrimIsModel)`
    fn model_flags() -> usd_core::prim_flags::PrimFlagsPredicate {
        PrimFlagsConjunction::from_term(USD_PRIM_IS_ACTIVE)
            .and(USD_PRIM_IS_ABSTRACT.not())
            .and(USD_PRIM_IS_DEFINED)
            .and(USD_PRIM_IS_MODEL)
            .traverse_instance_proxies(true)
            .into_predicate()
    }

    /// Traverse all descendants looking for lights and light filters.
    ///
    /// Handles native instances by resolving through prototypes and recording
    /// instance-unique paths. Uses proper prim predicates matching C++ `_Traverse`
    /// with `ComputeModeIgnoreCache`.
    fn traverse_for_lights(&self, prim: &Prim, lights: &mut HashSet<Path>) {
        if !prim.is_valid() {
            return;
        }

        // Accumulate if prim has LightAPI or is a light/filter type (matches C++ _Traverse).
        if Self::is_light_prim(prim) {
            lights.insert(prim.path().clone());
        }

        // Handle native instances: resolve through prototype and record
        // instance-unique paths (re-rooted under the instance prim).
        if prim.is_instance() {
            let prototype = prim.get_prototype();
            if prototype.is_valid() {
                let instance_path = prim.path();
                let proto_path = prototype.path();
                self.traverse_prototype_for_lights(&prototype, instance_path, proto_path, lights);
            }
            // Do not recurse into direct children of instances -- content lives in prototype.
            return;
        }

        // C++: UsdTraverseInstanceProxies(UsdPrimIsActive && !UsdPrimIsAbstract && UsdPrimIsDefined)
        for child in prim.get_filtered_children(Self::base_flags()) {
            self.traverse_for_lights(&child, lights);
        }
    }

    /// Traverse a prototype prim's descendants, remapping discovered light
    /// paths back into instance-unique space.
    fn traverse_prototype_for_lights(
        &self,
        prim: &Prim,
        instance_path: &Path,
        proto_path: &Path,
        lights: &mut HashSet<Path>,
    ) {
        if !prim.is_valid() {
            return;
        }

        // Build instance-unique path by replacing prototype prefix with instance path.
        let prim_path = prim.path();
        let instance_unique = if prim_path == proto_path {
            instance_path.clone()
        } else if let Some(suffix) = prim_path.as_str().strip_prefix(proto_path.as_str()) {
            Path::from(format!("{}{}", instance_path.as_str(), suffix).as_str())
        } else {
            prim_path.clone()
        };

        // Check for light / light filter at this prim.
        if Self::is_light_prim(prim) {
            lights.insert(instance_unique.clone());
        }

        // Recurse into prototype children with proper flags.
        for child in prim.get_filtered_children(Self::base_flags()) {
            self.traverse_prototype_for_lights(&child, instance_path, proto_path, lights);
        }
    }

    /// Traverse model hierarchy consulting caches.
    ///
    /// Matches C++ `_Traverse` with `ComputeModeConsultModelHierarchyCache` exactly:
    /// 1. Check lightList cache FIRST (and return immediately on consumeAndHalt).
    /// 2. THEN check if prim is a light/filter.
    /// 3. Recurse only into model-hierarchy children with instance proxy support.
    fn traverse_model_hierarchy(&self, prim: &Prim, lights: &mut HashSet<Path>) {
        if !prim.is_valid() {
            return;
        }

        // Step 1 -- Check cache first, matching C++ order.
        // Skip pseudoRoot (not a real prim path).
        if prim.path().is_prim_path() {
            let api = LightListAPI::new(prim.clone());
            if let Some(behavior_attr) = api.get_light_list_cache_behavior_attr() {
                if let Some(behavior) = behavior_attr.get_typed::<Token>(TimeCode::default()) {
                    if behavior == tokens().consume_and_continue
                        || behavior == tokens().consume_and_halt
                    {
                        // Consume cached targets using get_forwarded_targets() (P1-3 fix):
                        // resolves forwarded relationship chains, not just direct targets.
                        if let Some(rel) = api.get_light_list_rel() {
                            for target in rel.get_forwarded_targets() {
                                lights.insert(target);
                            }
                        }

                        // consumeAndHalt: return immediately -- do NOT check prim as light,
                        // do NOT recurse. This is the key order fix vs old Rust code.
                        if behavior == tokens().consume_and_halt {
                            return;
                        }
                    }
                }
            }
        }

        // Step 2 -- Accumulate prim itself if it is a light or light filter.
        if Self::is_light_prim(prim) {
            lights.insert(prim.path().clone());
        }

        // Step 3 -- Recurse into model hierarchy children.
        // C++: UsdTraverseInstanceProxies(flags && UsdPrimIsModel)
        for child in prim.get_filtered_children(Self::model_flags()) {
            self.traverse_model_hierarchy(&child, lights);
        }
    }

    // =========================================================================
    // Schema attribute names
    // =========================================================================

    /// Returns all pre-declared attributes for this schema class.
    pub fn get_schema_attribute_names(_include_inherited: bool) -> Vec<Token> {
        vec![tokens().light_list_cache_behavior.clone()]
    }
}

// ============================================================================
// Trait implementations
// ============================================================================

impl From<Prim> for LightListAPI {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<LightListAPI> for Prim {
    fn from(api: LightListAPI) -> Self {
        api.prim
    }
}

impl AsRef<Prim> for LightListAPI {
    fn as_ref(&self) -> &Prim {
        &self.prim
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::tokens;
    use std::sync::Arc;
    use usd_core::{InitialLoadSet, Stage};

    #[test]
    fn test_schema_type_name() {
        assert_eq!(LightListAPI::SCHEMA_TYPE_NAME, "LightListAPI");
    }

    #[test]
    fn test_schema_kind() {
        assert_eq!(LightListAPI::SCHEMA_KIND, SchemaKind::SingleApplyAPI);
    }

    #[test]
    fn test_compute_mode_values() {
        assert_ne!(
            ComputeMode::IgnoreCache,
            ComputeMode::ConsultModelHierarchyCache
        );
    }

    #[test]
    fn test_traverse_for_lights_invalid_prim() {
        let api = LightListAPI::new(Prim::invalid());
        let result = api.compute_light_list(ComputeMode::IgnoreCache);
        // No lights found on invalid prim
        assert!(result.is_empty());
    }

    #[test]
    fn test_traverse_model_hierarchy_invalid_prim() {
        let api = LightListAPI::new(Prim::invalid());
        let result = api.compute_light_list(ComputeMode::ConsultModelHierarchyCache);
        assert!(result.is_empty());
    }

    #[test]
    fn test_store_light_list_api() {
        // Verify store_light_list + invalidate_light_list API exists
        let api = LightListAPI::new(Prim::invalid());
        let empty: HashSet<Path> = HashSet::new();
        api.store_light_list(&empty);
        api.invalidate_light_list();
        // No panic on invalid prim
    }

    #[test]
    fn create_light_list_cache_behavior_attr_sets_token_default() {
        let _ = usd_sdf::init();
        let stage = Arc::new(Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap());
        let prim = stage.define_prim("/World", "").expect("prim");
        let api = LightListAPI::apply(&prim).expect("apply");
        let tok = tokens().ignore.clone();
        let attr = api.create_light_list_cache_behavior_attr(Some(Value::from(tok.clone())), false);
        assert!(attr.is_valid());
        assert_eq!(attr.get_typed::<Token>(TimeCode::default()), Some(tok));
    }
}
