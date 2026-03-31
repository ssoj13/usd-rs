//! Geometry wrappers for JavaScript bindings.
//!
//! These types mirror the WebIDL-exposed classes in Draco's Emscripten build
//! and provide access to core geometry data (point clouds, meshes, attributes).

use std::cell::RefCell;
use std::rc::Rc;

use crate::types::{data_type_to_i32, transform_type_to_i32};
use draco_core::attributes::attribute_octahedron_transform::AttributeOctahedronTransform as CoreOctTransform;
use draco_core::attributes::attribute_quantization_transform::AttributeQuantizationTransform as CoreQuantTransform;
use draco_core::attributes::attribute_transform::AttributeTransform;
use draco_core::attributes::attribute_transform_data::AttributeTransformData as CoreTransformData;
use draco_core::attributes::geometry_attribute::{
    GeometryAttribute as CoreGeometryAttribute, GeometryAttributeType,
};
use draco_core::attributes::geometry_indices::AttributeValueIndex;
use draco_core::attributes::point_attribute::PointAttribute as CorePointAttribute;
use draco_core::mesh::mesh::Mesh as CoreMesh;
use draco_core::point_cloud::point_cloud::PointCloud as CorePointCloud;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[derive(Clone)]
pub(crate) enum GeometrySource {
    PointCloud(Rc<RefCell<CorePointCloud>>),
    Mesh(Rc<RefCell<CoreMesh>>),
}

impl GeometrySource {
    fn with_attribute<R>(
        &self,
        att_id: i32,
        f: impl FnOnce(&CorePointAttribute) -> R,
    ) -> Option<R> {
        match self {
            GeometrySource::PointCloud(pc) => {
                let pc_ref = pc.borrow();
                pc_ref.attribute(att_id).map(f)
            }
            GeometrySource::Mesh(mesh) => {
                let mesh_ref = mesh.borrow();
                mesh_ref.attribute(att_id).map(f)
            }
        }
    }

    // Parity: mutation helper is used by JS bindings (wasm32) only.
    #[allow(dead_code)]
    fn with_attribute_mut<R>(
        &self,
        att_id: i32,
        f: impl FnOnce(&mut CorePointAttribute) -> R,
    ) -> Option<R> {
        match self {
            GeometrySource::PointCloud(pc) => {
                let mut pc_ref = pc.borrow_mut();
                pc_ref.attribute_mut(att_id).map(f)
            }
            GeometrySource::Mesh(mesh) => {
                let mut mesh_ref = mesh.borrow_mut();
                mesh_ref.attribute_mut(att_id).map(f)
            }
        }
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[allow(dead_code)] // JS-exposed wrapper; not referenced in native builds.
pub struct GeometryAttribute {
    inner: CoreGeometryAttribute,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl GeometryAttribute {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    pub fn new() -> Self {
        Self {
            inner: CoreGeometryAttribute::new(),
        }
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct AttributeTransformData {
    inner: CoreTransformData,
}

impl AttributeTransformData {
    pub(crate) fn from_core(data: CoreTransformData) -> Self {
        Self { inner: data }
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl AttributeTransformData {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    pub fn new() -> Self {
        Self {
            inner: CoreTransformData::new(),
        }
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = transform_type))]
    pub fn transform_type(&self) -> i32 {
        transform_type_to_i32(self.inner.transform_type())
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct PointAttribute {
    source: GeometrySource,
    att_id: i32,
}

impl PointAttribute {
    pub(crate) fn from_source(source: GeometrySource, att_id: i32) -> Self {
        Self { source, att_id }
    }

    pub(crate) fn with_attribute<R>(&self, f: impl FnOnce(&CorePointAttribute) -> R) -> Option<R> {
        self.source.with_attribute(self.att_id, f)
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl PointAttribute {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    pub fn new() -> Self {
        Self {
            source: GeometrySource::PointCloud(Rc::new(RefCell::new(CorePointCloud::new()))),
            att_id: -1,
        }
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = size))]
    pub fn size(&self) -> i32 {
        self.with_attribute(|att| att.size() as i32).unwrap_or(0)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = GetAttributeTransformData))]
    pub fn get_attribute_transform_data(&self) -> Option<AttributeTransformData> {
        self.with_attribute(|att| att.get_attribute_transform_data().cloned())
            .flatten()
            .map(AttributeTransformData::from_core)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = attribute_type))]
    pub fn attribute_type(&self) -> i32 {
        self.with_attribute(|att| att.attribute_type() as i32)
            .unwrap_or(GeometryAttributeType::Invalid as i32)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = data_type))]
    pub fn data_type(&self) -> i32 {
        self.with_attribute(|att| data_type_to_i32(att.data_type()))
            .unwrap_or(0)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = num_components))]
    pub fn num_components(&self) -> i32 {
        self.with_attribute(|att| att.num_components() as i32)
            .unwrap_or(0)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = normalized))]
    pub fn normalized(&self) -> bool {
        self.with_attribute(|att| att.normalized()).unwrap_or(false)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = byte_stride))]
    pub fn byte_stride(&self) -> i32 {
        self.with_attribute(|att| att.byte_stride() as i32)
            .unwrap_or(0)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = byte_offset))]
    pub fn byte_offset(&self) -> i32 {
        self.with_attribute(|att| att.byte_offset() as i32)
            .unwrap_or(0)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = unique_id))]
    pub fn unique_id(&self) -> i32 {
        self.with_attribute(|att| att.unique_id() as i32)
            .unwrap_or(-1)
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct AttributeQuantizationTransform {
    inner: CoreQuantTransform,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl AttributeQuantizationTransform {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    pub fn new() -> Self {
        Self {
            inner: CoreQuantTransform::new(),
        }
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = InitFromAttribute))]
    pub fn init_from_attribute(&mut self, attribute: &PointAttribute) -> bool {
        attribute
            .with_attribute(|att| self.inner.init_from_attribute(att))
            .unwrap_or(false)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = quantization_bits))]
    pub fn quantization_bits(&self) -> i32 {
        self.inner.quantization_bits()
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = min_value))]
    pub fn min_value(&self, axis: i32) -> f32 {
        let axis = axis.max(0) as usize;
        self.inner.min_value(axis)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = range))]
    pub fn range(&self) -> f32 {
        self.inner.range()
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct AttributeOctahedronTransform {
    inner: CoreOctTransform,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl AttributeOctahedronTransform {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    pub fn new() -> Self {
        Self {
            inner: CoreOctTransform::new(),
        }
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = InitFromAttribute))]
    pub fn init_from_attribute(&mut self, attribute: &PointAttribute) -> bool {
        attribute
            .with_attribute(|att| self.inner.init_from_attribute(att))
            .unwrap_or(false)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = quantization_bits))]
    pub fn quantization_bits(&self) -> i32 {
        self.inner.quantization_bits()
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct PointCloud {
    inner: Rc<RefCell<CorePointCloud>>,
}

impl PointCloud {
    pub(crate) fn source(&self) -> GeometrySource {
        GeometrySource::PointCloud(self.inner.clone())
    }

    pub(crate) fn inner(&self) -> Rc<RefCell<CorePointCloud>> {
        self.inner.clone()
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl PointCloud {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    pub fn new() -> Self {
        Self {
            inner: Rc::new(RefCell::new(CorePointCloud::new())),
        }
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = num_attributes))]
    pub fn num_attributes(&self) -> i32 {
        self.inner.borrow().num_attributes()
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = num_points))]
    pub fn num_points(&self) -> i32 {
        self.inner.borrow().num_points() as i32
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct Mesh {
    inner: Rc<RefCell<CoreMesh>>,
}

impl Mesh {
    pub(crate) fn source(&self) -> GeometrySource {
        GeometrySource::Mesh(self.inner.clone())
    }

    pub(crate) fn inner(&self) -> Rc<RefCell<CoreMesh>> {
        self.inner.clone()
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl Mesh {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    pub fn new() -> Self {
        Self {
            inner: Rc::new(RefCell::new(CoreMesh::new())),
        }
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = num_faces))]
    pub fn num_faces(&self) -> i32 {
        self.inner.borrow().num_faces() as i32
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = num_attributes))]
    pub fn num_attributes(&self) -> i32 {
        self.inner.borrow().num_attributes()
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = num_points))]
    pub fn num_points(&self) -> i32 {
        self.inner.borrow().num_points() as i32
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = set_num_points))]
    pub fn set_num_points(&self, num_points: i32) {
        if num_points >= 0 {
            self.inner.borrow_mut().set_num_points(num_points as u32);
        }
    }
}

pub(crate) fn attribute_from_point_cloud(pc: &PointCloud, att_id: i32) -> PointAttribute {
    PointAttribute::from_source(pc.source(), att_id)
}

pub(crate) fn attribute_from_mesh(mesh: &Mesh, att_id: i32) -> PointAttribute {
    PointAttribute::from_source(mesh.source(), att_id)
}

#[allow(dead_code)] // Used by JS bindings; kept for parity in native builds.
pub(crate) fn set_attribute_normalized(attribute: &PointAttribute, normalized: bool) -> bool {
    attribute
        .source
        .with_attribute_mut(attribute.att_id, |att| {
            att.set_normalized(normalized);
        })
        .is_some()
}

pub(crate) fn attribute_value_index(att_index: i32) -> Option<AttributeValueIndex> {
    if att_index < 0 {
        None
    } else {
        Some(AttributeValueIndex::from(att_index as u32))
    }
}
