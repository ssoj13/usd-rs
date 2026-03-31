//! Edgebreaker mesh encoder wrapper.
//! Reference: `_ref/draco/src/draco/compression/mesh/mesh_edgebreaker_encoder.h|cc`.

use draco_core::attributes::geometry_indices::{VertexIndex, INVALID_CORNER_INDEX};
use draco_core::mesh::corner_table::CornerTable;
use draco_core::mesh::mesh_attribute_corner_table::MeshAttributeCornerTable;

use crate::compression::attributes::mesh_attribute_indices_encoding_data::MeshAttributeIndicesEncodingData;
use crate::compression::config::compression_shared::{
    EncodedGeometryType, MeshEdgebreakerConnectivityEncodingMethod, MeshEncoderMethod,
};
use crate::compression::config::encoding_features::{K_EDGEBREAKER, K_PREDICTIVE_EDGEBREAKER};
use crate::compression::mesh::edgebreaker_shared::EdgebreakerTopologyBitPattern;
use crate::compression::mesh::mesh_edgebreaker_encoder_impl::MeshEdgebreakerEncoderImpl;
use crate::compression::mesh::mesh_edgebreaker_encoder_impl_interface::MeshEdgebreakerEncoderImplInterface;
use crate::compression::mesh::mesh_edgebreaker_traversal_encoder::MeshEdgebreakerTraversalEncoder;
use crate::compression::mesh::mesh_edgebreaker_traversal_valence_encoder::MeshEdgebreakerTraversalValenceEncoder;
use crate::compression::mesh::{MeshEncoder, MeshEncoderBase};
use crate::compression::point_cloud::PointCloudEncoder;
use draco_core::core::status::{ok_status, Status, StatusCode};

pub struct MeshEdgebreakerEncoder {
    base: MeshEncoderBase,
    impl_: Option<Box<dyn MeshEdgebreakerEncoderImplInterface>>,
}

impl MeshEdgebreakerEncoder {
    pub fn new() -> Self {
        Self {
            base: MeshEncoderBase::new(),
            impl_: None,
        }
    }

    pub fn get_corner_table(&self) -> Option<&CornerTable> {
        self.impl_.as_ref().and_then(|imp| imp.get_corner_table())
    }

    pub fn get_attribute_corner_table(&self, att_id: i32) -> Option<&MeshAttributeCornerTable<'_>> {
        self.impl_
            .as_ref()
            .and_then(|imp| imp.get_attribute_corner_table(att_id))
    }

    pub fn get_attribute_encoding_data(
        &self,
        att_id: i32,
    ) -> Option<&MeshAttributeIndicesEncodingData> {
        self.impl_
            .as_ref()
            .map(|imp| imp.get_attribute_encoding_data(att_id))
    }

    /// Returns the traversal symbol sequence (Standard encoder only; for parity debugging).
    pub fn get_traversal_symbols(&self) -> Option<Vec<EdgebreakerTopologyBitPattern>> {
        self.impl_
            .as_ref()
            .and_then(|imp| imp.get_traversal_symbols())
    }
}

impl Default for MeshEdgebreakerEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl PointCloudEncoder for MeshEdgebreakerEncoder {
    fn base(&self) -> &crate::compression::point_cloud::PointCloudEncoderBase {
        &self.base.pc_base
    }

    fn base_mut(&mut self) -> &mut crate::compression::point_cloud::PointCloudEncoderBase {
        &mut self.base.pc_base
    }

    fn get_geometry_type(&self) -> EncodedGeometryType {
        EncodedGeometryType::TriangularMesh
    }

    fn mesh_prediction_scheme_data(
        &self,
        att_id: i32,
    ) -> crate::compression::point_cloud::MeshPredictionSchemeDataForEncoder {
        let imp = match self.impl_.as_ref() {
            Some(imp) => imp,
            None => return None,
        };
        let corner_table = match imp.get_corner_table() {
            Some(ct) => ct,
            None => return None,
        };
        let enc_data = imp.get_attribute_encoding_data(att_id);
        let mut mesh_data = crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_data::MeshPredictionSchemeData::new();
        if let Some(mesh) = self.mesh() {
            mesh_data.set(
                mesh,
                corner_table,
                &enc_data.encoded_attribute_value_index_to_corner_map,
                &enc_data.vertex_to_encoded_attribute_value_index_map,
            );
            Some(mesh_data)
        } else {
            None
        }
    }

    fn get_encoding_method(&self) -> u8 {
        MeshEncoderMethod::MeshEdgebreakerEncoding as u8
    }

    fn initialize_encoder(&mut self) -> bool {
        let is_standard_available = self.options().is_feature_supported(K_EDGEBREAKER);
        let is_predictive_available = self
            .options()
            .is_feature_supported(K_PREDICTIVE_EDGEBREAKER);

        let is_tiny_mesh = self
            .mesh()
            .map(|mesh| mesh.num_faces() < 1000)
            .unwrap_or(false);

        let mut selected_method = self.options().get_global_int("edgebreaker_method", -1);
        if selected_method == -1 {
            if is_standard_available
                && (self.options().get_speed() >= 5 || !is_predictive_available || is_tiny_mesh)
            {
                selected_method =
                    MeshEdgebreakerConnectivityEncodingMethod::MeshEdgebreakerStandardEncoding
                        as i32;
            } else {
                selected_method =
                    MeshEdgebreakerConnectivityEncodingMethod::MeshEdgebreakerValenceEncoding
                        as i32;
            }
        }

        self.impl_ = None;
        if selected_method
            == MeshEdgebreakerConnectivityEncodingMethod::MeshEdgebreakerStandardEncoding as i32
        {
            if is_standard_available {
                if let Some(buffer) = self.buffer() {
                    buffer.encode(
                        MeshEdgebreakerConnectivityEncodingMethod::MeshEdgebreakerStandardEncoding
                            as u8,
                    );
                } else {
                    return false;
                }
                self.impl_ = Some(Box::new(MeshEdgebreakerEncoderImpl::<
                    MeshEdgebreakerTraversalEncoder,
                >::new()));
            }
        } else if selected_method
            == MeshEdgebreakerConnectivityEncodingMethod::MeshEdgebreakerValenceEncoding as i32
        {
            if let Some(buffer) = self.buffer() {
                buffer.encode(
                    MeshEdgebreakerConnectivityEncodingMethod::MeshEdgebreakerValenceEncoding as u8,
                );
            } else {
                return false;
            }
            self.impl_ = Some(Box::new(MeshEdgebreakerEncoderImpl::<
                MeshEdgebreakerTraversalValenceEncoder,
            >::new()));
        }

        let self_ptr = self as *mut MeshEdgebreakerEncoder;
        let imp = match self.impl_.as_mut() {
            Some(imp) => imp,
            None => return false,
        };
        imp.init(self_ptr)
    }

    fn encode_geometry_data(&mut self) -> Status {
        let status = self.encode_connectivity();
        if !status.is_ok() {
            return status;
        }
        if self
            .options()
            .get_global_bool("store_number_of_encoded_faces", false)
        {
            self.compute_number_of_encoded_faces();
        }
        ok_status()
    }

    fn generate_attributes_encoder(&mut self, att_id: i32) -> bool {
        let imp = match self.impl_.as_mut() {
            Some(imp) => imp,
            None => return false,
        };
        imp.generate_attributes_encoder(att_id)
    }

    fn encode_attributes_encoder_identifier(&mut self, att_encoder_id: i32) -> bool {
        let imp = match self.impl_.as_mut() {
            Some(imp) => imp,
            None => return false,
        };
        imp.encode_attributes_encoder_identifier(att_encoder_id)
    }

    fn compute_number_of_encoded_points(&mut self) {
        let corner_table = match self.get_corner_table() {
            Some(ct) => ct,
            None => return,
        };
        let mut num_points =
            corner_table.num_vertices() - corner_table.num_isolated_vertices() as usize;

        if let Some(mesh) = self.mesh() {
            if mesh.num_attributes() > 1 {
                let mut attribute_corner_tables: Vec<&MeshAttributeCornerTable> = Vec::new();
                for i in 0..mesh.num_attributes() {
                    if let Some(att) = mesh.attribute(i) {
                        if att.attribute_type()
                            == draco_core::attributes::geometry_attribute::GeometryAttributeType::Position
                        {
                            continue;
                        }
                        if let Some(att_corner_table) = self.get_attribute_corner_table(i) {
                            attribute_corner_tables.push(att_corner_table);
                        }
                    }
                }

                for vi in 0..corner_table.num_vertices() {
                    let vert = VertexIndex::from(vi as u32);
                    if corner_table.is_vertex_isolated(vert) {
                        continue;
                    }
                    let first_corner_index = corner_table.left_most_corner(vert);
                    let first_point_index = mesh.corner_to_point_id(first_corner_index);
                    let mut last_point_index = first_point_index;
                    let mut last_corner_index = first_corner_index;
                    let mut corner_index = corner_table.swing_right(first_corner_index);
                    let mut num_attribute_seams = 0usize;
                    while corner_index != INVALID_CORNER_INDEX {
                        let point_index = mesh.corner_to_point_id(corner_index);
                        let mut seam_found = false;
                        if point_index != last_point_index {
                            seam_found = true;
                            last_point_index = point_index;
                        } else {
                            for table in &attribute_corner_tables {
                                if table.vertex(corner_index) != table.vertex(last_corner_index) {
                                    seam_found = true;
                                    break;
                                }
                            }
                        }
                        if seam_found {
                            num_attribute_seams += 1;
                        }
                        if corner_index == first_corner_index {
                            break;
                        }
                        last_corner_index = corner_index;
                        corner_index = corner_table.swing_right(corner_index);
                    }
                    if !corner_table.is_on_boundary(vert) && num_attribute_seams > 0 {
                        num_points += num_attribute_seams - 1;
                    } else {
                        num_points += num_attribute_seams;
                    }
                }
            }
        }
        self.set_num_encoded_points(num_points);
    }
}

impl MeshEncoder for MeshEdgebreakerEncoder {
    fn mesh_base(&self) -> &MeshEncoderBase {
        &self.base
    }

    fn mesh_base_mut(&mut self) -> &mut MeshEncoderBase {
        &mut self.base
    }

    fn encode_connectivity(&mut self) -> Status {
        let imp = match self.impl_.as_mut() {
            Some(imp) => imp,
            None => {
                return Status::new(
                    StatusCode::DracoError,
                    "Edgebreaker encoder not initialized.",
                )
            }
        };
        imp.encode_connectivity()
    }

    fn compute_number_of_encoded_faces(&mut self) {
        let corner_table = match self.get_corner_table() {
            Some(ct) => ct,
            None => return,
        };
        let num_faces = corner_table.num_faces() as i32 - corner_table.num_degenerated_faces();
        if num_faces < 0 {
            return;
        }
        self.set_num_encoded_faces(num_faces as usize);
    }
}
