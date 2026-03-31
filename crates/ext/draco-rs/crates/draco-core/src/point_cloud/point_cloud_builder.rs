//! Point cloud builder.
//! Reference: `_ref/draco/src/draco/point_cloud/point_cloud_builder.h` + `.cc`.

use crate::attributes::geometry_attribute::{GeometryAttribute, GeometryAttributeType};
use crate::attributes::geometry_indices::PointIndex;
use crate::core::draco_types::{data_type_length, DataType};
use crate::metadata::geometry_metadata::AttributeMetadata;
use crate::point_cloud::point_cloud::PointCloud;

pub struct PointCloudBuilder {
    point_cloud: Option<PointCloud>,
}

impl PointCloudBuilder {
    pub fn new() -> Self {
        Self { point_cloud: None }
    }

    pub fn start(&mut self, num_points: u32) {
        let mut pc = PointCloud::new();
        pc.set_num_points(num_points);
        self.point_cloud = Some(pc);
    }

    pub fn add_attribute(
        &mut self,
        attribute_type: GeometryAttributeType,
        num_components: i8,
        data_type: DataType,
    ) -> i32 {
        self.add_attribute_with_normalized(attribute_type, num_components, data_type, false)
    }

    pub fn add_attribute_with_normalized(
        &mut self,
        attribute_type: GeometryAttributeType,
        num_components: i8,
        data_type: DataType,
        normalized: bool,
    ) -> i32 {
        let mut ga = GeometryAttribute::new();
        let stride = (data_type_length(data_type) as i64) * (num_components as i64);
        ga.init(
            attribute_type,
            None,
            num_components as u8,
            data_type,
            normalized,
            stride,
            0,
        );
        let pc = self
            .point_cloud
            .as_mut()
            .expect("PointCloudBuilder not started");
        pc.add_attribute_from_geometry(&ga, true, pc.num_points())
    }

    pub fn set_attribute_value_for_point(
        &mut self,
        att_id: i32,
        point_index: PointIndex,
        attribute_value: &[u8],
    ) {
        let pc = self
            .point_cloud
            .as_mut()
            .expect("PointCloudBuilder not started");
        let att = pc.attribute_mut(att_id).expect("Invalid attribute id");
        let entry_index = att.mapped_index(point_index);
        att.geometry_attribute()
            .set_attribute_value(entry_index, attribute_value);
    }

    pub fn set_attribute_values_for_all_points(
        &mut self,
        att_id: i32,
        attribute_values: &[u8],
        stride: i32,
    ) {
        let pc = self
            .point_cloud
            .as_mut()
            .expect("PointCloudBuilder not started");
        let num_points = pc.num_points();
        let att = pc.attribute_mut(att_id).expect("Invalid attribute id");
        let data_stride =
            (data_type_length(att.data_type()) as i32) * (att.num_components() as i32);
        let mut stride = stride;
        if stride == 0 {
            stride = data_stride;
        }
        if stride == data_stride {
            if let Some(buffer) = att.buffer() {
                let total = (num_points as i32) * data_stride;
                let slice = &attribute_values[..total as usize];
                buffer.borrow_mut().write(0, slice);
            }
        } else {
            for i in 0..num_points {
                let offset = (stride as u32) * i;
                let start = offset as usize;
                let end = start + data_stride as usize;
                let entry_index = att.mapped_index(PointIndex::from(i));
                att.geometry_attribute()
                    .set_attribute_value(entry_index, &attribute_values[start..end]);
            }
        }
    }

    pub fn set_attribute_unique_id(&mut self, att_id: i32, unique_id: u32) {
        let pc = self
            .point_cloud
            .as_mut()
            .expect("PointCloudBuilder not started");
        if let Some(att) = pc.attribute_mut(att_id) {
            att.set_unique_id(unique_id);
        }
    }

    pub fn set_attribute_name(&mut self, att_id: i32, name: &str) {
        let pc = self
            .point_cloud
            .as_mut()
            .expect("PointCloudBuilder not started");
        if let Some(att) = pc.attribute_mut(att_id) {
            att.set_name(name);
        }
    }

    pub fn finalize(&mut self, deduplicate_points: bool) -> Option<PointCloud> {
        if self.point_cloud.is_none() {
            return None;
        }
        if deduplicate_points {
            if let Some(pc) = self.point_cloud.as_mut() {
                pc.deduplicate_attribute_values();
                pc.deduplicate_point_ids();
            }
        }
        self.point_cloud.take()
    }

    pub fn add_attribute_metadata(&mut self, att_id: i32, metadata: AttributeMetadata) {
        let pc = self
            .point_cloud
            .as_mut()
            .expect("PointCloudBuilder not started");
        pc.add_attribute_metadata(att_id, metadata);
    }
}

impl Default for PointCloudBuilder {
    fn default() -> Self {
        Self::new()
    }
}
