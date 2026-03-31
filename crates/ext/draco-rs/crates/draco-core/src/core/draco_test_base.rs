//! Test base utilities.
//! Reference: `_ref/draco/src/draco/core/draco_test_base.h`.

use std::sync::atomic::AtomicBool;

pub static FLAGS_UPDATE_GOLDEN_FILES: AtomicBool = AtomicBool::new(false);
