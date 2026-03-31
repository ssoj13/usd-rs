//! Mesh utilities (transcoder-related).
//!
//! What: Helper operations for transforming meshes, metadata merge, and quantization.
//! Why: Mirrors Draco `mesh_utils` used by glTF transcoder paths.
//! How: Implements transform, metadata merge, texture UV flips, and degenerate face checks.
//! Where used: Mesh processing utilities and transcoder workflows.

use std::collections::HashSet;

use crate::attributes::attribute_quantization_transform::AttributeQuantizationTransform;
use crate::attributes::geometry_attribute::GeometryAttributeType;
use crate::attributes::geometry_indices::{AttributeValueIndex, FaceIndex};
use crate::attributes::point_attribute::PointAttribute;
use crate::core::quantization_utils::Quantizer;
use crate::core::status::{ok_status, Status, StatusCode};
use crate::core::status_or::StatusOr;
use crate::core::vector_d::{Vector3f, Vector4f};
use crate::mesh::Mesh;
use crate::metadata::geometry_metadata::GeometryMetadata;
use crate::metadata::metadata::Metadata;
use crate::texture::texture::Texture;

/// Row-major 3x3 matrix (f64).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix3d {
    pub m: [[f64; 3]; 3],
}

impl Matrix3d {
    pub fn identity() -> Self {
        Self {
            m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        }
    }

    pub fn transpose(self) -> Self {
        let mut out = Self::identity();
        for r in 0..3 {
            for c in 0..3 {
                out.m[r][c] = self.m[c][r];
            }
        }
        out
    }

    pub fn inverse(self) -> Self {
        let m = self.m;
        let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
        let inv_det = 1.0 / det;
        let mut out = Self::identity();
        out.m[0][0] = (m[1][1] * m[2][2] - m[1][2] * m[2][1]) * inv_det;
        out.m[0][1] = (m[0][2] * m[2][1] - m[0][1] * m[2][2]) * inv_det;
        out.m[0][2] = (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * inv_det;
        out.m[1][0] = (m[1][2] * m[2][0] - m[1][0] * m[2][2]) * inv_det;
        out.m[1][1] = (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * inv_det;
        out.m[1][2] = (m[0][2] * m[1][0] - m[0][0] * m[1][2]) * inv_det;
        out.m[2][0] = (m[1][0] * m[2][1] - m[1][1] * m[2][0]) * inv_det;
        out.m[2][1] = (m[0][1] * m[2][0] - m[0][0] * m[2][1]) * inv_det;
        out.m[2][2] = (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * inv_det;
        out
    }

    pub fn mul_vec3(self, v: [f64; 3]) -> [f64; 3] {
        [
            self.m[0][0] * v[0] + self.m[0][1] * v[1] + self.m[0][2] * v[2],
            self.m[1][0] * v[0] + self.m[1][1] * v[1] + self.m[1][2] * v[2],
            self.m[2][0] * v[0] + self.m[2][1] * v[1] + self.m[2][2] * v[2],
        ]
    }
}

/// Row-major 4x4 matrix (f64).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix4d {
    pub m: [[f64; 4]; 4],
}

impl Matrix4d {
    pub fn identity() -> Self {
        Self {
            m: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    pub fn block_3x3(&self) -> Matrix3d {
        Matrix3d {
            m: [
                [self.m[0][0], self.m[0][1], self.m[0][2]],
                [self.m[1][0], self.m[1][1], self.m[1][2]],
                [self.m[2][0], self.m[2][1], self.m[2][2]],
            ],
        }
    }

    pub fn set_block_3x3(&mut self, block: Matrix3d) {
        for r in 0..3 {
            for c in 0..3 {
                self.m[r][c] = block.m[r][c];
            }
        }
    }

    /// Matrix multiplication. C++ parity: single shared implementation.
    pub fn mul(&self, other: &Matrix4d) -> Matrix4d {
        let mut out = Matrix4d::identity();
        for r in 0..4 {
            for c in 0..4 {
                out.m[r][c] = self.m[r][0] * other.m[0][c]
                    + self.m[r][1] * other.m[1][c]
                    + self.m[r][2] * other.m[2][c]
                    + self.m[r][3] * other.m[3][c];
            }
        }
        out
    }

    pub fn mul_vec4(&self, v: [f64; 4]) -> [f64; 4] {
        [
            self.m[0][0] * v[0] + self.m[0][1] * v[1] + self.m[0][2] * v[2] + self.m[0][3] * v[3],
            self.m[1][0] * v[0] + self.m[1][1] * v[1] + self.m[1][2] * v[2] + self.m[1][3] * v[3],
            self.m[2][0] * v[0] + self.m[2][1] * v[1] + self.m[2][2] * v[2] + self.m[2][3] * v[3],
            self.m[3][0] * v[0] + self.m[3][1] * v[1] + self.m[3][2] * v[2] + self.m[3][3] * v[3],
        ]
    }
}

/// Helper class containing various utilities operating on `Mesh`.
pub struct MeshUtils;

impl MeshUtils {
    /// Transforms `mesh` using the `transform` matrix (in-place).
    pub fn transform_mesh(transform: &Matrix4d, mesh: &mut Mesh) {
        let pos_att_id = mesh.get_named_attribute_id(GeometryAttributeType::Position);
        {
            let pos_att = match mesh.attribute_mut(pos_att_id) {
                Some(att) => att,
                None => return,
            };
            for avi in 0..pos_att.size() {
                let mut pos = [0.0f32; 3];
                pos_att.get_value_array_into(AttributeValueIndex::from(avi as u32), &mut pos);
                let transformed =
                    transform.mul_vec4([pos[0] as f64, pos[1] as f64, pos[2] as f64, 1.0]);
                let out = [
                    transformed[0] as f32,
                    transformed[1] as f32,
                    transformed[2] as f32,
                ];
                pos_att.set_attribute_value_array(AttributeValueIndex::from(avi as u32), &out);
            }
        }

        let needs_normal = mesh.num_named_attributes(GeometryAttributeType::Normal) > 0;
        let needs_tangent = mesh.num_named_attributes(GeometryAttributeType::Tangent) > 0;
        if needs_normal || needs_tangent {
            let it_transform = transform.block_3x3().inverse().transpose();
            if needs_normal {
                let id = mesh.get_named_attribute_id(GeometryAttributeType::Normal);
                if let Some(att) = mesh.attribute_mut(id) {
                    Self::transform_normalized_attribute(&it_transform, att);
                }
            }
            if needs_tangent {
                let id = mesh.get_named_attribute_id(GeometryAttributeType::Tangent);
                if let Some(att) = mesh.attribute_mut(id) {
                    Self::transform_normalized_attribute(&it_transform, att);
                }
            }
        }
    }

    /// Merges metadata from `src_mesh` to `dst_mesh` (dst entries win).
    pub fn merge_metadata(src_mesh: &Mesh, dst_mesh: &mut Mesh) {
        let src_metadata = match src_mesh.get_metadata() {
            Some(metadata) => metadata,
            None => return,
        };
        let has_metadata = dst_mesh.get_metadata().is_some();
        if !has_metadata {
            dst_mesh.add_metadata(GeometryMetadata::new());
        }
        let dst_metadata = dst_mesh.metadata_mut().expect("metadata missing");
        merge_metadata_internal(src_metadata, dst_metadata);

        let named_types = [
            GeometryAttributeType::Position,
            GeometryAttributeType::Normal,
            GeometryAttributeType::Color,
            GeometryAttributeType::TexCoord,
            GeometryAttributeType::Generic,
            GeometryAttributeType::Tangent,
            GeometryAttributeType::Material,
            GeometryAttributeType::Joints,
            GeometryAttributeType::Weights,
        ];

        for att_type in named_types {
            if src_mesh.num_named_attributes(att_type) != dst_mesh.num_named_attributes(att_type) {
                continue;
            }
            let count = src_mesh.num_named_attributes(att_type);
            for j in 0..count {
                let src_att = match src_mesh.get_named_attribute_by_index(att_type, j) {
                    Some(att) => att,
                    None => continue,
                };
                let src_att_meta = match src_mesh
                    .get_metadata()
                    .and_then(|m| m.get_attribute_metadata_by_unique_id(src_att.unique_id() as i32))
                {
                    Some(meta) => meta,
                    None => continue,
                };

                let dst_att_unique_id = match dst_mesh.get_named_attribute_by_index(att_type, j) {
                    Some(att) => att.unique_id(),
                    None => continue,
                };
                let dst_meta = dst_mesh.metadata_mut().expect("metadata missing");
                if let Some(dst_att_meta) = dst_meta.attribute_metadata(dst_att_unique_id as i32) {
                    merge_metadata_internal(src_att_meta, dst_att_meta);
                } else {
                    let mut new_meta = src_att_meta.clone();
                    new_meta.set_att_unique_id(dst_att_unique_id);
                    let _ = dst_meta.add_attribute_metadata(Some(Box::new(new_meta)));
                }
            }
        }
    }

    /// Removes unused mesh features from `mesh`.
    pub fn remove_unused_mesh_features(mesh: &mut Mesh) -> Status {
        let used_materials = find_used_materials(mesh);
        let mut unused = Vec::new();
        for i in 0..mesh.num_mesh_features() {
            let index = crate::mesh::MeshFeaturesIndex::from(i as u32);
            let mut is_used = mesh.num_mesh_features_material_masks(index) == 0;
            if !is_used {
                for mask_i in 0..mesh.num_mesh_features_material_masks(index) {
                    let material_index = mesh.mesh_features_material_mask(index, mask_i as i32);
                    if used_materials.contains(&material_index) {
                        is_used = true;
                        break;
                    }
                }
            }
            if !is_used {
                unused.push(index);
            }
        }

        for index in unused.iter().rev() {
            mesh.remove_mesh_features(*index);
        }

        let mut used_textures: HashSet<*const Texture> = HashSet::new();
        for i in 0..mesh.num_mesh_features() {
            let index = crate::mesh::MeshFeaturesIndex::from(i as u32);
            if let Some(texture) = mesh.mesh_features(index).texture_map().texture() {
                used_textures.insert(texture as *const Texture);
            }
        }

        if !used_textures.is_empty() && mesh.non_material_texture_library().num_textures() == 0 {
            return Status::new(
                StatusCode::DracoError,
                "Trying to remove mesh features textures that are not owned by the mesh.",
            );
        }

        let num_textures = mesh.non_material_texture_library().num_textures() as i32;
        for ti in (0..num_textures).rev() {
            let texture = match mesh.non_material_texture_library().texture(ti) {
                Some(texture) => texture as *const Texture,
                None => continue,
            };
            if !used_textures.contains(&texture) {
                let _ = mesh.non_material_texture_library_mut().remove_texture(ti);
            }
        }
        ok_status()
    }

    /// Removes unused property attributes indices from `mesh`.
    pub fn remove_unused_property_attributes_indices(mesh: &mut Mesh) -> Status {
        let used_materials = find_used_materials(mesh);
        let mut unused = Vec::new();
        for i in 0..mesh.num_property_attributes_indices() {
            let mut is_used = mesh.num_property_attributes_index_material_masks(i) == 0;
            if !is_used {
                for mask_i in 0..mesh.num_property_attributes_index_material_masks(i) {
                    let material_index =
                        mesh.property_attributes_index_material_mask(i, mask_i as i32);
                    if used_materials.contains(&material_index) {
                        is_used = true;
                        break;
                    }
                }
            }
            if !is_used {
                unused.push(i);
            }
        }
        for i in unused.iter().rev() {
            mesh.remove_property_attributes_index(*i);
        }
        ok_status()
    }

    /// Flips UV values in `att`.
    pub fn flip_texture_uv_values(flip_u: bool, flip_v: bool, att: &mut PointAttribute) -> bool {
        if att.attribute_type() != GeometryAttributeType::TexCoord {
            return false;
        }
        if att.data_type() != crate::core::draco_types::DataType::Float32 {
            return false;
        }
        if att.num_components() != 2 {
            return false;
        }
        let mut value = [0.0f32; 2];
        for avi in 0..att.size() {
            let index = AttributeValueIndex::from(avi as u32);
            if !att.get_value_array_into(index, &mut value) {
                return false;
            }
            if flip_u {
                value[0] = 1.0 - value[0];
            }
            if flip_v {
                value[1] = 1.0 - value[1];
            }
            att.set_attribute_value_array(index, &value);
        }
        true
    }

    /// Counts degenerate faces for attribute |att_id|.
    pub fn count_degenerate_faces(mesh: &Mesh, att_id: i32) -> i32 {
        let att = match mesh.attribute(att_id) {
            Some(att) => att,
            None => return -1,
        };
        match att.num_components() {
            2 => count_degenerate_faces::<2>(mesh, att),
            3 => count_degenerate_faces::<3>(mesh, att),
            4 => count_degenerate_faces::<4>(mesh, att),
            _ => -1,
        }
    }

    /// Finds lowest texture quantization bits without introducing new degenerates.
    pub fn find_lowest_texture_quantization(
        mesh: &Mesh,
        pos_att: &PointAttribute,
        pos_quantization_bits: i32,
        tex_att: &PointAttribute,
        tex_target_quantization_bits: i32,
    ) -> StatusOr<i32> {
        if tex_target_quantization_bits < 0 || tex_target_quantization_bits >= 30 {
            return StatusOr::new_status(Status::new(
                StatusCode::DracoError,
                "Target texture quantization is out of range.",
            ));
        }
        if tex_target_quantization_bits == 0 {
            return StatusOr::new_value(0);
        }
        let pos_max_quantized_value = (1u32 << pos_quantization_bits) - 1;
        let mut pos_transform = AttributeQuantizationTransform::new();
        if !pos_transform.compute_parameters(pos_att, pos_quantization_bits) {
            return StatusOr::new_status(Status::new(
                StatusCode::DracoError,
                "Failed computing position quantization parameters.",
            ));
        }

        let pos_degenerate_faces_sorted = list_degenerate_quantized_faces(
            mesh,
            pos_att,
            pos_transform.range(),
            pos_max_quantized_value,
            false,
        );

        let mut lowest_quantization_bits = 0;
        let mut min_bits = tex_target_quantization_bits;
        let mut max_bits = 29;
        while min_bits <= max_bits {
            let curr_bits = min_bits + (max_bits - min_bits) / 2;
            let mut tex_transform = AttributeQuantizationTransform::new();
            if !tex_transform.compute_parameters(tex_att, curr_bits) {
                return StatusOr::new_status(Status::new(
                    StatusCode::DracoError,
                    "Failed computing texture quantization parameters.",
                ));
            }
            let max_quantized_value = (1u32 << curr_bits) - 1;
            let tex_degenerate_faces_sorted = list_degenerate_quantized_faces(
                mesh,
                tex_att,
                tex_transform.range(),
                max_quantized_value,
                true,
            );

            if tex_degenerate_faces_sorted.len() <= pos_degenerate_faces_sorted.len() {
                if is_subset_sorted(&tex_degenerate_faces_sorted, &pos_degenerate_faces_sorted) {
                    lowest_quantization_bits = curr_bits;
                }
            }

            if lowest_quantization_bits == curr_bits {
                max_bits = curr_bits - 1;
            } else {
                min_bits = curr_bits + 1;
            }
        }
        StatusOr::new_value(lowest_quantization_bits)
    }

    /// Checks whether a mesh has auto-generated tangents.
    pub fn has_auto_generated_tangents(mesh: &Mesh) -> bool {
        let tangent_att_id = mesh.get_named_attribute_id(GeometryAttributeType::Tangent);
        if tangent_att_id == -1 {
            return false;
        }
        if let Some(metadata) = mesh.get_attribute_metadata_by_attribute_id(tangent_att_id) {
            let mut is_auto_generated = 0i32;
            if metadata.get_entry_int("auto_generated", &mut is_auto_generated)
                && is_auto_generated == 1
            {
                return true;
            }
        }
        false
    }

    fn transform_normalized_attribute(transform: &Matrix3d, att: &mut PointAttribute) {
        for avi in 0..att.size() {
            let index = AttributeValueIndex::from(avi as u32);
            let mut val = [0.0f32; 4];
            match att.num_components() {
                2 => {
                    let mut tmp = [0.0f32; 2];
                    att.get_value_array_into(index, &mut tmp);
                    val[0] = tmp[0];
                    val[1] = tmp[1];
                    val[3] = 1.0;
                }
                3 => {
                    let mut tmp = [0.0f32; 3];
                    att.get_value_array_into(index, &mut tmp);
                    val[0] = tmp[0];
                    val[1] = tmp[1];
                    val[2] = tmp[2];
                    val[3] = 1.0;
                }
                4 => {
                    att.get_value_array_into(index, &mut val);
                }
                _ => {
                    continue;
                }
            }
            let transformed = transform.mul_vec3([val[0] as f64, val[1] as f64, val[2] as f64]);
            let mut out = Vector4f::new4(
                transformed[0] as f32,
                transformed[1] as f32,
                transformed[2] as f32,
                val[3],
            );
            let mut norm = Vector3f::new3(out[0], out[1], out[2]);
            norm.normalize();
            out[0] = norm[0];
            out[1] = norm[1];
            out[2] = norm[2];

            match att.num_components() {
                2 => {
                    let tmp = [out[0], out[1]];
                    att.set_attribute_value_array(index, &tmp);
                }
                3 => {
                    let tmp = [out[0], out[1], out[2]];
                    att.set_attribute_value_array(index, &tmp);
                }
                _ => {
                    let tmp = [out[0], out[1], out[2], out[3]];
                    att.set_attribute_value_array(index, &tmp);
                }
            }
        }
    }
}

fn merge_metadata_internal(src: &Metadata, dst: &mut Metadata) {
    for (name, entry) in src.entries() {
        if dst.entries().contains_key(name) {
            continue;
        }
        dst.add_entry_binary(name, entry.data());
    }

    for (name, sub) in src.sub_metadatas() {
        if dst.sub_metadatas().contains_key(name) {
            if let Some(dst_sub) = dst.sub_metadata(name) {
                merge_metadata_internal(sub.as_ref(), dst_sub);
            }
            continue;
        }
        let _ = dst.add_sub_metadata(name, sub.as_ref().clone());
    }
}

fn find_used_materials(mesh: &Mesh) -> HashSet<i32> {
    let mut used_materials = HashSet::new();
    let mat_att = mesh.get_named_attribute(GeometryAttributeType::Material);
    if let Some(att) = mat_att {
        for avi in 0..att.size() {
            let mut value = [0u32; 1];
            let index = AttributeValueIndex::from(avi as u32);
            if !att.convert_value(index, 1, &mut value) {
                value[0] = 0;
            }
            used_materials.insert(value[0] as i32);
        }
    } else {
        used_materials.insert(0);
    }
    used_materials
}

fn count_degenerate_faces<const N: usize>(mesh: &Mesh, att: &PointAttribute) -> i32 {
    if att.data_type() != crate::core::draco_types::DataType::Float32 {
        return -1;
    }
    let mut values = [[0.0f32; N]; 3];
    let mut degenerate = 0;
    for fi in 0..mesh.num_faces() {
        let face = mesh.face(FaceIndex::from(fi));
        for c in 0..3 {
            let mapped = att.mapped_index(face[c]);
            att.get_value_array_into(mapped, &mut values[c]);
        }
        if values[0] == values[1] || values[0] == values[2] || values[1] == values[2] {
            degenerate += 1;
        }
    }
    degenerate
}

fn list_degenerate_quantized_faces(
    mesh: &Mesh,
    att: &PointAttribute,
    range: f32,
    max_quantized_value: u32,
    quantized_degenerate_only: bool,
) -> Vec<FaceIndex> {
    match att.num_components() {
        2 => list_degenerate_quantized_faces_typed::<2>(
            mesh,
            att,
            range,
            max_quantized_value,
            quantized_degenerate_only,
        ),
        3 => list_degenerate_quantized_faces_typed::<3>(
            mesh,
            att,
            range,
            max_quantized_value,
            quantized_degenerate_only,
        ),
        4 => list_degenerate_quantized_faces_typed::<4>(
            mesh,
            att,
            range,
            max_quantized_value,
            quantized_degenerate_only,
        ),
        _ => Vec::new(),
    }
}

fn list_degenerate_quantized_faces_typed<const N: usize>(
    mesh: &Mesh,
    att: &PointAttribute,
    range: f32,
    max_quantized_value: u32,
    quantized_degenerate_only: bool,
) -> Vec<FaceIndex> {
    let mut values = [[0.0f32; N]; 3];
    let mut quantized_values = [[0i32; N]; 3];
    let mut quantizer = Quantizer::new();
    quantizer.init_range(range, max_quantized_value as i32);
    let mut degenerate_faces = Vec::new();

    for fi in 0..mesh.num_faces() {
        let face = mesh.face(FaceIndex::from(fi));
        for c in 0..3 {
            let mapped = att.mapped_index(face[c]);
            att.get_value_array_into(mapped, &mut values[c]);
            for i in 0..N {
                quantized_values[c][i] = quantizer.quantize_float(values[c][i]);
            }
        }
        if quantized_degenerate_only
            && (values[0] == values[1] || values[0] == values[2] || values[1] == values[2])
        {
            continue;
        }
        if quantized_values[0] == quantized_values[1]
            || quantized_values[0] == quantized_values[2]
            || quantized_values[1] == quantized_values[2]
        {
            degenerate_faces.push(FaceIndex::from(fi));
        }
    }
    degenerate_faces
}

fn is_subset_sorted<T: PartialEq>(needle: &[T], haystack: &[T]) -> bool {
    let mut i = 0usize;
    let mut j = 0usize;
    while i < needle.len() && j < haystack.len() {
        if needle[i] == haystack[j] {
            i += 1;
            j += 1;
        } else {
            j += 1;
        }
    }
    i == needle.len()
}
