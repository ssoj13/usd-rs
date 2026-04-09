//! Tests for UsdStageCache.
//!
//! Ported from:
//!   - pxr/usd/usd/testenv/testUsdStageCache.py (core subset)

mod common;

use std::sync::Arc;
use usd_ar::ResolverContext;
use usd_core::{InitialLoadSet, Stage, StageCache};
use usd_sdf::{Layer, LayerHandle};

// ============================================================================
// Insert / Find / Erase
// ============================================================================

#[test]
fn cache_insert_find_erase() {
    common::setup();

    let cache = StageCache::new();

    let layer = Layer::create_anonymous(Some("test.usda"));
    let stage = Stage::open_with_root_layer(Arc::clone(&layer), InitialLoadSet::LoadAll)
        .expect("open stage");

    // Insert
    let id = cache.insert(Arc::clone(&stage));
    assert!(id.is_valid(), "inserted ID should be valid");

    // Find by ID
    let found = cache.find(id);
    assert!(found.is_some(), "should find stage by ID");
    assert!(Arc::ptr_eq(&found.unwrap(), &stage));

    // Erase by ID
    assert!(cache.erase(id), "erase should succeed");

    // Should no longer be found
    let not_found = cache.find(id);
    assert!(not_found.is_none(), "should not find erased stage");
}

// ============================================================================
// FindOneMatching / FindAllMatching
// ============================================================================

#[test]
fn cache_find_matching() {
    common::setup();

    let cache = StageCache::new();

    let layer1 = Layer::create_anonymous(Some("a.usda"));
    let layer2 = Layer::create_anonymous(Some("b.usda"));

    let stage1 = Stage::open_with_root_layer(Arc::clone(&layer1), InitialLoadSet::LoadAll)
        .expect("open stage1");
    let stage2 = Stage::open_with_root_layer(Arc::clone(&layer2), InitialLoadSet::LoadAll)
        .expect("open stage2");

    cache.insert(Arc::clone(&stage1));
    cache.insert(Arc::clone(&stage2));

    // FindOneMatching by root layer
    let handle1 = LayerHandle::from_layer(&layer1);
    let found = cache.find_one_matching(&handle1);
    assert!(found.is_some(), "should find stage by root layer");
    assert!(Arc::ptr_eq(&found.unwrap(), &stage1));

    // FindAllMatching
    let all1 = cache.find_all_matching(&handle1);
    assert_eq!(all1.len(), 1, "should find exactly 1 matching");

    // Non-matching layer
    let other_layer = Layer::create_anonymous(Some("c.usda"));
    let other_handle = LayerHandle::from_layer(&other_layer);
    let none = cache.find_one_matching(&other_handle);
    assert!(none.is_none(), "should not find non-inserted layer");
}

// ============================================================================
// GetAllStages
// ============================================================================

#[test]
fn cache_get_all_stages() {
    common::setup();

    let cache = StageCache::new();

    assert!(cache.get_all_stages().is_empty(), "cache starts empty");

    let layer1 = Layer::create_anonymous(Some("x.usda"));
    let layer2 = Layer::create_anonymous(Some("y.usda"));
    let stage1 =
        Stage::open_with_root_layer(Arc::clone(&layer1), InitialLoadSet::LoadAll).expect("open");
    let stage2 =
        Stage::open_with_root_layer(Arc::clone(&layer2), InitialLoadSet::LoadAll).expect("open");

    cache.insert(Arc::clone(&stage1));
    cache.insert(Arc::clone(&stage2));

    let all = cache.get_all_stages();
    assert_eq!(all.len(), 2, "should have 2 stages");
}

// ============================================================================
// Clear
// ============================================================================

#[test]
fn cache_clear() {
    common::setup();

    let cache = StageCache::new();

    let layer = Layer::create_anonymous(Some("z.usda"));
    let stage =
        Stage::open_with_root_layer(Arc::clone(&layer), InitialLoadSet::LoadAll).expect("open");
    let id = cache.insert(Arc::clone(&stage));

    assert!(!cache.get_all_stages().is_empty());

    cache.clear();

    assert!(
        cache.get_all_stages().is_empty(),
        "cache should be empty after clear"
    );
    assert!(cache.find(id).is_none(), "should not find after clear");
}

// ============================================================================
// EraseStage / EraseAll
// ============================================================================

#[test]
fn cache_erase_stage() {
    common::setup();

    let cache = StageCache::new();

    let layer = Layer::create_anonymous(Some("w.usda"));
    let stage =
        Stage::open_with_root_layer(Arc::clone(&layer), InitialLoadSet::LoadAll).expect("open");
    cache.insert(Arc::clone(&stage));

    assert!(cache.erase_stage(&stage), "erase_stage should succeed");
    assert!(cache.get_all_stages().is_empty());
}

#[test]
fn cache_erase_all_matching() {
    common::setup();

    let cache = StageCache::new();

    let layer = Layer::create_anonymous(Some("multi.usda"));
    let stage1 =
        Stage::open_with_root_layer(Arc::clone(&layer), InitialLoadSet::LoadAll).expect("open");
    let stage2 =
        Stage::open_with_root_layer(Arc::clone(&layer), InitialLoadSet::LoadAll).expect("open");

    cache.insert(Arc::clone(&stage1));
    cache.insert(Arc::clone(&stage2));

    let handle = LayerHandle::from_layer(&layer);
    let erased = cache.erase_all(&handle);
    assert_eq!(erased, 2, "should erase both stages");
    assert!(cache.get_all_stages().is_empty());
}

// ============================================================================
// Duplicate insert
// ============================================================================

#[test]
fn cache_duplicate_insert() {
    common::setup();

    let cache = StageCache::new();

    let layer = Layer::create_anonymous(Some("dup.usda"));
    let stage =
        Stage::open_with_root_layer(Arc::clone(&layer), InitialLoadSet::LoadAll).expect("open");

    let id1 = cache.insert(Arc::clone(&stage));
    let id2 = cache.insert(Arc::clone(&stage));

    // Same stage inserted twice should return same ID
    assert_eq!(id1, id2, "duplicate insert should return same ID");
    assert_eq!(cache.get_all_stages().len(), 1, "should still have 1 stage");
}

/// Stages from `CreateInMemory` store no path resolver (`None`); an empty `ResolverContext`
/// query must still find them (matches `testUsdStageCache` / OpenUSD semantics).
#[test]
fn cache_find_matching_empty_resolver_query_matches_none_storage() {
    common::setup();

    let cache = StageCache::new();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create");
    let root = stage.get_root_layer();
    let handle = LayerHandle::from_layer(&root);
    cache.insert(Arc::clone(&stage));

    let empty = ResolverContext::new();
    assert!(empty.is_empty());
    let found = cache.find_all_matching_with_resolver(&handle, &empty);
    assert_eq!(found.len(), 1);
    assert!(Arc::ptr_eq(&found[0], &stage));
}
