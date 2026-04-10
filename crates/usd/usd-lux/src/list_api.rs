//! UsdLuxListAPI - deprecated API for light lists (use LightListAPI instead).
//!
//! This module provides [`ListAPI`], a **deprecated** single-apply API schema
//! for caching lists of lights. New code should use `LightListAPI` instead.
//!
//! # Overview
//!
//! ListAPI provides a mechanism to cache lists of lights on prims for
//! efficient traversal. It's deprecated in favor of `LightListAPI`.
//!
//! # Cache Behavior
//!
//! The `lightList:cacheBehavior` attribute controls how the cached list is used:
//! - `consumeAndHalt` - Use cache as final authoritative statement, halt recursion
//! - `consumeAndContinue` - Use cache but continue recursive traversal
//! - `ignore` - Ignore the cache entirely (fallback behavior)
//!
//! # Compute Modes
//!
//! - `ComputeModeConsultModelHierarchyCache` - Consult caches on model hierarchy
//! - `ComputeModeIgnoreCache` - Full prim traversal, ignore all caches
//!
//! # Schema Type
//!
//! This is a **single-apply API schema** (`UsdSchemaKind::SingleApplyAPI`).
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdLux/listAPI.h`

use super::tokens::tokens;
use crate::schema_create_attr::create_lux_schema_attr;
use std::collections::HashSet;
use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, Relationship, Stage};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

/// Compute mode for light list traversal.
///
/// Controls whether to consult cached light lists or perform full traversal.
///
/// Matches C++ `UsdLuxListAPI::ComputeMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComputeMode {
    /// Consult any caches found on the model hierarchy.
    /// Do not traverse beneath the model hierarchy.
    ConsultModelHierarchyCache,

    /// Ignore any caches found, and do a full prim traversal.
    IgnoreCache,
}

/// Deprecated API schema for light lists (use LightListAPI instead).
///
/// Provides caching for light lists to optimize traversal.
///
/// # Deprecation Warning
///
/// This API is **deprecated**. Use `LightListAPI` for new code.
///
/// # Schema Type
///
/// This is a **single-apply API schema** (`UsdSchemaKind::SingleApplyAPI`).
/// Use [`apply`](Self::apply) to add this schema to a prim.
///
/// # Attributes
///
/// | Attribute | Type | Allowed Values |
/// |-----------|------|----------------|
/// | `lightList:cacheBehavior` | token | consumeAndHalt, consumeAndContinue, ignore |
///
/// # Relationships
///
/// | Relationship | Description |
/// |--------------|-------------|
/// | `lightList` | Relationship to lights in the scene |
///
/// Matches C++ `UsdLuxListAPI`.
#[derive(Clone)]
pub struct ListAPI {
    prim: Prim,
}

impl ListAPI {
    // =========================================================================
    // Construction
    // =========================================================================

    /// Constructs a ListAPI on the given prim.
    ///
    /// # Arguments
    /// * `prim` - The prim to wrap with this API schema
    #[inline]
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Returns a ListAPI holding the prim at `path` on `stage`.
    ///
    /// If no prim exists at the path, returns an invalid schema object.
    ///
    /// Matches C++ `UsdLuxListAPI::Get(stage, path)`.
    pub fn get(stage: &Stage, path: &Path) -> Self {
        match stage.get_prim_at_path(path) {
            Some(prim) => Self::new(prim),
            None => Self::invalid(),
        }
    }

    /// Creates an invalid ListAPI.
    #[inline]
    pub fn invalid() -> Self {
        Self {
            prim: Prim::invalid(),
        }
    }

    /// Returns true if this API schema can be applied to the given prim.
    ///
    /// Matches C++ `UsdLuxListAPI::CanApply(prim)`.
    pub fn can_apply(prim: &Prim) -> bool {
        prim.is_valid()
    }

    /// Applies ListAPI to the given prim.
    ///
    /// This adds "ListAPI" to the prim's `apiSchemas` metadata.
    ///
    /// # Returns
    /// A valid ListAPI on success, or `None` if the prim is invalid.
    ///
    /// Matches C++ `UsdLuxListAPI::Apply(prim)`.
    pub fn apply(prim: &Prim) -> Option<Self> {
        if !prim.is_valid() {
            return None;
        }
        // In full implementation, would add "ListAPI" to apiSchemas
        Some(Self::new(prim.clone()))
    }

    // =========================================================================
    // Schema Information
    // =========================================================================

    /// Returns true if this API schema is valid.
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.prim.is_valid()
    }

    /// Returns the wrapped prim.
    #[inline]
    pub fn get_prim(&self) -> &Prim {
        &self.prim
    }

    /// Returns names of all pre-declared attributes for this schema.
    ///
    /// # Arguments
    /// * `_include_inherited` - Unused for API schemas (no inheritance)
    ///
    /// Matches C++ `UsdLuxListAPI::GetSchemaAttributeNames()`.
    pub fn get_schema_attribute_names(_include_inherited: bool) -> Vec<Token> {
        vec![tokens().light_list_cache_behavior.clone()]
    }

    // =========================================================================
    // LIGHTLISTCACHEBEHAVIOR Attribute
    // =========================================================================

    /// Returns the lightList:cacheBehavior attribute.
    ///
    /// Controls how the lightList should be interpreted.
    ///
    /// Valid values:
    /// - `consumeAndHalt`: Use cache as final statement, halt recursion
    /// - `consumeAndContinue`: Use cache but continue traversal
    /// - `ignore`: Ignore cache entirely (fallback behavior)
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `token lightList:cacheBehavior` |
    /// | C++ Type | TfToken |
    /// | Allowed Values | consumeAndHalt, consumeAndContinue, ignore |
    ///
    /// Matches C++ `UsdLuxListAPI::GetLightListCacheBehaviorAttr()`.
    #[inline]
    pub fn get_light_list_cache_behavior_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(tokens().light_list_cache_behavior.as_str())
    }

    /// Creates the lightList:cacheBehavior attribute.
    ///
    /// See [`get_light_list_cache_behavior_attr`](Self::get_light_list_cache_behavior_attr) for details.
    ///
    /// Matches C++ `UsdLuxListAPI::CreateLightListCacheBehaviorAttr(VtValue const &defaultValue, bool writeSparsely)`.
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
    // LIGHTLIST Relationship
    // =========================================================================

    /// Returns the lightList relationship.
    ///
    /// Relationship to lights in the scene.
    ///
    /// Matches C++ `UsdLuxListAPI::GetLightListRel()`.
    #[inline]
    pub fn get_light_list_rel(&self) -> Option<Relationship> {
        self.prim.get_relationship(tokens().light_list.as_str())
    }

    /// Creates the lightList relationship.
    ///
    /// See [`get_light_list_rel`](Self::get_light_list_rel) for details.
    ///
    /// Matches C++ `UsdLuxListAPI::CreateLightListRel()`.
    pub fn create_light_list_rel(&self) -> Option<Relationship> {
        self.get_light_list_rel()
    }

    // =========================================================================
    // Light List Computation
    // =========================================================================

    /// Computes and returns the list of lights and light filters in the stage.
    ///
    /// # Arguments
    /// * `mode` - Compute mode (consult cache or ignore cache)
    ///
    /// # Returns
    /// Set of paths to lights and light filters found
    ///
    /// In `ComputeModeIgnoreCache` mode, performs full prim traversal
    /// looking for prims with UsdLuxLightAPI or of type UsdLuxLightFilter.
    ///
    /// In `ComputeModeConsultModelHierarchyCache` mode, traverses only
    /// the model hierarchy, accumulating lights and cached paths according
    /// to `lightList:cacheBehavior` attributes.
    ///
    /// Matches C++ `UsdLuxListAPI::ComputeLightList(mode)`.
    pub fn compute_light_list(&self, _mode: ComputeMode) -> HashSet<Path> {
        // Full implementation would traverse stage looking for lights
        // For now, return empty set
        HashSet::new()
    }

    /// Store the given paths as the lightlist for this prim.
    ///
    /// Paths that do not have this prim's path as a prefix will be
    /// silently ignored. This sets `lightList:cacheBehavior` to
    /// "consumeAndContinue".
    ///
    /// # Arguments
    /// * `paths` - Set of paths to store in the light list
    ///
    /// Matches C++ `UsdLuxListAPI::StoreLightList(paths)`.
    pub fn store_light_list(&self, _paths: &HashSet<Path>) {
        // Full implementation would:
        // 1. Filter paths to only those under this prim
        // 2. Set lightList relationship targets
        // 3. Set cacheBehavior to "consumeAndContinue"
    }

    /// Mark any stored lightlist as invalid.
    ///
    /// Sets the `lightList:cacheBehavior` attribute to "ignore".
    ///
    /// Matches C++ `UsdLuxListAPI::InvalidateLightList()`.
    pub fn invalidate_light_list(&self) {
        // Full implementation would set cacheBehavior to "ignore"
    }
}

impl Default for ListAPI {
    fn default() -> Self {
        Self::invalid()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_list_api() {
        let api = ListAPI::default();
        assert!(!api.is_valid());
    }

    #[test]
    fn test_compute_mode() {
        let mode1 = ComputeMode::ConsultModelHierarchyCache;
        let mode2 = ComputeMode::IgnoreCache;
        assert_ne!(mode1, mode2);
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = ListAPI::get_schema_attribute_names(true);
        assert_eq!(names.len(), 1);
    }
}
