//! Keyframe animation.
//!
//! What: Stores keyframe time series as a PointCloud with attributes.
//! Why: Draco exposes keyframe animations for non-scene animation payloads.
//! How: Uses PointCloud attributes for timestamps and per-track data.
//! Where used: Animation compression/decoding and tests.

use draco_core::attributes::geometry_attribute::GeometryAttributeType;
use draco_core::attributes::geometry_indices::PointIndex;
use draco_core::attributes::point_attribute::PointAttribute;
use draco_core::core::draco_types::DataType;
use draco_core::point_cloud::PointCloud;

/// Timestamp type for keyframe animations.
pub type TimestampType = f32;

/// Keyframe animation stored as a point cloud.
#[derive(Default)]
pub struct KeyframeAnimation {
    point_cloud: PointCloud,
}

impl KeyframeAnimation {
    /// Attribute id of timestamp is fixed to 0.
    pub const TIMESTAMP_ID: i32 = 0;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn point_cloud(&self) -> &PointCloud {
        &self.point_cloud
    }

    pub fn point_cloud_mut(&mut self) -> &mut PointCloud {
        &mut self.point_cloud
    }

    pub fn set_timestamps(&mut self, timestamps: &[TimestampType]) -> bool {
        let num_frames = timestamps.len() as i32;
        if self.point_cloud.num_attributes() > 0 {
            if let Some(att) = self
                .point_cloud
                .get_attribute_by_unique_id(Self::TIMESTAMP_ID as u32)
            {
                if att.size() > 0 {
                    return false;
                }
            } else {
                if num_frames as u32 != self.point_cloud.num_points() {
                    return false;
                }
            }
        } else {
            self.set_num_frames(num_frames);
        }

        let mut timestamp_att = PointAttribute::new();
        timestamp_att.init(
            GeometryAttributeType::Generic,
            1,
            DataType::Float32,
            false,
            num_frames as usize,
        );
        for i in 0..num_frames {
            let pi = PointIndex::from(i as u32);
            let v = timestamps[i as usize];
            timestamp_att.set_attribute_value(timestamp_att.mapped_index(pi), &v);
        }
        self.point_cloud
            .set_attribute(Self::TIMESTAMP_ID, timestamp_att);
        true
    }

    pub fn add_keyframes<T: Copy>(
        &mut self,
        data_type: DataType,
        num_components: u32,
        data: &[T],
    ) -> i32 {
        if num_components == 0 {
            return -1;
        }
        if self.point_cloud.num_attributes() == 0 {
            let mut temp_att = PointAttribute::new();
            temp_att.init(
                GeometryAttributeType::Generic,
                num_components as i8,
                data_type,
                false,
                0,
            );
            self.point_cloud.add_attribute(temp_att);
            let num_frames = (data.len() as u32) / num_components;
            self.set_num_frames(num_frames as i32);
        }

        if data.len() as u32 != num_components * (self.num_frames() as u32) {
            return -1;
        }

        let mut keyframe_att = PointAttribute::new();
        keyframe_att.init(
            GeometryAttributeType::Generic,
            num_components as i8,
            data_type,
            false,
            self.num_frames() as usize,
        );
        let stride = num_components as usize;
        for i in 0..self.num_frames() {
            let pi = PointIndex::from(i as u32);
            let start = (i as usize) * stride;
            let slice = &data[start..start + stride];
            keyframe_att.set_attribute_value_bytes(keyframe_att.mapped_index(pi), unsafe {
                std::slice::from_raw_parts(
                    slice.as_ptr() as *const u8,
                    std::mem::size_of::<T>() * stride,
                )
            });
        }
        self.point_cloud.add_attribute(keyframe_att)
    }

    pub fn timestamps(&self) -> Option<&PointAttribute> {
        self.point_cloud
            .get_attribute_by_unique_id(Self::TIMESTAMP_ID as u32)
    }

    pub fn keyframes(&self, animation_id: i32) -> Option<&PointAttribute> {
        self.point_cloud
            .get_attribute_by_unique_id(animation_id as u32)
    }

    pub fn set_num_frames(&mut self, num_frames: i32) {
        self.point_cloud.set_num_points(num_frames as u32);
    }

    pub fn num_frames(&self) -> i32 {
        self.point_cloud.num_points() as i32
    }

    pub fn num_animations(&self) -> i32 {
        self.point_cloud.num_attributes() - 1
    }
}
