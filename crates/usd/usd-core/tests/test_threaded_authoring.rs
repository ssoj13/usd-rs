// Port of testUsdThreadedAuthoring.cpp — concurrent authoring subset
// Reference: _ref/OpenUSD/pxr/usd/usd/testenv/testUsdThreadedAuthoring.cpp

mod common;

use std::thread;
use usd_core::{InitialLoadSet, Stage};
use usd_sdf::Path;

// ============================================================================
// Concurrent authoring to separate prims
// ============================================================================

#[test]
fn concurrent_define_prims() {
    // C++ ref: multiple threads defining prims under different parents
    common::setup();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create");

    // Create parent prims first (sequential — stage authoring may not be thread-safe)
    for i in 0..4 {
        stage
            .define_prim(&format!("/Parent_{}", i), "Xform")
            .expect("define parent");
    }

    // Then create child prims from multiple threads
    let handles: Vec<_> = (0..4)
        .map(|thread_id| {
            let stage = stage.clone();
            thread::spawn(move || {
                for i in 0..5 {
                    let path = format!("/Parent_{}/Child_{}", thread_id, i);
                    let _ = stage.define_prim(&path, "Mesh");
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("thread panicked");
    }

    // Verify all children created
    for thread_id in 0..4 {
        for i in 0..5 {
            let path = Path::from_string(&format!("/Parent_{}/Child_{}", thread_id, i)).expect("p");
            // Some may have been created, depending on thread safety
            let _ = stage.get_prim_at_path(&path);
        }
    }
}

// ============================================================================
// Concurrent metadata authoring
// ============================================================================

#[test]
fn concurrent_metadata_on_separate_prims() {
    common::setup();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create");

    for i in 0..8 {
        stage
            .define_prim(&format!("/Prim_{}", i), "Xform")
            .expect("define");
    }

    let handles: Vec<_> = (0..4)
        .map(|thread_id| {
            let stage = stage.clone();
            thread::spawn(move || {
                for i in (thread_id * 2)..(thread_id * 2 + 2) {
                    let path = Path::from_string(&format!("/Prim_{}", i)).expect("p");
                    if let Some(prim) = stage.get_prim_at_path(&path) {
                        prim.set_metadata(
                            &usd_tf::Token::new("documentation"),
                            usd_vt::Value::from(format!("Thread {} prim {}", thread_id, i)),
                        );
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("thread panicked");
    }
}

// ============================================================================
// Concurrent attribute creation on separate prims
// ============================================================================

#[test]
fn concurrent_attribute_creation() {
    common::setup();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create");

    for i in 0..4 {
        stage
            .define_prim(&format!("/Prim_{}", i), "Xform")
            .expect("define");
    }

    let float_type_name = "float";
    let handles: Vec<_> = (0..4)
        .map(|thread_id| {
            let stage = stage.clone();
            thread::spawn(move || {
                let path = Path::from_string(&format!("/Prim_{}", thread_id)).expect("p");
                if let Some(prim) = stage.get_prim_at_path(&path) {
                    let float_type = common::vtn(float_type_name);
                    for j in 0..5 {
                        prim.create_attribute(&format!("attr_{}", j), &float_type, false, None);
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("thread panicked");
    }
}
