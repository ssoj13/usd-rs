//! Misc mesh utilities.
//! Reference: `_ref/draco/src/draco/mesh/mesh_misc_functions.h` + `.cc`.

use crate::attributes::geometry_attribute::GeometryAttributeType;
use crate::attributes::geometry_indices::{
    CornerIndex, FaceIndex, VertexIndex, INVALID_CORNER_INDEX,
};
use crate::attributes::point_attribute::PointAttribute;
use crate::core::draco_index_type_vector::IndexTypeVector;
use crate::core::vector_d::{DracoFromF32, DracoToF32, VectorD};
use crate::mesh::corner_table::{CornerTable, FaceType};
use crate::mesh::mesh::Mesh;

pub fn create_corner_table_from_position_attribute(mesh: &Mesh) -> Option<CornerTable> {
    create_corner_table_from_attribute(mesh, GeometryAttributeType::Position)
}

pub fn create_corner_table_from_attribute(
    mesh: &Mesh,
    attr_type: GeometryAttributeType,
) -> Option<CornerTable> {
    let att = mesh.get_named_attribute(attr_type)?;
    let num_points = mesh.num_points();
    let mut faces = IndexTypeVector::<FaceIndex, FaceType>::with_size(mesh.num_faces() as usize);
    let mut new_face: FaceType = [VertexIndex::from(0u32); 3];
    for i in 0..mesh.num_faces() {
        let fi = FaceIndex::from(i);
        let face = mesh.face(fi);
        for j in 0..3 {
            let pi = face[j].value();
            if pi >= num_points {
                return None;
            }
            let att_index = att.mapped_index(face[j]).value();
            new_face[j] = VertexIndex::from(att_index);
        }
        faces[fi] = new_face;
    }
    CornerTable::create(&faces)
}

pub fn create_corner_table_from_all_attributes(mesh: &Mesh) -> Option<CornerTable> {
    let mut faces = IndexTypeVector::<FaceIndex, FaceType>::with_size(mesh.num_faces() as usize);
    let mut new_face: FaceType = [VertexIndex::from(0u32); 3];
    for i in 0..mesh.num_faces() {
        let fi = FaceIndex::from(i);
        let face = mesh.face(fi);
        for j in 0..3 {
            new_face[j] = VertexIndex::from(face[j].value());
        }
        faces[fi] = new_face;
    }
    CornerTable::create(&faces)
}

pub fn is_corner_opposite_to_attribute_seam(
    ci: CornerIndex,
    att: &PointAttribute,
    mesh: &Mesh,
    ct: &CornerTable,
) -> bool {
    let opp_ci = ct.opposite(ci);
    if opp_ci == INVALID_CORNER_INDEX {
        return false;
    }
    let c0 = ct.next(ci);
    let c1 = ct.previous(opp_ci);
    if att.mapped_index(mesh.corner_to_point_id(c0))
        != att.mapped_index(mesh.corner_to_point_id(c1))
    {
        return true;
    }
    let c0 = ct.previous(ci);
    let c1 = ct.next(opp_ci);
    if att.mapped_index(mesh.corner_to_point_id(c0))
        != att.mapped_index(mesh.corner_to_point_id(c1))
    {
        return true;
    }
    false
}

pub trait InterpolatedScalar: DracoToF32 + DracoFromF32 + Copy + Default + PartialEq {
    const IS_INTEGRAL: bool;
}

impl InterpolatedScalar for f32 {
    const IS_INTEGRAL: bool = false;
}
impl InterpolatedScalar for f64 {
    const IS_INTEGRAL: bool = false;
}
impl InterpolatedScalar for i8 {
    const IS_INTEGRAL: bool = true;
}
impl InterpolatedScalar for i16 {
    const IS_INTEGRAL: bool = true;
}
impl InterpolatedScalar for i32 {
    const IS_INTEGRAL: bool = true;
}
impl InterpolatedScalar for i64 {
    const IS_INTEGRAL: bool = true;
}
impl InterpolatedScalar for u8 {
    const IS_INTEGRAL: bool = true;
}
impl InterpolatedScalar for u16 {
    const IS_INTEGRAL: bool = true;
}
impl InterpolatedScalar for u32 {
    const IS_INTEGRAL: bool = true;
}
impl InterpolatedScalar for u64 {
    const IS_INTEGRAL: bool = true;
}

pub fn compute_interpolated_attribute_value_on_mesh_face<
    Scalar: InterpolatedScalar,
    const N: usize,
>(
    mesh: &Mesh,
    attribute: &PointAttribute,
    fi: FaceIndex,
    barycentric_coord: [f32; 3],
) -> VectorD<Scalar, N> {
    let face = mesh.face(fi);
    let mut vals: [[Scalar; N]; 3] = [[Scalar::default(); N]; 3];
    for c in 0..3 {
        let att_index = attribute.mapped_index(face[c]);
        vals[c] = attribute.get_value_array::<Scalar, N>(att_index);
    }
    if vals[1] == vals[0] && vals[2] == vals[0] {
        return vector_from_array(vals[0]);
    }

    let mut res = VectorD::<Scalar, N>::default();
    for d in 0..N {
        let v0 = vals[0][d].to_f32();
        let v1 = vals[1][d].to_f32();
        let v2 = vals[2][d].to_f32();
        let interpolated: f32 =
            barycentric_coord[0] * v0 + barycentric_coord[1] * v1 + barycentric_coord[2] * v2;
        let value = if Scalar::IS_INTEGRAL {
            (interpolated + 0.5f32).floor()
        } else {
            interpolated
        };
        res[d] = Scalar::from_f32(value);
    }
    res
}

fn vector_from_array<Scalar: Copy + Default, const N: usize>(
    arr: [Scalar; N],
) -> VectorD<Scalar, N> {
    let mut v = VectorD::<Scalar, N>::default();
    for i in 0..N {
        v[i] = arr[i];
    }
    v
}
