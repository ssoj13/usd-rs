// Port of pxr/imaging/hd/testenv/testHdCollectionExpressionEvaluator.cpp

//! Tests for HdCollectionExpressionEvaluator and HdCollectionPredicateLibrary.
//!
//! NOTE on current API state:
//!   The Rust HdCollectionExpressionEvaluator is a placeholder struct that stores
//!   a string expression and does simple glob matching. It does NOT yet implement
//!   the full C++ API:
//!     - HdCollectionExpressionEvaluator::new(scene_index, expr, library)
//!     - eval.Match(path) with predicate evaluation against live scene data
//!     - eval.PopulateAllMatches / PopulateMatches / MatchKind
//!   Tests that require the full API are marked #[ignore] with an explanation.
//!
//!   The Rust HdCollectionPredicateLibrary IS registered with standard predicates
//!   (hdType, hdVisible, hdPurpose, hdHasDataSource, hdHasPrimvar,
//!   hdHasMaterialBinding) but their implementations are stubs that do not
//!   correctly read real scene data. Tests exercising predicate correctness
//!   are therefore also marked #[ignore].

use parking_lot::RwLock;
use std::collections::BTreeSet;
use std::sync::Arc;

use usd_hd::HdCollectionExpressionEvaluator;
use usd_hd::collection_expression_evaluator::MatchKind;
use usd_hd::collection_predicate_library::{
    HdCollectionPredicateLibrary, hd_get_collection_predicate_library,
};
use usd_hd::data_source::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdRetainedContainerDataSource,
    HdRetainedTypedSampledDataSource, cast_to_container,
};
use usd_hd::scene_index::retained::{HdRetainedSceneIndex, RetainedAddedPrimEntry};
use usd_hd::scene_index::{HdSceneIndexBase, HdSceneIndexPrim};
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn tok(s: &str) -> Token {
    Token::new(s)
}

fn path(s: &str) -> SdfPath {
    SdfPath::from_string(s).expect("valid path")
}

/// Build a visibility data source matching HdVisibilitySchema layout:
///   { "visibility": <bool> }
fn make_visibility_ds(visible: bool) -> HdDataSourceBaseHandle {
    let inner_bool: HdDataSourceBaseHandle = HdRetainedTypedSampledDataSource::new(visible);
    let container = HdRetainedContainerDataSource::new_1(tok("visibility"), inner_bool);
    container as HdDataSourceBaseHandle
}

/// Build a purpose data source matching HdPurposeSchema layout:
///   { "purpose": <Token> }
fn make_purpose_ds(purpose: &str) -> HdDataSourceBaseHandle {
    let inner: HdDataSourceBaseHandle = HdRetainedTypedSampledDataSource::new(tok(purpose));
    let container = HdRetainedContainerDataSource::new_1(tok("purpose"), inner);
    container as HdDataSourceBaseHandle
}

/// Build a primvars data source:
///   { <name>: { "primvarValue": 1 }, ... }
fn make_primvars_ds(primvar_names: &[&str]) -> HdDataSourceBaseHandle {
    let pv_val: HdDataSourceBaseHandle = HdRetainedTypedSampledDataSource::new(1_i32);
    let primvar_entry = HdRetainedContainerDataSource::new_1(tok("primvarValue"), pv_val);

    let entries: Vec<(Token, HdDataSourceBaseHandle)> = primvar_names
        .iter()
        .map(|n| (tok(n), primvar_entry.clone() as HdDataSourceBaseHandle))
        .collect();
    HdRetainedContainerDataSource::from_entries(&entries) as HdDataSourceBaseHandle
}

/// Build a materialBindings data source:
///   { <purpose>: { "path": <SdfPath> }, ... }
///
/// The empty string purpose ("") corresponds to C++ `allPurpose`.
fn make_material_bindings_ds(bindings: &[(&str, &str)]) -> HdDataSourceBaseHandle {
    let entries: Vec<(Token, HdDataSourceBaseHandle)> = bindings
        .iter()
        .map(|(purpose, mat_path)| {
            let path_ds: HdDataSourceBaseHandle =
                HdRetainedTypedSampledDataSource::new(path(mat_path));
            let binding_container = HdRetainedContainerDataSource::new_1(tok("path"), path_ds);
            (tok(purpose), binding_container as HdDataSourceBaseHandle)
        })
        .collect();
    HdRetainedContainerDataSource::from_entries(&entries) as HdDataSourceBaseHandle
}

/// Build the top-level prim container matching C++ _MakePrimContainer:
/// ```text
/// {
///   "visibility": { "visibility": <bool> },
///   "purpose":    { "purpose": <token> },
///   "primvars":   { <name>: { "primvarValue": 1 }, ... },
///   "materialBindings": { <purpose>: { "path": <SdfPath> }, ... },
/// }
/// ```
fn make_prim_container(
    visible: bool,
    purpose: &str,
    primvar_names: &[&str],
    material_bindings: &[(&str, &str)],
) -> HdContainerDataSourceHandle {
    HdRetainedContainerDataSource::from_entries(&[
        (tok("visibility"), make_visibility_ds(visible)),
        (tok("purpose"), make_purpose_ds(purpose)),
        (tok("primvars"), make_primvars_ds(primvar_names)),
        (
            tok("materialBindings"),
            make_material_bindings_ds(material_bindings),
        ),
    ])
}

/// Create the test scene matching C++ `_CreateTestScene()`.
fn create_test_scene() -> Arc<RwLock<HdRetainedSceneIndex>> {
    let scene = HdRetainedSceneIndex::new();
    {
        let mut si = scene.write();
        si.add_prims(&[
            RetainedAddedPrimEntry::new(path("/A"), tok("scope"), None),
            RetainedAddedPrimEntry::new(path("/A/B"), tok("scope"), None),
            RetainedAddedPrimEntry::new(
                path("/A/B/Carrot"),
                tok("veg"),
                Some(make_prim_container(
                    true,
                    "food",
                    &["fresh"],
                    &[("", "/Looks/OrangeMat")], // "" = allPurpose
                )),
            ),
            RetainedAddedPrimEntry::new(
                path("/A/B/Broccoli"),
                tok("veg"),
                Some(make_prim_container(
                    true,
                    "food",
                    &[],
                    &[
                        ("preview", "/Looks/GreenMat"),
                        ("", "/Looks/WiltedGreenMat"),
                    ],
                )),
            ),
            RetainedAddedPrimEntry::new(
                path("/A/B/Tomato"),
                tok("fruit"),
                Some(make_prim_container(
                    true,
                    "food",
                    &["fresh", "foo:glossy"],
                    &[("preview", "/Looks/GlossyRedMat")],
                )),
            ),
            RetainedAddedPrimEntry::new(
                path("/A/B/Apricot"),
                tok("fruit"),
                Some(make_prim_container(
                    true,
                    "food",
                    &[],
                    &[
                        ("preview", "/Looks/DriedOrangeMat"),
                        ("", "/Looks/DriedOrangeMat"),
                    ],
                )),
            ),
            RetainedAddedPrimEntry::new(path("/A/C"), tok("scope"), None),
            RetainedAddedPrimEntry::new(
                path("/A/C/Table"),
                tok("mesh"),
                Some(make_prim_container(true, "furniture", &[], &[])),
            ),
            RetainedAddedPrimEntry::new(
                path("/A/C/Chair1"),
                tok("mesh"),
                Some(make_prim_container(
                    true,
                    "furniture",
                    &["foo:glossy"],
                    &[("preview", "/Looks/MetallicMat")],
                )),
            ),
            RetainedAddedPrimEntry::new(
                path("/A/C/Chair2"),
                tok("mesh"),
                Some(make_prim_container(false, "furniture", &[], &[])),
            ),
        ]);
    }
    scene
}

// ---------------------------------------------------------------------------
// TestEmptyEvaluator
// ---------------------------------------------------------------------------

/// Port of C++ `TestEmptyEvaluator`.
///
/// An empty expression → `is_empty() == true`, never matches.
#[test]
fn test_empty_evaluator_default() {
    let eval = HdCollectionExpressionEvaluator::new("");
    assert!(eval.is_empty(), "default evaluator must be empty");
    assert!(
        !eval.matches(&path("/A")),
        "empty evaluator must not match any path"
    );
}

#[test]
fn test_empty_evaluator_explicit_empty_string() {
    let eval = HdCollectionExpressionEvaluator::new("");
    assert!(eval.is_empty());
    assert!(!eval.matches(&path("/Foo")));
}

// The C++ test also constructs with nullptr scene index + non-empty expression
// and with valid scene + empty expression — both should be empty.
// In Rust the evaluator does not yet take a scene index argument, so we just
// test the expression-empty cases here.

// ---------------------------------------------------------------------------
// TestPathExpressions — path-only expressions (no predicates)
// ---------------------------------------------------------------------------

/// Exact path match.
#[test]
fn test_path_expression_exact() {
    let eval = HdCollectionExpressionEvaluator::new("/a/b");
    assert!(eval.matches(&path("/a/b")));
    assert!(!eval.matches(&path("/a/b/c")));
    assert!(!eval.matches(&path("/a")));
}

/// Single-level wildcard: `/World/*`.
#[test]
fn test_path_expression_single_wildcard() {
    let eval = HdCollectionExpressionEvaluator::new("/World/*");
    assert!(eval.matches(&path("/World/Mesh")));
    assert!(!eval.matches(&path("/World/Sub/Mesh")));
    assert!(!eval.matches(&path("/Other/Mesh")));
}

/// Recursive wildcard: `/World//*`.
#[test]
fn test_path_expression_recursive_wildcard() {
    let eval = HdCollectionExpressionEvaluator::new("/World//*");
    assert!(eval.matches(&path("/World/Mesh")));
    assert!(eval.matches(&path("/World/Sub/Deep/Mesh")));
    assert!(!eval.matches(&path("/Other/Mesh")));
}

/// Full `//name` semantics — match any descendant whose last component is "b".
///
/// This requires the full SdfPathExpression grammar which the current Rust
/// evaluator does not implement.
#[test]
fn test_path_expression_descendant_named() {
    // C++ scene: /a/b/c/x/y/z/a/b/c  and  /a/b/c/d/e/f/a/b/a/b/c
    // Expression "//b" should match all prims whose last component is "b".
    let eval = HdCollectionExpressionEvaluator::new("//b");

    assert!(eval.matches(&path("/a/b")));
    assert!(eval.matches(&path("/a/b/c/x/y/z/a/b")));
    assert!(eval.matches(&path("/a/b/c/d/e/f/a/b")));
    assert!(eval.matches(&path("/a/b/c/d/e/f/a/b/a/b")));
    // C++ documents this XXX: non-existent prims still match the pattern
    assert!(eval.matches(&path("/PrimDoesNotExist/b")));

    // Should NOT match
    assert!(!eval.matches(&path("/a/b/c")));
    assert!(!eval.matches(&path("/a/b/c/x/y/z/a/b/c")));
}

#[test]
fn test_path_expression_nested_recursive() {
    let eval = HdCollectionExpressionEvaluator::new("//x//a//");

    assert!(eval.matches(&path("/a/b/c/x/y/z/a")));
    assert!(eval.matches(&path("/a/b/c/x/y/z/a/b")));
    assert!(eval.matches(&path("/a/b/c/x/y/z/a/b/c")));
    assert!(eval.matches(&path("/a/b/PrimDoesNotExist/x/y/z/a")));
}

// ---------------------------------------------------------------------------
// TestHdPredicateLibrary — verify registration, not evaluation
// ---------------------------------------------------------------------------

/// Verify the shipped predicate library registers all expected functions.
/// This does NOT test evaluation against live scene data.
#[test]
fn test_predicate_library_standard_registrations() {
    let lib = hd_get_collection_predicate_library();

    assert!(lib.has_function("hdType"), "hdType");
    assert!(lib.has_function("hdVisible"), "hdVisible");
    assert!(lib.has_function("hdPurpose"), "hdPurpose");
    assert!(lib.has_function("hdHasDataSource"), "hdHasDataSource");
    assert!(lib.has_function("hdHasPrimvar"), "hdHasPrimvar");
    assert!(
        lib.has_function("hdHasMaterialBinding"),
        "hdHasMaterialBinding"
    );
    // Deprecated aliases
    assert!(lib.has_function("type"), "deprecated 'type' alias");
}

/// Predicate evaluation against actual scene data requires the evaluator
/// to pass HdSceneIndexPrim to the bound functions. The current evaluator
/// does not support this path. All per-predicate match tests are ignored.

#[test]
fn test_hd_type_scope() {
    // "//{hdType:scope}" → /A, /A/B, /A/C match; /A/B/Carrot does not.
    let scene = create_test_scene();
    let handle = usd_hd::scene_index::base::scene_index_to_handle(scene);
    let eval = HdCollectionExpressionEvaluator::with_scene_default_lib(handle, "//{hdType:scope}");
    assert!(!eval.is_empty());
    assert!(eval.matches(&path("/A")), "/A should match (scope)");
    assert!(eval.matches(&path("/A/B")), "/A/B should match (scope)");
    assert!(eval.matches(&path("/A/C")), "/A/C should match (scope)");
    assert!(
        !eval.matches(&path("/A/B/Carrot")),
        "/A/B/Carrot should NOT match (veg)"
    );
}

#[test]
fn test_hd_type_fruit_deprecated_alias() {
    let scene = create_test_scene();
    let handle = usd_hd::scene_index::base::scene_index_to_handle(scene);
    let eval = HdCollectionExpressionEvaluator::with_scene_default_lib(handle, "//B/{type:fruit}");
    assert!(eval.matches(&path("/A/B/Tomato")));
    assert!(eval.matches(&path("/A/B/Apricot")));
    assert!(!eval.matches(&path("/A/B/Carrot")));
    assert!(!eval.matches(&path("/A/C")));
}

#[test]
fn test_hd_has_data_source_purpose() {
    let scene = create_test_scene();
    let handle = usd_hd::scene_index::base::scene_index_to_handle(scene);
    let eval = HdCollectionExpressionEvaluator::with_scene_default_lib(
        handle,
        "//{hdHasDataSource:purpose}",
    );
    assert!(eval.matches(&path("/A/B/Carrot")));
    assert!(eval.matches(&path("/A/C/Table")));
    assert!(!eval.matches(&path("/A/B")));
    assert!(!eval.matches(&path("/A")));
}

#[test]
fn test_hd_has_data_source_material_bindings_all_purpose() {
    let scene = create_test_scene();
    let handle = usd_hd::scene_index::base::scene_index_to_handle(scene);
    let eval = HdCollectionExpressionEvaluator::with_scene_default_lib(
        handle,
        r#"//{hdHasDataSource:"materialBindings."}"#,
    );
    assert!(eval.matches(&path("/A/B/Carrot")));
    assert!(eval.matches(&path("/A/B/Broccoli")));
    assert!(eval.matches(&path("/A/B/Apricot")));
    assert!(!eval.matches(&path("/A/B/Tomato")));
    assert!(!eval.matches(&path("/A/B")));
    assert!(!eval.matches(&path("/A/C/Chair1")));
}

#[test]
fn test_hd_has_primvar_fresh() {
    let scene = create_test_scene();
    let handle = usd_hd::scene_index::base::scene_index_to_handle(scene);
    let eval =
        HdCollectionExpressionEvaluator::with_scene_default_lib(handle, "//{hdHasPrimvar:fresh}");
    assert!(eval.matches(&path("/A/B/Carrot")));
    assert!(eval.matches(&path("/A/B/Tomato")));
    assert!(!eval.matches(&path("/A/B/Broccoli")));
    assert!(!eval.matches(&path("/A")));
}

#[test]
fn test_hd_has_primvar_foo_glossy_deprecated() {
    let scene = create_test_scene();
    let handle = usd_hd::scene_index::base::scene_index_to_handle(scene);
    let eval = HdCollectionExpressionEvaluator::with_scene_default_lib(
        handle,
        "//{hasPrimvar:'foo:glossy'}",
    );
    assert!(eval.matches(&path("/A/B/Tomato")));
    assert!(eval.matches(&path("/A/C/Chair1")));
    assert!(!eval.matches(&path("/A/B/Broccoli")));
    assert!(!eval.matches(&path("/A")));
}

#[test]
fn test_hd_purpose_food() {
    let scene = create_test_scene();
    let handle = usd_hd::scene_index::base::scene_index_to_handle(scene);
    let eval =
        HdCollectionExpressionEvaluator::with_scene_default_lib(handle, "//{hdPurpose:food}");
    assert!(eval.matches(&path("/A/B/Carrot")));
    assert!(eval.matches(&path("/A/B/Broccoli")));
    assert!(!eval.matches(&path("/A")));
    assert!(!eval.matches(&path("/A/C/Table")));
}

#[test]
fn test_hd_purpose_furniture() {
    let scene = create_test_scene();
    let handle = usd_hd::scene_index::base::scene_index_to_handle(scene);
    let eval =
        HdCollectionExpressionEvaluator::with_scene_default_lib(handle, "//{hdPurpose:furniture}");
    assert!(eval.matches(&path("/A/C/Table")));
    assert!(eval.matches(&path("/A/C/Chair2")));
    assert!(!eval.matches(&path("/A/B/Tomato")));
    assert!(!eval.matches(&path("/A/B/Apricot")));
}

#[test]
fn test_hd_visible_true_explicit() {
    let scene = create_test_scene();
    let handle = usd_hd::scene_index::base::scene_index_to_handle(scene);
    let eval =
        HdCollectionExpressionEvaluator::with_scene_default_lib(handle, "//{hdVisible:true}");
    assert!(eval.matches(&path("/A/B/Carrot")));
    assert!(eval.matches(&path("/A/C/Table")));
    assert!(eval.matches(&path("/A/B/Broccoli")));
    assert!(!eval.matches(&path("/A")));
    assert!(!eval.matches(&path("/A/C/Chair2")));
}

#[test]
fn test_hd_visible_default_arg() {
    let scene = create_test_scene();
    let handle = usd_hd::scene_index::base::scene_index_to_handle(scene);
    let eval = HdCollectionExpressionEvaluator::with_scene_default_lib(handle, "//{hdVisible}");
    assert!(eval.matches(&path("/A/B/Carrot")));
    assert!(eval.matches(&path("/A/C/Table")));
    assert!(!eval.matches(&path("/A")));
    assert!(!eval.matches(&path("/A/C/Chair2")));
}

#[test]
fn test_hd_has_material_binding_orange() {
    let scene = create_test_scene();
    let handle = usd_hd::scene_index::base::scene_index_to_handle(scene);
    let eval = HdCollectionExpressionEvaluator::with_scene_default_lib(
        handle,
        r#"//{hdHasMaterialBinding:"Orange"}"#,
    );
    assert!(eval.matches(&path("/A/B/Carrot")));
    assert!(eval.matches(&path("/A/B/Apricot")));
    assert!(!eval.matches(&path("/A/B/Tomato")));
    assert!(!eval.matches(&path("/A/B")));
    assert!(!eval.matches(&path("/A/C/Chair1")));
}

// ---------------------------------------------------------------------------
// TestCustomPredicateLibrary
// ---------------------------------------------------------------------------

/// Build a custom predicate library extending the standard one with "eatable".
///
/// Matches C++ `_MakeCustomPredicateLibrary`.
fn make_custom_predicate_library() -> HdCollectionPredicateLibrary {
    use usd_sdf::PredicateLibFunctionResult as PredResult;

    // Clone the static base library and add our predicate.
    let lib = hd_get_collection_predicate_library().clone();

    lib.define_binder("eatable", |args| {
        let expect_eatable = args
            .first()
            .and_then(|a| a.value.get::<bool>().copied())
            .unwrap_or(true);
        Some(Arc::new(move |prim: &HdSceneIndexPrim| {
            let is_food = prim.prim_type == "veg" || prim.prim_type == "fruit";
            PredResult::make_varying(is_food == expect_eatable)
        }))
    })
}

/// The custom library registers "eatable" and retains all base predicates.
#[test]
fn test_custom_predicate_library_registration() {
    let lib = make_custom_predicate_library();
    assert!(lib.has_function("eatable"), "custom 'eatable' predicate");
    // Base predicates still present
    assert!(lib.has_function("hdType"));
    assert!(lib.has_function("hdPurpose"));
    assert!(lib.has_function("hdHasPrimvar"));
}

#[test]
fn test_custom_eatable_true() {
    let scene = create_test_scene();
    let handle = usd_hd::scene_index::base::scene_index_to_handle(scene);
    let lib = make_custom_predicate_library();
    let eval = HdCollectionExpressionEvaluator::with_scene(handle, "//{eatable:true}", &lib);
    assert!(eval.matches(&path("/A/B/Tomato")));
    assert!(eval.matches(&path("/A/B/Apricot")));
    assert!(eval.matches(&path("/A/B/Carrot")));
    assert!(!eval.matches(&path("/A/C")));
}

#[test]
fn test_custom_eatable_default_arg() {
    let scene = create_test_scene();
    let handle = usd_hd::scene_index::base::scene_index_to_handle(scene);
    let lib = make_custom_predicate_library();
    let eval = HdCollectionExpressionEvaluator::with_scene(handle, "//{eatable}", &lib);
    assert!(eval.matches(&path("/A/B/Tomato")));
    assert!(eval.matches(&path("/A/B/Apricot")));
    assert!(eval.matches(&path("/A/B/Carrot")));
    assert!(!eval.matches(&path("/A/C")));
}

#[test]
fn test_custom_library_base_predicates_still_work() {
    let scene = create_test_scene();
    let handle = usd_hd::scene_index::base::scene_index_to_handle(scene);
    let lib = make_custom_predicate_library();
    let eval = HdCollectionExpressionEvaluator::with_scene(handle, "//{hdPurpose:furniture}", &lib);
    assert!(eval.matches(&path("/A/C/Table")));
    assert!(eval.matches(&path("/A/C/Chair2")));
    assert!(!eval.matches(&path("/A/B/Tomato")));
    assert!(!eval.matches(&path("/A/B/Apricot")));
}

// ---------------------------------------------------------------------------
// TestEvaluatorUtilities — PopulateAllMatches / PopulateMatches / MatchKind
// ---------------------------------------------------------------------------

#[test]
fn test_populate_all_matches_food_and_fresh() {
    let scene = create_test_scene();
    let handle = usd_hd::scene_index::base::scene_index_to_handle(scene);
    let eval = HdCollectionExpressionEvaluator::with_scene_default_lib(
        handle,
        "//{hdPurpose:food and hdHasPrimvar:fresh}",
    );
    let mut result = Vec::new();
    eval.populate_all_matches(&SdfPath::absolute_root(), &mut result);
    let set: BTreeSet<String> = result.iter().map(|p| p.get_string().to_string()).collect();
    assert!(set.contains("/A/B/Carrot"), "Carrot should match");
    assert!(set.contains("/A/B/Tomato"), "Tomato should match");
    assert_eq!(set.len(), 2, "Only Carrot and Tomato match food+fresh");
}

#[test]
fn test_populate_all_matches_invisible() {
    let scene = create_test_scene();
    let handle = usd_hd::scene_index::base::scene_index_to_handle(scene);
    let eval = HdCollectionExpressionEvaluator::with_scene_default_lib(
        handle,
        "//{hdHasDataSource:visibility and hdVisible:false}",
    );
    let mut result = Vec::new();
    eval.populate_all_matches(&SdfPath::absolute_root(), &mut result);
    let set: BTreeSet<String> = result.iter().map(|p| p.get_string().to_string()).collect();
    assert!(
        set.contains("/A/C/Chair2"),
        "Chair2 should match (invisible)"
    );
    assert_eq!(set.len(), 1, "Only Chair2 is invisible");
}

/// Port of the C++ TestEvaluatorUtilities `*bar` glob test — MatchAll variant.
///
/// Scene: { /a/foobar, /a/foobar/b, /a/foobar/bar, /a/foobar/baz }
/// Expression "//*bar" — any path whose last component ends with "bar".
/// MatchAll → { /a/foobar, /a/foobar/bar }
#[test]
fn test_populate_matches_match_all() {
    let si = HdRetainedSceneIndex::new();
    {
        let mut w = si.write();
        for p in &[
            "/a",
            "/a/foobar",
            "/a/foobar/b",
            "/a/foobar/bar",
            "/a/foobar/baz",
        ] {
            w.add_prims(&[RetainedAddedPrimEntry::new(path(p), tok("test"), None)]);
        }
    }

    let eval = HdCollectionExpressionEvaluator::with_scene(
        si,
        "//*bar",
        usd_hd::collection_predicate_library::hd_get_collection_predicate_library(),
    );

    let mut result = Vec::new();
    eval.populate_matches(&path("/a"), MatchKind::MatchAll, &mut result);

    let result_set: BTreeSet<SdfPath> = result.into_iter().collect();
    assert!(
        result_set.contains(&path("/a/foobar")),
        "must contain /a/foobar"
    );
    assert!(
        result_set.contains(&path("/a/foobar/bar")),
        "must contain /a/foobar/bar"
    );
    assert!(
        !result_set.contains(&path("/a/foobar/b")),
        "must NOT contain /a/foobar/b"
    );
    assert!(
        !result_set.contains(&path("/a/foobar/baz")),
        "must NOT contain /a/foobar/baz"
    );
    assert_eq!(
        result_set.len(),
        2,
        "MatchAll should return exactly 2 paths"
    );
}

/// ShallowestMatches — only the topmost match in each subtree.
///
/// Expression "//*bar" on the same scene → { /a/foobar } only (not /a/foobar/bar
/// because /a/foobar was already matched and its subtree is pruned).
#[test]
fn test_populate_matches_shallowest() {
    let si = HdRetainedSceneIndex::new();
    {
        let mut w = si.write();
        for p in &[
            "/a",
            "/a/foobar",
            "/a/foobar/b",
            "/a/foobar/bar",
            "/a/foobar/baz",
        ] {
            w.add_prims(&[RetainedAddedPrimEntry::new(path(p), tok("test"), None)]);
        }
    }

    let eval = HdCollectionExpressionEvaluator::with_scene(
        si,
        "//*bar",
        usd_hd::collection_predicate_library::hd_get_collection_predicate_library(),
    );

    let mut result = Vec::new();
    eval.populate_matches(&path("/a"), MatchKind::ShallowestMatches, &mut result);

    let result_set: BTreeSet<SdfPath> = result.into_iter().collect();
    assert!(
        result_set.contains(&path("/a/foobar")),
        "must contain /a/foobar"
    );
    assert!(
        !result_set.contains(&path("/a/foobar/bar")),
        "subtree pruned after first match"
    );
    assert_eq!(
        result_set.len(),
        1,
        "ShallowestMatches should return exactly 1 path"
    );
}

/// ShallowestMatchesAndAllDescendants — the shallowest match plus all its descendants.
///
/// Expression "//*bar" → { /a/foobar, /a/foobar/b, /a/foobar/bar, /a/foobar/baz }
#[test]
fn test_populate_matches_shallowest_and_all_descendants() {
    let si = HdRetainedSceneIndex::new();
    {
        let mut w = si.write();
        for p in &[
            "/a",
            "/a/foobar",
            "/a/foobar/b",
            "/a/foobar/bar",
            "/a/foobar/baz",
        ] {
            w.add_prims(&[RetainedAddedPrimEntry::new(path(p), tok("test"), None)]);
        }
    }

    let eval = HdCollectionExpressionEvaluator::with_scene(
        si,
        "//*bar",
        usd_hd::collection_predicate_library::hd_get_collection_predicate_library(),
    );

    let mut result = Vec::new();
    eval.populate_matches(
        &path("/a"),
        MatchKind::ShallowestMatchesAndAllDescendants,
        &mut result,
    );

    let result_set: BTreeSet<SdfPath> = result.into_iter().collect();
    for p in &["/a/foobar", "/a/foobar/b", "/a/foobar/bar", "/a/foobar/baz"] {
        assert!(result_set.contains(&path(p)), "must contain {}", p);
    }
    assert_eq!(
        result_set.len(),
        4,
        "ShallowestMatchesAndAllDescendants should return 4 paths"
    );
}

// ---------------------------------------------------------------------------
// Scene structure — infrastructure validation
// ---------------------------------------------------------------------------

/// Verify the test scene topology matches the C++ _CreateTestScene() spec.
#[test]
fn test_scene_has_expected_prims() {
    let si_arc = create_test_scene();
    let si = si_arc.read();

    // /A must be a child of root
    let root_children = si.get_child_prim_paths(&SdfPath::absolute_root());
    assert!(
        root_children.contains(&path("/A")),
        "root must have /A; got {:?}",
        root_children
    );

    // /A/B children
    let ab: BTreeSet<SdfPath> = si.get_child_prim_paths(&path("/A/B")).into_iter().collect();
    for p in &[
        "/A/B/Carrot",
        "/A/B/Broccoli",
        "/A/B/Tomato",
        "/A/B/Apricot",
    ] {
        assert!(ab.contains(&path(p)), "{} missing from /A/B children", p);
    }

    // /A/C children
    let ac: BTreeSet<SdfPath> = si.get_child_prim_paths(&path("/A/C")).into_iter().collect();
    for p in &["/A/C/Table", "/A/C/Chair1", "/A/C/Chair2"] {
        assert!(ac.contains(&path(p)), "{} missing from /A/C children", p);
    }
}

/// Verify prim types.
#[test]
fn test_scene_prim_types() {
    let si_arc = create_test_scene();
    let si = si_arc.read();

    assert_eq!(si.get_prim(&path("/A")).prim_type.as_str(), "scope");
    assert_eq!(si.get_prim(&path("/A/B/Carrot")).prim_type.as_str(), "veg");
    assert_eq!(
        si.get_prim(&path("/A/B/Tomato")).prim_type.as_str(),
        "fruit"
    );
    assert_eq!(si.get_prim(&path("/A/C/Table")).prim_type.as_str(), "mesh");
    assert_eq!(si.get_prim(&path("/A/C/Chair2")).prim_type.as_str(), "mesh");
}

/// Verify data source structure for Carrot: has visibility, purpose, primvars, materialBindings.
#[test]
fn test_scene_carrot_data_sources() {
    let si_arc = create_test_scene();
    let si = si_arc.read();

    let carrot = si.get_prim(&path("/A/B/Carrot"));
    let ds = carrot.data_source.expect("Carrot must have a data source");

    assert!(
        ds.get(&tok("visibility")).is_some(),
        "Carrot must have visibility"
    );
    assert!(
        ds.get(&tok("purpose")).is_some(),
        "Carrot must have purpose"
    );

    let primvars_ds = ds.get(&tok("primvars")).expect("Carrot must have primvars");
    let primvars_c = cast_to_container(&primvars_ds).expect("primvars must be a container");
    assert!(
        primvars_c.get(&tok("fresh")).is_some(),
        "Carrot must have 'fresh' primvar"
    );

    let bindings_ds = ds
        .get(&tok("materialBindings"))
        .expect("Carrot must have materialBindings");
    let bindings_c = cast_to_container(&bindings_ds).expect("materialBindings must be a container");
    // allPurpose key is the empty string ""
    assert!(
        bindings_c.get(&tok("")).is_some(),
        "Carrot must have allPurpose (\"\") binding"
    );
}

/// Verify Tomato has two primvars and only a preview (not allPurpose) binding.
#[test]
fn test_scene_tomato_data_sources() {
    let si_arc = create_test_scene();
    let si = si_arc.read();

    let tomato = si.get_prim(&path("/A/B/Tomato"));
    let ds = tomato.data_source.expect("Tomato must have a data source");

    let primvars_ds = ds.get(&tok("primvars")).expect("primvars");
    let primvars_c = cast_to_container(&primvars_ds).unwrap();
    assert!(
        primvars_c.get(&tok("fresh")).is_some(),
        "Tomato must have 'fresh'"
    );
    assert!(
        primvars_c.get(&tok("foo:glossy")).is_some(),
        "Tomato must have 'foo:glossy'"
    );

    let bindings_ds = ds.get(&tok("materialBindings")).unwrap();
    let bindings_c = cast_to_container(&bindings_ds).unwrap();
    // Has "preview" binding
    assert!(
        bindings_c.get(&tok("preview")).is_some(),
        "Tomato must have 'preview' binding"
    );
    // Does NOT have allPurpose binding (empty string key)
    assert!(
        bindings_c.get(&tok("")).is_none(),
        "Tomato must NOT have allPurpose binding"
    );
}

/// Chair2 is invisible — verify the stored bool is false.
#[test]
fn test_scene_chair2_invisible() {
    let si_arc = create_test_scene();
    let si = si_arc.read();

    let chair2 = si.get_prim(&path("/A/C/Chair2"));
    let ds = chair2.data_source.expect("Chair2 must have a data source");

    let vis_ds = ds
        .get(&tok("visibility"))
        .expect("Chair2 must have visibility");
    let vis_c = cast_to_container(&vis_ds).expect("visibility must be a container");
    let inner = vis_c
        .get(&tok("visibility"))
        .expect("inner visibility bool must exist");
    let sampled = inner.as_sampled().expect("must be sampled");
    let val = sampled.get_value(0.0);
    let visible = val.get::<bool>().copied().expect("value must be bool");
    assert!(!visible, "Chair2 must be invisible");
}

/// /A, /A/B, /A/C are scope prims with no data source (None) in the test scene.
#[test]
fn test_scene_scope_prims_have_no_data_source() {
    let si_arc = create_test_scene();
    let si = si_arc.read();

    // Scope prims were added with None data source.
    // HdRetainedSceneIndex wraps that in a new_empty() container on get_prim(),
    // so is_defined() returns true but data_source is an empty container.
    // The important thing: get_prim() does not panic.
    let a = si.get_prim(&path("/A"));
    assert_eq!(a.prim_type.as_str(), "scope");

    let ab = si.get_prim(&path("/A/B"));
    assert_eq!(ab.prim_type.as_str(), "scope");
}
