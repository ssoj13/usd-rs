//! Integration tests for SpatialAudio.
//!
//! Ported from C++ reference: pxr/usd/usdMedia/testenv/testUsdMediaSpatialAudio.py

use usd_core::{InitialLoadSet, Stage};
use usd_media::SpatialAudio;
use usd_sdf::{Layer, LayerOffset, Path};
use usd_vt::{TimeCode, Value};

/// Initialize file format plugins (USDA, USDC).
fn init() {
    usd_sdf::init();
}

/// Port of test_TimeAttrs from testUsdMediaSpatialAudio.py
///
/// Tests layer offset behavior with timecode attributes:
/// - startTime and endTime are timecodes: layer offsets ARE applied
/// - mediaOffset is a double: layer offsets are NOT applied
///
/// Reference has layer offset scale=2.0, offset=10.0:
///   startTime: 10 -> 10*2+10 = 30
///   endTime: 200 -> 200*2+10 = 410
///   mediaOffset: 5.0 -> 5.0 (unchanged)
#[test]
fn test_time_attrs() {
    init();

    // Create reference layer and stage with a SpatialAudio prim
    let ref_layer = Layer::create_anonymous(None);
    let ref_stage = Stage::open_with_root_layer(ref_layer.clone(), InitialLoadSet::LoadAll)
        .expect("Failed to open ref stage");
    let ref_audio = SpatialAudio::define(&ref_stage, &Path::from_string("/RefAudio").unwrap())
        .expect("Failed to define SpatialAudio on ref stage");

    // Author values on the ref audio (must create attrs first, no schema fallbacks)
    let start_attr = ref_audio.create_start_time_attr(None, false);
    assert!(
        start_attr.set(
            Value::from(TimeCode::new(10.0)),
            usd_sdf::TimeCode::default_time()
        ),
        "Setting startTime should succeed"
    );

    let end_attr = ref_audio.create_end_time_attr(None, false);
    assert!(
        end_attr.set(
            Value::from(TimeCode::new(200.0)),
            usd_sdf::TimeCode::default_time()
        ),
        "Setting endTime should succeed"
    );

    let media_attr = ref_audio.create_media_offset_attr(None, false);
    assert!(
        media_attr.set(Value::from(5.0_f64), usd_sdf::TimeCode::default_time()),
        "Setting mediaOffset should succeed"
    );

    // Verify authored values on the ref audio directly
    let ref_start_val = ref_audio
        .get_start_time_attr()
        .and_then(|a| a.get(usd_core::TimeCode::default_time()))
        .and_then(|v| v.downcast_clone::<TimeCode>())
        .expect("RefAudio startTime should be authored");
    assert_eq!(
        ref_start_val.value(),
        10.0,
        "RefAudio startTime should be 10"
    );

    let ref_end_val = ref_audio
        .get_end_time_attr()
        .and_then(|a| a.get(usd_core::TimeCode::default_time()))
        .and_then(|v| v.downcast_clone::<TimeCode>())
        .expect("RefAudio endTime should be authored");
    assert_eq!(ref_end_val.value(), 200.0, "RefAudio endTime should be 200");

    let ref_media_val = ref_audio
        .get_media_offset_attr()
        .and_then(|a| a.get(usd_core::TimeCode::default_time()))
        .and_then(|v| v.downcast_clone::<f64>())
        .expect("RefAudio mediaOffset should be authored");
    assert_eq!(ref_media_val, 5.0, "RefAudio mediaOffset should be 5.0");

    // Create a new stage with SpatialAudio that references the above prim.
    // The reference has layer offset scale=2.0, offset=10.0
    let stage =
        Stage::create_in_memory(InitialLoadSet::LoadAll).expect("Failed to create in-memory stage");
    let audio = SpatialAudio::define(&stage, &Path::from_string("/Audio").unwrap())
        .expect("Failed to define SpatialAudio");

    let refs = audio.get_prim().get_references();
    let layer_offset = LayerOffset::new(10.0, 2.0);
    assert!(
        refs.add_reference_with_path(
            ref_layer.identifier(),
            &Path::from_string("/RefAudio").unwrap(),
            layer_offset,
            usd_core::ListPosition::BackOfPrependList,
        ),
        "AddReference should succeed"
    );

    // Verify composed values with layer offset applied:
    // Layer offset: offset=10.0, scale=2.0
    // For timecode attrs: value * scale + offset
    //   startTime: 10*2 + 10 = 30
    //   endTime: 200*2 + 10 = 410
    // For double attrs (mediaOffset): NOT affected by layer offset
    //   mediaOffset: 5.0 (unchanged)
    //
    // NOTE: Layer offset application to timecodes requires deep PCP value
    // resolution integration. Test documents expected behavior.
    let start_resolved = audio
        .get_start_time_attr()
        .and_then(|a| a.get(usd_core::TimeCode::default_time()))
        .and_then(|v| v.downcast_clone::<TimeCode>());
    let end_resolved = audio
        .get_end_time_attr()
        .and_then(|a| a.get(usd_core::TimeCode::default_time()))
        .and_then(|v| v.downcast_clone::<TimeCode>());
    let media_resolved = audio
        .get_media_offset_attr()
        .and_then(|a| a.get(usd_core::TimeCode::default_time()))
        .and_then(|v| v.downcast_clone::<f64>());

    // Timecode values must have layer offset applied: value * scale + offset
    let start = start_resolved.expect("startTime must resolve through reference");
    assert_eq!(start.value(), 30.0, "startTime should be 30 (10*2+10)");

    let end = end_resolved.expect("endTime must resolve through reference");
    assert_eq!(end.value(), 410.0, "endTime should be 410 (200*2+10)");

    // Double values are NOT affected by layer offset
    let media = media_resolved.expect("mediaOffset must resolve through reference");
    assert_eq!(media, 5.0, "mediaOffset stays 5.0 (double, not timecode)");
}

/// Test schema attribute names for SpatialAudio.
#[test]
fn test_schema_attribute_names() {
    let local_names = SpatialAudio::get_schema_attribute_names(false);
    assert_eq!(local_names.len(), 7, "SpatialAudio has 7 local attributes");

    // Verify all expected attribute names are present
    let expected = [
        "filePath",
        "auralMode",
        "playbackMode",
        "startTime",
        "endTime",
        "mediaOffset",
        "gain",
    ];
    for name in &expected {
        assert!(
            local_names.iter().any(|n| n.as_str() == *name),
            "Missing local attribute: {name}"
        );
    }

    // Inherited should include Xformable attrs (xformOpOrder, visibility, etc.)
    let inherited_names = SpatialAudio::get_schema_attribute_names(true);
    assert!(
        inherited_names.len() > local_names.len(),
        "Inherited should include more attributes"
    );
    assert!(
        inherited_names.iter().any(|n| n.as_str() == "xformOpOrder"),
        "Should include xformOpOrder from Xformable"
    );
}

/// Test Define and Get factory methods.
#[test]
fn test_define_and_get() {
    init();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("Failed to create stage");

    // Define a SpatialAudio prim
    let audio = SpatialAudio::define(&stage, &Path::from_string("/TestAudio").unwrap())
        .expect("Define should succeed");
    assert!(audio.is_valid(), "Defined SpatialAudio should be valid");
    assert_eq!(audio.get_schema_kind(), usd_core::SchemaKind::ConcreteTyped);

    // Get it back
    let audio2 = SpatialAudio::get(&stage, &Path::from_string("/TestAudio").unwrap())
        .expect("Get should succeed for defined prim");
    assert!(audio2.is_valid());

    // Get non-existent should fail
    assert!(
        SpatialAudio::get(&stage, &Path::from_string("/NonExistent").unwrap()).is_none(),
        "Get should return None for non-existent path"
    );
}

/// Test attribute creation and round-trip values.
#[test]
fn test_create_attributes() {
    init();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("Failed to create stage");
    let audio = SpatialAudio::define(&stage, &Path::from_string("/Audio").unwrap())
        .expect("Failed to define SpatialAudio");

    // Create all attributes
    let file_attr = audio.create_file_path_attr(None, false);
    assert!(file_attr.is_valid(), "filePath attr should be valid");

    let aural_attr = audio.create_aural_mode_attr(None, false);
    assert!(aural_attr.is_valid(), "auralMode attr should be valid");

    let playback_attr = audio.create_playback_mode_attr(None, false);
    assert!(
        playback_attr.is_valid(),
        "playbackMode attr should be valid"
    );

    let start_attr = audio.create_start_time_attr(None, false);
    assert!(start_attr.is_valid(), "startTime attr should be valid");

    let end_attr = audio.create_end_time_attr(None, false);
    assert!(end_attr.is_valid(), "endTime attr should be valid");

    let media_attr = audio.create_media_offset_attr(None, false);
    assert!(media_attr.is_valid(), "mediaOffset attr should be valid");

    let gain_attr = audio.create_gain_attr(None, false);
    assert!(gain_attr.is_valid(), "gain attr should be valid");

    // Verify we can get them back
    assert!(
        audio.get_file_path_attr().is_some(),
        "Should get filePath attr"
    );
    assert!(
        audio.get_aural_mode_attr().is_some(),
        "Should get auralMode attr"
    );
    assert!(
        audio.get_playback_mode_attr().is_some(),
        "Should get playbackMode attr"
    );
    assert!(
        audio.get_start_time_attr().is_some(),
        "Should get startTime attr"
    );
    assert!(
        audio.get_end_time_attr().is_some(),
        "Should get endTime attr"
    );
    assert!(
        audio.get_media_offset_attr().is_some(),
        "Should get mediaOffset attr"
    );
    assert!(audio.get_gain_attr().is_some(), "Should get gain attr");
}
