// Port of testUsdStageThreading.cpp — threading safety subset
// Reference: _ref/OpenUSD/pxr/usd/usd/testenv/testUsdStageThreading.cpp

mod common;

use std::sync::Arc;
use std::thread;
use usd_core::Stage;
use usd_core::common::InitialLoadSet;
use usd_sdf::Path;

fn setup_stage() -> Arc<Stage> {
    common::setup();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    for i in 0..20 {
        stage
            .define_prim(&format!("/Prim_{}", i), "Xform")
            .expect("define prim");
    }
    stage
}

// ============================================================================
// Concurrent read access
// ============================================================================

#[test]
fn concurrent_prim_reads() {
    // C++ ref: testUsdStageThreading — concurrent reading from multiple threads
    let stage = setup_stage();

    let handles: Vec<_> = (0..4)
        .map(|thread_id| {
            let stage = stage.clone();
            thread::spawn(move || {
                for i in 0..20 {
                    let path = Path::from_string(&format!("/Prim_{}", i)).expect("path");
                    let prim = stage.get_prim_at_path(&path);
                    assert!(prim.is_some(), "thread {} prim {} not found", thread_id, i);
                    let prim = prim.unwrap();
                    assert!(prim.is_valid());
                    let _ = prim.get_type_name();
                    let _ = prim.path();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("thread panicked");
    }
}

#[test]
fn concurrent_traverse() {
    // Multiple threads traversing simultaneously
    let stage = setup_stage();

    let handles: Vec<_> = (0..4)
        .map(|_| {
            let stage = stage.clone();
            thread::spawn(move || {
                let prims: Vec<_> = stage.traverse().into_iter().collect();
                assert_eq!(prims.len(), 20);
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("thread panicked");
    }
}

// ============================================================================
// Concurrent attribute reads
// ============================================================================

#[test]
fn concurrent_attribute_reads() {
    common::setup();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create");
    let prim = stage.define_prim("/Root", "Xform").expect("define");

    let float_type = common::vtn("float");
    for i in 0..10 {
        prim.create_attribute(&format!("attr_{}", i), &float_type, false, None);
    }

    let handles: Vec<_> = (0..4)
        .map(|_| {
            let stage = stage.clone();
            thread::spawn(move || {
                let path = Path::from_string("/Root").expect("p");
                let prim = stage.get_prim_at_path(&path).expect("prim");
                let names = prim.get_attribute_names();
                assert_eq!(names.len(), 10);
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("thread panicked");
    }
}

// ============================================================================
// Stage open from multiple threads
// ============================================================================

#[test]
fn concurrent_stage_creation() {
    common::setup();

    let handles: Vec<_> = (0..4)
        .map(|i| {
            thread::spawn(move || {
                let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create");
                stage
                    .define_prim(&format!("/ThreadPrim_{}", i), "Xform")
                    .expect("define");
                assert!(
                    stage
                        .get_prim_at_path(
                            &Path::from_string(&format!("/ThreadPrim_{}", i)).expect("p")
                        )
                        .is_some()
                );
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("thread panicked");
    }
}
