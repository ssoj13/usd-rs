/// Port of testUsdPhysicsMetrics.py
use std::sync::Arc;

use usd_core::{InitialLoadSet, Stage};
use usd_physics::{
    MassUnits, get_stage_kilograms_per_unit, mass_units_are, set_stage_kilograms_per_unit,
    stage_has_authored_kilograms_per_unit,
};

fn new_stage() -> Arc<Stage> {
    usd_sdf::init();
    Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create_in_memory")
}

/// Port of TestUsdPhysicsMetrics.test_kilogramsPerUnit
#[test]
fn test_kilograms_per_unit() {
    let stage = new_stage();

    // Default is kilograms (1.0)
    assert_eq!(get_stage_kilograms_per_unit(&stage), MassUnits::KILOGRAMS);
    assert!(!stage_has_authored_kilograms_per_unit(&stage));

    // Set to grams
    assert!(set_stage_kilograms_per_unit(&stage, MassUnits::GRAMS));
    assert!(stage_has_authored_kilograms_per_unit(&stage));

    let authored = get_stage_kilograms_per_unit(&stage);
    assert!(mass_units_are(authored, MassUnits::GRAMS, 1e-5));
}
