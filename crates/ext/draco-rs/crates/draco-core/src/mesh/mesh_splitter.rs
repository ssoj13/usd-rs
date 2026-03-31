//! Mesh splitter utilities (transcoder-related).
//!
//! What: Splits a mesh into sub-meshes by attribute values or connected components.
//! Why: Matches Draco mesh splitter behavior for glTF and transcoder workflows.
//! How: Uses TriangleSoupMeshBuilder/PointCloudBuilder and copies metadata/materials/features.
//! Where used: Transcoder pipelines and scene/mesh partitioning.

use std::collections::HashMap;
use std::marker::PhantomData;

use crate::attributes::geometry_attribute::GeometryAttributeType;
use crate::attributes::geometry_indices::{FaceIndex, PointIndex};
use crate::attributes::point_attribute::PointAttribute;
use crate::core::draco_types::DataType;
use crate::core::status::{ok_status, Status, StatusCode};
use crate::core::status_or::StatusOr;
use crate::mesh::mesh::Mesh;
use crate::mesh::mesh_connected_components::MeshConnectedComponents;
use crate::mesh::mesh_features::MeshFeatures;
use crate::mesh::mesh_indices::MeshFeaturesIndex;
use crate::mesh::mesh_utils::MeshUtils;
use crate::mesh::triangle_soup_mesh_builder::TriangleSoupMeshBuilder;
use crate::point_cloud::point_cloud_builder::PointCloudBuilder;
use crate::texture::texture::Texture;

/// Vector of optional meshes (unused slots are None, matching C++ null entries).
pub type MeshVector = Vec<Option<Box<Mesh>>>;

/// Class that can be used to split a single mesh into multiple sub-meshes.
pub struct MeshSplitter {
    preserve_materials: bool,
    remove_unused_material_indices: bool,
    preserve_mesh_features: bool,
    preserve_structural_metadata: bool,
    deduplicate_vertices: bool,
    att_id_map: Vec<i32>,
}

impl MeshSplitter {
    pub fn new() -> Self {
        Self {
            preserve_materials: false,
            remove_unused_material_indices: true,
            preserve_mesh_features: false,
            preserve_structural_metadata: false,
            deduplicate_vertices: true,
            att_id_map: Vec::new(),
        }
    }

    /// Preserves all materials on the input mesh during splitting.
    pub fn set_preserve_materials(&mut self, flag: bool) {
        self.preserve_materials = flag;
    }

    /// Removes unused material indices on generated sub-meshes (default true).
    pub fn set_remove_unused_material_indices(&mut self, flag: bool) {
        self.remove_unused_material_indices = flag;
    }

    /// Preserves all mesh features on the input mesh during splitting.
    pub fn set_preserve_mesh_features(&mut self, flag: bool) {
        self.preserve_mesh_features = flag;
    }

    /// Preserves split-specific structural metadata on the input mesh.
    ///
    /// Top-level `StructuralMetadata` is still copied to output meshes
    /// unconditionally for Draco parity; this flag controls split-specific
    /// property-attribute propagation.
    pub fn set_preserve_structural_metadata(&mut self, flag: bool) {
        self.preserve_structural_metadata = flag;
    }

    /// Enables/disables vertex deduplication for point-cloud splits.
    pub fn set_deduplicate_vertices(&mut self, flag: bool) {
        self.deduplicate_vertices = flag;
    }

    /// Splits |mesh| according to attribute values stored in |split_attribute_id|.
    pub fn split_mesh(&mut self, mesh: &Mesh, split_attribute_id: u32) -> StatusOr<MeshVector> {
        let split_attribute_id = split_attribute_id as i32;
        if split_attribute_id < 0 || split_attribute_id >= mesh.num_attributes() {
            return StatusOr::new_status(Status::new(
                StatusCode::DracoError,
                "Invalid attribute id.",
            ));
        }
        if mesh.num_faces() == 0 {
            self.split_mesh_internal::<PointCloudBuilder>(mesh, split_attribute_id)
        } else {
            self.split_mesh_internal::<TriangleSoupMeshBuilder>(mesh, split_attribute_id)
        }
    }

    /// Splits |mesh| into separate meshes defined by |connected_components|.
    pub fn split_mesh_to_components(
        &mut self,
        mesh: &Mesh,
        connected_components: &MeshConnectedComponents,
    ) -> StatusOr<MeshVector> {
        let num_out_meshes = connected_components.num_connected_components();
        let mut splitter_internal = MeshSplitterInternal::<TriangleSoupMeshBuilder>::new();
        let mut work_data = WorkData::<TriangleSoupMeshBuilder>::new(num_out_meshes as usize);
        self.att_id_map = vec![-1; mesh.num_attributes() as usize];
        // Initialize builders before the loop that calls initialize_builder
        work_data.builders = (0..num_out_meshes as usize)
            .map(|_| TriangleSoupMeshBuilder::default())
            .collect();

        for mi in 0..num_out_meshes {
            let num_faces = connected_components.num_connected_component_faces(mi);
            work_data.num_sub_mesh_elements[mi as usize] = num_faces;
            splitter_internal.initialize_builder(
                mi as usize,
                num_faces,
                mesh,
                -1,
                &mut work_data,
                &mut self.att_id_map,
            );
        }

        for mi in 0..num_out_meshes {
            let num_faces = connected_components.num_connected_component_faces(mi);
            for cfi in 0..num_faces {
                let face_index = connected_components.get_connected_component_face(mi, cfi);
                let source_fi = FaceIndex::from(face_index as u32);
                let target_fi = FaceIndex::from(cfi as u32);
                add_element_to_builder_triangle(
                    mi as usize,
                    source_fi,
                    target_fi,
                    mesh,
                    &mut work_data,
                    &self.att_id_map,
                );
            }
        }

        let out_meshes =
            splitter_internal.build_meshes(mesh, &mut work_data, self.deduplicate_vertices);
        if !out_meshes.is_ok() {
            return StatusOr::new_status(out_meshes.status().clone());
        }
        self.finalize_meshes(mesh, &work_data, out_meshes.into_value())
    }

    /// Returns attribute index on each split mesh that corresponds to the source.
    pub fn get_split_mesh_attribute_index(&self, source_mesh_att_index: i32) -> i32 {
        if source_mesh_att_index < 0 || source_mesh_att_index as usize >= self.att_id_map.len() {
            return -1;
        }
        self.att_id_map[source_mesh_att_index as usize]
    }

    fn split_mesh_internal<BuilderT>(
        &mut self,
        mesh: &Mesh,
        split_attribute_id: i32,
    ) -> StatusOr<MeshVector>
    where
        BuilderT: MeshSplitBuilder + Default,
        MeshSplitterInternal<BuilderT>: MeshSplitterInternalOps<BuilderT>,
    {
        let split_attribute = match mesh.attribute(split_attribute_id) {
            Some(att) => att,
            None => {
                return StatusOr::new_status(Status::new(
                    StatusCode::DracoError,
                    "Invalid attribute id.",
                ))
            }
        };

        let preserve_split_attribute = self.preserve_materials
            && split_attribute.attribute_type() == GeometryAttributeType::Material;

        let num_out_meshes = split_attribute.size();
        let mut splitter_internal = MeshSplitterInternal::<BuilderT>::new();
        let mut work_data = WorkData::<BuilderT>::new(num_out_meshes);
        work_data.split_by_materials =
            split_attribute.attribute_type() == GeometryAttributeType::Material;

        let status = splitter_internal.initialize_work_data_num_elements(
            mesh,
            split_attribute_id,
            &mut work_data,
        );
        if !status.is_ok() {
            return StatusOr::new_status(status);
        }

        work_data.builders = (0..num_out_meshes).map(|_| BuilderT::default()).collect();
        self.att_id_map = vec![-1; mesh.num_attributes() as usize];
        let ignored_att_id = if preserve_split_attribute {
            -1
        } else {
            split_attribute_id
        };

        for mi in 0..num_out_meshes {
            if work_data.num_sub_mesh_elements[mi] == 0 {
                continue;
            }
            let num_elements = work_data.num_sub_mesh_elements[mi];
            splitter_internal.initialize_builder(
                mi,
                num_elements,
                mesh,
                ignored_att_id,
                &mut work_data,
                &mut self.att_id_map,
            );
            work_data.num_sub_mesh_elements[mi] = 0;
        }

        splitter_internal.add_elements_to_builder(
            mesh,
            split_attribute,
            &mut work_data,
            &self.att_id_map,
        );

        let out_meshes =
            splitter_internal.build_meshes(mesh, &mut work_data, self.deduplicate_vertices);
        if !out_meshes.is_ok() {
            return StatusOr::new_status(out_meshes.status().clone());
        }
        self.finalize_meshes(mesh, &work_data, out_meshes.into_value())
    }

    fn finalize_meshes(
        &self,
        mesh: &Mesh,
        work_data: &WorkData<impl MeshSplitBuilder>,
        mut out_meshes: MeshVector,
    ) -> StatusOr<MeshVector> {
        let mut features_texture_to_index_map: HashMap<*const Texture, i32> = HashMap::new();
        if self.preserve_mesh_features {
            features_texture_to_index_map = mesh
                .non_material_texture_library()
                .compute_texture_to_index_map();
        }

        for (mi, out_mesh_opt) in out_meshes.iter_mut().enumerate() {
            let out_mesh = match out_mesh_opt.as_mut() {
                Some(mesh) => mesh.as_mut(),
                None => continue,
            };

            out_mesh.set_name(mesh.name());

            if self.preserve_materials {
                if work_data.split_by_materials {
                    if out_mesh.num_points() != 0 && mesh.material_library().num_materials() != 0 {
                        let material_index = {
                            let mat_att = match out_mesh
                                .get_named_attribute(GeometryAttributeType::Material)
                            {
                                Some(attribute) => attribute,
                                None => {
                                    return StatusOr::new_status(Status::new(
                                        StatusCode::DracoError,
                                        "Missing material attribute.",
                                    ));
                                }
                            };
                            let mapped = mat_att.mapped_index(PointIndex::from(0u32));
                            let mut value = [0i32; 1];
                            if !mat_att.convert_value(mapped, 1, &mut value) {
                                return StatusOr::new_status(Status::new(
                                    StatusCode::DracoError,
                                    "Invalid material attribute value.",
                                ));
                            }
                            value[0]
                        };

                        let src_material_library = mesh.material_library();
                        let src_num_materials = src_material_library.num_materials();
                        let src_num_textures =
                            src_material_library.texture_library().num_textures();

                        {
                            let dst_library = out_mesh.material_library_mut();
                            if src_num_materials > 0 {
                                let _ =
                                    dst_library.mutable_material((src_num_materials - 1) as i32);
                            }
                            for _ in 0..src_num_textures {
                                let _ = dst_library
                                    .texture_library_mut()
                                    .push_texture(Box::new(Texture::new()));
                            }
                        }

                        if let Some(src_material) = src_material_library.material(material_index) {
                            if let Some(dst_material) = out_mesh
                                .material_library_mut()
                                .mutable_material(material_index)
                            {
                                dst_material.copy_from(src_material);
                            }

                            let texture_to_index = src_material_library
                                .texture_library()
                                .compute_texture_to_index_map();

                            for tmi in 0..src_material.num_texture_maps() {
                                let source_map = match src_material.texture_map_by_index(tmi as i32)
                                {
                                    Some(map) => map,
                                    None => continue,
                                };
                                let source_texture = match source_map.texture() {
                                    Some(texture) => texture,
                                    None => continue,
                                };
                                let texture_index = match texture_to_index
                                    .get(&(source_texture as *const Texture))
                                {
                                    Some(index) => *index,
                                    None => continue,
                                };

                                let dst_texture_ptr = {
                                    let dst_library = out_mesh.material_library_mut();
                                    let dst_texture = match dst_library
                                        .texture_library_mut()
                                        .texture_mut(texture_index)
                                    {
                                        Some(texture) => texture,
                                        None => continue,
                                    };
                                    dst_texture as *mut Texture
                                };

                                let dst_library = out_mesh.material_library_mut();
                                if let Some(dst_material) =
                                    dst_library.mutable_material(material_index)
                                {
                                    if let Some(dst_map) =
                                        dst_material.texture_map_by_index_mut(tmi as i32)
                                    {
                                        dst_map.set_texture_ptr(dst_texture_ptr);
                                        if let Some(dst_texture) = dst_map.texture_mut() {
                                            dst_texture.copy_from(source_texture);
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    out_mesh
                        .material_library_mut()
                        .copy_from(mesh.material_library());
                }
            }

            if let Some(metadata) = mesh.get_metadata() {
                out_mesh.add_metadata(metadata.clone());
            }

            for att_id in 0..mesh.num_attributes() {
                let mapped_att_id = self.att_id_map[att_id as usize];
                if mapped_att_id == -1 {
                    continue;
                }
                let src_att = mesh.attribute(att_id).expect("Invalid attribute id");
                let dst_att = out_mesh
                    .attribute_mut(mapped_att_id)
                    .expect("Invalid attribute id");
                dst_att.set_unique_id(src_att.unique_id());
            }

            out_mesh.set_compression_enabled(mesh.is_compression_enabled());
            out_mesh.set_compression_options(mesh.compression_options().clone());

            if self.preserve_mesh_features {
                for mfi in 0..mesh.num_mesh_features() {
                    let mfi_index = MeshFeaturesIndex::from(mfi as u32);
                    if work_data.split_by_materials {
                        let mut is_used = false;
                        if mesh.num_mesh_features_material_masks(mfi_index) == 0 {
                            is_used = true;
                        } else {
                            for mask_index in 0..mesh.num_mesh_features_material_masks(mfi_index) {
                                if mesh.mesh_features_material_mask(mfi_index, mask_index as i32)
                                    == mi as i32
                                {
                                    is_used = true;
                                    break;
                                }
                            }
                        }
                        if !is_used {
                            continue;
                        }
                    }

                    let mut mf = Box::new(MeshFeatures::new());
                    mf.copy_from(mesh.mesh_features(mfi_index));
                    if mf.attribute_index() != -1 {
                        let new_index = self.att_id_map[mf.attribute_index() as usize];
                        mf.set_attribute_index(new_index);
                    }
                    let new_mfi = out_mesh.add_mesh_features(mf);
                    if work_data.split_by_materials && !self.preserve_materials {
                        out_mesh.add_mesh_features_material_mask(new_mfi, 0);
                    } else {
                        for mask_index in 0..mesh.num_mesh_features_material_masks(mfi_index) {
                            out_mesh.add_mesh_features_material_mask(
                                new_mfi,
                                mesh.mesh_features_material_mask(mfi_index, mask_index as i32),
                            );
                        }
                    }
                }

                out_mesh
                    .non_material_texture_library_mut()
                    .copy_from(mesh.non_material_texture_library());

                // Use raw pointers to avoid aliasing mutable borrows of out_mesh.
                let texture_library_ptr = out_mesh.non_material_texture_library_mut() as *mut _;
                for mfi in 0..out_mesh.num_mesh_features() {
                    let mfi_index = MeshFeaturesIndex::from(mfi as u32);
                    let mesh_features_ptr = out_mesh.mesh_features_mut(mfi_index) as *mut _;
                    unsafe {
                        Mesh::update_mesh_features_texture_pointer(
                            &features_texture_to_index_map,
                            &mut *texture_library_ptr,
                            &mut *mesh_features_ptr,
                        );
                    }
                }

                let status = MeshUtils::remove_unused_mesh_features(out_mesh);
                if !status.is_ok() {
                    return StatusOr::new_status(status);
                }
            }

            if self.preserve_structural_metadata {
                for i in 0..mesh.num_property_attributes_indices() {
                    if work_data.split_by_materials {
                        let mut is_used = false;
                        if mesh.num_property_attributes_index_material_masks(i) == 0 {
                            is_used = true;
                        } else {
                            for mask_index in
                                0..mesh.num_property_attributes_index_material_masks(i)
                            {
                                if mesh
                                    .property_attributes_index_material_mask(i, mask_index as i32)
                                    == mi as i32
                                {
                                    is_used = true;
                                    break;
                                }
                            }
                        }
                        if !is_used {
                            continue;
                        }
                    }
                    let new_i =
                        out_mesh.add_property_attributes_index(mesh.property_attributes_index(i));
                    if work_data.split_by_materials && !self.preserve_materials {
                        out_mesh.add_property_attributes_index_material_mask(new_i, 0);
                    } else {
                        for mask_index in 0..mesh.num_property_attributes_index_material_masks(i) {
                            out_mesh.add_property_attributes_index_material_mask(
                                new_i,
                                mesh.property_attributes_index_material_mask(i, mask_index as i32),
                            );
                        }
                    }
                }

                let status = MeshUtils::remove_unused_property_attributes_indices(out_mesh);
                if !status.is_ok() {
                    return StatusOr::new_status(status);
                }
            }

            if self.preserve_materials {
                out_mesh.remove_unused_materials_with(self.remove_unused_material_indices);
            }

            out_mesh
                .structural_metadata_mut()
                .copy_from(mesh.structural_metadata());
        }
        StatusOr::new_value(out_meshes)
    }
}

impl Default for MeshSplitter {
    fn default() -> Self {
        Self::new()
    }
}

struct WorkData<BuilderT> {
    num_sub_mesh_elements: Vec<i32>,
    split_by_materials: bool,
    builders: Vec<BuilderT>,
}

impl<BuilderT> WorkData<BuilderT> {
    fn new(num_out_meshes: usize) -> Self {
        Self {
            num_sub_mesh_elements: vec![0; num_out_meshes],
            split_by_materials: false,
            builders: Vec::new(),
        }
    }
}

trait MeshSplitBuilder {
    fn start(&mut self, num_elements: i32);
    fn add_attribute(
        &mut self,
        attribute_type: GeometryAttributeType,
        num_components: i8,
        data_type: DataType,
        normalized: bool,
    ) -> i32;
    fn set_attribute_name(&mut self, att_id: i32, name: &str);
}

impl MeshSplitBuilder for TriangleSoupMeshBuilder {
    fn start(&mut self, num_elements: i32) {
        TriangleSoupMeshBuilder::start(self, num_elements);
    }

    fn add_attribute(
        &mut self,
        attribute_type: GeometryAttributeType,
        num_components: i8,
        data_type: DataType,
        normalized: bool,
    ) -> i32 {
        TriangleSoupMeshBuilder::add_attribute_with_normalized(
            self,
            attribute_type,
            num_components,
            data_type,
            normalized,
        )
    }

    fn set_attribute_name(&mut self, att_id: i32, name: &str) {
        TriangleSoupMeshBuilder::set_attribute_name(self, att_id, name);
    }
}

impl MeshSplitBuilder for PointCloudBuilder {
    fn start(&mut self, num_elements: i32) {
        PointCloudBuilder::start(self, num_elements as u32);
    }

    fn add_attribute(
        &mut self,
        attribute_type: GeometryAttributeType,
        num_components: i8,
        data_type: DataType,
        normalized: bool,
    ) -> i32 {
        PointCloudBuilder::add_attribute_with_normalized(
            self,
            attribute_type,
            num_components,
            data_type,
            normalized,
        )
    }

    fn set_attribute_name(&mut self, att_id: i32, name: &str) {
        PointCloudBuilder::set_attribute_name(self, att_id, name);
    }
}

struct MeshSplitterInternal<BuilderT> {
    _marker: PhantomData<BuilderT>,
}

impl<BuilderT> MeshSplitterInternal<BuilderT> {
    fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<BuilderT> MeshSplitterInternal<BuilderT>
where
    BuilderT: MeshSplitBuilder,
{
    fn initialize_builder(
        &mut self,
        b_index: usize,
        num_elements: i32,
        mesh: &Mesh,
        ignored_attribute_id: i32,
        work_data: &mut WorkData<BuilderT>,
        att_id_map: &mut [i32],
    ) {
        work_data.builders[b_index].start(num_elements);
        for ai in 0..mesh.num_attributes() {
            if ai == ignored_attribute_id {
                continue;
            }
            let src_att = mesh.attribute(ai).expect("Invalid attribute id");
            let new_att_id = work_data.builders[b_index].add_attribute(
                src_att.attribute_type(),
                src_att.num_components() as i8,
                src_att.data_type(),
                src_att.normalized(),
            );
            att_id_map[ai as usize] = new_att_id;
            work_data.builders[b_index].set_attribute_name(new_att_id, src_att.name());
        }
    }
}

trait MeshSplitterInternalOps<BuilderT> {
    fn initialize_work_data_num_elements(
        &mut self,
        mesh: &Mesh,
        split_attribute_id: i32,
        work_data: &mut WorkData<BuilderT>,
    ) -> Status;

    fn add_elements_to_builder(
        &mut self,
        mesh: &Mesh,
        split_attribute: &PointAttribute,
        work_data: &mut WorkData<BuilderT>,
        att_id_map: &[i32],
    );

    fn build_meshes(
        &mut self,
        mesh: &Mesh,
        work_data: &mut WorkData<BuilderT>,
        deduplicate_vertices: bool,
    ) -> StatusOr<MeshVector>;
}

impl MeshSplitterInternalOps<TriangleSoupMeshBuilder>
    for MeshSplitterInternal<TriangleSoupMeshBuilder>
{
    fn initialize_work_data_num_elements(
        &mut self,
        mesh: &Mesh,
        split_attribute_id: i32,
        work_data: &mut WorkData<TriangleSoupMeshBuilder>,
    ) -> Status {
        let split_attribute = mesh
            .attribute(split_attribute_id)
            .expect("Invalid attribute id");
        for fi in 0..mesh.num_faces() {
            let face = mesh.face(FaceIndex::from(fi as u32));
            let avi = split_attribute.mapped_index(face[0]);
            for c in 1..3 {
                if split_attribute.mapped_index(face[c]) != avi {
                    return Status::new(
                        StatusCode::DracoError,
                        "Attribute values not consistent on a face.",
                    );
                }
            }
            work_data.num_sub_mesh_elements[avi.value() as usize] += 1;
        }
        ok_status()
    }

    fn add_elements_to_builder(
        &mut self,
        mesh: &Mesh,
        split_attribute: &PointAttribute,
        work_data: &mut WorkData<TriangleSoupMeshBuilder>,
        att_id_map: &[i32],
    ) {
        for fi in 0..mesh.num_faces() {
            let face = mesh.face(FaceIndex::from(fi as u32));
            let sub_mesh_id = split_attribute.mapped_index(face[0]).value() as usize;
            let target_index = work_data.num_sub_mesh_elements[sub_mesh_id] as u32;
            let target_fi = FaceIndex::from(target_index);
            add_element_to_builder_triangle(
                sub_mesh_id,
                FaceIndex::from(fi as u32),
                target_fi,
                mesh,
                work_data,
                att_id_map,
            );
            work_data.num_sub_mesh_elements[sub_mesh_id] += 1;
        }
    }

    fn build_meshes(
        &mut self,
        _mesh: &Mesh,
        work_data: &mut WorkData<TriangleSoupMeshBuilder>,
        _deduplicate_vertices: bool,
    ) -> StatusOr<MeshVector> {
        let num_out_meshes = work_data.builders.len();
        let mut out_meshes: MeshVector = Vec::with_capacity(num_out_meshes);
        for _ in 0..num_out_meshes {
            out_meshes.push(None);
        }
        for mi in 0..num_out_meshes {
            if work_data.num_sub_mesh_elements[mi] == 0 {
                continue;
            }
            let mesh = match work_data.builders[mi].finalize() {
                Some(mesh) => mesh,
                None => continue,
            };
            out_meshes[mi] = Some(Box::new(mesh));
        }
        StatusOr::new_value(out_meshes)
    }
}

impl MeshSplitterInternalOps<PointCloudBuilder> for MeshSplitterInternal<PointCloudBuilder> {
    fn initialize_work_data_num_elements(
        &mut self,
        mesh: &Mesh,
        split_attribute_id: i32,
        work_data: &mut WorkData<PointCloudBuilder>,
    ) -> Status {
        let split_attribute = mesh
            .attribute(split_attribute_id)
            .expect("Invalid attribute id");
        for pi in 0..mesh.num_points() {
            let avi = split_attribute.mapped_index(PointIndex::from(pi));
            work_data.num_sub_mesh_elements[avi.value() as usize] += 1;
        }
        ok_status()
    }

    fn add_elements_to_builder(
        &mut self,
        mesh: &Mesh,
        split_attribute: &PointAttribute,
        work_data: &mut WorkData<PointCloudBuilder>,
        att_id_map: &[i32],
    ) {
        for pi in 0..mesh.num_points() {
            let sub_mesh_id = split_attribute.mapped_index(PointIndex::from(pi)).value() as usize;
            let target_index = work_data.num_sub_mesh_elements[sub_mesh_id] as u32;
            let target_pi = PointIndex::from(target_index);
            add_element_to_builder_point(
                sub_mesh_id,
                PointIndex::from(pi),
                target_pi,
                mesh,
                work_data,
                att_id_map,
            );
            work_data.num_sub_mesh_elements[sub_mesh_id] += 1;
        }
    }

    fn build_meshes(
        &mut self,
        _mesh: &Mesh,
        work_data: &mut WorkData<PointCloudBuilder>,
        deduplicate_vertices: bool,
    ) -> StatusOr<MeshVector> {
        let num_out_meshes = work_data.builders.len();
        let mut out_meshes: MeshVector = Vec::with_capacity(num_out_meshes);
        for _ in 0..num_out_meshes {
            out_meshes.push(None);
        }
        for mi in 0..num_out_meshes {
            if work_data.num_sub_mesh_elements[mi] == 0 {
                continue;
            }
            let pc = match work_data.builders[mi].finalize(deduplicate_vertices) {
                Some(pc) => pc,
                None => continue,
            };
            let mut mesh = Mesh::new();
            mesh.copy(&pc);
            out_meshes[mi] = Some(Box::new(mesh));
        }
        StatusOr::new_value(out_meshes)
    }
}

fn add_element_to_builder_triangle(
    b_index: usize,
    source_i: FaceIndex,
    target_i: FaceIndex,
    mesh: &Mesh,
    work_data: &mut WorkData<TriangleSoupMeshBuilder>,
    att_id_map: &[i32],
) {
    let face = mesh.face(source_i);
    for ai in 0..mesh.num_attributes() {
        let target_att_id = att_id_map[ai as usize];
        if target_att_id == -1 {
            continue;
        }
        let src_att = mesh.attribute(ai).expect("Invalid attribute id");
        let stride = src_att.byte_stride() as usize;
        let mut bytes_0 = vec![0u8; stride];
        let mut bytes_1 = vec![0u8; stride];
        let mut bytes_2 = vec![0u8; stride];
        src_att.get_value_bytes_by_point(face[0], &mut bytes_0);
        src_att.get_value_bytes_by_point(face[1], &mut bytes_1);
        src_att.get_value_bytes_by_point(face[2], &mut bytes_2);
        work_data.builders[b_index].set_attribute_values_for_face_bytes(
            target_att_id,
            target_i,
            &bytes_0,
            &bytes_1,
            &bytes_2,
        );
    }
}

fn add_element_to_builder_point(
    b_index: usize,
    source_i: PointIndex,
    target_i: PointIndex,
    mesh: &Mesh,
    work_data: &mut WorkData<PointCloudBuilder>,
    att_id_map: &[i32],
) {
    for ai in 0..mesh.num_attributes() {
        let target_att_id = att_id_map[ai as usize];
        if target_att_id == -1 {
            continue;
        }
        let src_att = mesh.attribute(ai).expect("Invalid attribute id");
        let stride = src_att.byte_stride() as usize;
        let mut bytes = vec![0u8; stride];
        src_att.get_value_bytes_by_point(source_i, &mut bytes);
        work_data.builders[b_index].set_attribute_value_for_point(target_att_id, target_i, &bytes);
    }
}
