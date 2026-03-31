//! Point id sequencer for attribute encoding/decoding.
//! Reference: `_ref/draco/src/draco/compression/attributes/points_sequencer.h`.

use draco_core::attributes::geometry_indices::PointIndex;
use draco_core::attributes::point_attribute::PointAttribute;

pub trait PointsSequencer {
    fn generate_sequence_internal(&mut self, out_point_ids: &mut Vec<PointIndex>) -> bool;

    fn generate_sequence(&mut self, out_point_ids: &mut Vec<PointIndex>) -> bool {
        self.generate_sequence_internal(out_point_ids)
    }

    fn add_point_id(&self, out_point_ids: &mut Vec<PointIndex>, point_id: PointIndex) {
        out_point_ids.push(point_id);
    }

    fn update_point_to_attribute_index_mapping(&mut self, _attr: &mut PointAttribute) -> bool {
        false
    }
}
