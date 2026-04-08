//! Scene index that filters prims based on generative procedural type.
//!
//! Unlike the resolving scene index which evaluates procedurals, this scene index
//! re-types prims representing generative procedurals within its incoming scene
//! against a requested pattern:
//!   - Prims whose `hdGp:proceduralType` primvar matches allowed types => `_allowedPrimTypeName`
//!   - Prims whose `hdGp:proceduralType` primvar does NOT match allowed types => `_skippedPrimTypeName`
//!   - Prims of any other type => passed through unchanged (Ignore)
//!
//! Port of pxr/imaging/hdGp/generativeProceduralFilteringSceneIndex.h/cpp

use super::generative_procedural::tokens;
use parking_lot::RwLock;
use std::sync::Arc;
use usd_hd::scene_index::{
    HdSceneIndexBase, HdSceneIndexHandle, HdSceneIndexPrim, HdSingleInputFilteringSceneIndexBase,
    SdfPathVector,
    observer::{AddedPrimEntry, DirtiedPrimEntry, HdSceneIndexObserverHandle, RemovedPrimEntry},
    si_ref,
};
use usd_hd::schema::HdPrimvarsSchema;
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

/// Result of the should-skip check for a procedural prim.
/// Matches C++ `HdGpGenerativeProceduralFilteringSceneIndex::_ShouldSkipResult`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShouldSkipResult {
    /// Prim is not the target type — leave it unchanged.
    Ignore,
    /// Prim IS the target type but proc type is NOT in allowed list — re-type to skipped.
    Skip,
    /// Prim IS the target type and proc type IS in allowed list — re-type to allowed.
    Allow,
}

/// Scene index that filters generative procedural prims by their `hdGp:proceduralType`.
///
/// For each prim whose prim type matches the configured target (default:
/// `"hydraGenerativeProcedural"`), reads its `hdGp:proceduralType` primvar and
/// checks it against the allowed-types list:
///   - If the proc type is in the list (or `"*"` wildcard is present) => re-type to `allowedPrimTypeName`
///   - Otherwise => re-type to `skippedPrimTypeName`
/// Prims of any other type pass through unchanged. Child paths pass through directly.
///
/// This is used in multi-resolver pipelines to stage which procedurals are
/// resolved at each point in the scene index chain.
///
/// Port of C++ `HdGpGenerativeProceduralFilteringSceneIndex`.
pub struct HdGpGenerativeProceduralFilteringSceneIndex {
    /// Base filtering infrastructure (observer list + input scene).
    base: HdSingleInputFilteringSceneIndexBase,
    /// Allowed procedural type names; if any entry is `"*"` all types are allowed.
    allowed_procedural_types: Vec<TfToken>,
    /// The prim type we target for filtering (default: `"hydraGenerativeProcedural"`).
    target_prim_type_name: TfToken,
    /// Type to assign prims whose proc type is in the allowed list.
    allowed_prim_type_name: TfToken,
    /// Type to assign prims whose proc type is NOT in the allowed list.
    skipped_prim_type_name: TfToken,
}

/// Handle type for the filtering scene index.
pub type HdGpGenerativeProceduralFilteringSceneIndexHandle =
    Arc<RwLock<HdGpGenerativeProceduralFilteringSceneIndex>>;

impl HdGpGenerativeProceduralFilteringSceneIndex {
    // -----------------------------------------------------------------------
    // Constructors — match C++ New() overloads
    // -----------------------------------------------------------------------

    /// Create with default names.
    ///
    /// Target type: `"hydraGenerativeProcedural"`
    /// Allowed name: same as target type
    /// Skipped name: `"skippedHydraGenerativeProcedural"`
    pub fn new(
        input_scene: HdSceneIndexHandle,
        allowed_procedural_types: Vec<TfToken>,
    ) -> HdGpGenerativeProceduralFilteringSceneIndexHandle {
        let target = tokens::GENERATIVE_PROCEDURAL.clone();
        let allowed_name = target.clone();
        let skipped_name = tokens::SKIPPED_GENERATIVE_PROCEDURAL.clone();
        Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input_scene)),
            allowed_procedural_types,
            target_prim_type_name: target,
            allowed_prim_type_name: allowed_name,
            skipped_prim_type_name: skipped_name,
        }))
    }

    /// Create with explicit optional names.
    ///
    /// `maybe_target_prim_type_name`: if `None`, defaults to `"hydraGenerativeProcedural"`.
    /// `maybe_allowed_prim_type_name`: if `None`, defaults to the target type name.
    /// `maybe_skipped_prim_type_name`: if `None`, defaults to `"skippedHydraGenerativeProcedural"`.
    pub fn new_with_options(
        input_scene: HdSceneIndexHandle,
        allowed_procedural_types: Vec<TfToken>,
        maybe_target_prim_type_name: Option<TfToken>,
        maybe_allowed_prim_type_name: Option<TfToken>,
        maybe_skipped_prim_type_name: Option<TfToken>,
    ) -> HdGpGenerativeProceduralFilteringSceneIndexHandle {
        let target =
            maybe_target_prim_type_name.unwrap_or_else(|| tokens::GENERATIVE_PROCEDURAL.clone());
        let allowed_name = maybe_allowed_prim_type_name.unwrap_or_else(|| target.clone());
        let skipped_name = maybe_skipped_prim_type_name
            .unwrap_or_else(|| tokens::SKIPPED_GENERATIVE_PROCEDURAL.clone());
        Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input_scene)),
            allowed_procedural_types,
            target_prim_type_name: target,
            allowed_prim_type_name: allowed_name,
            skipped_prim_type_name: skipped_name,
        }))
    }

    // -----------------------------------------------------------------------
    // Internal helpers — match C++ private methods
    // -----------------------------------------------------------------------

    /// Read `hdGp:proceduralType` primvar from a prim's data source.
    /// Matches C++ `_GetProceduralType`.
    fn get_procedural_type(prim: &HdSceneIndexPrim) -> TfToken {
        if let Some(ref data_source) = prim.data_source {
            let primvars = HdPrimvarsSchema::get_from_parent(data_source);
            let primvar = primvars.get_primvar_schema(&tokens::PROCEDURAL_TYPE);
            if let Some(value_ds) = primvar.get_primvar_value() {
                if let Some(sampled) = value_ds.as_sampled() {
                    let value = sampled.get_value(0.0);
                    if let Some(token) = value.get::<TfToken>() {
                        return token.clone();
                    }
                }
            }
        }
        TfToken::empty()
    }

    /// Determine whether a prim should be ignored, skipped, or allowed.
    /// Matches C++ `_ShouldSkipPrim`.
    fn should_skip_prim(&self, prim: &HdSceneIndexPrim) -> ShouldSkipResult {
        if prim.prim_type != self.target_prim_type_name {
            // Not a target procedural — pass through unchanged
            return ShouldSkipResult::Ignore;
        }

        let proc_type = Self::get_procedural_type(prim);
        for allowed in &self.allowed_procedural_types {
            // "*" wildcard matches any proc type (anyProceduralType token)
            if *allowed == *tokens::ANY_PROCEDURAL_TYPE || *allowed == proc_type {
                return ShouldSkipResult::Allow;
            }
        }
        ShouldSkipResult::Skip
    }

    /// Apply the skip result to a prim: re-type or leave unchanged.
    fn apply_result(
        result: ShouldSkipResult,
        mut prim: HdSceneIndexPrim,
        allowed_name: &TfToken,
        skipped_name: &TfToken,
    ) -> HdSceneIndexPrim {
        match result {
            ShouldSkipResult::Ignore => {}
            ShouldSkipResult::Allow => {
                prim.prim_type = allowed_name.clone();
            }
            ShouldSkipResult::Skip => {
                prim.prim_type = skipped_name.clone();
            }
        }
        prim
    }

    // -----------------------------------------------------------------------
    // Notice forwarders — called by the observer machinery
    // -----------------------------------------------------------------------

    /// Handle added prims: re-type procedurals in the notice entries.
    /// Matches C++ `_PrimsAdded`.
    pub fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        // Fast path: no procedurals in the batch => forward as-is.
        let any_target = entries
            .iter()
            .any(|e| e.prim_type == self.target_prim_type_name);
        if !any_target {
            self.base.forward_prims_added(self, entries);
            return;
        }

        // Apply filtering to each entry that is the target type.
        let mut filtered: Vec<AddedPrimEntry> = Vec::with_capacity(entries.len());
        for entry in entries {
            if entry.prim_type != self.target_prim_type_name {
                filtered.push(entry.clone());
                continue;
            }
            // Read the full prim to get its proc type from primvars.
            let prim = if let Some(input) = self.base.get_input_scene() {
                si_ref(&input).get_prim(&entry.prim_path)
            } else {
                HdSceneIndexPrim {
                    prim_type: entry.prim_type.clone(),
                    data_source: None,
                }
            };

            let result = self.should_skip_prim(&prim);
            let new_type = match result {
                ShouldSkipResult::Ignore => entry.prim_type.clone(),
                ShouldSkipResult::Allow => self.allowed_prim_type_name.clone(),
                ShouldSkipResult::Skip => self.skipped_prim_type_name.clone(),
            };
            filtered.push(AddedPrimEntry::new(entry.prim_path.clone(), new_type));
        }
        self.base.forward_prims_added(self, &filtered);
    }

    /// Pass through removed prims to observers.
    /// Matches C++ `_PrimsRemoved`.
    pub fn on_prims_removed(
        &mut self,
        _sender: &dyn HdSceneIndexBase,
        entries: &[RemovedPrimEntry],
    ) {
        self.base.forward_prims_removed(self, entries);
    }

    /// Pass through dirtied prims to observers.
    /// Matches C++ `_PrimsDirtied`.
    pub fn on_prims_dirtied(
        &mut self,
        _sender: &dyn HdSceneIndexBase,
        entries: &[DirtiedPrimEntry],
    ) {
        self.base.forward_prims_dirtied(self, entries);
    }

    // Accessors for testing
    pub fn get_target_prim_type_name(&self) -> &TfToken {
        &self.target_prim_type_name
    }
    pub fn get_allowed_prim_type_name(&self) -> &TfToken {
        &self.allowed_prim_type_name
    }
    pub fn get_skipped_prim_type_name(&self) -> &TfToken {
        &self.skipped_prim_type_name
    }
    pub fn get_allowed_procedural_types(&self) -> &[TfToken] {
        &self.allowed_procedural_types
    }
}

impl HdSceneIndexBase for HdGpGenerativeProceduralFilteringSceneIndex {
    /// Re-type target procedural prims based on their proc type.
    /// Matches C++ `GetPrim`.
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        if let Some(input) = self.base.get_input_scene() {
            let locked = input.read();
            let prim = locked.get_prim(prim_path);
            let result = self.should_skip_prim(&prim);
            return Self::apply_result(
                result,
                prim,
                &self.allowed_prim_type_name,
                &self.skipped_prim_type_name,
            );
        }
        HdSceneIndexPrim::empty()
    }

    /// Pass child paths through directly — no filtering of children.
    /// Matches C++ `GetChildPrimPaths` which simply returns `_GetInputSceneIndex()->GetChildPrimPaths(primPath)`.
    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        if let Some(input) = self.base.get_input_scene() {
            return si_ref(&input).get_child_prim_paths(prim_path);
        }
        Vec::new()
    }

    fn add_observer(&self, observer: HdSceneIndexObserverHandle) {
        self.base.base().add_observer(observer);
    }

    fn remove_observer(&self, observer: &HdSceneIndexObserverHandle) {
        self.base.base().remove_observer(observer);
    }

    fn get_display_name(&self) -> String {
        "HdGpGenerativeProceduralFilteringSceneIndex".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_hd::scene_index::{
        HdRetainedSceneIndex, RetainedAddedPrimEntry, observer::AddedPrimEntry,
    };

    // ---------------------------------------------------------------------------
    // Helpers
    // ---------------------------------------------------------------------------

    fn make_prim(prim_type: &str) -> HdSceneIndexPrim {
        HdSceneIndexPrim {
            prim_type: TfToken::new(prim_type),
            data_source: None,
        }
    }

    fn target_type() -> TfToken {
        TfToken::new("hydraGenerativeProcedural")
    }
    fn allowed_type() -> TfToken {
        TfToken::new("myProc")
    }
    fn other_type() -> TfToken {
        TfToken::new("mesh")
    }

    /// Add a single prim to a retained scene index
    fn add_prim(si: &Arc<RwLock<HdRetainedSceneIndex>>, path: SdfPath, prim_type: TfToken) {
        si.write()
            .add_prims(&[RetainedAddedPrimEntry::new(path, prim_type, None)]);
    }

    // ---------------------------------------------------------------------------
    // ShouldSkipResult tests
    // ---------------------------------------------------------------------------

    #[test]
    fn test_should_skip_ignore_non_target() {
        // Prim whose type != target => Ignore
        let si = HdGpGenerativeProceduralFilteringSceneIndex::new(
            HdRetainedSceneIndex::new(),
            vec![allowed_type()],
        );
        let guard = si.read();
        let prim = make_prim("mesh");
        assert_eq!(guard.should_skip_prim(&prim), ShouldSkipResult::Ignore);
    }

    #[test]
    fn test_should_skip_skip_when_not_in_allowed_list() {
        // Target prim, proc type not in allowed list => Skip
        let si = HdGpGenerativeProceduralFilteringSceneIndex::new(
            HdRetainedSceneIndex::new(),
            vec![allowed_type()],
        );
        let guard = si.read();
        // prim has target type but no primvar proc type (empty) => not in allowed => Skip
        let prim = make_prim("hydraGenerativeProcedural");
        assert_eq!(guard.should_skip_prim(&prim), ShouldSkipResult::Skip);
    }

    #[test]
    fn test_should_skip_allow_with_wildcard() {
        // Wildcard "*" => any proc type => Allow
        let si = HdGpGenerativeProceduralFilteringSceneIndex::new(
            HdRetainedSceneIndex::new(),
            vec![TfToken::new("*")],
        );
        let guard = si.read();
        let prim = make_prim("hydraGenerativeProcedural");
        assert_eq!(guard.should_skip_prim(&prim), ShouldSkipResult::Allow);
    }

    #[test]
    fn test_should_skip_empty_allowed_list() {
        // Empty allowed list => everything Skipped
        let si =
            HdGpGenerativeProceduralFilteringSceneIndex::new(HdRetainedSceneIndex::new(), vec![]);
        let guard = si.read();
        let prim = make_prim("hydraGenerativeProcedural");
        assert_eq!(guard.should_skip_prim(&prim), ShouldSkipResult::Skip);
    }

    // ---------------------------------------------------------------------------
    // Constructor / naming tests
    // ---------------------------------------------------------------------------

    #[test]
    fn test_default_names() {
        let si =
            HdGpGenerativeProceduralFilteringSceneIndex::new(HdRetainedSceneIndex::new(), vec![]);
        let guard = si.read();
        assert_eq!(
            guard.get_target_prim_type_name().as_str(),
            "hydraGenerativeProcedural"
        );
        // allowed_prim_type_name defaults to target
        assert_eq!(
            guard.get_allowed_prim_type_name().as_str(),
            "hydraGenerativeProcedural"
        );
        assert_eq!(
            guard.get_skipped_prim_type_name().as_str(),
            "skippedHydraGenerativeProcedural"
        );
    }

    #[test]
    fn test_custom_names_with_options() {
        let si = HdGpGenerativeProceduralFilteringSceneIndex::new_with_options(
            HdRetainedSceneIndex::new(),
            vec![TfToken::new("procA")],
            Some(TfToken::new("customTarget")),
            Some(TfToken::new("customAllowed")),
            Some(TfToken::new("customSkipped")),
        );
        let guard = si.read();
        assert_eq!(guard.get_target_prim_type_name().as_str(), "customTarget");
        assert_eq!(guard.get_allowed_prim_type_name().as_str(), "customAllowed");
        assert_eq!(guard.get_skipped_prim_type_name().as_str(), "customSkipped");
        assert_eq!(
            guard.get_allowed_procedural_types(),
            &[TfToken::new("procA")]
        );
    }

    #[test]
    fn test_options_with_none_uses_defaults() {
        // When Options are None, fall back to defaults
        let si = HdGpGenerativeProceduralFilteringSceneIndex::new_with_options(
            HdRetainedSceneIndex::new(),
            vec![],
            None,
            None,
            None,
        );
        let guard = si.read();
        assert_eq!(
            guard.get_target_prim_type_name().as_str(),
            "hydraGenerativeProcedural"
        );
        assert_eq!(
            guard.get_allowed_prim_type_name().as_str(),
            "hydraGenerativeProcedural"
        );
        assert_eq!(
            guard.get_skipped_prim_type_name().as_str(),
            "skippedHydraGenerativeProcedural"
        );
    }

    // ---------------------------------------------------------------------------
    // GetPrim re-typing tests
    // ---------------------------------------------------------------------------

    #[test]
    fn test_get_prim_non_procedural_passes_through() {
        // Non-procedural prims pass through with original type (Ignore)
        let input = HdRetainedSceneIndex::new();
        let path = SdfPath::from_string("/Mesh").unwrap();
        add_prim(&input, path.clone(), other_type());
        let si = HdGpGenerativeProceduralFilteringSceneIndex::new(input, vec![TfToken::new("*")]);
        let guard = si.read();
        let prim = guard.get_prim(&path);
        assert_eq!(
            prim.prim_type,
            other_type(),
            "non-procedural must pass through"
        );
    }

    #[test]
    fn test_get_prim_target_with_wildcard_allowed() {
        // Target prim with wildcard => allowed prim type (same as target by default)
        let input = HdRetainedSceneIndex::new();
        let path = SdfPath::from_string("/Proc").unwrap();
        add_prim(&input, path.clone(), target_type());
        let si = HdGpGenerativeProceduralFilteringSceneIndex::new(input, vec![TfToken::new("*")]);
        let guard = si.read();
        let prim = guard.get_prim(&path);
        // wildcard => Allow => re-typed to allowed_prim_type_name (= target by default)
        assert_eq!(prim.prim_type.as_str(), "hydraGenerativeProcedural");
    }

    #[test]
    fn test_get_prim_target_not_in_allowed_list() {
        // Target prim with empty proc type and no wildcard => Skipped
        let input = HdRetainedSceneIndex::new();
        let path = SdfPath::from_string("/Proc").unwrap();
        add_prim(&input, path.clone(), target_type());
        // allowed = ["someProc"] but prim has no proc type primvar => Skip
        let si =
            HdGpGenerativeProceduralFilteringSceneIndex::new(input, vec![TfToken::new("someProc")]);
        let guard = si.read();
        let prim = guard.get_prim(&path);
        assert_eq!(prim.prim_type.as_str(), "skippedHydraGenerativeProcedural");
    }

    // ---------------------------------------------------------------------------
    // GetChildPrimPaths passthrough test
    // ---------------------------------------------------------------------------

    #[test]
    fn test_get_child_prim_paths_passthrough() {
        // GetChildPrimPaths must pass through directly (C++ does no filtering)
        let input = HdRetainedSceneIndex::new();
        let parent = SdfPath::from_string("/Parent").unwrap();
        let child1 = SdfPath::from_string("/Parent/Child1").unwrap();
        let child2 = SdfPath::from_string("/Parent/Child2").unwrap();
        add_prim(&input, parent.clone(), other_type());
        add_prim(&input, child1.clone(), target_type()); // would be skipped in GetPrim
        add_prim(&input, child2.clone(), other_type());
        // Even with empty allowed list (all target prims skipped), children still visible
        let si = HdGpGenerativeProceduralFilteringSceneIndex::new(input, vec![]);
        let guard = si.read();
        let children = guard.get_child_prim_paths(&parent);
        // Both children must appear — no child filtering
        assert_eq!(
            children.len(),
            2,
            "get_child_prim_paths must not filter children"
        );
        assert!(children.contains(&child1));
        assert!(children.contains(&child2));
    }

    // ---------------------------------------------------------------------------
    // Notice propagation test
    // ---------------------------------------------------------------------------

    #[test]
    fn test_on_prims_added_fast_path_no_procedurals() {
        // If no entries are target type, forward unchanged (fast path)
        let si =
            HdGpGenerativeProceduralFilteringSceneIndex::new(HdRetainedSceneIndex::new(), vec![]);
        let guard = si.write();
        // Call with non-procedural entries — should not panic
        let entries = vec![AddedPrimEntry::new(
            SdfPath::from_string("/A").unwrap(),
            other_type(),
        )];
        struct NoopSender;
        impl HdSceneIndexBase for NoopSender {
            fn get_prim(&self, _: &SdfPath) -> HdSceneIndexPrim {
                HdSceneIndexPrim::empty()
            }
            fn get_child_prim_paths(&self, _: &SdfPath) -> SdfPathVector {
                vec![]
            }
            fn add_observer(&self, _: HdSceneIndexObserverHandle) {}
            fn remove_observer(&self, _: &HdSceneIndexObserverHandle) {}
            fn get_display_name(&self) -> String {
                "noop".to_string()
            }
        }
        guard.on_prims_added(&NoopSender, &entries); // must not panic
    }

    // ---------------------------------------------------------------------------
    // Display name test
    // ---------------------------------------------------------------------------

    #[test]
    fn test_display_name() {
        let si =
            HdGpGenerativeProceduralFilteringSceneIndex::new(HdRetainedSceneIndex::new(), vec![]);
        let guard = si.read();
        assert_eq!(
            guard.get_display_name(),
            "HdGpGenerativeProceduralFilteringSceneIndex"
        );
    }
}
