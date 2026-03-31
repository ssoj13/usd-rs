//! Mesh traversal sequencer for attribute encoding/decoding order.
//! Reference: `_ref/draco/src/draco/compression/mesh/traverser/mesh_traversal_sequencer.h`.

use draco_core::attributes::geometry_indices::{
    AttributeValueIndex, CornerIndex, FaceIndex, PointIndex, INVALID_VERTEX_INDEX,
};
use draco_core::attributes::point_attribute::PointAttribute;
use draco_core::mesh::mesh::Mesh;

use crate::compression::attributes::mesh_attribute_indices_encoding_data::MeshAttributeIndicesEncodingData;
use crate::compression::attributes::points_sequencer::PointsSequencer;
use crate::compression::mesh::traverser::mesh_attribute_indices_encoding_observer::MeshAttributeIndicesEncodingObserver;
use crate::compression::mesh::traverser::traverser_base::{MeshTraverser, TraversalCornerTable};

pub struct MeshTraversalSequencer<TraverserT, CornerTableT>
where
    TraverserT: MeshTraverser<CornerTable = CornerTableT>,
    CornerTableT: TraversalCornerTable,
{
    mesh: *const Mesh,
    encoding_data: *mut MeshAttributeIndicesEncodingData,
    corner_order: Option<*const Vec<CornerIndex>>,
    corner_table: *const CornerTableT,
    traverser: TraverserT,
}

impl<TraverserT, CornerTableT> MeshTraversalSequencer<TraverserT, CornerTableT>
where
    TraverserT: MeshTraverser<CornerTable = CornerTableT> + Default,
    CornerTableT: TraversalCornerTable,
{
    pub fn new(
        mesh: *const Mesh,
        encoding_data: *mut MeshAttributeIndicesEncodingData,
        corner_table: *const CornerTableT,
    ) -> Self {
        Self {
            mesh,
            encoding_data,
            corner_order: None,
            corner_table,
            traverser: TraverserT::default(),
        }
    }

    pub fn set_traverser(&mut self, traverser: TraverserT) {
        self.traverser = traverser;
    }

    pub fn set_corner_order(&mut self, corner_order: &Vec<CornerIndex>) {
        self.corner_order = Some(corner_order as *const Vec<CornerIndex>);
    }
}

impl<TraverserT, CornerTableT> PointsSequencer for MeshTraversalSequencer<TraverserT, CornerTableT>
where
    TraverserT: MeshTraverser<
            CornerTable = CornerTableT,
            Observer = MeshAttributeIndicesEncodingObserver<CornerTableT>,
        > + Default,
    CornerTableT: TraversalCornerTable,
{
    fn update_point_to_attribute_index_mapping(&mut self, attribute: &mut PointAttribute) -> bool {
        let mesh = unsafe { &*self.mesh };
        let encoding_data = unsafe { &*self.encoding_data };
        let corner_table = self.traverser.corner_table();

        attribute.set_explicit_mapping(mesh.num_points() as usize);
        let num_faces = mesh.num_faces() as usize;
        let num_points = mesh.num_points() as usize;
        for f in 0..num_faces {
            let face = mesh.face(FaceIndex::from(f as u32));
            for p in 0..3 {
                let point_id = face[p];
                let vert_id = corner_table.vertex(CornerIndex::from((3 * f + p) as u32));
                if vert_id == INVALID_VERTEX_INDEX {
                    return false;
                }
                let att_index_val = encoding_data.vertex_to_encoded_attribute_value_index_map
                    [vert_id.value() as usize];
                if att_index_val < 0 {
                    return false;
                }
                let att_entry_id = AttributeValueIndex::from(att_index_val as u32);
                if point_id.value() as usize >= num_points
                    || att_entry_id.value() as usize >= num_points
                {
                    return false;
                }
                attribute.set_point_map_entry(point_id, att_entry_id);
            }
        }
        true
    }

    fn generate_sequence_internal(&mut self, out_point_ids: &mut Vec<PointIndex>) -> bool {
        let mesh = unsafe { &*self.mesh };
        let corner_table_ptr = self.corner_table;
        let corner_table = unsafe { &*corner_table_ptr };
        out_point_ids.reserve(corner_table.num_vertices());

        let observer = MeshAttributeIndicesEncodingObserver::new(
            self.corner_table,
            self.mesh,
            out_point_ids as *mut Vec<PointIndex>,
            self.encoding_data,
        );
        self.traverser.init(corner_table, observer);

        self.traverser.on_traversal_start();
        if let Some(order_ptr) = self.corner_order {
            let order = unsafe { &*order_ptr };
            for &corner in order {
                if !self.traverser.traverse_from_corner(corner) {
                    return false;
                }
            }
        } else {
            let num_faces = corner_table.num_faces();
            for i in 0..num_faces {
                if !self
                    .traverser
                    .traverse_from_corner(CornerIndex::from((3 * i) as u32))
                {
                    return false;
                }
            }
        }
        self.traverser.on_traversal_end();
        // Ensure points were generated for the traversed mesh.
        if out_point_ids.is_empty() && mesh.num_faces() > 0 {
            return false;
        }
        true
    }
}
