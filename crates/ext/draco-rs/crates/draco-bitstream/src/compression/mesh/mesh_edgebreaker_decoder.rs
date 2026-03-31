//! Edgebreaker mesh decoder wrapper.
//! Reference: `_ref/draco/src/draco/compression/mesh/mesh_edgebreaker_decoder.h|cc`.

use draco_core::mesh::corner_table::CornerTable;
use draco_core::mesh::mesh::Mesh;
use draco_core::mesh::mesh_attribute_corner_table::MeshAttributeCornerTable;

use crate::compression::attributes::attributes_decoder_interface::AttributesDecoderInterface;
use crate::compression::attributes::mesh_attribute_indices_encoding_data::MeshAttributeIndicesEncodingData;
use crate::compression::config::compression_shared::{
    EncodedGeometryType, MeshEdgebreakerConnectivityEncodingMethod,
};
use crate::compression::mesh::mesh_edgebreaker_decoder_impl::MeshEdgebreakerDecoderImpl;
use crate::compression::mesh::mesh_edgebreaker_decoder_impl_interface::MeshEdgebreakerDecoderImplInterface;
use crate::compression::mesh::mesh_edgebreaker_traversal_decoder::MeshEdgebreakerTraversalDecoder;
use crate::compression::mesh::mesh_edgebreaker_traversal_predictive_decoder::MeshEdgebreakerTraversalPredictiveDecoder;
use crate::compression::mesh::mesh_edgebreaker_traversal_valence_decoder::MeshEdgebreakerTraversalValenceDecoder;
use crate::compression::mesh::MeshDecoder;
use crate::compression::point_cloud::{PointCloudDecoder, PointCloudDecoderBase};

pub struct MeshEdgebreakerDecoder {
    base: PointCloudDecoderBase,
    mesh: *mut Mesh,
    impl_: Option<Box<dyn MeshEdgebreakerDecoderImplInterface>>,
}

impl MeshEdgebreakerDecoder {
    pub fn new() -> Self {
        Self {
            base: PointCloudDecoderBase::new(),
            mesh: std::ptr::null_mut(),
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

    pub(crate) fn mesh_mut(&mut self) -> Option<&mut Mesh> {
        unsafe { self.mesh.as_mut() }
    }

    pub(crate) fn num_attributes_decoders(&self) -> i32 {
        self.base.attributes_decoders.len() as i32
    }

    pub(crate) fn attributes_decoder(
        &self,
        decoder_id: i32,
    ) -> Option<&dyn AttributesDecoderInterface> {
        if decoder_id < 0 {
            return None;
        }
        self.base
            .attributes_decoders
            .get(decoder_id as usize)
            .map(|dec| dec.as_ref())
    }
}

impl Default for MeshEdgebreakerDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl PointCloudDecoder for MeshEdgebreakerDecoder {
    fn base(&self) -> &PointCloudDecoderBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut PointCloudDecoderBase {
        &mut self.base
    }

    fn as_mesh_decoder(&self) -> Option<&dyn MeshDecoder> {
        Some(self)
    }

    fn get_geometry_type(&self) -> EncodedGeometryType {
        EncodedGeometryType::TriangularMesh
    }

    fn initialize_decoder(&mut self) -> bool {
        let mut traversal_decoder_type: u8 = 0;
        {
            let buffer = match self.buffer_mut() {
                Some(buf) => buf,
                None => return false,
            };
            if !buffer.decode(&mut traversal_decoder_type) {
                return false;
            }
        }
        self.impl_ = None;
        if traversal_decoder_type
            == MeshEdgebreakerConnectivityEncodingMethod::MeshEdgebreakerStandardEncoding as u8
        {
            self.impl_ = Some(Box::new(MeshEdgebreakerDecoderImpl::<
                MeshEdgebreakerTraversalDecoder,
            >::new()));
        } else if traversal_decoder_type
            == MeshEdgebreakerConnectivityEncodingMethod::MeshEdgebreakerPredictiveEncoding as u8
        {
            self.impl_ = Some(Box::new(MeshEdgebreakerDecoderImpl::<
                MeshEdgebreakerTraversalPredictiveDecoder,
            >::new()));
        } else if traversal_decoder_type
            == MeshEdgebreakerConnectivityEncodingMethod::MeshEdgebreakerValenceEncoding as u8
        {
            self.impl_ = Some(Box::new(MeshEdgebreakerDecoderImpl::<
                MeshEdgebreakerTraversalValenceDecoder,
            >::new()));
        }
        let self_ptr = self as *mut MeshEdgebreakerDecoder;
        let imp = match self.impl_.as_mut() {
            Some(imp) => imp,
            None => return false,
        };
        imp.init(self_ptr)
    }

    fn create_attributes_decoder(&mut self, att_decoder_id: i32) -> bool {
        let imp = match self.impl_.as_mut() {
            Some(imp) => imp,
            None => return false,
        };
        imp.create_attributes_decoder(att_decoder_id)
    }

    fn decode_geometry_data(&mut self) -> bool {
        let imp = match self.impl_.as_mut() {
            Some(imp) => imp,
            None => return false,
        };
        imp.decode_connectivity()
    }

    fn on_attributes_decoded(&mut self) -> bool {
        let imp = match self.impl_.as_mut() {
            Some(imp) => imp,
            None => return false,
        };
        imp.on_attributes_decoded()
    }
}

impl MeshDecoder for MeshEdgebreakerDecoder {
    fn set_mesh(&mut self, mesh: &mut Mesh) {
        self.mesh = mesh as *mut Mesh;
        self.base.point_cloud =
            mesh as *mut _ as *mut draco_core::point_cloud::point_cloud::PointCloud;
    }

    fn mesh(&self) -> Option<&Mesh> {
        unsafe { self.mesh.as_ref() }
    }

    fn get_corner_table(&self) -> Option<&CornerTable> {
        MeshEdgebreakerDecoder::get_corner_table(self)
    }

    fn get_attribute_corner_table(&self, att_id: i32) -> Option<&MeshAttributeCornerTable<'_>> {
        MeshEdgebreakerDecoder::get_attribute_corner_table(self, att_id)
    }

    fn get_attribute_encoding_data(
        &self,
        att_id: i32,
    ) -> Option<&MeshAttributeIndicesEncodingData> {
        MeshEdgebreakerDecoder::get_attribute_encoding_data(self, att_id)
    }
}
