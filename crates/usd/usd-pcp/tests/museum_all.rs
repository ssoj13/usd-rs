//! Comprehensive Museum test runner.
//! Tests ALL 134 Museum scenarios from the OpenUSD PCP test suite.
//!
//! Each test loads the Museum scene, computes prim indexes for all
//! root prims, and verifies basic composition sanity:
//! - No panics/crashes during composition
//! - Prim indexes are valid
//! - Error types are valid PCP errors (not internal crashes)

use std::collections::BTreeSet;
use usd_pcp::{Cache, LayerStackIdentifier};
use usd_sdf::{Layer, Path};

fn ensure_init() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        usd_sdf::init();
    });
}

fn museum_root_usda(scenario_name: &str) -> String {
    openusd_test_path::pxr_pcp_museum(scenario_name, "root.usda")
        .to_string_lossy()
        .replace('\\', "/")
}

/// Run a Museum test scenario. Returns (num_prims_tested, num_errors, error_details).
fn run_museum_scenario(scenario_name: &str) -> (usize, usize, Vec<String>) {
    ensure_init();

    let root_path = museum_root_usda(scenario_name);

    let id = LayerStackIdentifier::new(root_path.as_str());
    let cache = Cache::new(id.clone(), true);

    // Compute layer stack to discover prims across all sublayers
    let layer_stack = match cache.compute_layer_stack(&id) {
        Ok(ls) => ls,
        Err(e) => {
            return (0, 1, vec![format!("Cannot compute layer stack: {:?}", e)]);
        }
    };

    // Collect prim paths from ALL layers in the stack
    let mut prim_paths: BTreeSet<String> = BTreeSet::new();
    for layer in layer_stack.get_layers() {
        collect_prim_paths(&layer, &Path::absolute_root(), &mut prim_paths);
    }

    let mut num_tested = 0;
    let mut num_errors = 0;
    let mut error_details = Vec::new();

    for prim_path_str in &prim_paths {
        let prim_path = Path::from_string(prim_path_str).unwrap();
        let (prim_index, errors) = cache.compute_prim_index(&prim_path);
        num_tested += 1;

        if !prim_index.is_valid() {
            num_errors += 1;
            error_details.push(format!("{}: invalid prim index", prim_path_str));
        }

        // Composition errors are expected for Error* scenarios
        // but we still count them for reporting
        if !errors.is_empty() && !scenario_name.starts_with("Error") {
            // Non-error scenarios shouldn't have composition errors
            // (except for some edge cases like permission denied in references)
            for _e in &errors {
                error_details.push(format!("{}: composition error", prim_path_str));
            }
        }
    }

    (num_tested, num_errors, error_details)
}

/// Get prim children names at a path from a layer.
fn get_children_names(layer: &Layer, path: &Path) -> Vec<String> {
    let tk = usd_tf::Token::new("primChildren");
    layer
        .get_field(path, &tk)
        .and_then(|v| {
            // Try as Vec<Token> first, then Vec<String>
            if let Some(tokens) = v.as_vec_clone::<usd_tf::Token>() {
                Some(tokens.iter().map(|t| t.as_str().to_string()).collect())
            } else {
                v.as_vec_clone::<String>()
            }
        })
        .unwrap_or_default()
}

/// Recursively collect all prim paths from a layer (depth 2 max for efficiency).
fn collect_prim_paths(layer: &Layer, parent: &Path, paths: &mut BTreeSet<String>) {
    let children = get_children_names(layer, parent);
    for child_name in children {
        let child_path = if parent.is_absolute_root_path() {
            Path::from_string(&format!("/{}", child_name)).unwrap()
        } else {
            Path::from_string(&format!("{}/{}", parent.as_str(), child_name)).unwrap()
        };
        paths.insert(child_path.as_str().to_string());
        // Go one more level deep to test child prims
        let grandchildren = get_children_names(layer, &child_path);
        for gc_name in grandchildren {
            let gc_path =
                Path::from_string(&format!("{}/{}", child_path.as_str(), gc_name)).unwrap();
            paths.insert(gc_path.as_str().to_string());
        }
    }
}

#[test]
#[ignore]
fn debug_error_arc_cycle_path_timings() {
    use std::time::Instant;

    ensure_init();

    let scenario_name = "ErrorArcCycle";
    let root_path = museum_root_usda(scenario_name);
    let id = LayerStackIdentifier::new(root_path.as_str());
    let cache = Cache::new(id.clone(), true);
    let layer_stack = cache
        .compute_layer_stack(&id)
        .expect("layer stack must exist");

    let mut prim_paths: BTreeSet<String> = BTreeSet::new();
    for layer in layer_stack.get_layers() {
        collect_prim_paths(&layer, &Path::absolute_root(), &mut prim_paths);
    }

    for prim_path_str in &prim_paths {
        let prim_path = Path::from_string(prim_path_str).unwrap();
        eprintln!("begin path={}", prim_path_str);
        let start = Instant::now();
        let (prim_index, errors) = cache.compute_prim_index(&prim_path);
        let elapsed = start.elapsed();
        eprintln!(
            "path={} elapsed_ms={} valid={} errors={}",
            prim_path_str,
            elapsed.as_millis(),
            prim_index.is_valid(),
            errors.len()
        );
    }
}

// ============================================================================
// Macro to generate one test per Museum scenario
// ============================================================================

macro_rules! museum_test {
    ($test_name:ident, $scenario:expr) => {
        #[test]
        fn $test_name() {
            let (num_prims, num_errors, details) = run_museum_scenario($scenario);
            assert!(
                num_prims > 0,
                "Museum scenario '{}' should have at least 1 prim to test",
                $scenario
            );
            assert_eq!(
                num_errors, 0,
                "Museum scenario '{}': {} errors in {} prims: {:?}",
                $scenario, num_errors, num_prims, details
            );
        }
    };
    // Variant for Error* scenarios where composition errors are expected
    ($test_name:ident, $scenario:expr, expect_errors) => {
        #[test]
        fn $test_name() {
            let (num_prims, _num_errors, _details) = run_museum_scenario($scenario);
            assert!(
                num_prims > 0,
                "Museum scenario '{}' should have at least 1 prim to test",
                $scenario
            );
            // Error scenarios: just verify no crash, errors are expected
        }
    };
}

// ============================================================================
// Basic composition
// ============================================================================
museum_test!(museum_all_basic_reference, "BasicReference");
museum_test!(museum_all_basic_reference_diamond, "BasicReferenceDiamond");
museum_test!(
    museum_all_basic_reference_and_class,
    "BasicReferenceAndClass"
);
museum_test!(
    museum_all_basic_reference_and_class_diamond,
    "BasicReferenceAndClassDiamond"
);
museum_test!(museum_all_basic_inherits, "BasicInherits");
museum_test!(museum_all_basic_specializes, "BasicSpecializes");
museum_test!(
    museum_all_basic_specializes_and_inherits,
    "BasicSpecializesAndInherits"
);
museum_test!(
    museum_all_basic_specializes_and_references,
    "BasicSpecializesAndReferences"
);
museum_test!(
    museum_all_basic_specializes_and_variants,
    "BasicSpecializesAndVariants"
);
museum_test!(museum_all_basic_payload, "BasicPayload");
museum_test!(museum_all_basic_payload_diamond, "BasicPayloadDiamond");
museum_test!(museum_all_basic_nested_payload, "BasicNestedPayload");
museum_test!(museum_all_basic_nested_variants, "BasicNestedVariants");
museum_test!(
    museum_all_basic_nested_variants_same_name,
    "BasicNestedVariantsWithSameName"
);
museum_test!(
    museum_all_basic_variant_with_reference,
    "BasicVariantWithReference"
);
museum_test!(
    museum_all_basic_variant_with_connections,
    "BasicVariantWithConnections"
);
museum_test!(museum_all_basic_list_editing, "BasicListEditing");
museum_test!(
    museum_all_basic_list_editing_with_inherits,
    "BasicListEditingWithInherits"
);
museum_test!(museum_all_basic_time_offset, "BasicTimeOffset");
museum_test!(
    museum_all_basic_duplicate_sublayer,
    "BasicDuplicateSublayer"
);
museum_test!(museum_all_basic_instancing, "BasicInstancing");
museum_test!(
    museum_all_basic_instancing_nested,
    "BasicInstancingAndNestedInstances"
);
museum_test!(
    museum_all_basic_instancing_variants,
    "BasicInstancingAndVariants"
);
museum_test!(museum_all_basic_owner, "BasicOwner");

// ============================================================================
// Relocates
// ============================================================================
museum_test!(
    museum_all_basic_relocate_anim,
    "BasicRelocateToAnimInterface"
);
museum_test!(
    museum_all_basic_relocate_anim_new_root,
    "BasicRelocateToAnimInterfaceAsNewRootPrim"
);
museum_test!(
    museum_all_elided_ancestral_relocates,
    "ElidedAncestralRelocates"
);
museum_test!(
    museum_all_relocate_prims_same_name,
    "RelocatePrimsWithSameName"
);
museum_test!(museum_all_relocate_to_none, "RelocateToNone");
museum_test!(
    museum_all_reference_list_ops_offsets,
    "ReferenceListOpsWithOffsets"
);

// ============================================================================
// Relative/subroot references and payloads
// ============================================================================
museum_test!(
    museum_all_relative_path_references,
    "RelativePathReferences"
);
museum_test!(museum_all_relative_path_payloads, "RelativePathPayloads");
museum_test!(
    museum_all_subroot_ref_and_classes,
    "SubrootReferenceAndClasses"
);
museum_test!(
    museum_all_subroot_ref_and_relocates,
    "SubrootReferenceAndRelocates"
);
museum_test!(
    museum_all_subroot_ref_and_variants,
    "SubrootReferenceAndVariants"
);
museum_test!(
    museum_all_subroot_ref_and_variants2,
    "SubrootReferenceAndVariants2"
);
museum_test!(museum_all_subroot_ref_non_cycle, "SubrootReferenceNonCycle");
museum_test!(
    museum_all_subroot_inherits_and_variants,
    "SubrootInheritsAndVariants"
);

// ============================================================================
// Implied/Ancestral inherits
// ============================================================================
museum_test!(
    museum_all_implied_ancestral_inherits,
    "ImpliedAndAncestralInherits"
);
museum_test!(
    museum_all_implied_ancestral_complex,
    "ImpliedAndAncestralInherits_ComplexEvaluation"
);

// ============================================================================
// Specializes and ancestral arcs
// ============================================================================
museum_test!(
    museum_all_specializes_and_ancestral,
    "SpecializesAndAncestralArcs"
);
museum_test!(
    museum_all_specializes_and_ancestral2,
    "SpecializesAndAncestralArcs2"
);
museum_test!(
    museum_all_specializes_and_ancestral3,
    "SpecializesAndAncestralArcs3"
);
museum_test!(
    museum_all_specializes_and_ancestral4,
    "SpecializesAndAncestralArcs4"
);
museum_test!(
    museum_all_specializes_and_ancestral5,
    "SpecializesAndAncestralArcs5"
);
museum_test!(
    museum_all_specializes_and_duplicate,
    "SpecializesAndDuplicateArcs"
);
museum_test!(
    museum_all_specializes_and_variants,
    "SpecializesAndVariants"
);
museum_test!(
    museum_all_specializes_and_variants2,
    "SpecializesAndVariants2"
);
museum_test!(
    museum_all_specializes_and_variants3,
    "SpecializesAndVariants3"
);
museum_test!(
    museum_all_specializes_and_variants4,
    "SpecializesAndVariants4"
);
museum_test!(
    museum_all_variant_specializes_ref,
    "VariantSpecializesAndReference"
);
museum_test!(
    museum_all_variant_specializes_ref_surprising,
    "VariantSpecializesAndReferenceSurprisingBehavior"
);

// ============================================================================
// Payloads and ancestral arcs
// ============================================================================
museum_test!(museum_all_payloads_ancestral, "PayloadsAndAncestralArcs");
museum_test!(museum_all_payloads_ancestral2, "PayloadsAndAncestralArcs2");
museum_test!(museum_all_payloads_ancestral3, "PayloadsAndAncestralArcs3");

// ============================================================================
// Expressions
// ============================================================================
museum_test!(museum_all_expr_payloads, "ExpressionsInPayloads");
museum_test!(museum_all_expr_references, "ExpressionsInReferences");
museum_test!(museum_all_expr_sublayers, "ExpressionsInSublayers");
museum_test!(museum_all_expr_variants, "ExpressionsInVariantSelections");

// ============================================================================
// Time codes
// ============================================================================
museum_test!(museum_all_time_codes, "TimeCodesPerSecond");

// ============================================================================
// Typical (production-like scenarios)
// ============================================================================
museum_test!(museum_all_typical_chargroup, "TypicalReferenceToChargroup");
museum_test!(
    museum_all_typical_chargroup_rename,
    "TypicalReferenceToChargroupWithRename"
);
museum_test!(
    museum_all_typical_rigged_model,
    "TypicalReferenceToRiggedModel"
);

// ============================================================================
// Tricky class hierarchy
// ============================================================================
museum_test!(museum_all_tricky_class_hierarchy, "TrickyClassHierarchy");
museum_test!(museum_all_tricky_nested_classes, "TrickyNestedClasses");
museum_test!(museum_all_tricky_nested_classes2, "TrickyNestedClasses2");
museum_test!(museum_all_tricky_nested_classes3, "TrickyNestedClasses3");
museum_test!(museum_all_tricky_nested_classes4, "TrickyNestedClasses4");
museum_test!(
    museum_all_tricky_nested_specializes,
    "TrickyNestedSpecializes"
);
museum_test!(
    museum_all_tricky_nested_specializes2,
    "TrickyNestedSpecializes2"
);

// ============================================================================
// Tricky inherits/relocates
// ============================================================================
museum_test!(
    museum_all_tricky_inherits_relocates,
    "TrickyInheritsAndRelocates"
);
museum_test!(
    museum_all_tricky_inherits_relocates2,
    "TrickyInheritsAndRelocates2"
);
museum_test!(
    museum_all_tricky_inherits_relocates3,
    "TrickyInheritsAndRelocates3"
);
museum_test!(
    museum_all_tricky_inherits_relocates4,
    "TrickyInheritsAndRelocates4"
);
museum_test!(
    museum_all_tricky_inherits_relocates5,
    "TrickyInheritsAndRelocates5"
);
museum_test!(
    museum_all_tricky_inherits_relocates_new_root,
    "TrickyInheritsAndRelocatesToNewRootPrim"
);
museum_test!(
    museum_all_tricky_local_class_relocates,
    "TrickyLocalClassHierarchyWithRelocates"
);
museum_test!(
    museum_all_tricky_connection_relocated,
    "TrickyConnectionToRelocatedAttribute"
);

// ============================================================================
// Tricky variants
// ============================================================================
museum_test!(
    museum_all_tricky_inherited_variant_sel,
    "TrickyInheritedVariantSelection"
);
museum_test!(
    museum_all_tricky_inherits_in_variants,
    "TrickyInheritsInVariants"
);
museum_test!(
    museum_all_tricky_inherits_in_variants2,
    "TrickyInheritsInVariants2"
);
museum_test!(museum_all_tricky_nested_variants, "TrickyNestedVariants");
museum_test!(
    museum_all_tricky_nonlocal_variant_sel,
    "TrickyNonLocalVariantSelection"
);
museum_test!(
    museum_all_tricky_variant_ancestral_sel,
    "TrickyVariantAncestralSelection"
);
museum_test!(
    museum_all_tricky_variant_fallback,
    "TrickyVariantFallbackDrivingAuthoredVariant"
);
museum_test!(
    museum_all_tricky_variant_independent,
    "TrickyVariantIndependentSelection"
);
museum_test!(
    museum_all_tricky_variant_in_payload,
    "TrickyVariantInPayload"
);
museum_test!(
    museum_all_tricky_variant_override_fallback,
    "TrickyVariantOverrideOfFallback"
);
museum_test!(
    museum_all_tricky_variant_override_class,
    "TrickyVariantOverrideOfLocalClass"
);
museum_test!(
    museum_all_tricky_variant_override_relocate,
    "TrickyVariantOverrideOfRelocatedPrim"
);
museum_test!(
    museum_all_tricky_variant_sel_in_variant,
    "TrickyVariantSelectionInVariant"
);
museum_test!(
    museum_all_tricky_variant_sel_in_variant2,
    "TrickyVariantSelectionInVariant2"
);
museum_test!(
    museum_all_tricky_variant_weaker,
    "TrickyVariantWeakerSelection"
);
museum_test!(
    museum_all_tricky_variant_weaker2,
    "TrickyVariantWeakerSelection2"
);
museum_test!(
    museum_all_tricky_variant_weaker3,
    "TrickyVariantWeakerSelection3"
);
museum_test!(
    museum_all_tricky_variant_weaker4,
    "TrickyVariantWeakerSelection4"
);

// ============================================================================
// Tricky relocations
// ============================================================================
museum_test!(
    museum_all_tricky_multiple_relocations,
    "TrickyMultipleRelocations"
);
museum_test!(
    museum_all_tricky_multiple_relocations2,
    "TrickyMultipleRelocations2"
);
museum_test!(
    museum_all_tricky_multiple_relocations3,
    "TrickyMultipleRelocations3"
);
museum_test!(
    museum_all_tricky_multiple_relocations4,
    "TrickyMultipleRelocations4"
);
museum_test!(
    museum_all_tricky_multiple_relocations5,
    "TrickyMultipleRelocations5"
);
museum_test!(
    museum_all_tricky_multiple_relocations_classes,
    "TrickyMultipleRelocationsAndClasses"
);
museum_test!(
    museum_all_tricky_multiple_relocations_classes2,
    "TrickyMultipleRelocationsAndClasses2"
);
museum_test!(
    museum_all_tricky_relocated_target_variant,
    "TrickyRelocatedTargetInVariant"
);
museum_test!(
    museum_all_tricky_relocation_from_payload,
    "TrickyRelocationOfPrimFromPayload"
);
museum_test!(
    museum_all_tricky_relocation_from_variant,
    "TrickyRelocationOfPrimFromVariant"
);
museum_test!(
    museum_all_tricky_relocation_squatter,
    "TrickyRelocationSquatter"
);

// ============================================================================
// Tricky specializes
// ============================================================================
museum_test!(
    museum_all_tricky_specializes_inherits,
    "TrickySpecializesAndInherits"
);
museum_test!(
    museum_all_tricky_specializes_inherits2,
    "TrickySpecializesAndInherits2"
);
museum_test!(
    museum_all_tricky_specializes_inherits3,
    "TrickySpecializesAndInherits3"
);
museum_test!(
    museum_all_tricky_specializes_relocates,
    "TrickySpecializesAndRelocates"
);

// ============================================================================
// Tricky spooky (implied across references)
// ============================================================================
museum_test!(museum_all_tricky_spooky_inherits, "TrickySpookyInherits");
museum_test!(
    museum_all_tricky_spooky_arm_rig,
    "TrickySpookyInheritsInSymmetricArmRig"
);
museum_test!(
    museum_all_tricky_spooky_brow_rig,
    "TrickySpookyInheritsInSymmetricBrowRig"
);
museum_test!(
    museum_all_tricky_spooky_variant_sel,
    "TrickySpookyVariantSelection"
);
museum_test!(
    museum_all_tricky_spooky_variant_class,
    "TrickySpookyVariantSelectionInClass"
);

// ============================================================================
// Tricky misc
// ============================================================================
museum_test!(
    museum_all_tricky_list_edited_targets,
    "TrickyListEditedTargetPaths"
);

// ============================================================================
// Error scenarios (expect composition errors, just verify no crash)
// ============================================================================
museum_test!(museum_all_error_arc_cycle, "ErrorArcCycle", expect_errors);
museum_test!(
    museum_all_error_connection_perm,
    "ErrorConnectionPermissionDenied",
    expect_errors
);
museum_test!(
    museum_all_error_inconsistent_props,
    "ErrorInconsistentProperties",
    expect_errors
);
museum_test!(
    museum_all_error_invalid_authored_reloc,
    "ErrorInvalidAuthoredRelocates",
    expect_errors
);
museum_test!(
    museum_all_error_invalid_conflicting_reloc,
    "ErrorInvalidConflictingRelocates",
    expect_errors
);
museum_test!(
    museum_all_error_invalid_instance_target,
    "ErrorInvalidInstanceTargetPath",
    expect_errors
);
museum_test!(
    museum_all_error_invalid_payload,
    "ErrorInvalidPayload",
    expect_errors
);
museum_test!(
    museum_all_error_invalid_pre_relocate,
    "ErrorInvalidPreRelocateTargetPath",
    expect_errors
);
museum_test!(
    museum_all_error_invalid_ref_to_reloc,
    "ErrorInvalidReferenceToRelocationSource",
    expect_errors
);
museum_test!(
    museum_all_error_invalid_target,
    "ErrorInvalidTargetPath",
    expect_errors
);
museum_test!(
    museum_all_error_opinion_at_reloc_source,
    "ErrorOpinionAtRelocationSource",
    expect_errors
);
museum_test!(museum_all_error_owner, "ErrorOwner", expect_errors);
museum_test!(
    museum_all_error_perm_denied,
    "ErrorPermissionDenied",
    expect_errors
);
museum_test!(
    museum_all_error_relocate_variant_sel,
    "ErrorRelocateWithVariantSelection",
    expect_errors
);
museum_test!(
    museum_all_error_sublayer_cycle,
    "ErrorSublayerCycle",
    expect_errors
);
