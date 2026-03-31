//! OBJ encoder.
//! Reference: `_ref/draco/src/draco/io/obj_encoder.h` + `.cc`.

use std::collections::{BTreeMap, HashMap};

use crate::attributes::geometry_attribute::GeometryAttributeType;
use crate::attributes::geometry_indices::{
    AttributeValueIndex, CornerIndex, FaceIndex, PointIndex, INVALID_CORNER_INDEX,
};
use crate::core::draco_types::DataType;
use crate::core::encoder_buffer::EncoderBuffer;
use crate::io::file_writer_factory::FileWriterFactory;
use crate::mesh::corner_table::CornerTable;
use crate::mesh::mesh_misc_functions::create_corner_table_from_position_attribute;
use crate::mesh::Mesh;
use crate::metadata::metadata::{MetadataName, MetadataString};
use crate::point_cloud::PointCloud;

pub struct ObjEncoder {
    // Attribute IDs instead of raw pointers
    pos_att_id: i32,
    tex_coord_att_id: i32,
    normal_att_id: i32,
    material_att_id: i32,
    sub_obj_att_id: i32,
    added_edges_att_id: i32,

    sub_obj_id_to_name: HashMap<i32, String>,
    current_sub_obj_id: i32,

    material_id_to_name: HashMap<i32, String>,
    current_material_id: i32,

    file_name: String,
}

impl ObjEncoder {
    fn metadata_name_to_string(name: &MetadataName) -> String {
        name.to_utf8_lossy().into_owned()
    }

    pub fn new() -> Self {
        Self {
            pos_att_id: -1,
            tex_coord_att_id: -1,
            normal_att_id: -1,
            material_att_id: -1,
            sub_obj_att_id: -1,
            added_edges_att_id: -1,
            sub_obj_id_to_name: HashMap::new(),
            current_sub_obj_id: -1,
            material_id_to_name: HashMap::new(),
            current_material_id: -1,
            file_name: String::new(),
        }
    }

    pub fn encode_to_file_point_cloud(&mut self, pc: &PointCloud, file_name: &str) -> bool {
        let mut file = match FileWriterFactory::open_writer(file_name) {
            Some(file) => file,
            None => return false,
        };
        self.file_name = file_name.to_string();
        let mut buffer = EncoderBuffer::new();
        if !self.encode_internal(pc, None, &mut buffer) {
            return false;
        }
        file.write(buffer.data())
    }

    pub fn encode_to_file_mesh(&mut self, mesh: &Mesh, file_name: &str) -> bool {
        let mut file = match FileWriterFactory::open_writer(file_name) {
            Some(file) => file,
            None => return false,
        };
        self.file_name = file_name.to_string();
        let mut buffer = EncoderBuffer::new();
        let pc: &PointCloud = mesh;
        if !self.encode_internal(pc, Some(mesh), &mut buffer) {
            return false;
        }
        file.write(buffer.data())
    }

    pub fn encode_to_buffer_point_cloud(
        &mut self,
        pc: &PointCloud,
        out_buffer: &mut EncoderBuffer,
    ) -> bool {
        let ok = self.encode_internal(pc, None, out_buffer);
        self.cleanup();
        ok
    }

    pub fn encode_to_buffer_mesh(&mut self, mesh: &Mesh, out_buffer: &mut EncoderBuffer) -> bool {
        let pc: &PointCloud = mesh;
        let ok = self.encode_internal(pc, Some(mesh), out_buffer);
        self.cleanup();
        ok
    }

    fn encode_internal(
        &mut self,
        pc: &PointCloud,
        mesh: Option<&Mesh>,
        buffer: &mut EncoderBuffer,
    ) -> bool {
        // Reset attribute IDs
        self.pos_att_id = -1;
        self.tex_coord_att_id = -1;
        self.normal_att_id = -1;
        self.material_att_id = -1;
        self.sub_obj_att_id = -1;
        self.added_edges_att_id = -1;
        self.current_sub_obj_id = -1;
        self.current_material_id = -1;

        if !self.get_sub_objects(pc) {
            return false;
        }
        if let Some(m) = mesh {
            if !self.get_added_edges(m) {
                return false;
            }
        }
        if !self.encode_material_file_name(pc, buffer) {
            return false;
        }
        if !self.encode_positions(pc, buffer) {
            return false;
        }
        if !self.encode_texture_coordinates(pc, buffer) {
            return false;
        }
        if !self.encode_normals(pc, buffer) {
            return false;
        }
        if let Some(m) = mesh {
            if !self.encode_faces(pc, m, buffer) {
                return false;
            }
        }
        true
    }

    fn cleanup(&mut self) {
        self.pos_att_id = -1;
        self.tex_coord_att_id = -1;
        self.normal_att_id = -1;
        self.material_att_id = -1;
        self.sub_obj_att_id = -1;
        self.added_edges_att_id = -1;
        self.sub_obj_id_to_name.clear();
        self.current_sub_obj_id = -1;
        self.material_id_to_name.clear();
        self.current_material_id = -1;
        self.file_name.clear();
    }

    fn get_added_edges(&mut self, mesh: &Mesh) -> bool {
        let mesh_metadata = match mesh.get_metadata() {
            Some(metadata) => metadata,
            None => return true,
        };
        let att_metadata =
            match mesh_metadata.get_attribute_metadata_by_string_entry("name", "added_edges") {
                Some(metadata) => metadata,
                None => return true,
            };
        let att = match mesh.get_attribute_by_unique_id(att_metadata.att_unique_id()) {
            Some(att) => att,
            None => return true,
        };
        if att.size() == 0 || att.num_components() != 1 || att.data_type() != DataType::Uint8 {
            return false;
        }
        self.added_edges_att_id = mesh.get_attribute_id_by_unique_id(att_metadata.att_unique_id());
        true
    }

    fn get_sub_objects(&mut self, pc: &PointCloud) -> bool {
        let metadata = match pc.get_metadata() {
            Some(metadata) => metadata,
            None => return true,
        };
        let sub_obj_metadata =
            match metadata.get_attribute_metadata_by_string_entry("name", "sub_obj") {
                Some(metadata) => metadata,
                None => return true,
            };
        let mut entries: Vec<(String, i32)> = Vec::new();
        for (name, entry) in sub_obj_metadata.entries() {
            let mut value: i32 = 0;
            if !entry.get_value(&mut value) {
                continue;
            }
            entries.push((Self::metadata_name_to_string(name), value));
        }
        let att_unique_id = sub_obj_metadata.att_unique_id();
        let att = match pc.get_attribute_by_unique_id(att_unique_id) {
            Some(att) => att,
            None => return false,
        };
        if att.size() == 0 || att.num_components() != 1 {
            return false;
        }
        self.sub_obj_id_to_name.clear();
        for (name, value) in entries {
            self.sub_obj_id_to_name.insert(value, name);
        }
        self.sub_obj_att_id = pc.get_attribute_id_by_unique_id(att_unique_id);
        true
    }

    fn encode_material_file_name(&mut self, pc: &PointCloud, buffer: &mut EncoderBuffer) -> bool {
        let material_metadata = match pc.get_attribute_metadata_by_string_entry("name", "material")
        {
            Some(metadata) => metadata,
            None => return true,
        };
        let mut material_file_name = MetadataString::default();
        if !material_metadata.get_entry_string("file_name", &mut material_file_name) {
            return false;
        }
        let mut entries: Vec<(String, i32)> = Vec::new();
        for (name, entry) in material_metadata.entries() {
            let mut value: i32 = 0;
            if !entry.get_value(&mut value) {
                continue;
            }
            entries.push((Self::metadata_name_to_string(name), value));
        }
        let att_unique_id = material_metadata.att_unique_id();
        let att = match pc.get_attribute_by_unique_id(att_unique_id) {
            Some(att) => att,
            None => return false,
        };
        if att.size() == 0 {
            return false;
        }
        buffer.encode_bytes(b"mtllib ");
        buffer.encode_bytes(material_file_name.as_bytes());
        buffer.encode_bytes(b"\n");

        self.material_id_to_name.clear();
        for (name, value) in entries {
            self.material_id_to_name.insert(value, name);
        }
        self.material_att_id = pc.get_attribute_id_by_unique_id(att_unique_id);
        true
    }

    fn encode_positions(&mut self, pc: &PointCloud, buffer: &mut EncoderBuffer) -> bool {
        let att = match pc.get_named_attribute(GeometryAttributeType::Position) {
            Some(att) => att,
            None => return false,
        };
        if att.size() == 0 {
            return false;
        }
        for i in 0..att.size() {
            let idx = AttributeValueIndex::from(i as u32);
            let mut value = [0.0f32; 3];
            if !att.convert_value(idx, 3, &mut value) {
                return false;
            }
            buffer.encode_bytes(b"v ");
            self.encode_float_list(buffer, &value);
            buffer.encode_bytes(b"\n");
        }
        self.pos_att_id = pc.get_named_attribute_id(GeometryAttributeType::Position);
        true
    }

    fn encode_texture_coordinates(&mut self, pc: &PointCloud, buffer: &mut EncoderBuffer) -> bool {
        let att = match pc.get_named_attribute(GeometryAttributeType::TexCoord) {
            Some(att) => att,
            None => return true,
        };
        if att.size() == 0 {
            return true;
        }
        for i in 0..att.size() {
            let idx = AttributeValueIndex::from(i as u32);
            let mut value = [0.0f32; 2];
            if !att.convert_value(idx, 2, &mut value) {
                return false;
            }
            buffer.encode_bytes(b"vt ");
            self.encode_float_list(buffer, &value);
            buffer.encode_bytes(b"\n");
        }
        self.tex_coord_att_id = pc.get_named_attribute_id(GeometryAttributeType::TexCoord);
        true
    }

    fn encode_normals(&mut self, pc: &PointCloud, buffer: &mut EncoderBuffer) -> bool {
        let att = match pc.get_named_attribute(GeometryAttributeType::Normal) {
            Some(att) => att,
            None => return true,
        };
        if att.size() == 0 {
            return true;
        }
        for i in 0..att.size() {
            let idx = AttributeValueIndex::from(i as u32);
            let mut value = [0.0f32; 3];
            if !att.convert_value(idx, 3, &mut value) {
                return false;
            }
            buffer.encode_bytes(b"vn ");
            self.encode_float_list(buffer, &value);
            buffer.encode_bytes(b"\n");
        }
        self.normal_att_id = pc.get_named_attribute_id(GeometryAttributeType::Normal);
        true
    }

    fn encode_faces(&mut self, pc: &PointCloud, mesh: &Mesh, buffer: &mut EncoderBuffer) -> bool {
        if self.added_edges_att_id >= 0 {
            return self.encode_polygonal_faces(pc, mesh, buffer);
        }
        for fi in 0..mesh.num_faces() {
            let face_id = FaceIndex::from(fi);
            if !self.encode_face_attributes(pc, mesh, buffer, face_id) {
                return false;
            }
            buffer.encode_bytes(b"f");
            for corner in 0..3 {
                if !self.encode_face_corner(pc, mesh, buffer, face_id, corner) {
                    return false;
                }
            }
            buffer.encode_bytes(b"\n");
        }
        true
    }

    fn encode_polygonal_faces(
        &mut self,
        pc: &PointCloud,
        mesh: &Mesh,
        buffer: &mut EncoderBuffer,
    ) -> bool {
        let mut triangle_visited = vec![false; mesh.num_faces() as usize];
        let mut polygon_edges: PolygonEdges = BTreeMap::new();
        let corner_table = match create_corner_table_from_position_attribute(mesh) {
            Some(ct) => ct,
            None => return false,
        };
        for fi in 0..mesh.num_faces() {
            let face_id = FaceIndex::from(fi);
            if !self.encode_face_attributes(pc, mesh, buffer, face_id) {
                return false;
            }
            polygon_edges.clear();
            self.find_original_face_edges(
                pc,
                mesh,
                face_id,
                &corner_table,
                &mut triangle_visited,
                &mut polygon_edges,
            );
            if polygon_edges.is_empty() {
                continue;
            }
            let first_position_index = *polygon_edges.keys().next().expect("polygon_edges empty");
            let mut position_index = first_position_index;
            buffer.encode_bytes(b"f");
            loop {
                let point_index = match polygon_edges.get(&position_index) {
                    Some(pi) => *pi,
                    None => return false,
                };
                if !self.encode_face_corner_point(pc, buffer, point_index) {
                    return false;
                }
                let pos_att = pc
                    .attribute(self.pos_att_id)
                    .expect("Position attribute not set");
                position_index = pos_att.mapped_index(point_index);
                if position_index == first_position_index {
                    break;
                }
            }
            buffer.encode_bytes(b"\n");
        }
        true
    }

    fn encode_face_attributes(
        &mut self,
        pc: &PointCloud,
        mesh: &Mesh,
        buffer: &mut EncoderBuffer,
        face_id: FaceIndex,
    ) -> bool {
        if self.sub_obj_att_id >= 0 {
            if !self.encode_sub_object(pc, mesh, buffer, face_id) {
                return false;
            }
        }
        if self.material_att_id >= 0 {
            if !self.encode_material(pc, mesh, buffer, face_id) {
                return false;
            }
        }
        true
    }

    fn encode_sub_object(
        &mut self,
        pc: &PointCloud,
        mesh: &Mesh,
        buffer: &mut EncoderBuffer,
        face_id: FaceIndex,
    ) -> bool {
        let sub_obj_att = pc
            .attribute(self.sub_obj_att_id)
            .expect("Sub object attribute missing");
        let vert_index = mesh.face(face_id)[0];
        let index_id = sub_obj_att.mapped_index(vert_index);
        let mut sub_obj_id: i32 = 0;
        if !sub_obj_att.convert_value(index_id, 1, std::slice::from_mut(&mut sub_obj_id)) {
            return false;
        }
        if sub_obj_id != self.current_sub_obj_id {
            buffer.encode_bytes(b"o ");
            let name = match self.sub_obj_id_to_name.get(&sub_obj_id) {
                Some(name) => name,
                None => return false,
            };
            buffer.encode_bytes(name.as_bytes());
            buffer.encode_bytes(b"\n");
            self.current_sub_obj_id = sub_obj_id;
        }
        true
    }

    fn encode_material(
        &mut self,
        pc: &PointCloud,
        mesh: &Mesh,
        buffer: &mut EncoderBuffer,
        face_id: FaceIndex,
    ) -> bool {
        let material_att = pc
            .attribute(self.material_att_id)
            .expect("Material attribute missing");
        let vert_index = mesh.face(face_id)[0];
        let index_id = material_att.mapped_index(vert_index);
        let mut material_id: i32 = 0;
        if !material_att.convert_value(index_id, 1, std::slice::from_mut(&mut material_id)) {
            return false;
        }
        if material_id != self.current_material_id {
            buffer.encode_bytes(b"usemtl ");
            let name = match self.material_id_to_name.get(&material_id) {
                Some(name) => name,
                None => return false,
            };
            buffer.encode_bytes(name.as_bytes());
            buffer.encode_bytes(b"\n");
            self.current_material_id = material_id;
        }
        true
    }

    fn encode_face_corner(
        &self,
        pc: &PointCloud,
        mesh: &Mesh,
        buffer: &mut EncoderBuffer,
        face_id: FaceIndex,
        local_corner_id: i32,
    ) -> bool {
        let vert_index = mesh.face(face_id)[local_corner_id as usize];
        self.encode_face_corner_point(pc, buffer, vert_index)
    }

    fn encode_face_corner_point(
        &self,
        pc: &PointCloud,
        buffer: &mut EncoderBuffer,
        vert_index: PointIndex,
    ) -> bool {
        buffer.encode_bytes(b" ");
        let pos_att = pc
            .attribute(self.pos_att_id)
            .expect("Position attribute not set");
        buffer.encode_bytes(
            format!("{}", pos_att.mapped_index(vert_index).value() as i32 + 1).as_bytes(),
        );
        if self.tex_coord_att_id >= 0 || self.normal_att_id >= 0 {
            buffer.encode_bytes(b"/");
            if self.tex_coord_att_id >= 0 {
                let tex_att = pc.attribute(self.tex_coord_att_id).unwrap();
                buffer.encode_bytes(
                    format!("{}", tex_att.mapped_index(vert_index).value() as i32 + 1).as_bytes(),
                );
            }
            if self.normal_att_id >= 0 {
                buffer.encode_bytes(b"/");
                let norm_att = pc.attribute(self.normal_att_id).unwrap();
                buffer.encode_bytes(
                    format!("{}", norm_att.mapped_index(vert_index).value() as i32 + 1).as_bytes(),
                );
            }
        }
        true
    }

    fn encode_float(&self, buffer: &mut EncoderBuffer, val: f32) {
        let text = format!("{:.6}", val);
        buffer.encode_bytes(text.as_bytes());
    }

    fn encode_float_list<const N: usize>(&self, buffer: &mut EncoderBuffer, vals: &[f32; N]) {
        for (i, val) in vals.iter().enumerate() {
            if i > 0 {
                buffer.encode_bytes(b" ");
            }
            self.encode_float(buffer, *val);
        }
    }

    fn is_new_edge(
        &self,
        pc: &PointCloud,
        _ct: &CornerTable,
        ci: CornerIndex,
        mesh: &Mesh,
    ) -> bool {
        let pi = mesh.corner_to_point_id(ci);
        if self.added_edges_att_id >= 0 {
            let att = pc.attribute(self.added_edges_att_id).unwrap();
            let mut value = [0u8; 1];
            att.get_mapped_value(pi, &mut value);
            return value[0] == 1;
        }
        false
    }

    fn find_original_face_edges(
        &self,
        pc: &PointCloud,
        mesh: &Mesh,
        face_index: FaceIndex,
        corner_table: &CornerTable,
        triangle_visited: &mut [bool],
        polygon_edges: &mut PolygonEdges,
    ) {
        if triangle_visited[face_index.value() as usize] {
            return;
        }
        triangle_visited[face_index.value() as usize] = true;
        let face = mesh.face(face_index);
        for c in 0..3 {
            let ci = corner_table.first_corner(face_index) + c as u32;
            let co = corner_table.opposite(ci);
            let mut is_new_edge = self.is_new_edge(pc, corner_table, ci, mesh);
            if !is_new_edge && co != INVALID_CORNER_INDEX {
                is_new_edge = self.is_new_edge(pc, corner_table, co, mesh);
            }
            if is_new_edge && co != INVALID_CORNER_INDEX {
                let opposite_face_index = corner_table.face(co);
                self.find_original_face_edges(
                    pc,
                    mesh,
                    opposite_face_index,
                    corner_table,
                    triangle_visited,
                    polygon_edges,
                );
            } else {
                let point_from = face[(c + 1) % 3];
                let point_to = face[(c + 2) % 3];
                let pos_att = pc
                    .attribute(self.pos_att_id)
                    .expect("Position attribute not set");
                let key = pos_att.mapped_index(point_from);
                let _ = polygon_edges.insert(key, point_to);
            }
        }
    }
}

type PolygonEdges = BTreeMap<AttributeValueIndex, PointIndex>;

impl Default for ObjEncoder {
    fn default() -> Self {
        Self::new()
    }
}
