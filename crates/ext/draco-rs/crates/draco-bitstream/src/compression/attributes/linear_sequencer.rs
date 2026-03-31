//! Linear point sequencer.
//! Reference: `_ref/draco/src/draco/compression/attributes/linear_sequencer.h`.

use crate::compression::attributes::points_sequencer::PointsSequencer;
use draco_core::attributes::geometry_indices::PointIndex;
use draco_core::attributes::point_attribute::PointAttribute;

pub struct LinearSequencer {
    num_points: i32,
}

impl LinearSequencer {
    pub fn new(num_points: i32) -> Self {
        Self { num_points }
    }
}

impl PointsSequencer for LinearSequencer {
    fn update_point_to_attribute_index_mapping(&mut self, attribute: &mut PointAttribute) -> bool {
        attribute.set_identity_mapping();
        true
    }

    fn generate_sequence_internal(&mut self, out_point_ids: &mut Vec<PointIndex>) -> bool {
        if self.num_points < 0 {
            return false;
        }
        out_point_ids.resize(self.num_points as usize, PointIndex::from(0u32));
        for i in 0..self.num_points {
            out_point_ids[i as usize] = PointIndex::from(i as u32);
        }
        true
    }
}
