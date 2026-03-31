//! Triangle soup mesh builder.
//! Reference: `_ref/draco/src/draco/mesh/triangle_soup_mesh_builder.h` + `.cc`.

use crate::attributes::draco_numeric::DracoNumeric;
use crate::attributes::geometry_attribute::{GeometryAttribute, GeometryAttributeType};
use crate::attributes::geometry_indices::{AttributeValueIndex, FaceIndex, PointIndex};
use crate::core::draco_types::data_type_length;
use crate::core::draco_types::DataType;
use crate::mesh::mesh::{Mesh, MeshAttributeElementType};
use crate::metadata::geometry_metadata::{AttributeMetadata, GeometryMetadata};

pub struct TriangleSoupMeshBuilder {
    attribute_element_types: Vec<i8>,
    mesh: Option<Mesh>,
}

impl TriangleSoupMeshBuilder {
    pub fn new() -> Self {
        Self {
            attribute_element_types: Vec::new(),
            mesh: None,
        }
    }

    pub fn start(&mut self, num_faces: i32) {
        let mut mesh = Mesh::new();
        mesh.set_num_faces(num_faces as usize);
        mesh.set_num_points((num_faces * 3) as u32);
        self.attribute_element_types.clear();
        self.mesh = Some(mesh);
    }

    /// Sets mesh name.
    pub fn set_name(&mut self, name: &str) {
        let mesh = self.mesh.as_mut().expect("Builder not started");
        mesh.set_name(name);
    }

    pub fn add_attribute(
        &mut self,
        attribute_type: GeometryAttributeType,
        num_components: i8,
        data_type: DataType,
    ) -> i32 {
        self.add_attribute_with_normalized(attribute_type, num_components, data_type, false)
    }

    pub fn add_attribute_with_normalized(
        &mut self,
        attribute_type: GeometryAttributeType,
        num_components: i8,
        data_type: DataType,
        normalized: bool,
    ) -> i32 {
        let mesh = self.mesh.as_mut().expect("Builder not started");
        let mut va = GeometryAttribute::new();
        let stride = (data_type_length(data_type) as i64) * (num_components as i64);
        va.init(
            attribute_type,
            None,
            num_components as u8,
            data_type,
            normalized,
            stride,
            0,
        );
        self.attribute_element_types.push(-1);
        mesh.add_attribute_from_geometry(&va, true, mesh.num_points())
    }

    /// Sets the name for a given attribute.
    pub fn set_attribute_name(&mut self, att_id: i32, name: &str) {
        let mesh = self.mesh.as_mut().expect("Builder not started");
        if let Some(att) = mesh.attribute_mut(att_id) {
            att.set_name(name);
        }
    }

    pub fn set_attribute_values_for_face<T: Copy, const N: usize>(
        &mut self,
        att_id: i32,
        face_id: FaceIndex,
        corner_value_0: &[T; N],
        corner_value_1: &[T; N],
        corner_value_2: &[T; N],
    ) {
        let mesh = self.mesh.as_mut().expect("Builder not started");
        let start_index = 3 * face_id.value();
        let att = mesh.attribute_mut(att_id).expect("Invalid attribute id");
        att.set_attribute_value_array(AttributeValueIndex::from(start_index), corner_value_0);
        att.set_attribute_value_array(AttributeValueIndex::from(start_index + 1), corner_value_1);
        att.set_attribute_value_array(AttributeValueIndex::from(start_index + 2), corner_value_2);
        mesh.set_face(
            face_id,
            [
                PointIndex::from(start_index),
                PointIndex::from(start_index + 1),
                PointIndex::from(start_index + 2),
            ],
        );
        self.attribute_element_types[att_id as usize] =
            MeshAttributeElementType::MeshCornerAttribute as i8;
    }

    /// Converts values from InT to the attribute's stored type and sets for a face.
    /// C++ parity: ConvertAndSetAttributeValuesForFace (triangle_soup_mesh_builder.h:64-69).
    pub fn convert_and_set_attribute_values_for_face<
        InT: DracoNumeric + Default,
        const N: usize,
    >(
        &mut self,
        att_id: i32,
        face_id: FaceIndex,
        corner_value_0: &[InT; N],
        corner_value_1: &[InT; N],
        corner_value_2: &[InT; N],
    ) -> bool {
        let mesh = match &mut self.mesh {
            Some(m) => m,
            None => return false,
        };
        let start_index = 3 * face_id.value();
        let att = match mesh.attribute_mut(att_id) {
            Some(a) => a,
            None => return false,
        };
        if !att.convert_and_set_value(AttributeValueIndex::from(start_index), corner_value_0) {
            return false;
        }
        if !att.convert_and_set_value(AttributeValueIndex::from(start_index + 1), corner_value_1) {
            return false;
        }
        if !att.convert_and_set_value(AttributeValueIndex::from(start_index + 2), corner_value_2) {
            return false;
        }
        mesh.set_face(
            face_id,
            [
                PointIndex::from(start_index),
                PointIndex::from(start_index + 1),
                PointIndex::from(start_index + 2),
            ],
        );
        self.attribute_element_types[att_id as usize] =
            MeshAttributeElementType::MeshCornerAttribute as i8;
        true
    }

    /// Sets attribute values for a face using raw bytes (source bytes are copied).
    pub fn set_attribute_values_for_face_bytes(
        &mut self,
        att_id: i32,
        face_id: FaceIndex,
        corner_value_0: &[u8],
        corner_value_1: &[u8],
        corner_value_2: &[u8],
    ) {
        let mesh = self.mesh.as_mut().expect("Builder not started");
        let start_index = 3 * face_id.value();
        let att = mesh.attribute_mut(att_id).expect("Invalid attribute id");
        att.set_attribute_value_bytes(AttributeValueIndex::from(start_index), corner_value_0);
        att.set_attribute_value_bytes(AttributeValueIndex::from(start_index + 1), corner_value_1);
        att.set_attribute_value_bytes(AttributeValueIndex::from(start_index + 2), corner_value_2);
        mesh.set_face(
            face_id,
            [
                PointIndex::from(start_index),
                PointIndex::from(start_index + 1),
                PointIndex::from(start_index + 2),
            ],
        );
        self.attribute_element_types[att_id as usize] =
            MeshAttributeElementType::MeshCornerAttribute as i8;
    }

    pub fn set_per_face_attribute_value_for_face<T: Copy, const N: usize>(
        &mut self,
        att_id: i32,
        face_id: FaceIndex,
        value: &[T; N],
    ) {
        let mesh = self.mesh.as_mut().expect("Builder not started");
        let start_index = 3 * face_id.value();
        let att = mesh.attribute_mut(att_id).expect("Invalid attribute id");
        att.set_attribute_value_array(AttributeValueIndex::from(start_index), value);
        att.set_attribute_value_array(AttributeValueIndex::from(start_index + 1), value);
        att.set_attribute_value_array(AttributeValueIndex::from(start_index + 2), value);
        mesh.set_face(
            face_id,
            [
                PointIndex::from(start_index),
                PointIndex::from(start_index + 1),
                PointIndex::from(start_index + 2),
            ],
        );
        let element_type = &mut self.attribute_element_types[att_id as usize];
        if *element_type < 0 {
            *element_type = MeshAttributeElementType::MeshFaceAttribute as i8;
        }
    }

    pub fn add_metadata(&mut self, metadata: GeometryMetadata) {
        let mesh = self.mesh.as_mut().expect("Builder not started");
        mesh.add_metadata(metadata);
    }

    pub fn add_attribute_metadata(&mut self, att_id: i32, metadata: AttributeMetadata) {
        let mesh = self.mesh.as_mut().expect("Builder not started");
        mesh.add_attribute_metadata(att_id, metadata);
    }

    pub fn set_attribute_unique_id(&mut self, att_id: i32, unique_id: u32) {
        let mesh = self.mesh.as_mut().expect("Builder not started");
        let att = mesh.attribute_mut(att_id).expect("Invalid attribute id");
        att.set_unique_id(unique_id);
    }

    pub fn finalize(&mut self) -> Option<Mesh> {
        let mut mesh = self.mesh.take()?;
        if !mesh.deduplicate_attribute_values() {
            return None;
        }
        mesh.deduplicate_point_ids();
        for (i, element_type) in self.attribute_element_types.iter().enumerate() {
            if *element_type >= 0 {
                if let Some(et) = mesh_attribute_element_type_from_i8(*element_type) {
                    mesh.set_attribute_element_type(i as i32, et);
                }
            }
        }
        Some(mesh)
    }
}

impl Default for TriangleSoupMeshBuilder {
    fn default() -> Self {
        Self::new()
    }
}

fn mesh_attribute_element_type_from_i8(value: i8) -> Option<MeshAttributeElementType> {
    match value {
        0 => Some(MeshAttributeElementType::MeshVertexAttribute),
        1 => Some(MeshAttributeElementType::MeshCornerAttribute),
        2 => Some(MeshAttributeElementType::MeshFaceAttribute),
        _ => None,
    }
}
