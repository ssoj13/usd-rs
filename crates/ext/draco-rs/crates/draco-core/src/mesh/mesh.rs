//! Mesh implementation.
//! Reference: `_ref/draco/src/draco/mesh/mesh.h` + `.cc`.

use std::collections::{BTreeMap, HashMap};

use crate::attributes::geometry_attribute::{GeometryAttribute, GeometryAttributeType};
use crate::attributes::geometry_indices::{
    AttributeValueIndex, CornerIndex, FaceIndex, PointIndex, INVALID_CORNER_INDEX,
    INVALID_POINT_INDEX,
};
use crate::attributes::point_attribute::PointAttribute;
use crate::core::draco_index_type_vector::IndexTypeVector;
use crate::core::draco_types::DataType;
use crate::core::hash_utils::hash_combine_with;
use crate::draco_dcheck;
use crate::material::material_library::MaterialLibrary;
use crate::mesh::mesh_features::MeshFeatures;
use crate::mesh::mesh_indices::MeshFeaturesIndex;
use crate::metadata::structural_metadata::StructuralMetadata;
use crate::point_cloud::{build_point_deduplication_map, PointCloud, PointCloudHasher};
use crate::texture::texture::Texture;
use crate::texture::texture_library::TextureLibrary;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum MeshAttributeElementType {
    MeshVertexAttribute = 0,
    MeshCornerAttribute = 1,
    MeshFaceAttribute = 2,
}

impl Default for MeshAttributeElementType {
    fn default() -> Self {
        MeshAttributeElementType::MeshCornerAttribute
    }
}

pub type Face = [PointIndex; 3];

#[derive(Clone, Copy, Debug)]
pub struct AttributeData {
    pub element_type: MeshAttributeElementType,
}

impl Default for AttributeData {
    fn default() -> Self {
        Self {
            element_type: MeshAttributeElementType::MeshCornerAttribute,
        }
    }
}

pub struct Mesh {
    point_cloud: PointCloud,
    attribute_data: Vec<AttributeData>,
    faces: IndexTypeVector<FaceIndex, Face>,
    // Mesh name (transcoder metadata).
    name: String,
    // Materials applied to this mesh.
    material_library: MaterialLibrary,
    // Mesh feature ID sets (EXT_mesh_features).
    mesh_features: Vec<Box<MeshFeatures>>,
    // Optional material masks for mesh feature sets.
    mesh_features_material_mask: Vec<Vec<i32>>,
    // Non-material texture library (used by mesh features).
    non_material_texture_library: TextureLibrary,
    // Structural metadata (EXT_structural_metadata).
    structural_metadata: StructuralMetadata,
    // Indices pointing to property attributes stored in StructuralMetadata.
    property_attributes: Vec<i32>,
    // Optional material masks for property attributes indices.
    property_attributes_material_mask: Vec<Vec<i32>>,
}

impl Mesh {
    pub fn new() -> Self {
        Self {
            point_cloud: PointCloud::new(),
            attribute_data: Vec::new(),
            faces: IndexTypeVector::new(),
            name: String::new(),
            material_library: MaterialLibrary::new(),
            mesh_features: Vec::new(),
            mesh_features_material_mask: Vec::new(),
            non_material_texture_library: TextureLibrary::new(),
            structural_metadata: StructuralMetadata::new(),
            property_attributes: Vec::new(),
            property_attributes_material_mask: Vec::new(),
        }
    }

    /// Copies all data from `src` into this mesh.
    pub fn copy_from(&mut self, src: &Mesh) {
        self.point_cloud.copy(&src.point_cloud);
        self.name = src.name.clone();
        self.faces = src.faces.clone();
        self.attribute_data = src.attribute_data.clone();
        self.material_library.copy_from(&src.material_library);

        self.mesh_features.clear();
        for mesh_features in &src.mesh_features {
            let mut new_features = Box::new(MeshFeatures::new());
            new_features.copy_from(mesh_features);
            self.mesh_features.push(new_features);
        }
        self.mesh_features_material_mask = src.mesh_features_material_mask.clone();

        self.non_material_texture_library
            .copy_from(&src.non_material_texture_library);
        if self.non_material_texture_library.num_textures() != 0 {
            let texture_to_index_map = src
                .non_material_texture_library
                .compute_texture_to_index_map();
            for features in &mut self.mesh_features {
                Mesh::update_mesh_features_texture_pointer(
                    &texture_to_index_map,
                    &mut self.non_material_texture_library,
                    features,
                );
            }
        }
        self.structural_metadata = src.structural_metadata.clone();
        self.property_attributes = src.property_attributes.clone();
        self.property_attributes_material_mask = src.property_attributes_material_mask.clone();
    }

    pub fn add_face(&mut self, face: Face) {
        self.faces.push_back(face);
    }

    pub fn set_face(&mut self, face_id: FaceIndex, face: Face) {
        if face_id.value() as usize >= self.faces.size() {
            self.faces
                .resize_with_value(face_id.value() as usize + 1, default_face());
        }
        self.faces[face_id] = face;
    }

    pub fn set_num_faces(&mut self, num_faces: usize) {
        self.faces.resize_with_value(num_faces, default_face());
    }

    pub fn num_faces(&self) -> u32 {
        self.faces.size() as u32
    }

    pub fn face(&self, face_id: FaceIndex) -> &Face {
        draco_dcheck!(face_id.value() < self.faces.size() as u32);
        &self.faces[face_id]
    }

    pub fn set_attribute(&mut self, att_id: i32, pa: PointAttribute) {
        self.point_cloud.set_attribute(att_id, pa);
        if att_id < 0 {
            return;
        }
        let idx = att_id as usize;
        if self.attribute_data.len() <= idx {
            self.attribute_data
                .resize(idx + 1, AttributeData::default());
        }
    }

    /// Ensures attribute element metadata is sized for all attributes.
    pub fn sync_attribute_data(&mut self) {
        let num_attributes = self.num_attributes();
        if num_attributes <= 0 {
            return;
        }
        let num_attributes = num_attributes as usize;
        if self.attribute_data.len() < num_attributes {
            self.attribute_data
                .resize(num_attributes, AttributeData::default());
        }
    }

    /// Sets mesh name.
    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    /// Returns mesh name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns material library.
    pub fn material_library(&self) -> &MaterialLibrary {
        &self.material_library
    }

    /// Returns mutable material library.
    pub fn material_library_mut(&mut self) -> &mut MaterialLibrary {
        &mut self.material_library
    }

    /// Removes unused materials (default behavior removes unused indices).
    pub fn remove_unused_materials(&mut self) {
        self.remove_unused_materials_with(true);
    }

    /// Removes unused materials with optional index removal control.
    pub fn remove_unused_materials_with(&mut self, remove_unused_material_indices: bool) {
        let mat_att_index = self.get_named_attribute_id(GeometryAttributeType::Material);
        if mat_att_index == -1 {
            while self.material_library.num_materials() > 1 {
                let _ = self.material_library.remove_material(1);
            }
            self.material_library.remove_unused_textures();
            return;
        }

        if let Some(att) = self.attribute_mut(mat_att_index) {
            let _ = att.deduplicate_values();
        }

        let num_materials = self.material_library.num_materials();
        let mut is_material_used = vec![false; num_materials];
        let mut num_used_materials = 0usize;

        {
            let mat_att = match self.attribute(mat_att_index) {
                Some(att) => att,
                None => return,
            };
            let mut update_used_materials = |pi: PointIndex| {
                let mut value = [0u32; 1];
                // Convert stored attribute data to u32 (matches Draco GetMappedValue behavior).
                let _ = mat_att.convert_value(mat_att.mapped_index(pi), 1, &mut value);
                let mat_index = value[0] as usize;
                if mat_index < num_materials && !is_material_used[mat_index] {
                    is_material_used[mat_index] = true;
                    num_used_materials += 1;
                }
            };

            if self.num_faces() > 0 {
                for fi in 0..self.num_faces() {
                    update_used_materials(self.faces[FaceIndex::from(fi)][0]);
                }
            } else {
                for pi in 0..self.num_points() {
                    update_used_materials(PointIndex::from(pi));
                }
            }
        }

        for (mfi, masks) in self.mesh_features_material_mask.iter().enumerate() {
            for &mat_index in masks {
                let mat_index = mat_index as usize;
                if mat_index < num_materials && !is_material_used[mat_index] {
                    is_material_used[mat_index] = true;
                    num_used_materials += 1;
                }
            }
            if mfi >= self.mesh_features.len() {
                break;
            }
        }

        for masks in &self.property_attributes_material_mask {
            for &mat_index in masks {
                let mat_index = mat_index as usize;
                if mat_index < num_materials && !is_material_used[mat_index] {
                    is_material_used[mat_index] = true;
                    num_used_materials += 1;
                }
            }
        }

        if num_used_materials == num_materials {
            return;
        }

        for mi in (0..num_materials).rev() {
            if !is_material_used[mi] && mi < self.material_library.num_materials() {
                if remove_unused_material_indices {
                    let _ = self.material_library.remove_material(mi as i32);
                } else if let Some(material) = self.material_library.mutable_material(mi as i32) {
                    material.clear();
                }
            }
        }
        self.material_library.remove_unused_textures();

        if !remove_unused_material_indices {
            return;
        }

        let mut old_to_new_material_index_map = vec![-1; num_materials];
        let mut new_material_index = 0i32;
        for mi in 0..num_materials {
            if is_material_used[mi] {
                old_to_new_material_index_map[mi] = new_material_index;
                new_material_index += 1;
            }
        }

        let mut old_to_new_material_attribute_value_index_map: Vec<i32> = Vec::new();
        if let Some(mat_att) = self.attribute(mat_att_index) {
            old_to_new_material_attribute_value_index_map = vec![-1; mat_att.size()];
            for avi in 0..mat_att.size() {
                let mut value = [0u32; 1];
                // Convert stored attribute data to u32 for stable index mapping.
                let _ = mat_att.convert_value(AttributeValueIndex::from(avi as u32), 1, &mut value);
                let mat_index = value[0] as usize;
                if mat_index < num_materials && is_material_used[mat_index] {
                    old_to_new_material_attribute_value_index_map[avi] =
                        old_to_new_material_index_map[mat_index];
                }
            }
        }

        let num_points = self.num_points();
        if let Some(mat_att) = self.attribute_mut(mat_att_index) {
            let _ = mat_att.reset(num_used_materials);
            for avi in 0..mat_att.size() {
                let mat_index = avi as u32;
                // Preserve attribute storage width when rewriting material indices.
                match mat_att.data_type() {
                    DataType::Uint8 => {
                        let v = mat_index as u8;
                        mat_att.set_attribute_value(AttributeValueIndex::from(avi as u32), &v);
                    }
                    DataType::Uint16 => {
                        let v = mat_index as u16;
                        mat_att.set_attribute_value(AttributeValueIndex::from(avi as u32), &v);
                    }
                    _ => {
                        mat_att
                            .set_attribute_value(AttributeValueIndex::from(avi as u32), &mat_index);
                    }
                }
            }
            for pi in 0..num_points {
                let old_avi = mat_att.mapped_index(PointIndex::from(pi));
                let new_avi =
                    old_to_new_material_attribute_value_index_map[old_avi.value() as usize];
                if new_avi >= 0 {
                    mat_att.set_point_map_entry(
                        PointIndex::from(pi),
                        AttributeValueIndex::from(new_avi as u32),
                    );
                }
            }
        }

        for masks in &mut self.mesh_features_material_mask {
            for mask in masks.iter_mut() {
                let old_index = *mask as usize;
                if old_index < num_materials && is_material_used[old_index] {
                    *mask = old_to_new_material_index_map[old_index];
                }
            }
        }

        for masks in &mut self.property_attributes_material_mask {
            for mask in masks.iter_mut() {
                let old_index = *mask as usize;
                if old_index < num_materials && is_material_used[old_index] {
                    *mask = old_to_new_material_index_map[old_index];
                }
            }
        }
    }

    /// Returns non-material texture library (used by mesh features).
    pub fn non_material_texture_library(&self) -> &TextureLibrary {
        &self.non_material_texture_library
    }

    /// Returns mutable non-material texture library.
    pub fn non_material_texture_library_mut(&mut self) -> &mut TextureLibrary {
        &mut self.non_material_texture_library
    }

    /// Returns both texture libraries for move operations (e.g. move_non_material_textures).
    /// SAFETY: material_library and non_material_texture_library are disjoint fields.
    #[inline]
    pub fn texture_libraries_for_move_mut(&mut self) -> (&mut TextureLibrary, &mut TextureLibrary) {
        let base = self as *mut Mesh;
        unsafe {
            (
                (*base).material_library.texture_library_mut(),
                &mut (*base).non_material_texture_library,
            )
        }
    }

    /// Adds mesh features and returns its index.
    pub fn add_mesh_features(&mut self, mesh_features: Box<MeshFeatures>) -> MeshFeaturesIndex {
        self.mesh_features.push(mesh_features);
        self.mesh_features_material_mask.push(Vec::new());
        MeshFeaturesIndex::from((self.mesh_features.len() - 1) as u32)
    }

    /// Returns number of mesh feature sets.
    pub fn num_mesh_features(&self) -> usize {
        self.mesh_features.len()
    }

    /// Returns mesh features by index.
    pub fn mesh_features(&self, index: MeshFeaturesIndex) -> &MeshFeatures {
        &self.mesh_features[index.value_usize()]
    }

    /// Returns mutable mesh features by index.
    pub fn mesh_features_mut(&mut self, index: MeshFeaturesIndex) -> &mut MeshFeatures {
        &mut self.mesh_features[index.value_usize()]
    }

    /// Removes mesh features by index.
    pub fn remove_mesh_features(&mut self, index: MeshFeaturesIndex) {
        let index = index.value_usize();
        self.mesh_features.remove(index);
        self.mesh_features_material_mask.remove(index);
    }

    /// Returns true if attribute is referenced by any mesh features.
    pub fn is_attribute_used_by_mesh_features(&self, att_id: i32) -> bool {
        self.mesh_features
            .iter()
            .any(|mf| mf.attribute_index() == att_id)
    }

    /// Adds a material mask for mesh features.
    pub fn add_mesh_features_material_mask(
        &mut self,
        index: MeshFeaturesIndex,
        material_index: i32,
    ) {
        self.mesh_features_material_mask[index.value_usize()].push(material_index);
    }

    /// Returns number of material masks for mesh features.
    pub fn num_mesh_features_material_masks(&self, index: MeshFeaturesIndex) -> usize {
        self.mesh_features_material_mask[index.value_usize()].len()
    }

    /// Returns material mask entry for mesh features.
    pub fn mesh_features_material_mask(&self, index: MeshFeaturesIndex, mask_index: i32) -> i32 {
        self.mesh_features_material_mask[index.value_usize()][mask_index as usize]
    }

    /// Updates mesh features texture pointer to new texture library.
    pub fn update_mesh_features_texture_pointer(
        texture_to_index_map: &HashMap<*const Texture, i32>,
        texture_library: &mut TextureLibrary,
        mesh_features: &mut MeshFeatures,
    ) {
        let texture_map = mesh_features.texture_map_mut();
        let texture_ptr = match texture_map.texture() {
            Some(texture) => texture as *const Texture,
            None => return,
        };
        let texture_index = match texture_to_index_map.get(&texture_ptr) {
            Some(index) => *index,
            None => return,
        };
        if let Some(texture) = texture_library.texture_mut(texture_index) {
            texture_map.set_texture_ptr(texture as *mut _);
        }
    }

    /// Copies mesh features for a given material index.
    pub fn copy_mesh_features_for_material(
        source_mesh: &Mesh,
        target_mesh: &mut Mesh,
        material_index: i32,
    ) {
        for (i, mesh_features) in source_mesh.mesh_features.iter().enumerate() {
            let mut is_used = source_mesh.mesh_features_material_mask[i].is_empty();
            if !is_used {
                for mask in &source_mesh.mesh_features_material_mask[i] {
                    if *mask == material_index {
                        is_used = true;
                        break;
                    }
                }
            }
            if is_used {
                let mut new_mf = Box::new(MeshFeatures::new());
                new_mf.copy_from(mesh_features);
                target_mesh.add_mesh_features(new_mf);
            }
        }
    }

    /// Returns structural metadata attached to this mesh.
    pub fn structural_metadata(&self) -> &StructuralMetadata {
        &self.structural_metadata
    }

    /// Returns mutable structural metadata attached to this mesh.
    pub fn structural_metadata_mut(&mut self) -> &mut StructuralMetadata {
        &mut self.structural_metadata
    }

    /// Adds a property attributes index and returns its slot index.
    pub fn add_property_attributes_index(&mut self, property_attribute_index: i32) -> i32 {
        self.property_attributes.push(property_attribute_index);
        self.property_attributes_material_mask.push(Vec::new());
        (self.property_attributes.len() - 1) as i32
    }

    /// Returns the number of property attributes indices.
    pub fn num_property_attributes_indices(&self) -> i32 {
        self.property_attributes.len() as i32
    }

    /// Returns a property attributes index by slot.
    pub fn property_attributes_index(&self, index: i32) -> i32 {
        self.property_attributes[index as usize]
    }

    /// Returns a mutable reference to a property attributes index by slot.
    pub fn property_attributes_index_mut(&mut self, index: i32) -> &mut i32 {
        &mut self.property_attributes[index as usize]
    }

    /// Removes a property attributes index by slot.
    pub fn remove_property_attributes_index(&mut self, index: i32) {
        self.property_attributes.remove(index as usize);
        self.property_attributes_material_mask
            .remove(index as usize);
    }

    /// Adds a material mask entry for a property attributes slot.
    pub fn add_property_attributes_index_material_mask(&mut self, index: i32, material_index: i32) {
        self.property_attributes_material_mask[index as usize].push(material_index);
    }

    /// Returns the number of material masks for a property attributes slot.
    pub fn num_property_attributes_index_material_masks(&self, index: i32) -> usize {
        self.property_attributes_material_mask[index as usize].len()
    }

    /// Returns the material mask value for a given slot/mask index.
    pub fn property_attributes_index_material_mask(&self, index: i32, mask_index: i32) -> i32 {
        self.property_attributes_material_mask[index as usize][mask_index as usize]
    }

    /// Copies property attributes indices from `source_mesh` that match `material_index`.
    pub fn copy_property_attributes_indices_for_material(
        source_mesh: &Mesh,
        target_mesh: &mut Mesh,
        material_index: i32,
    ) {
        for i in 0..source_mesh.num_property_attributes_indices() {
            let mut is_used = source_mesh.num_property_attributes_index_material_masks(i) == 0;
            let mask_count = source_mesh.num_property_attributes_index_material_masks(i);
            for mask_index in 0..mask_count {
                if !is_used
                    && source_mesh.property_attributes_index_material_mask(i, mask_index as i32)
                        == material_index
                {
                    is_used = true;
                }
            }
            if is_used {
                target_mesh.add_property_attributes_index(source_mesh.property_attributes_index(i));
            }
        }
    }

    fn update_mesh_features_after_deleted_attribute(&mut self, att_id: i32) {
        for mesh_features in &mut self.mesh_features {
            let attr_index = mesh_features.attribute_index();
            if attr_index == att_id {
                mesh_features.set_attribute_index(-1);
            } else if attr_index > att_id {
                mesh_features.set_attribute_index(attr_index - 1);
            }
        }
    }

    /// Adds an attribute with arbitrary connectivity defined by corner-to-value mapping.
    pub fn add_attribute_with_connectivity(
        &mut self,
        mut att: PointAttribute,
        corner_to_value: &IndexTypeVector<CornerIndex, AttributeValueIndex>,
    ) -> i32 {
        let num_corners = self.num_faces() * 3;
        // BTreeMap for deterministic iteration order matching C++ std::map.
        let mut old_to_new_point_map: BTreeMap<(PointIndex, AttributeValueIndex), PointIndex> =
            BTreeMap::new();
        let mut corner_to_point = IndexTypeVector::<CornerIndex, PointIndex>::with_size_value(
            num_corners as usize,
            INVALID_POINT_INDEX,
        );
        let mut is_point_used =
            IndexTypeVector::<PointIndex, bool>::with_size_value(self.num_points() as usize, false);

        let mut new_num_points = self.num_points();
        for ci in 0..num_corners {
            let corner_index = CornerIndex::from(ci);
            let point_index = self.corner_to_point_id(corner_index);
            let attribute_value_index = corner_to_value[corner_index];
            let key = (point_index, attribute_value_index);
            if let Some(existing) = old_to_new_point_map.get(&key) {
                corner_to_point[corner_index] = *existing;
            } else {
                let new_point_index = if !is_point_used[point_index] {
                    is_point_used[point_index] = true;
                    point_index
                } else {
                    let new_point_index = PointIndex::from(new_num_points);
                    new_num_points += 1;
                    new_point_index
                };
                old_to_new_point_map.insert(key, new_point_index);
                corner_to_point[corner_index] = new_point_index;
            }
        }

        att.set_explicit_mapping(new_num_points as usize);
        for ci in 0..num_corners {
            let corner_index = CornerIndex::from(ci);
            att.set_point_map_entry(corner_to_point[corner_index], corner_to_value[corner_index]);
        }

        if new_num_points > self.num_points() {
            self.set_num_points(new_num_points);

            for ai in 0..self.num_attributes() {
                if let Some(existing_att) = self.attribute_mut(ai) {
                    let mapping_was_identity = existing_att.is_mapping_identity();
                    existing_att.set_explicit_mapping(new_num_points as usize);
                    if mapping_was_identity {
                        for avi in 0..existing_att.size() {
                            let index = AttributeValueIndex::from(avi as u32);
                            existing_att.set_point_map_entry(PointIndex::from(avi as u32), index);
                        }
                    }
                }
            }

            for ci in 0..num_corners {
                let fi = FaceIndex::from(ci / 3);
                let corner = (ci % 3) as usize;
                let old_point_index = self.faces[fi][corner];
                let new_point_index = corner_to_point[CornerIndex::from(ci)];
                if old_point_index == new_point_index {
                    continue;
                }
                for ai in 0..self.num_attributes() {
                    if let Some(existing_att) = self.attribute_mut(ai) {
                        let mapped = existing_att.mapped_index(old_point_index);
                        existing_att.set_point_map_entry(new_point_index, mapped);
                    }
                }
                self.faces[fi][corner] = new_point_index;
            }
        }

        for pi in 0..is_point_used.size() {
            let point_index = PointIndex::from(pi as u32);
            if !is_point_used[point_index] {
                att.set_point_map_entry(point_index, AttributeValueIndex::from(0u32));
            }
        }

        self.add_attribute(att)
    }

    /// Adds a per-vertex attribute with the same connectivity as position.
    pub fn add_per_vertex_attribute(&mut self, mut att: PointAttribute) -> i32 {
        let pos_att = match self.get_named_attribute(GeometryAttributeType::Position) {
            Some(att) => att,
            None => return -1,
        };
        if att.size() != pos_att.size() {
            return -1;
        }
        if pos_att.is_mapping_identity() {
            att.set_identity_mapping();
        } else {
            att.set_explicit_mapping(self.num_points() as usize);
            for pi in 0..self.num_points() {
                let point_index = PointIndex::from(pi);
                att.set_point_map_entry(point_index, pos_att.mapped_index(point_index));
            }
        }
        self.add_attribute(att)
    }

    /// Removes points that are not referenced by any face.
    pub fn remove_isolated_points(&mut self) {
        let num_points = self.num_points();
        let mut is_point_used =
            IndexTypeVector::<PointIndex, bool>::with_size_value(num_points as usize, false);
        let mut num_used_points = 0u32;
        for fi in 0..self.num_faces() {
            let face = self.face(FaceIndex::from(fi));
            for c in 0..3 {
                if !is_point_used[face[c]] {
                    is_point_used[face[c]] = true;
                    num_used_points += 1;
                }
            }
        }
        if num_used_points == num_points {
            return;
        }

        let mut old_to_new_point_map = IndexTypeVector::<PointIndex, PointIndex>::with_size_value(
            num_points as usize,
            INVALID_POINT_INDEX,
        );
        let mut new_point_index = 0u32;
        for pi in 0..num_points {
            let point_index = PointIndex::from(pi);
            if is_point_used[point_index] {
                old_to_new_point_map[point_index] = PointIndex::from(new_point_index);
                new_point_index += 1;
            }
        }

        for ai in 0..self.num_attributes() {
            if let Some(att) = self.attribute_mut(ai) {
                if att.is_mapping_identity() {
                    let stride = att.byte_stride() as usize;
                    let buf_rc = att.buffer().or_else(|| att.geometry_attribute().buffer());
                    if let Some(buf_rc) = buf_rc {
                        let mut buf = buf_rc.borrow_mut();
                        for pi in 0..num_points {
                            let old_pi = PointIndex::from(pi);
                            let new_pi = old_to_new_point_map[old_pi];
                            if new_pi == old_pi || new_pi == INVALID_POINT_INDEX {
                                continue;
                            }
                            let src_start = (old_pi.value() as usize) * stride;
                            let src_end = src_start + stride;
                            let dst_start = (new_pi.value() as usize) * stride;
                            buf.copy_within(src_start..src_end, dst_start);
                        }
                    }
                    att.resize(num_used_points as usize);
                } else {
                    for pi in 0..num_points {
                        let old_pi = PointIndex::from(pi);
                        let new_pi = old_to_new_point_map[old_pi];
                        if new_pi == old_pi || new_pi == INVALID_POINT_INDEX {
                            continue;
                        }
                        att.set_point_map_entry(new_pi, att.mapped_index(old_pi));
                    }
                    att.set_explicit_mapping(num_used_points as usize);
                    att.remove_unused_values();
                }
            }
        }

        for fi in 0..self.num_faces() {
            let face_index = FaceIndex::from(fi);
            let face = &mut self.faces[face_index];
            for c in 0..3 {
                face[c] = old_to_new_point_map[face[c]];
            }
        }
        self.set_num_points(num_used_points);
    }

    /// Adds a per-face attribute (attribute values are per-face).
    pub fn add_per_face_attribute(&mut self, att: PointAttribute) -> i32 {
        let num_corners = self.num_faces() * 3;
        let mut corner_map = IndexTypeVector::<CornerIndex, AttributeValueIndex>::with_size_value(
            num_corners as usize,
            AttributeValueIndex::from(0u32),
        );
        for ci in 0..num_corners {
            corner_map[CornerIndex::from(ci)] = AttributeValueIndex::from(ci / 3);
        }
        self.add_attribute_with_connectivity(att, &corner_map)
    }

    pub fn add_attribute(&mut self, pa: PointAttribute) -> i32 {
        let att_id = self.point_cloud.num_attributes();
        self.set_attribute(att_id, pa);
        self.point_cloud.num_attributes() - 1
    }

    pub fn add_attribute_from_geometry(
        &mut self,
        att: &GeometryAttribute,
        identity_mapping: bool,
        num_attribute_values: u32,
    ) -> i32 {
        let pa = self
            .point_cloud
            .create_attribute(att, identity_mapping, num_attribute_values);
        match pa {
            Some(pa) => self.add_attribute(pa),
            None => -1,
        }
    }

    pub fn delete_attribute(&mut self, att_id: i32) {
        self.point_cloud.delete_attribute(att_id);
        if att_id >= 0 && (att_id as usize) < self.attribute_data.len() {
            self.attribute_data.remove(att_id as usize);
        }
        self.update_mesh_features_after_deleted_attribute(att_id);
    }

    pub fn get_attribute_element_type(&self, att_id: i32) -> MeshAttributeElementType {
        self.attribute_data
            .get(att_id as usize)
            .map(|data| data.element_type)
            .unwrap_or_default()
    }

    pub fn set_attribute_element_type(&mut self, att_id: i32, et: MeshAttributeElementType) {
        if att_id < 0 {
            return;
        }
        let idx = att_id as usize;
        if self.attribute_data.len() <= idx {
            self.attribute_data
                .resize(idx + 1, AttributeData::default());
        }
        self.attribute_data[idx].element_type = et;
    }

    pub fn corner_to_point_id_i32(&self, ci: i32) -> PointIndex {
        if ci < 0 || (ci as u32) == INVALID_CORNER_INDEX.value() {
            return INVALID_POINT_INDEX;
        }
        let face_id = FaceIndex::from((ci as u32) / 3);
        let corner = (ci as u32) % 3;
        self.face(face_id)[corner as usize]
    }

    pub fn corner_to_point_id(&self, ci: CornerIndex) -> PointIndex {
        self.corner_to_point_id_i32(ci.value() as i32)
    }

    pub fn deduplicate_point_ids(&mut self) {
        let num_points = self.num_points();
        let (num_unique_points, index_map, unique_points) =
            build_point_deduplication_map(num_points, |point| self.point_signature(point));
        if num_unique_points == num_points {
            return;
        }

        let size_after = self.apply_point_id_deduplication(&index_map, &unique_points);
        self.set_num_points(size_after);
    }

    pub fn faces(&self) -> &IndexTypeVector<FaceIndex, Face> {
        &self.faces
    }

    pub fn faces_mut(&mut self) -> &mut IndexTypeVector<FaceIndex, Face> {
        &mut self.faces
    }

    /// Applies point ID deduplication: remaps attribute point maps and face indices.
    /// Mirrors C++ PointCloud::ApplyPointIdDeduplication + Mesh::ApplyPointIdDeduplication.
    fn apply_point_id_deduplication(
        &mut self,
        id_map: &IndexTypeVector<PointIndex, PointIndex>,
        unique_point_ids: &[PointIndex],
    ) -> u32 {
        // --- PointCloud base class logic (point_cloud.cc:264-282) ---
        let mut num_unique_points = 0u32;
        for &point_id in unique_point_ids {
            let new_point_id = id_map[point_id];
            if new_point_id.value() >= num_unique_points {
                // Copy attribute indices to the proper position for each unique vertex.
                for a in 0..self.num_attributes() {
                    if let Some(att) = self.attribute_mut(a) {
                        let mapped = att.mapped_index(point_id);
                        att.set_point_map_entry(new_point_id, mapped);
                    }
                }
                num_unique_points = new_point_id.value() + 1;
            }
        }
        // Resize explicit mappings to the new (smaller) point count.
        // C++ PointCloud::ApplyPointIdDeduplication resizes indices; Rust uses
        // set_explicit_mapping for parity. Safe: we have populated entries for
        // 0..num_unique_points via set_point_map_entry above.
        for a in 0..self.num_attributes() {
            if let Some(att) = self.attribute_mut(a) {
                att.set_explicit_mapping(num_unique_points as usize);
            }
        }
        // --- Mesh-specific logic (mesh.cc:538-542) ---
        for f in 0..self.num_faces() {
            let fi = FaceIndex::from(f);
            let face = &mut self.faces[fi];
            for c in 0..3 {
                face[c] = id_map[face[c]];
            }
        }
        num_unique_points
    }

    fn point_signature(&self, point: PointIndex) -> Vec<u32> {
        let mut signature = Vec::with_capacity(self.num_attributes() as usize);
        for i in 0..self.num_attributes() {
            let att = self.attribute(i).unwrap();
            signature.push(att.mapped_index(point).value());
        }
        signature
    }
}

impl Default for Mesh {
    fn default() -> Self {
        Self::new()
    }
}

impl std::ops::Deref for Mesh {
    type Target = PointCloud;
    fn deref(&self) -> &Self::Target {
        &self.point_cloud
    }
}

impl std::ops::DerefMut for Mesh {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.point_cloud
    }
}

pub struct MeshHasher;

impl MeshHasher {
    pub fn hash(&self, mesh: &Mesh) -> u64 {
        let pc_hasher = PointCloudHasher;
        let mut hash = pc_hasher.hash(mesh);
        for i in 0..mesh.num_faces() {
            let face = mesh.face(FaceIndex::from(i));
            for c in 0..3 {
                hash = hash_combine_with(&face[c].value(), hash);
            }
        }
        hash
    }
}

fn default_face() -> Face {
    [PointIndex::from(0u32); 3]
}
