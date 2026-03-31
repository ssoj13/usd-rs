//! Point cloud tests mirroring reference transcoder-only coverage.
//!
//! What: Verifies copy + metadata behavior using encoded point cloud input.
//! Why: Matches `_ref/draco/src/draco/point_cloud/point_cloud_test.cc` copy test.
//! Where used: `draco-rs` test suite; exercises IO decode + core copy path.

use crate::io::test_utils::read_point_cloud_from_test_file;
use crate::metadata::geometry_metadata::{AttributeMetadata, GeometryMetadata};
use crate::point_cloud::PointCloud;
use draco_core::metadata::metadata::MetadataString;

#[test]
fn point_cloud_copy_from_test_file() {
    let status_or = read_point_cloud_from_test_file("pc_kd_color.drc");
    assert!(
        status_or.is_ok(),
        "{}",
        status_or.status().error_msg_string()
    );
    let mut pc = status_or.into_value();

    let mut metadata = GeometryMetadata::new();
    metadata.add_entry_int("speed", 1050);
    metadata.add_entry_string("code", "YT-1300f");

    let mut att_metadata = AttributeMetadata::new();
    let att_unique_id = pc.attribute(0).expect("Missing attribute 0").unique_id();
    assert_eq!(att_unique_id, 0, "Expected attribute 0 unique id to be 0");
    att_metadata.set_att_unique_id(att_unique_id);
    att_metadata.add_entry_int("attribute_test", 3);
    metadata.add_attribute_metadata(Some(Box::new(att_metadata)));
    pc.add_metadata(metadata);

    let mut pc_copy = PointCloud::new();
    pc_copy.copy(&pc);

    assert_eq!(pc.num_points(), pc_copy.num_points());
    assert_eq!(pc.num_attributes(), pc_copy.num_attributes());
    for i in 0..pc.num_attributes() {
        assert_eq!(
            pc.attribute(i).unwrap().attribute_type(),
            pc_copy.attribute(i).unwrap().attribute_type()
        );
    }

    let metadata_copy = pc_copy.get_metadata().expect("Missing metadata");
    let mut speed = 0i32;
    let mut code = MetadataString::default();
    assert!(metadata_copy.get_entry_int("speed", &mut speed));
    assert!(metadata_copy.get_entry_string("code", &mut code));
    assert_eq!(speed, 1050);
    assert_eq!(code.as_bytes(), b"YT-1300f");

    let att_metadata_copy = metadata_copy
        .get_attribute_metadata_by_unique_id(att_unique_id as i32)
        .expect("Missing attribute metadata");
    let mut att_test = 0i32;
    assert!(att_metadata_copy.get_entry_int("attribute_test", &mut att_test));
    assert_eq!(att_test, 3);
}
