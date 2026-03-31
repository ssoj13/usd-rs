//! glTF decoder.
//!
//! What: Loads glTF/GLB assets into Draco meshes or scenes.
//! Why: Provides parity with Draco C++ glTF transcoder and extensions.
//! How: Parses glTF JSON + buffers, maps attributes/materials, and applies
//!      transforms while decoding mesh features and structural metadata.
//! Where used: `mesh_io` (mesh decode) and scene transcoding tools.

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use gltf::validation::Checked;
use gltf_json as gltf;
use serde_json::{Map, Value};

use crate::io::file_utils;
use crate::io::tiny_gltf_utils::{FloatPack, GltfModel, TinyGltfUtils};
use crate::scene::{
    LightIndex, LightType, MaterialsVariantsMapping, MeshGroupIndex, MeshIndex, MeshInstance,
    Quaterniond, Scene, SceneNodeIndex, SkinIndex, TrsMatrix, Vector3d, INVALID_MESH_INDEX,
    INVALID_SCENE_NODE_INDEX,
};
use crate::texture::texture_transform::TextureTransform;
use crate::texture::Texture;
use draco_bitstream::compression::decode::Decoder;
use draco_core::attributes::geometry_attribute::GeometryAttributeType;
use draco_core::attributes::geometry_indices::{AttributeValueIndex, FaceIndex, PointIndex};
use draco_core::attributes::point_attribute::PointAttribute;
use draco_core::core::constants::DRACO_PI;
use draco_core::core::decoder_buffer::DecoderBuffer;
use draco_core::core::draco_types::{data_type_length, DataType};
use draco_core::core::status::{ok_status, Status, StatusCode};
use draco_core::core::status_or::StatusOr;
use draco_core::core::vector_d::{Vector3f, Vector4f};
use draco_core::material::material::Material;
use draco_core::material::material_library::MaterialLibrary;
use draco_core::mesh::mesh_features::MeshFeatures;
use draco_core::mesh::mesh_indices::MeshFeaturesIndex;
use draco_core::mesh::mesh_utils::Matrix4d;
use draco_core::mesh::triangle_soup_mesh_builder::TriangleSoupMeshBuilder;
use draco_core::mesh::{Mesh, MeshAttributeElementType};
use draco_core::metadata::geometry_metadata::AttributeMetadata;
use draco_core::metadata::property_attribute::{
    Property as PropertyAttributeProperty, PropertyAttribute,
};
use draco_core::metadata::property_table::{
    Data as PropertyTableData, Property as PropertyTableProperty, PropertyTable,
};
use draco_core::metadata::structural_metadata::StructuralMetadata;
use draco_core::metadata::structural_metadata_schema::{
    Object as StructuralMetadataObject, StructuralMetadataSchema,
};
use draco_core::metadata::Metadata;
use draco_core::point_cloud::point_cloud_builder::PointCloudBuilder;
use draco_core::point_cloud::PointCloud;
use draco_core::texture::source_image::SourceImage;
use draco_core::texture::texture_library::TextureLibrary;
use draco_core::texture::texture_map::{
    TextureMapAxisWrappingMode, TextureMapFilterType, TextureMapType, TextureMapWrappingMode,
};

const KHR_MATERIALS_UNLIT: &str = "KHR_materials_unlit";
const KHR_TEXTURE_TRANSFORM: &str = "KHR_texture_transform";
const KHR_DRACO_MESH_COMPRESSION: &str = "KHR_draco_mesh_compression";
const KHR_LIGHTS_PUNCTUAL: &str = "KHR_lights_punctual";
const KHR_MATERIALS_VARIANTS: &str = "KHR_materials_variants";
const EXT_STRUCTURAL_METADATA: &str = "EXT_structural_metadata";
const EXT_MESH_FEATURES: &str = "EXT_mesh_features";

const SUPPORTED_REQUIRED_EXTENSIONS: &[&str] = &[
    KHR_MATERIALS_UNLIT,
    KHR_TEXTURE_TRANSFORM,
    KHR_DRACO_MESH_COMPRESSION,
    KHR_LIGHTS_PUNCTUAL,
    KHR_MATERIALS_VARIANTS,
    EXT_STRUCTURAL_METADATA,
    EXT_MESH_FEATURES,
];

fn supports_required_extension(extension: &str) -> bool {
    SUPPORTED_REQUIRED_EXTENSIONS.contains(&extension)
}

/// Safe conversion to bytes for material attribute values (u8, u16, u32).
trait MaterialValueBytes {
    fn write_bytes(&self, out: &mut [u8]);
}
impl MaterialValueBytes for u8 {
    fn write_bytes(&self, out: &mut [u8]) {
        out[0] = *self;
    }
}
impl MaterialValueBytes for u16 {
    fn write_bytes(&self, out: &mut [u8]) {
        out.copy_from_slice(&self.to_le_bytes());
    }
}
impl MaterialValueBytes for u32 {
    fn write_bytes(&self, out: &mut [u8]) {
        out.copy_from_slice(&self.to_le_bytes());
    }
}

/// Scene graph decode mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GltfSceneGraphMode {
    Tree,
    Dag,
}

#[derive(Clone, Debug)]
struct MeshAttributeData {
    component_type: gltf::accessor::ComponentType,
    attribute_type: gltf::accessor::Type,
    normalized: bool,
    total_attribute_counts: i32,
}

#[derive(Clone, Debug)]
struct PrimitiveSignature {
    attributes: Vec<(String, usize)>,
    indices: Option<usize>,
    mode: u32,
    extras: Option<Value>,
    extensions: Option<Value>,
    targets: Option<Value>,
}

#[derive(Clone, Debug)]
struct ParsedTextureInfo {
    index: i32,
    tex_coord: i32,
    extensions: Option<gltf::extensions::texture::Info>,
}

impl PrimitiveSignature {
    fn new(primitive: &gltf::mesh::Primitive) -> Self {
        let mut attributes: Vec<(String, usize)> = Vec::new();
        for (semantic, accessor) in primitive.attributes.iter() {
            let name = semantic_to_string(semantic);
            attributes.push((name, accessor.value()));
        }
        let indices = primitive.indices.as_ref().map(|idx| idx.value());
        let mode = mode_to_gl_enum(&primitive.mode);
        let extras = extras_to_value(&primitive.extras);
        let extensions = primitive
            .extensions
            .as_ref()
            .and_then(|ext| serde_json::to_value(ext).ok());
        let targets = primitive
            .targets
            .as_ref()
            .and_then(|targets| serde_json::to_value(targets).ok());
        Self {
            attributes,
            indices,
            mode,
            extras,
            extensions,
            targets,
        }
    }
}

impl PartialEq for PrimitiveSignature {
    fn eq(&self, other: &Self) -> bool {
        self.indices == other.indices
            && self.attributes == other.attributes
            && self.mode == other.mode
            && self.extras == other.extras
            && self.extensions == other.extensions
            && self.targets == other.targets
    }
}

impl Eq for PrimitiveSignature {}

impl Hash for PrimitiveSignature {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Match Draco C++ hashing: attributes + indices + mode only.
        self.attributes.len().hash(state);
        for (name, index) in &self.attributes {
            name.hash(state);
            index.hash(state);
        }
        self.indices.hash(state);
        self.mode.hash(state);
    }
}

pub struct GltfDecoder {
    gltf_model: GltfModel,
    input_file_name: String,
    input_files: Vec<String>,
    mb: TriangleSoupMeshBuilder,
    pb: PointCloudBuilder,
    feature_id_attribute_indices: HashMap<usize, i32>,
    next_face_id: i32,
    next_point_id: i32,
    total_face_indices_count: i32,
    total_point_indices_count: i32,
    material_att_id: i32,
    mesh_attribute_data: HashMap<String, MeshAttributeData>,
    attribute_name_to_draco_mesh_attribute_id: HashMap<String, i32>,
    gltf_primitive_material_to_draco_material: HashMap<i32, i32>,
    gltf_primitive_material_to_scales: HashMap<i32, Vec<f32>>,
    gltf_image_to_draco_texture: HashMap<i32, i32>,
    scene: Option<Scene>,
    gltf_node_to_scenenode_index: HashMap<i32, SceneNodeIndex>,
    gltf_mesh_to_scene_mesh_group: HashMap<i32, MeshGroupIndex>,
    gltf_scene_graph_mode: GltfSceneGraphMode,
    deduplicate_vertices: bool,
    gltf_primitive_to_draco_mesh_index: HashMap<PrimitiveSignature, MeshIndex>,
    /// True when decoding to a Scene (stays set even during scene.take() borrow).
    decoding_scene: bool,
}

impl GltfDecoder {
    pub fn new() -> Self {
        Self {
            gltf_model: GltfModel::default(),
            input_file_name: String::new(),
            input_files: Vec::new(),
            mb: TriangleSoupMeshBuilder::new(),
            pb: PointCloudBuilder::new(),
            feature_id_attribute_indices: HashMap::new(),
            next_face_id: 0,
            next_point_id: 0,
            total_face_indices_count: 0,
            total_point_indices_count: 0,
            material_att_id: -1,
            mesh_attribute_data: HashMap::new(),
            attribute_name_to_draco_mesh_attribute_id: HashMap::new(),
            gltf_primitive_material_to_draco_material: HashMap::new(),
            gltf_primitive_material_to_scales: HashMap::new(),
            gltf_image_to_draco_texture: HashMap::new(),
            scene: None,
            gltf_node_to_scenenode_index: HashMap::new(),
            gltf_mesh_to_scene_mesh_group: HashMap::new(),
            gltf_scene_graph_mode: GltfSceneGraphMode::Tree,
            deduplicate_vertices: true,
            gltf_primitive_to_draco_mesh_index: HashMap::new(),
            decoding_scene: false,
        }
    }

    pub fn set_scene_graph_mode(&mut self, mode: GltfSceneGraphMode) {
        self.gltf_scene_graph_mode = mode;
    }

    pub fn set_deduplicate_vertices(&mut self, deduplicate_vertices: bool) {
        self.deduplicate_vertices = deduplicate_vertices;
    }

    pub fn decode_from_file(
        &mut self,
        file_name: &str,
        mesh_files: Option<&mut Vec<String>>,
    ) -> StatusOr<Box<Mesh>> {
        self.reset_state();
        let status = self.load_file(file_name);
        if !status.is_ok() {
            return StatusOr::new_status(status);
        }
        let mesh_or = self.build_mesh();
        if let Some(mesh_files) = mesh_files {
            mesh_files.extend(self.input_files.iter().cloned());
        }
        mesh_or
    }

    pub fn decode_from_file_simple(&mut self, file_name: &str) -> StatusOr<Box<Mesh>> {
        self.decode_from_file(file_name, None)
    }

    pub fn decode_from_buffer(&mut self, buffer: &DecoderBuffer) -> StatusOr<Box<Mesh>> {
        self.reset_state();
        let status = self.load_buffer(buffer);
        if !status.is_ok() {
            return StatusOr::new_status(status);
        }
        self.build_mesh()
    }

    pub fn decode_from_file_to_scene(
        &mut self,
        file_name: &str,
        scene_files: Option<&mut Vec<String>>,
    ) -> StatusOr<Box<Scene>> {
        self.reset_state();
        let status = self.load_file(file_name);
        if !status.is_ok() {
            return StatusOr::new_status(status);
        }
        self.scene = Some(Scene::new());
        self.decoding_scene = true;
        let status = self.decode_gltf_to_scene();
        if !status.is_ok() {
            return StatusOr::new_status(status);
        }
        if let Some(scene_files) = scene_files {
            scene_files.extend(self.input_files.iter().cloned());
        }
        StatusOr::new_value(Box::new(self.scene.take().expect("scene")))
    }

    pub fn decode_from_file_to_scene_simple(&mut self, file_name: &str) -> StatusOr<Box<Scene>> {
        self.decode_from_file_to_scene(file_name, None)
    }

    pub fn decode_from_buffer_to_scene(&mut self, buffer: &DecoderBuffer) -> StatusOr<Box<Scene>> {
        self.reset_state();
        let status = self.load_buffer(buffer);
        if !status.is_ok() {
            return StatusOr::new_status(status);
        }
        self.scene = Some(Scene::new());
        self.decoding_scene = true;
        let status = self.decode_gltf_to_scene();
        if !status.is_ok() {
            return StatusOr::new_status(status);
        }
        StatusOr::new_value(Box::new(self.scene.take().expect("scene")))
    }

    fn reset_state(&mut self) {
        self.input_file_name.clear();
        self.input_files.clear();
        self.feature_id_attribute_indices.clear();
        self.next_face_id = 0;
        self.next_point_id = 0;
        self.total_face_indices_count = 0;
        self.total_point_indices_count = 0;
        self.material_att_id = -1;
        self.mesh_attribute_data.clear();
        self.attribute_name_to_draco_mesh_attribute_id.clear();
        self.gltf_primitive_material_to_draco_material.clear();
        self.gltf_primitive_material_to_scales.clear();
        self.gltf_image_to_draco_texture.clear();
        self.scene = None;
        self.gltf_node_to_scenenode_index.clear();
        self.gltf_mesh_to_scene_mesh_group.clear();
        self.gltf_primitive_to_draco_mesh_index.clear();
        self.decoding_scene = false;
    }

    fn load_file(&mut self, file_name: &str) -> Status {
        let extension = file_utils::lowercase_file_extension(file_name);
        let mut data: Vec<u8> = Vec::new();
        if !file_utils::read_file_to_buffer(file_name, &mut data) {
            return Status::new(StatusCode::IoError, "Unable to read glTF file.");
        }
        self.input_file_name = file_name.to_string();

        if extension == "glb" {
            let mut buffer = DecoderBuffer::new();
            buffer.init(&data);
            draco_core::draco_return_if_error!(self.load_buffer(&buffer));
        } else if extension == "gltf" {
            let json = match gltf::Root::from_slice(&data) {
                Ok(root) => root,
                Err(err) => {
                    return Status::new(
                        StatusCode::DracoError,
                        &format!("Failed to parse glTF JSON: {}", err),
                    )
                }
            };
            self.gltf_model = GltfModel {
                json,
                buffers: Vec::new(),
            };
            draco_core::draco_return_if_error!(self.load_buffers_from_model());
        } else {
            return Status::new(StatusCode::DracoError, "Unknown input file extension.");
        }

        draco_core::draco_return_if_error!(self.check_unsupported_features());
        ok_status()
    }

    fn load_buffer(&mut self, buffer: &DecoderBuffer) -> Status {
        let data = buffer.data_head();
        if data.len() < 12 {
            return Status::new(StatusCode::DracoError, "Invalid GLB header.");
        }
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        if magic != 0x46546c67 {
            return Status::new(StatusCode::DracoError, "Invalid GLB magic.");
        }
        let _version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let length = u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;
        if length > data.len() {
            return Status::new(StatusCode::DracoError, "GLB length exceeds buffer.");
        }

        let mut offset = 12usize;
        let mut json_chunk: Option<Vec<u8>> = None;
        let mut bin_chunk: Option<Vec<u8>> = None;
        while offset + 8 <= length {
            let chunk_len = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]) as usize;
            let chunk_type = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            offset += 8;
            if offset + chunk_len > data.len() {
                return Status::new(StatusCode::DracoError, "GLB chunk out of bounds.");
            }
            let chunk_data = &data[offset..offset + chunk_len];
            match chunk_type {
                0x4e4f534a => {
                    json_chunk = Some(chunk_data.to_vec());
                }
                0x004e4942 => {
                    bin_chunk = Some(chunk_data.to_vec());
                }
                _ => {}
            }
            offset += chunk_len;
        }

        let json_chunk = match json_chunk {
            Some(chunk) => chunk,
            None => return Status::new(StatusCode::DracoError, "Missing GLB JSON chunk."),
        };
        let json = match gltf::Root::from_slice(&json_chunk) {
            Ok(root) => root,
            Err(err) => {
                return Status::new(
                    StatusCode::DracoError,
                    &format!("Failed to parse GLB JSON: {}", err),
                )
            }
        };
        self.gltf_model = GltfModel {
            json,
            buffers: Vec::new(),
        };
        draco_core::draco_return_if_error!(self.load_buffers_from_model_with_bin(bin_chunk));

        draco_core::draco_return_if_error!(self.check_unsupported_features());
        self.input_file_name.clear();
        ok_status()
    }

    fn load_buffers_from_model(&mut self) -> Status {
        self.load_buffers_from_model_with_bin(None)
    }

    fn load_buffers_from_model_with_bin(&mut self, bin_chunk: Option<Vec<u8>>) -> Status {
        self.gltf_model.buffers.clear();
        let mut bin_consumed = false;
        let buffer_uris: Vec<Option<String>> = self
            .gltf_model
            .json
            .buffers
            .iter()
            .map(|buffer| buffer.uri.clone())
            .collect();

        for (index, uri) in buffer_uris.into_iter().enumerate() {
            if let Some(uri) = uri.as_ref() {
                if is_data_uri(uri) {
                    let (decoded, _mime) = match decode_data_uri(uri) {
                        Ok(v) => v,
                        Err(status) => return status,
                    };
                    self.gltf_model.buffers.push(decoded);
                } else {
                    let full_path = if self.input_file_name.is_empty() {
                        uri.to_string()
                    } else {
                        file_utils::get_full_path(uri, &self.input_file_name)
                    };
                    let mut data = Vec::new();
                    if !file_utils::read_file_to_buffer(&full_path, &mut data) {
                        return Status::new(StatusCode::IoError, "Unable to read glTF buffer.");
                    }
                    self.gltf_model.buffers.push(data);
                    self.push_input_file(&full_path);
                }
            } else if !bin_consumed {
                if let Some(bin) = bin_chunk.as_ref() {
                    self.gltf_model.buffers.push(bin.clone());
                    bin_consumed = true;
                } else {
                    return Status::new(StatusCode::DracoError, "Missing GLB binary chunk.");
                }
            } else {
                return Status::new(
                    StatusCode::DracoError,
                    &format!("Buffer {} has no URI and no BIN chunk.", index),
                );
            }
        }
        ok_status()
    }

    fn push_input_file(&mut self, path: &str) {
        if !self.input_files.contains(&path.to_string()) {
            self.input_files.push(path.to_string());
        }
    }

    fn build_mesh(&mut self) -> StatusOr<Box<Mesh>> {
        let status = self.gather_attribute_and_material_stats();
        if !status.is_ok() {
            return StatusOr::new_status(status);
        }
        if self.total_face_indices_count > 0 && self.total_point_indices_count > 0 {
            return StatusOr::new_status(Status::new(
                StatusCode::DracoError,
                "Decoding to mesh can't handle triangle and point primitives at the same time.",
            ));
        }
        if self.total_face_indices_count > 0 {
            self.mb.start(self.total_face_indices_count / 3);
            let status = self.add_attributes_to_draco_mesh_mesh();
            if !status.is_ok() {
                return StatusOr::new_status(status);
            }
        } else {
            self.pb.start(self.total_point_indices_count as u32);
            let status = self.add_attributes_to_draco_mesh_point();
            if !status.is_ok() {
                return StatusOr::new_status(status);
            }
        }

        // Clear attribute indices before populating attributes in builders.
        self.feature_id_attribute_indices.clear();

        let node_indices: Vec<i32> = self
            .gltf_model
            .json
            .scenes
            .iter()
            .flat_map(|scene| scene.nodes.iter().map(|node| node.value() as i32))
            .collect();
        for node_index in node_indices {
            let parent_matrix = Matrix4d::identity();
            let status = self.decode_node(node_index, &parent_matrix);
            if !status.is_ok() {
                return StatusOr::new_status(status);
            }
        }

        let mesh_or = self.build_mesh_from_builder();
        if !mesh_or.is_ok() {
            return StatusOr::new_status(Status::new(
                StatusCode::DracoError,
                "Failed to build Draco mesh from glTF data.",
            ));
        }
        let mut mesh = mesh_or.into_value();

        let status = self.copy_textures_to_mesh(&mut mesh);
        if !status.is_ok() {
            return StatusOr::new_status(status);
        }
        self.set_attribute_properties_on_draco_mesh(&mut mesh);
        let status = self.add_materials_to_draco_mesh(&mut mesh);
        if !status.is_ok() {
            return StatusOr::new_status(status);
        }
        let status = self.add_primitive_extensions_to_mesh(&mut mesh);
        if !status.is_ok() {
            return StatusOr::new_status(status);
        }
        let status = self.add_structural_metadata_to_mesh(&mut mesh);
        if !status.is_ok() {
            return StatusOr::new_status(status);
        }
        let status = self.maybe_generate_auto_tangents_for_mesh(&mut mesh);
        if !status.is_ok() {
            return StatusOr::new_status(status);
        }
        self.move_non_material_textures_mesh(&mut mesh);
        let status = self.add_asset_metadata_to_mesh(&mut mesh);
        if !status.is_ok() {
            return StatusOr::new_status(status);
        }
        StatusOr::new_value(mesh)
    }

    fn check_unsupported_features(&self) -> Status {
        // Morph targets.
        for mesh in &self.gltf_model.json.meshes {
            for primitive in &mesh.primitives {
                if primitive.targets.is_some() {
                    return Status::new(
                        StatusCode::UnsupportedFeature,
                        "Morph targets are unsupported.",
                    );
                }
            }
        }

        // Sparse accessors.
        for accessor in &self.gltf_model.json.accessors {
            if accessor.sparse.is_some() {
                return Status::new(
                    StatusCode::UnsupportedFeature,
                    "Sparse accessors are unsupported.",
                );
            }
        }

        // Extensions.
        for extension in &self.gltf_model.json.extensions_required {
            if !supports_required_extension(extension) {
                return Status::new(
                    StatusCode::UnsupportedFeature,
                    &format!("{} is unsupported.", extension),
                );
            }
        }
        ok_status()
    }

    fn decode_node(&mut self, node_index: i32, parent_matrix: &Matrix4d) -> Status {
        let (node_matrix, mesh_index, children) = {
            let node = &self.gltf_model.json.nodes[node_index as usize];
            let trsm = get_node_trs_matrix(node);
            let node_matrix = parent_matrix.mul(&trsm.compute_transformation_matrix());
            let mesh_index = node.mesh.as_ref().map(|m| m.value());
            let children: Vec<i32> = node
                .children
                .as_ref()
                .map(|c| c.iter().map(|child| child.value() as i32).collect())
                .unwrap_or_default();
            (node_matrix, mesh_index, children)
        };

        if let Some(mesh_index) = mesh_index {
            let primitives = self.gltf_model.json.meshes[mesh_index].primitives.clone();
            for primitive in primitives {
                draco_core::draco_return_if_error!(self.decode_primitive(&primitive, &node_matrix));
            }
        }
        for child_index in children {
            draco_core::draco_return_if_error!(self.decode_node(child_index, &node_matrix));
        }
        ok_status()
    }

    fn decode_primitive_attribute_count(&self, primitive: &gltf::mesh::Primitive) -> StatusOr<i32> {
        if primitive.attributes.is_empty() {
            return StatusOr::new_status(Status::new(
                StatusCode::DracoError,
                "Primitive has no attributes.",
            ));
        }
        let first = primitive.attributes.iter().next().unwrap();
        let accessor = &self.gltf_model.json.accessors[first.1.value()];
        let count = i32::try_from(accessor.count.0)
            .map_err(|_| Status::new(StatusCode::DracoError, "Accessor count exceeds i32."));
        match count {
            Ok(v) => StatusOr::new_value(v),
            Err(status) => StatusOr::new_status(status),
        }
    }

    fn decode_primitive_indices_count(&self, primitive: &gltf::mesh::Primitive) -> StatusOr<i32> {
        if primitive.indices.is_none() {
            return self.decode_primitive_attribute_count(primitive);
        }
        let accessor = &self.gltf_model.json.accessors[primitive.indices.as_ref().unwrap().value()];
        let count = i32::try_from(accessor.count.0)
            .map_err(|_| Status::new(StatusCode::DracoError, "Accessor count exceeds i32."));
        match count {
            Ok(v) => StatusOr::new_value(v),
            Err(status) => StatusOr::new_status(status),
        }
    }

    fn decode_primitive_indices(&self, primitive: &gltf::mesh::Primitive) -> StatusOr<Vec<u32>> {
        if primitive.indices.is_none() {
            let num_vertices_or = self.decode_primitive_attribute_count(primitive);
            if !num_vertices_or.is_ok() {
                return StatusOr::new_status(Status::new(
                    StatusCode::DracoError,
                    "Primitive has no attributes.",
                ));
            }
            let num_vertices = num_vertices_or.into_value();
            let mut indices = Vec::with_capacity(num_vertices as usize);
            for i in 0..num_vertices {
                indices.push(i as u32);
            }
            return StatusOr::new_value(indices);
        }
        let accessor = &self.gltf_model.json.accessors[primitive.indices.as_ref().unwrap().value()];
        let indices_or = copy_data_as_uint32(&self.gltf_model, accessor);
        if !indices_or.is_ok() {
            return StatusOr::new_status(indices_or.status().clone());
        }
        StatusOr::new_value(indices_or.into_value())
    }

    fn decode_primitive(
        &mut self,
        primitive: &gltf::mesh::Primitive,
        transform_matrix: &Matrix4d,
    ) -> Status {
        let primitive_mode = mode_to_gl_enum(&primitive.mode);
        if primitive_mode != gltf::mesh::Mode::Triangles.as_gl_enum()
            && primitive_mode != gltf::mesh::Mode::Points.as_gl_enum()
        {
            return Status::new(
                StatusCode::DracoError,
                "Primitive does not contain triangles or points.",
            );
        }

        if self.scene.is_none() {
            let scale = transform_matrix.m[0][0].mul_add(
                transform_matrix.m[0][0],
                transform_matrix.m[1][0] * transform_matrix.m[1][0],
            ) + transform_matrix.m[2][0] * transform_matrix.m[2][0];
            let scale = scale.sqrt() as f32;
            let material_index = primitive
                .material
                .as_ref()
                .map(|m| m.value() as i32)
                .unwrap_or(-1);
            self.gltf_primitive_material_to_scales
                .entry(material_index)
                .or_default()
                .push(scale);
        }

        let draco_extension = get_draco_extension(primitive);
        let mut draco_decoded: Option<DecodedDracoPrimitive> = None;
        if let Some(extension) = draco_extension.as_ref() {
            match decode_draco_primitive(&self.gltf_model, extension) {
                Ok(decoded) => draco_decoded = Some(decoded),
                Err(status) => return status,
            }
        }

        let (indices_data, number_of_faces, number_of_points) =
            if let Some(decoded) = draco_decoded.as_ref() {
                (
                    decoded.indices_data.clone(),
                    decoded.number_of_faces,
                    decoded.number_of_points,
                )
            } else {
                let indices_or = self.decode_primitive_indices(primitive);
                if !indices_or.is_ok() {
                    return Status::new(StatusCode::DracoError, "Could not convert indices.");
                }
                let indices = indices_or.into_value();
                let number_of_faces = (indices.len() / 3) as i32;
                let number_of_points = indices.len() as i32;
                (indices, number_of_faces, number_of_points)
            };

        let mut attribute_entries: Vec<(String, usize)> = primitive
            .attributes
            .iter()
            .map(|(semantic, accessor_index)| {
                (semantic_to_string(semantic), accessor_index.value())
            })
            .collect();
        attribute_entries.sort_by(|a, b| a.0.cmp(&b.0));
        for (attribute_name, accessor_index) in attribute_entries {
            let accessor = self.gltf_model.json.accessors[accessor_index].clone();
            let att_id = *self
                .attribute_name_to_draco_mesh_attribute_id
                .get(&attribute_name)
                .unwrap_or(&-1);
            if att_id == -1 {
                continue;
            }

            if let Some(decoded) = draco_decoded.as_ref() {
                if let Some(unique_id) = decoded.attribute_unique_ids.get(&attribute_name) {
                    match &decoded.geometry {
                        DracoGeometry::Mesh(mesh) => {
                            let attribute =
                                match find_attribute_by_unique_id_mesh(mesh.as_ref(), *unique_id) {
                                    Ok(attribute) => attribute,
                                    Err(status) => return status,
                                };
                            if primitive_mode == gltf::mesh::Mode::Triangles.as_gl_enum() {
                                if let Err(status) = self.add_attribute_values_from_draco_mesh(
                                    &attribute_name,
                                    &accessor,
                                    &indices_data,
                                    att_id,
                                    number_of_faces,
                                    transform_matrix,
                                    mesh.as_ref(),
                                    attribute,
                                ) {
                                    return status;
                                }
                            } else if let Err(status) = self
                                .add_attribute_values_from_draco_point_cloud(
                                    &attribute_name,
                                    &accessor,
                                    &indices_data,
                                    att_id,
                                    number_of_points,
                                    transform_matrix,
                                    mesh.as_ref(),
                                    attribute,
                                )
                            {
                                return status;
                            }
                            continue;
                        }
                        DracoGeometry::PointCloud(pc) => {
                            let attribute =
                                match find_attribute_by_unique_id_point(pc.as_ref(), *unique_id) {
                                    Ok(attribute) => attribute,
                                    Err(status) => return status,
                                };
                            if let Err(status) = self.add_attribute_values_from_draco_point_cloud(
                                &attribute_name,
                                &accessor,
                                &indices_data,
                                att_id,
                                number_of_points,
                                transform_matrix,
                                pc.as_ref(),
                                attribute,
                            ) {
                                return status;
                            }
                            continue;
                        }
                    }
                }
            }

            if primitive_mode == gltf::mesh::Mode::Triangles.as_gl_enum() {
                if let Err(status) = self.add_attribute_values_from_accessor_mesh(
                    &attribute_name,
                    &accessor,
                    &indices_data,
                    att_id,
                    number_of_faces,
                    transform_matrix,
                ) {
                    return status;
                }
            } else if let Err(status) = self.add_attribute_values_from_accessor_point(
                &attribute_name,
                &accessor,
                &indices_data,
                att_id,
                number_of_points,
                transform_matrix,
            ) {
                return status;
            }
        }

        if self.gltf_primitive_material_to_draco_material.len() > 1 {
            let material_index = primitive
                .material
                .as_ref()
                .map(|m| m.value() as i32)
                .unwrap_or(-1);
            if let Some(mapped) = self
                .gltf_primitive_material_to_draco_material
                .get(&material_index)
            {
                if primitive_mode == gltf::mesh::Mode::Triangles.as_gl_enum() {
                    draco_core::draco_return_if_error!(
                        self.add_material_data_to_builder_mesh(*mapped, number_of_faces,)
                    );
                } else {
                    draco_core::draco_return_if_error!(
                        self.add_material_data_to_builder_point(*mapped, number_of_points,)
                    );
                }
            }
        }

        self.next_face_id += number_of_faces;
        self.next_point_id += number_of_points;
        ok_status()
    }

    fn node_gather_attribute_and_material_stats(&mut self, node_index: i32) -> Status {
        let (mesh_index, children) = {
            let node = &self.gltf_model.json.nodes[node_index as usize];
            let mesh_index = node.mesh.as_ref().map(|m| m.value());
            let children: Vec<i32> = node
                .children
                .as_ref()
                .map(|c| c.iter().map(|child| child.value() as i32).collect())
                .unwrap_or_default();
            (mesh_index, children)
        };

        if let Some(mesh_index) = mesh_index {
            let primitives = self.gltf_model.json.meshes[mesh_index].primitives.clone();
            for primitive in primitives {
                draco_core::draco_return_if_error!(self.accumulate_primitive_stats(&primitive));
                let material_index = primitive
                    .material
                    .as_ref()
                    .map(|m| m.value() as i32)
                    .unwrap_or(-1);
                if !self
                    .gltf_primitive_material_to_draco_material
                    .contains_key(&material_index)
                {
                    let next_index = self.gltf_primitive_material_to_draco_material.len() as i32;
                    self.gltf_primitive_material_to_draco_material
                        .insert(material_index, next_index);
                }
            }
        }
        for child_index in children {
            draco_core::draco_return_if_error!(
                self.node_gather_attribute_and_material_stats(child_index)
            );
        }
        ok_status()
    }

    fn gather_attribute_and_material_stats(&mut self) -> Status {
        let node_indices: Vec<i32> = self
            .gltf_model
            .json
            .scenes
            .iter()
            .flat_map(|scene| scene.nodes.iter().map(|node| node.value() as i32))
            .collect();
        for node_index in node_indices {
            draco_core::draco_return_if_error!(
                self.node_gather_attribute_and_material_stats(node_index)
            );
        }
        ok_status()
    }

    fn sum_attribute_stats(&mut self, attribute_name: &str, count: i32) {
        if let Some(entry) = self.mesh_attribute_data.get_mut(attribute_name) {
            entry.total_attribute_counts += count;
        }
    }

    fn check_types(
        &mut self,
        attribute_name: &str,
        component_type: gltf::accessor::ComponentType,
        attribute_type: gltf::accessor::Type,
        normalized: bool,
    ) -> Status {
        if let Some(entry) = self.mesh_attribute_data.get(attribute_name) {
            if entry.component_type != component_type {
                return Status::new(
                    StatusCode::DracoError,
                    &format!(
                        "{} attribute component type does not match previous.",
                        attribute_name
                    ),
                );
            }
            if entry.attribute_type != attribute_type {
                return Status::new(
                    StatusCode::DracoError,
                    &format!("{} attribute type does not match previous.", attribute_name),
                );
            }
            if entry.normalized != normalized {
                return Status::new(
                    StatusCode::DracoError,
                    &format!(
                        "{} attribute normalized property does not match previous.",
                        attribute_name
                    ),
                );
            }
            return ok_status();
        }
        self.mesh_attribute_data.insert(
            attribute_name.to_string(),
            MeshAttributeData {
                component_type,
                attribute_type,
                normalized,
                total_attribute_counts: 0,
            },
        );
        ok_status()
    }

    fn accumulate_primitive_stats(&mut self, primitive: &gltf::mesh::Primitive) -> Status {
        let indices_count_or = self.decode_primitive_indices_count(primitive);
        if !indices_count_or.is_ok() {
            return Status::new(StatusCode::DracoError, "Invalid indices count.");
        }
        let indices_count = indices_count_or.into_value();
        let mode = mode_to_gl_enum(&primitive.mode);
        if mode == gltf::mesh::Mode::Triangles.as_gl_enum() {
            self.total_face_indices_count += indices_count;
        } else if mode == gltf::mesh::Mode::Points.as_gl_enum() {
            self.total_point_indices_count += indices_count;
        } else {
            return Status::new(
                StatusCode::DracoError,
                "Unsupported primitive indices mode.",
            );
        }

        for (semantic, accessor_index) in primitive.attributes.iter() {
            let attribute_name = semantic_to_string(semantic);
            if accessor_index.value() >= self.gltf_model.json.accessors.len() {
                return Status::new(StatusCode::DracoError, "Invalid accessor.");
            }
            let accessor = self.gltf_model.json.accessors[accessor_index.value()].clone();
            let component_type = match checked_component_type(&accessor) {
                Ok(component_type) => component_type,
                Err(status) => return status,
            };
            let attribute_type = match checked_accessor_type(&accessor) {
                Ok(attribute_type) => attribute_type,
                Err(status) => return status,
            };
            draco_core::draco_return_if_error!(self.check_types(
                &attribute_name,
                component_type,
                attribute_type,
                accessor.normalized,
            ));
            let count = i32::try_from(accessor.count.0)
                .map_err(|_| Status::new(StatusCode::DracoError, "Accessor count exceeds i32."));
            let count = match count {
                Ok(v) => v,
                Err(status) => return status,
            };
            self.sum_attribute_stats(&attribute_name, count);
        }
        ok_status()
    }

    fn add_attributes_to_draco_mesh_mesh(&mut self) -> Status {
        let mut entries: Vec<(String, MeshAttributeData)> = self
            .mesh_attribute_data
            .iter()
            .map(|(name, data)| (name.clone(), data.clone()))
            .collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        for (name, data) in entries {
            let draco_att_type = gltf_attribute_to_draco_attribute(&name);
            if draco_att_type == GeometryAttributeType::Invalid {
                self.attribute_name_to_draco_mesh_attribute_id
                    .insert(name, -1);
                continue;
            }
            let att_id = match self.add_attribute_by_type_mesh(draco_att_type, &data) {
                Ok(att_id) => att_id,
                Err(status) => return status,
            };
            self.attribute_name_to_draco_mesh_attribute_id
                .insert(name, att_id);
        }
        self.add_material_attribute_mesh();
        ok_status()
    }

    fn add_attributes_to_draco_mesh_point(&mut self) -> Status {
        let mut entries: Vec<(String, MeshAttributeData)> = self
            .mesh_attribute_data
            .iter()
            .map(|(name, data)| (name.clone(), data.clone()))
            .collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        for (name, data) in entries {
            let draco_att_type = gltf_attribute_to_draco_attribute(&name);
            if draco_att_type == GeometryAttributeType::Invalid {
                self.attribute_name_to_draco_mesh_attribute_id
                    .insert(name, -1);
                continue;
            }
            let att_id = match self.add_attribute_by_type_point(draco_att_type, &data) {
                Ok(att_id) => att_id,
                Err(status) => return status,
            };
            self.attribute_name_to_draco_mesh_attribute_id
                .insert(name, att_id);
        }
        self.add_material_attribute_point();
        ok_status()
    }

    fn add_attribute_by_type_mesh(
        &mut self,
        attribute_type: GeometryAttributeType,
        data: &MeshAttributeData,
    ) -> Result<i32, Status> {
        let num_components = TinyGltfUtils::get_num_components_for_type(data.attribute_type);
        if num_components == 0 {
            return Err(Status::new(
                StatusCode::DracoError,
                "Could not add attribute with 0 components.",
            ));
        }
        let draco_component_type = gltf_component_type_to_draco_type(data.component_type);
        if draco_component_type == DataType::Invalid {
            return Err(Status::new(
                StatusCode::DracoError,
                "Could not add attribute with invalid type.",
            ));
        }
        let att_id =
            self.mb
                .add_attribute(attribute_type, num_components as i8, draco_component_type);
        if att_id < 0 {
            return Err(Status::new(
                StatusCode::DracoError,
                "Could not add attribute.",
            ));
        }

        if self.scene.is_none() && attribute_type == GeometryAttributeType::Color {
            self.set_white_vertex_color_mesh(att_id, draco_component_type, num_components as usize);
        }
        Ok(att_id)
    }

    fn add_attribute_by_type_point(
        &mut self,
        attribute_type: GeometryAttributeType,
        data: &MeshAttributeData,
    ) -> Result<i32, Status> {
        let num_components = TinyGltfUtils::get_num_components_for_type(data.attribute_type);
        if num_components == 0 {
            return Err(Status::new(
                StatusCode::DracoError,
                "Could not add attribute with 0 components.",
            ));
        }
        let draco_component_type = gltf_component_type_to_draco_type(data.component_type);
        if draco_component_type == DataType::Invalid {
            return Err(Status::new(
                StatusCode::DracoError,
                "Could not add attribute with invalid type.",
            ));
        }
        let att_id =
            self.pb
                .add_attribute(attribute_type, num_components as i8, draco_component_type);
        if att_id < 0 {
            return Err(Status::new(
                StatusCode::DracoError,
                "Could not add attribute.",
            ));
        }

        if self.scene.is_none() && attribute_type == GeometryAttributeType::Color {
            self.set_white_vertex_color_point(
                att_id,
                draco_component_type,
                num_components as usize,
            );
        }
        Ok(att_id)
    }

    fn add_material_attribute_mesh(&mut self) {
        if self.gltf_model.json.materials.len() > 1 {
            let mut component_type = DataType::Uint32;
            if self.gltf_model.json.materials.len() < 256 {
                component_type = DataType::Uint8;
            } else if self.gltf_model.json.materials.len() < (1 << 16) {
                component_type = DataType::Uint16;
            }
            self.material_att_id =
                self.mb
                    .add_attribute(GeometryAttributeType::Material, 1, component_type);
        }
    }

    fn add_material_attribute_point(&mut self) {
        if self.gltf_model.json.materials.len() > 1 {
            let mut component_type = DataType::Uint32;
            if self.gltf_model.json.materials.len() < 256 {
                component_type = DataType::Uint8;
            } else if self.gltf_model.json.materials.len() < (1 << 16) {
                component_type = DataType::Uint16;
            }
            self.material_att_id =
                self.pb
                    .add_attribute(GeometryAttributeType::Material, 1, component_type);
        }
    }

    fn set_white_vertex_color_mesh(
        &mut self,
        color_att_id: i32,
        data_type: DataType,
        num_components: usize,
    ) {
        let num_faces = self.total_face_indices_count / 3;
        let bytes = white_color_bytes(data_type, num_components);
        for f in 0..num_faces {
            let face_index = FaceIndex::from((f + self.next_face_id) as u32);
            self.mb.set_attribute_values_for_face_bytes(
                color_att_id,
                face_index,
                &bytes,
                &bytes,
                &bytes,
            );
        }
    }

    fn set_white_vertex_color_point(
        &mut self,
        color_att_id: i32,
        data_type: DataType,
        num_components: usize,
    ) {
        let num_points = self.total_point_indices_count;
        let bytes = white_color_bytes(data_type, num_components);
        for p in 0..num_points {
            let point_index = PointIndex::from((p + self.next_point_id) as u32);
            self.pb
                .set_attribute_value_for_point(color_att_id, point_index, &bytes);
        }
    }

    fn add_attribute_values_from_accessor_mesh(
        &mut self,
        attribute_name: &str,
        accessor: &gltf::Accessor,
        indices_data: &[u32],
        att_id: i32,
        number_of_faces: i32,
        transform_matrix: &Matrix4d,
    ) -> Result<(), Status> {
        let reverse_winding = determinant(transform_matrix) < 0.0;
        if attribute_name == "TEXCOORD_0" || attribute_name == "TEXCOORD_1" {
            return self.add_tex_coord_to_builder_mesh(
                accessor,
                indices_data,
                att_id,
                number_of_faces,
                reverse_winding,
            );
        }
        if attribute_name == "TANGENT" {
            let matrix = update_matrix_for_normals(transform_matrix);
            return self.add_tangent_to_builder_mesh(
                accessor,
                indices_data,
                att_id,
                number_of_faces,
                &matrix,
                reverse_winding,
            );
        }
        if attribute_name == "POSITION" || attribute_name == "NORMAL" {
            let matrix = if attribute_name == "NORMAL" {
                update_matrix_for_normals(transform_matrix)
            } else {
                *transform_matrix
            };
            let normalize = attribute_name == "NORMAL";
            return self.add_transformed_data_to_builder_mesh(
                accessor,
                indices_data,
                att_id,
                number_of_faces,
                &matrix,
                normalize,
                reverse_winding,
            );
        }
        if attribute_name.starts_with("_FEATURE_ID_") {
            self.add_feature_id_to_builder_mesh(
                attribute_name,
                accessor,
                indices_data,
                att_id,
                number_of_faces,
                reverse_winding,
            )?;
            return Ok(());
        }
        if attribute_name.starts_with('_') {
            self.add_property_attribute_to_builder_mesh(
                attribute_name,
                accessor,
                indices_data,
                att_id,
                number_of_faces,
                reverse_winding,
            )?;
            return Ok(());
        }
        self.add_attribute_data_by_types_mesh(
            accessor,
            indices_data,
            att_id,
            number_of_faces,
            reverse_winding,
        )
    }

    fn add_attribute_values_from_accessor_point(
        &mut self,
        attribute_name: &str,
        accessor: &gltf::Accessor,
        indices_data: &[u32],
        att_id: i32,
        number_of_points: i32,
        transform_matrix: &Matrix4d,
    ) -> Result<(), Status> {
        let reverse_winding = determinant(transform_matrix) < 0.0;
        if attribute_name == "TEXCOORD_0" || attribute_name == "TEXCOORD_1" {
            return self.add_tex_coord_to_builder_point(
                accessor,
                indices_data,
                att_id,
                number_of_points,
                reverse_winding,
            );
        }
        if attribute_name == "TANGENT" {
            let matrix = update_matrix_for_normals(transform_matrix);
            return self.add_tangent_to_builder_point(
                accessor,
                indices_data,
                att_id,
                number_of_points,
                &matrix,
                reverse_winding,
            );
        }
        if attribute_name == "POSITION" || attribute_name == "NORMAL" {
            let matrix = if attribute_name == "NORMAL" {
                update_matrix_for_normals(transform_matrix)
            } else {
                *transform_matrix
            };
            let normalize = attribute_name == "NORMAL";
            return self.add_transformed_data_to_builder_point(
                accessor,
                indices_data,
                att_id,
                number_of_points,
                &matrix,
                normalize,
                reverse_winding,
            );
        }
        if attribute_name.starts_with("_FEATURE_ID_") {
            self.add_feature_id_to_builder_point(
                attribute_name,
                accessor,
                indices_data,
                att_id,
                number_of_points,
                reverse_winding,
            )?;
            return Ok(());
        }
        if attribute_name.starts_with('_') {
            self.add_property_attribute_to_builder_point(
                attribute_name,
                accessor,
                indices_data,
                att_id,
                number_of_points,
                reverse_winding,
            )?;
            return Ok(());
        }
        self.add_attribute_data_by_types_point(
            accessor,
            indices_data,
            att_id,
            number_of_points,
            reverse_winding,
        )
    }

    fn add_attribute_values_from_draco_mesh(
        &mut self,
        attribute_name: &str,
        accessor: &gltf::Accessor,
        indices_data: &[u32],
        att_id: i32,
        number_of_faces: i32,
        transform_matrix: &Matrix4d,
        mesh: &Mesh,
        attribute: &PointAttribute,
    ) -> Result<(), Status> {
        let reverse_winding = determinant(transform_matrix) < 0.0;
        if attribute_name == "TEXCOORD_0" || attribute_name == "TEXCOORD_1" {
            return self.add_tex_coord_from_draco_mesh(
                accessor,
                indices_data,
                att_id,
                number_of_faces,
                reverse_winding,
                mesh,
                attribute,
            );
        }
        if attribute_name == "TANGENT" {
            let matrix = update_matrix_for_normals(transform_matrix);
            return self.add_tangent_from_draco_mesh(
                accessor,
                indices_data,
                att_id,
                number_of_faces,
                &matrix,
                reverse_winding,
                mesh,
                attribute,
            );
        }
        if attribute_name == "POSITION" || attribute_name == "NORMAL" {
            let matrix = if attribute_name == "NORMAL" {
                update_matrix_for_normals(transform_matrix)
            } else {
                *transform_matrix
            };
            let normalize = attribute_name == "NORMAL";
            return self.add_transformed_from_draco_mesh(
                accessor,
                indices_data,
                att_id,
                number_of_faces,
                &matrix,
                normalize,
                reverse_winding,
                mesh,
                attribute,
            );
        }
        if attribute_name.starts_with("_FEATURE_ID_") {
            self.add_feature_id_from_draco_mesh(
                attribute_name,
                accessor,
                indices_data,
                att_id,
                number_of_faces,
                reverse_winding,
                mesh,
                attribute,
            )?;
            return Ok(());
        }
        if attribute_name.starts_with('_') {
            self.add_property_attribute_from_draco_mesh(
                attribute_name,
                accessor,
                indices_data,
                att_id,
                number_of_faces,
                reverse_winding,
                mesh,
                attribute,
            )?;
            return Ok(());
        }
        self.add_attribute_data_from_draco_mesh(
            accessor,
            indices_data,
            att_id,
            number_of_faces,
            reverse_winding,
            mesh,
            attribute,
        )
    }

    fn add_attribute_values_from_draco_point_cloud(
        &mut self,
        attribute_name: &str,
        accessor: &gltf::Accessor,
        indices_data: &[u32],
        att_id: i32,
        number_of_points: i32,
        transform_matrix: &Matrix4d,
        pc: &PointCloud,
        attribute: &PointAttribute,
    ) -> Result<(), Status> {
        let reverse_winding = determinant(transform_matrix) < 0.0;
        if attribute_name == "TEXCOORD_0" || attribute_name == "TEXCOORD_1" {
            return self.add_tex_coord_from_draco_point(
                accessor,
                indices_data,
                att_id,
                number_of_points,
                reverse_winding,
                pc,
                attribute,
            );
        }
        if attribute_name == "TANGENT" {
            let matrix = update_matrix_for_normals(transform_matrix);
            return self.add_tangent_from_draco_point(
                accessor,
                indices_data,
                att_id,
                number_of_points,
                &matrix,
                reverse_winding,
                pc,
                attribute,
            );
        }
        if attribute_name == "POSITION" || attribute_name == "NORMAL" {
            let matrix = if attribute_name == "NORMAL" {
                update_matrix_for_normals(transform_matrix)
            } else {
                *transform_matrix
            };
            let normalize = attribute_name == "NORMAL";
            return self.add_transformed_from_draco_point(
                accessor,
                indices_data,
                att_id,
                number_of_points,
                &matrix,
                normalize,
                reverse_winding,
                pc,
                attribute,
            );
        }
        if attribute_name.starts_with("_FEATURE_ID_") {
            self.add_feature_id_from_draco_point(
                attribute_name,
                accessor,
                indices_data,
                att_id,
                number_of_points,
                reverse_winding,
                pc,
                attribute,
            )?;
            return Ok(());
        }
        if attribute_name.starts_with('_') {
            self.add_property_attribute_from_draco_point(
                attribute_name,
                accessor,
                indices_data,
                att_id,
                number_of_points,
                reverse_winding,
                pc,
                attribute,
            )?;
            return Ok(());
        }
        self.add_attribute_data_from_draco_point(
            accessor,
            indices_data,
            att_id,
            number_of_points,
            reverse_winding,
            pc,
            attribute,
        )
    }

    fn add_tex_coord_from_draco_mesh(
        &mut self,
        accessor: &gltf::Accessor,
        indices_data: &[u32],
        att_id: i32,
        number_of_faces: i32,
        reverse_winding: bool,
        mesh: &Mesh,
        attribute: &PointAttribute,
    ) -> Result<(), Status> {
        check_accessor_float_2(accessor, attribute)?;
        let mut data = attribute_vec2_float(mesh.num_points() as usize, attribute)?;
        for uv in &mut data {
            uv[1] = 1.0 - uv[1];
        }
        set_values_for_mesh(
            &mut self.mb,
            indices_data,
            att_id,
            number_of_faces,
            &data,
            reverse_winding,
            self.next_face_id,
        );
        Ok(())
    }

    fn add_tex_coord_from_draco_point(
        &mut self,
        accessor: &gltf::Accessor,
        indices_data: &[u32],
        att_id: i32,
        number_of_points: i32,
        reverse_winding: bool,
        pc: &PointCloud,
        attribute: &PointAttribute,
    ) -> Result<(), Status> {
        check_accessor_float_2(accessor, attribute)?;
        let mut data = attribute_vec2_float(pc.num_points() as usize, attribute)?;
        for uv in &mut data {
            uv[1] = 1.0 - uv[1];
        }
        let _ = reverse_winding;
        set_values_for_point(
            &mut self.pb,
            indices_data,
            att_id,
            number_of_points,
            &data,
            self.next_point_id,
        );
        Ok(())
    }

    fn add_tangent_from_draco_mesh(
        &mut self,
        accessor: &gltf::Accessor,
        indices_data: &[u32],
        att_id: i32,
        number_of_faces: i32,
        transform_matrix: &Matrix4d,
        reverse_winding: bool,
        mesh: &Mesh,
        attribute: &PointAttribute,
    ) -> Result<(), Status> {
        check_accessor_float_4(accessor, attribute)?;
        let data = attribute_vec4_float(mesh.num_points() as usize, attribute)?;
        let mut transformed = Vec::with_capacity(data.len());
        for v in data {
            let mut vec4 = transform_matrix.mul_vec4([v[0] as f64, v[1] as f64, v[2] as f64, 1.0]);
            let mut vec3 = [vec4[0], vec4[1], vec4[2]];
            normalize_vec3(&mut vec3);
            vec4[0] = vec3[0];
            vec4[1] = vec3[1];
            vec4[2] = vec3[2];
            transformed.push([vec4[0] as f32, vec4[1] as f32, vec4[2] as f32, v[3]]);
        }
        set_values_for_mesh(
            &mut self.mb,
            indices_data,
            att_id,
            number_of_faces,
            &transformed,
            reverse_winding,
            self.next_face_id,
        );
        Ok(())
    }

    fn add_tangent_from_draco_point(
        &mut self,
        accessor: &gltf::Accessor,
        indices_data: &[u32],
        att_id: i32,
        number_of_points: i32,
        transform_matrix: &Matrix4d,
        reverse_winding: bool,
        pc: &PointCloud,
        attribute: &PointAttribute,
    ) -> Result<(), Status> {
        check_accessor_float_4(accessor, attribute)?;
        let data = attribute_vec4_float(pc.num_points() as usize, attribute)?;
        let mut transformed = Vec::with_capacity(data.len());
        for v in data {
            let mut vec4 = transform_matrix.mul_vec4([v[0] as f64, v[1] as f64, v[2] as f64, 1.0]);
            let mut vec3 = [vec4[0], vec4[1], vec4[2]];
            normalize_vec3(&mut vec3);
            vec4[0] = vec3[0];
            vec4[1] = vec3[1];
            vec4[2] = vec3[2];
            transformed.push([vec4[0] as f32, vec4[1] as f32, vec4[2] as f32, v[3]]);
        }
        let _ = reverse_winding;
        set_values_for_point(
            &mut self.pb,
            indices_data,
            att_id,
            number_of_points,
            &transformed,
            self.next_point_id,
        );
        Ok(())
    }

    fn add_transformed_from_draco_mesh(
        &mut self,
        accessor: &gltf::Accessor,
        indices_data: &[u32],
        att_id: i32,
        number_of_faces: i32,
        transform_matrix: &Matrix4d,
        normalize: bool,
        reverse_winding: bool,
        mesh: &Mesh,
        attribute: &PointAttribute,
    ) -> Result<(), Status> {
        check_accessor_float_3(accessor, attribute)?;
        let data = attribute_vec3_float(mesh.num_points() as usize, attribute)?;
        let mut transformed = Vec::with_capacity(data.len());
        for v in data {
            let vec4 = transform_matrix.mul_vec4([v[0] as f64, v[1] as f64, v[2] as f64, 1.0]);
            let mut vec3 = [vec4[0], vec4[1], vec4[2]];
            if normalize {
                normalize_vec3(&mut vec3);
            }
            transformed.push([vec3[0] as f32, vec3[1] as f32, vec3[2] as f32]);
        }
        set_values_for_mesh(
            &mut self.mb,
            indices_data,
            att_id,
            number_of_faces,
            &transformed,
            reverse_winding,
            self.next_face_id,
        );
        Ok(())
    }

    fn add_transformed_from_draco_point(
        &mut self,
        accessor: &gltf::Accessor,
        indices_data: &[u32],
        att_id: i32,
        number_of_points: i32,
        transform_matrix: &Matrix4d,
        normalize: bool,
        reverse_winding: bool,
        pc: &PointCloud,
        attribute: &PointAttribute,
    ) -> Result<(), Status> {
        check_accessor_float_3(accessor, attribute)?;
        let data = attribute_vec3_float(pc.num_points() as usize, attribute)?;
        let mut transformed = Vec::with_capacity(data.len());
        for v in data {
            let vec4 = transform_matrix.mul_vec4([v[0] as f64, v[1] as f64, v[2] as f64, 1.0]);
            let mut vec3 = [vec4[0], vec4[1], vec4[2]];
            if normalize {
                normalize_vec3(&mut vec3);
            }
            transformed.push([vec3[0] as f32, vec3[1] as f32, vec3[2] as f32]);
        }
        let _ = reverse_winding;
        set_values_for_point(
            &mut self.pb,
            indices_data,
            att_id,
            number_of_points,
            &transformed,
            self.next_point_id,
        );
        Ok(())
    }

    fn add_feature_id_from_draco_mesh(
        &mut self,
        attribute_name: &str,
        accessor: &gltf::Accessor,
        indices_data: &[u32],
        att_id: i32,
        number_of_faces: i32,
        reverse_winding: bool,
        mesh: &Mesh,
        attribute: &PointAttribute,
    ) -> Result<(), Status> {
        check_feature_id_accessor_basic(accessor)?;
        let index = get_index_from_feature_id_attribute_name(attribute_name)?;
        self.feature_id_attribute_indices.insert(index, att_id);
        self.add_attribute_data_from_draco_mesh(
            accessor,
            indices_data,
            att_id,
            number_of_faces,
            reverse_winding,
            mesh,
            attribute,
        )
    }

    fn add_feature_id_from_draco_point(
        &mut self,
        attribute_name: &str,
        accessor: &gltf::Accessor,
        indices_data: &[u32],
        att_id: i32,
        number_of_points: i32,
        reverse_winding: bool,
        pc: &PointCloud,
        attribute: &PointAttribute,
    ) -> Result<(), Status> {
        check_feature_id_accessor_basic(accessor)?;
        let index = get_index_from_feature_id_attribute_name(attribute_name)?;
        self.feature_id_attribute_indices.insert(index, att_id);
        self.add_attribute_data_from_draco_point(
            accessor,
            indices_data,
            att_id,
            number_of_points,
            reverse_winding,
            pc,
            attribute,
        )
    }

    fn add_property_attribute_from_draco_mesh(
        &mut self,
        attribute_name: &str,
        accessor: &gltf::Accessor,
        indices_data: &[u32],
        att_id: i32,
        number_of_faces: i32,
        reverse_winding: bool,
        mesh: &Mesh,
        attribute: &PointAttribute,
    ) -> Result<(), Status> {
        self.add_attribute_data_from_draco_mesh(
            accessor,
            indices_data,
            att_id,
            number_of_faces,
            reverse_winding,
            mesh,
            attribute,
        )?;
        self.mb.set_attribute_name(att_id, attribute_name);
        Ok(())
    }

    fn add_property_attribute_from_draco_point(
        &mut self,
        attribute_name: &str,
        accessor: &gltf::Accessor,
        indices_data: &[u32],
        att_id: i32,
        number_of_points: i32,
        reverse_winding: bool,
        pc: &PointCloud,
        attribute: &PointAttribute,
    ) -> Result<(), Status> {
        self.add_attribute_data_from_draco_point(
            accessor,
            indices_data,
            att_id,
            number_of_points,
            reverse_winding,
            pc,
            attribute,
        )?;
        self.pb.set_attribute_name(att_id, attribute_name);
        Ok(())
    }

    fn add_attribute_data_from_draco_mesh(
        &mut self,
        accessor: &gltf::Accessor,
        indices_data: &[u32],
        att_id: i32,
        number_of_faces: i32,
        reverse_winding: bool,
        mesh: &Mesh,
        attribute: &PointAttribute,
    ) -> Result<(), Status> {
        let _ = accessor;
        let raw = attribute_raw_bytes(mesh.num_points() as usize, attribute);
        set_values_for_mesh_bytes(
            &mut self.mb,
            indices_data,
            att_id,
            number_of_faces,
            &raw,
            reverse_winding,
            self.next_face_id,
        );
        Ok(())
    }

    fn add_attribute_data_from_draco_point(
        &mut self,
        accessor: &gltf::Accessor,
        indices_data: &[u32],
        att_id: i32,
        number_of_points: i32,
        reverse_winding: bool,
        pc: &PointCloud,
        attribute: &PointAttribute,
    ) -> Result<(), Status> {
        let _ = accessor;
        let _ = reverse_winding;
        let raw = attribute_raw_bytes(pc.num_points() as usize, attribute);
        set_values_for_point_bytes(
            &mut self.pb,
            indices_data,
            att_id,
            number_of_points,
            &raw,
            self.next_point_id,
        );
        Ok(())
    }

    fn add_tangent_to_builder_mesh(
        &mut self,
        accessor: &gltf::Accessor,
        indices_data: &[u32],
        att_id: i32,
        number_of_faces: i32,
        transform_matrix: &Matrix4d,
        reverse_winding: bool,
    ) -> Result<(), Status> {
        let data = copy_data_as_float_checked::<[f32; 4]>(&self.gltf_model, accessor)?;
        let mut transformed = Vec::with_capacity(data.len());
        for v in data {
            let mut vec4 = transform_matrix.mul_vec4([v[0] as f64, v[1] as f64, v[2] as f64, 1.0]);
            let mut vec3 = [vec4[0], vec4[1], vec4[2]];
            normalize_vec3(&mut vec3);
            vec4[0] = vec3[0];
            vec4[1] = vec3[1];
            vec4[2] = vec3[2];
            transformed.push([vec4[0] as f32, vec4[1] as f32, vec4[2] as f32, v[3]]);
        }
        set_values_for_mesh(
            &mut self.mb,
            indices_data,
            att_id,
            number_of_faces,
            &transformed,
            reverse_winding,
            self.next_face_id,
        );
        Ok(())
    }

    fn add_tangent_to_builder_point(
        &mut self,
        accessor: &gltf::Accessor,
        indices_data: &[u32],
        att_id: i32,
        number_of_points: i32,
        transform_matrix: &Matrix4d,
        _reverse_winding: bool,
    ) -> Result<(), Status> {
        let data = copy_data_as_float_checked::<[f32; 4]>(&self.gltf_model, accessor)?;
        let mut transformed = Vec::with_capacity(data.len());
        for v in data {
            let mut vec4 = transform_matrix.mul_vec4([v[0] as f64, v[1] as f64, v[2] as f64, 1.0]);
            let mut vec3 = [vec4[0], vec4[1], vec4[2]];
            normalize_vec3(&mut vec3);
            vec4[0] = vec3[0];
            vec4[1] = vec3[1];
            vec4[2] = vec3[2];
            transformed.push([vec4[0] as f32, vec4[1] as f32, vec4[2] as f32, v[3]]);
        }
        set_values_for_point(
            &mut self.pb,
            indices_data,
            att_id,
            number_of_points,
            &transformed,
            self.next_point_id,
        );
        Ok(())
    }

    fn add_tex_coord_to_builder_mesh(
        &mut self,
        accessor: &gltf::Accessor,
        indices_data: &[u32],
        att_id: i32,
        number_of_faces: i32,
        reverse_winding: bool,
    ) -> Result<(), Status> {
        let mut data = copy_data_as_float_checked::<[f32; 2]>(&self.gltf_model, accessor)?;
        for uv in &mut data {
            uv[1] = 1.0 - uv[1];
        }
        set_values_for_mesh(
            &mut self.mb,
            indices_data,
            att_id,
            number_of_faces,
            &data,
            reverse_winding,
            self.next_face_id,
        );
        Ok(())
    }

    fn add_tex_coord_to_builder_point(
        &mut self,
        accessor: &gltf::Accessor,
        indices_data: &[u32],
        att_id: i32,
        number_of_points: i32,
        _reverse_winding: bool,
    ) -> Result<(), Status> {
        let mut data = copy_data_as_float_checked::<[f32; 2]>(&self.gltf_model, accessor)?;
        for uv in &mut data {
            uv[1] = 1.0 - uv[1];
        }
        set_values_for_point(
            &mut self.pb,
            indices_data,
            att_id,
            number_of_points,
            &data,
            self.next_point_id,
        );
        Ok(())
    }

    fn add_feature_id_to_builder_mesh(
        &mut self,
        attribute_name: &str,
        accessor: &gltf::Accessor,
        indices_data: &[u32],
        att_id: i32,
        number_of_faces: i32,
        reverse_winding: bool,
    ) -> Result<(), Status> {
        self.check_feature_id_accessor(accessor)?;
        self.add_attribute_data_by_types_mesh(
            accessor,
            indices_data,
            att_id,
            number_of_faces,
            reverse_winding,
        )?;
        let index = get_index_from_feature_id_attribute_name(attribute_name)?;
        self.feature_id_attribute_indices.insert(index, att_id);
        Ok(())
    }

    fn add_feature_id_to_builder_point(
        &mut self,
        attribute_name: &str,
        accessor: &gltf::Accessor,
        indices_data: &[u32],
        att_id: i32,
        number_of_points: i32,
        reverse_winding: bool,
    ) -> Result<(), Status> {
        self.check_feature_id_accessor(accessor)?;
        self.add_attribute_data_by_types_point(
            accessor,
            indices_data,
            att_id,
            number_of_points,
            reverse_winding,
        )?;
        let index = get_index_from_feature_id_attribute_name(attribute_name)?;
        self.feature_id_attribute_indices.insert(index, att_id);
        Ok(())
    }

    fn add_property_attribute_to_builder_mesh(
        &mut self,
        attribute_name: &str,
        accessor: &gltf::Accessor,
        indices_data: &[u32],
        att_id: i32,
        number_of_faces: i32,
        reverse_winding: bool,
    ) -> Result<(), Status> {
        self.add_attribute_data_by_types_mesh(
            accessor,
            indices_data,
            att_id,
            number_of_faces,
            reverse_winding,
        )?;
        self.mb.set_attribute_name(att_id, attribute_name);
        Ok(())
    }

    fn add_property_attribute_to_builder_point(
        &mut self,
        attribute_name: &str,
        accessor: &gltf::Accessor,
        indices_data: &[u32],
        att_id: i32,
        number_of_points: i32,
        reverse_winding: bool,
    ) -> Result<(), Status> {
        self.add_attribute_data_by_types_point(
            accessor,
            indices_data,
            att_id,
            number_of_points,
            reverse_winding,
        )?;
        self.pb.set_attribute_name(att_id, attribute_name);
        Ok(())
    }

    fn add_transformed_data_to_builder_mesh(
        &mut self,
        accessor: &gltf::Accessor,
        indices_data: &[u32],
        att_id: i32,
        number_of_faces: i32,
        transform_matrix: &Matrix4d,
        normalize: bool,
        reverse_winding: bool,
    ) -> Result<(), Status> {
        let data = copy_data_as_float_checked::<[f32; 3]>(&self.gltf_model, accessor)?;
        let mut transformed = Vec::with_capacity(data.len());
        for v in data {
            let vec4 = transform_matrix.mul_vec4([v[0] as f64, v[1] as f64, v[2] as f64, 1.0]);
            let mut vec3 = [vec4[0], vec4[1], vec4[2]];
            if normalize {
                normalize_vec3(&mut vec3);
            }
            transformed.push([vec3[0] as f32, vec3[1] as f32, vec3[2] as f32]);
        }
        set_values_for_mesh(
            &mut self.mb,
            indices_data,
            att_id,
            number_of_faces,
            &transformed,
            reverse_winding,
            self.next_face_id,
        );
        Ok(())
    }

    fn add_transformed_data_to_builder_point(
        &mut self,
        accessor: &gltf::Accessor,
        indices_data: &[u32],
        att_id: i32,
        number_of_points: i32,
        transform_matrix: &Matrix4d,
        normalize: bool,
        _reverse_winding: bool,
    ) -> Result<(), Status> {
        let data = copy_data_as_float_checked::<[f32; 3]>(&self.gltf_model, accessor)?;
        let mut transformed = Vec::with_capacity(data.len());
        for v in data {
            let vec4 = transform_matrix.mul_vec4([v[0] as f64, v[1] as f64, v[2] as f64, 1.0]);
            let mut vec3 = [vec4[0], vec4[1], vec4[2]];
            if normalize {
                normalize_vec3(&mut vec3);
            }
            transformed.push([vec3[0] as f32, vec3[1] as f32, vec3[2] as f32]);
        }
        set_values_for_point(
            &mut self.pb,
            indices_data,
            att_id,
            number_of_points,
            &transformed,
            self.next_point_id,
        );
        Ok(())
    }

    fn add_attribute_data_by_types_mesh(
        &mut self,
        accessor: &gltf::Accessor,
        indices_data: &[u32],
        att_id: i32,
        number_of_faces: i32,
        reverse_winding: bool,
    ) -> Result<(), Status> {
        let raw = accessor_raw_bytes(&self.gltf_model, accessor)?;
        set_values_for_mesh_bytes(
            &mut self.mb,
            indices_data,
            att_id,
            number_of_faces,
            &raw,
            reverse_winding,
            self.next_face_id,
        );
        Ok(())
    }

    fn add_attribute_data_by_types_point(
        &mut self,
        accessor: &gltf::Accessor,
        indices_data: &[u32],
        att_id: i32,
        number_of_points: i32,
        _reverse_winding: bool,
    ) -> Result<(), Status> {
        let raw = accessor_raw_bytes(&self.gltf_model, accessor)?;
        set_values_for_point_bytes(
            &mut self.pb,
            indices_data,
            att_id,
            number_of_points,
            &raw,
            self.next_point_id,
        );
        Ok(())
    }

    fn check_feature_id_accessor(&self, accessor: &gltf::Accessor) -> Result<(), Status> {
        let num_components =
            TinyGltfUtils::get_num_components_for_type(checked_accessor_type(accessor)?);
        if num_components != 1 {
            return Err(Status::new(
                StatusCode::DracoError,
                "Invalid feature ID attribute type.",
            ));
        }
        let draco_component_type =
            gltf_component_type_to_draco_type(checked_component_type(accessor)?);
        if draco_component_type != DataType::Uint8
            && draco_component_type != DataType::Uint16
            && draco_component_type != DataType::Float32
        {
            return Err(Status::new(
                StatusCode::DracoError,
                "Invalid feature ID attribute component type.",
            ));
        }
        Ok(())
    }

    fn add_material_data_to_builder_mesh(
        &mut self,
        material_value: i32,
        number_of_faces: i32,
    ) -> Status {
        if self.gltf_primitive_material_to_draco_material.len() < 256 {
            let value = material_value as u8;
            return self.add_material_data_mesh_typed(value, number_of_faces);
        } else if self.gltf_primitive_material_to_draco_material.len() < (1 << 16) {
            let value = material_value as u16;
            return self.add_material_data_mesh_typed(value, number_of_faces);
        }
        let value = material_value as u32;
        self.add_material_data_mesh_typed(value, number_of_faces)
    }

    fn add_material_data_mesh_typed<T: Copy>(
        &mut self,
        material_value: T,
        number_of_faces: i32,
    ) -> Status {
        for f in 0..number_of_faces {
            let face_index = FaceIndex::from((f + self.next_face_id) as u32);
            let value = [material_value];
            self.mb
                .set_per_face_attribute_value_for_face(self.material_att_id, face_index, &value);
        }
        ok_status()
    }

    fn add_material_data_to_builder_point(
        &mut self,
        material_value: i32,
        number_of_points: i32,
    ) -> Status {
        if self.gltf_primitive_material_to_draco_material.len() < 256 {
            let value = material_value as u8;
            return self.add_material_data_point_typed(value, number_of_points);
        } else if self.gltf_primitive_material_to_draco_material.len() < (1 << 16) {
            let value = material_value as u16;
            return self.add_material_data_point_typed(value, number_of_points);
        }
        let value = material_value as u32;
        self.add_material_data_point_typed(value, number_of_points)
    }

    fn add_material_data_point_typed<T: Copy + MaterialValueBytes>(
        &mut self,
        material_value: T,
        number_of_points: i32,
    ) -> Status {
        let mut buf = [0u8; 4];
        let size = std::mem::size_of::<T>();
        material_value.write_bytes(&mut buf[..size]);
        for p in 0..number_of_points {
            let point_index = PointIndex::from((p + self.next_point_id) as u32);
            self.pb
                .set_attribute_value_for_point(self.material_att_id, point_index, &buf[..size]);
        }
        ok_status()
    }

    fn copy_textures_to_mesh(&mut self, mesh: &mut Mesh) -> Status {
        let library = mesh.material_library_mut();
        self.copy_textures_to_material_library(library)
    }

    fn copy_textures_to_scene(&mut self, scene: &mut Scene) -> Status {
        let library = scene.material_library_mut();
        self.copy_textures_to_material_library(library)
    }

    fn copy_textures_to_material_library(&mut self, library: &mut MaterialLibrary) -> Status {
        let images = self.gltf_model.json.images.clone();
        for (index, image) in images.iter().enumerate() {
            let mut draco_texture = Box::new(Texture::new());
            let mut source_image = match self.get_source_image(image) {
                Ok(image) => image,
                Err(status) => return status,
            };
            if source_image.encoded_data().is_empty() && !source_image.filename().is_empty() {
                if !self.input_file_name.is_empty() {
                    let mut folder = String::new();
                    let mut basename = String::new();
                    file_utils::split_path(&self.input_file_name, &mut folder, &mut basename);
                    let mut new_path = folder;
                    new_path.push_str("/");
                    new_path.push_str(source_image.filename());
                    source_image.set_filename(&new_path);
                    self.push_input_file(&new_path);
                }
            }
            draco_texture.set_source_image(&source_image);
            let tex_index = library.texture_library_mut().push_texture(draco_texture);
            self.gltf_image_to_draco_texture
                .insert(index as i32, tex_index);
        }
        ok_status()
    }

    fn get_source_image(&mut self, image: &gltf::Image) -> Result<SourceImage, Status> {
        let mut source_image = SourceImage::new();
        if let Some(buffer_view) = image.buffer_view.as_ref() {
            let mut data = Vec::new();
            copy_data_from_buffer_view(&self.gltf_model, buffer_view.value() as i32, &mut data)?;
            source_image.encoded_data_mut().extend_from_slice(&data);
        }
        if let Some(uri) = image.uri.as_ref() {
            if is_data_uri(uri) {
                let (decoded, mime) = decode_data_uri(uri)?;
                source_image.encoded_data_mut().extend_from_slice(&decoded);
                if source_image.mime_type().is_empty() {
                    if let Some(mime) = mime {
                        source_image.set_mime_type(&mime);
                    }
                }
            } else {
                source_image.set_filename(uri);
            }
        }
        if source_image.mime_type().is_empty() {
            if let Some(mime) = image.mime_type.as_ref() {
                source_image.set_mime_type(&mime.0);
            }
        }
        Ok(source_image)
    }

    fn set_attribute_properties_on_draco_mesh(&self, mesh: &mut Mesh) {
        for (name, data) in &self.mesh_attribute_data {
            let att_id = *self
                .attribute_name_to_draco_mesh_attribute_id
                .get(name)
                .unwrap_or(&-1);
            if att_id == -1 {
                continue;
            }
            if data.normalized {
                if let Some(att) = mesh.attribute_mut(att_id) {
                    att.set_normalized(true);
                }
            }
        }
    }

    fn add_materials_to_draco_mesh(&mut self, mesh: &mut Mesh) -> Status {
        let mut default_material_index = -1;
        if let Some(index) = self.gltf_primitive_material_to_draco_material.get(&-1) {
            default_material_index = *index;
        }

        for input_material_index in 0..self.gltf_model.json.materials.len() {
            let input_index = input_material_index as i32;
            let output_index = match self
                .gltf_primitive_material_to_draco_material
                .get(&input_index)
            {
                Some(index) => *index,
                None => continue,
            };

            if default_material_index == input_index {
                let _ = mesh.material_library_mut().mutable_material(output_index);
            }

            let output_material = mesh
                .material_library_mut()
                .mutable_material(output_index)
                .expect("material");
            draco_core::draco_return_if_error!(self.add_gltf_material(input_index, output_material));
        }
        ok_status()
    }

    fn check_and_add_texture_to_draco_material(
        &self,
        texture_index: i32,
        tex_coord_attribute_index: i32,
        tex_info_ext: Option<&gltf::extensions::texture::Info>,
        material: &mut Material,
        map_type: TextureMapType,
    ) -> Status {
        if texture_index < 0 {
            return ok_status();
        }
        let input_texture = &self.gltf_model.json.textures[texture_index as usize];
        let source_index = input_texture.source.value() as i32;

        if let Some(&texture_index) = self.gltf_image_to_draco_texture.get(&source_index) {
            let mut wrapping_mode = TextureMapWrappingMode::new(TextureMapAxisWrappingMode::Repeat);
            let mut min_filter = TextureMapFilterType::Unspecified;
            let mut mag_filter = TextureMapFilterType::Unspecified;

            if let Some(sampler_index) = input_texture.sampler.as_ref() {
                let sampler = &self.gltf_model.json.samplers[sampler_index.value()];
                wrapping_mode.s = match tiny_gltf_to_draco_axis_wrapping_mode(&sampler.wrap_s) {
                    Ok(mode) => mode,
                    Err(status) => return status,
                };
                wrapping_mode.t = match tiny_gltf_to_draco_axis_wrapping_mode(&sampler.wrap_t) {
                    Ok(mode) => mode,
                    Err(status) => return status,
                };
                min_filter = match tiny_gltf_to_draco_min_filter_type(sampler.min_filter.as_ref()) {
                    Ok(filter) => filter,
                    Err(status) => return status,
                };
                mag_filter = match tiny_gltf_to_draco_mag_filter_type(sampler.mag_filter.as_ref()) {
                    Ok(filter) => filter,
                    Err(status) => return status,
                };
            }

            if tex_coord_attribute_index < 0 || tex_coord_attribute_index > 1 {
                return Status::new(StatusCode::DracoError, "Incompatible tex coord index.");
            }

            let mut transform = TextureTransform::new();
            let has_transform = match check_khr_texture_transform(tex_info_ext, &mut transform) {
                Ok(has) => has,
                Err(status) => return status,
            };
            if has_transform {
                return material.set_texture_map_existing_by_index_with_transform(
                    texture_index,
                    map_type,
                    wrapping_mode,
                    min_filter,
                    mag_filter,
                    &transform,
                    tex_coord_attribute_index,
                );
            }
            return material.set_texture_map_existing_by_index_with_filters(
                texture_index,
                map_type,
                wrapping_mode,
                min_filter,
                mag_filter,
                tex_coord_attribute_index,
            );
        }
        ok_status()
    }

    fn decode_gltf_to_scene(&mut self) -> Status {
        draco_core::draco_return_if_error!(self.gather_attribute_and_material_stats());
        let mut scene = match self.scene.take() {
            Some(s) => s,
            None => return ok_status(),
        };
        let status = self.decode_gltf_to_scene_inner(&mut scene);
        self.scene = Some(scene);
        status
    }

    fn decode_gltf_to_scene_inner(&mut self, scene: &mut Scene) -> Status {
        draco_core::draco_return_if_error!(self.add_lights_to_scene(scene));
        draco_core::draco_return_if_error!(self.add_materials_variants_names_to_scene(scene));
        draco_core::draco_return_if_error!(self.add_structural_metadata_to_scene(scene));
        draco_core::draco_return_if_error!(self.copy_textures_to_scene(scene));

        let root_nodes: Vec<i32> = self
            .gltf_model
            .json
            .scenes
            .iter()
            .flat_map(|s| s.nodes.iter().map(|node| node.value() as i32))
            .collect();
        for node_index in root_nodes {
            draco_core::draco_return_if_error!(self.decode_node_for_scene(
                node_index,
                INVALID_SCENE_NODE_INDEX,
                scene
            ));
            let mapped = self
                .gltf_node_to_scenenode_index
                .get(&node_index)
                .copied()
                .unwrap_or(INVALID_SCENE_NODE_INDEX);
            if mapped != INVALID_SCENE_NODE_INDEX {
                scene.add_root_node_index(mapped);
            }
        }

        draco_core::draco_return_if_error!(self.add_animations_to_scene(scene));
        draco_core::draco_return_if_error!(self.add_materials_to_scene(scene));
        draco_core::draco_return_if_error!(self.add_skins_to_scene(scene));
        self.move_non_material_textures_scene(scene);
        draco_core::draco_return_if_error!(self.add_asset_metadata_to_scene(scene));
        if let Err(status) = self.decode_cesium_rtc(scene) {
            return status;
        }
        ok_status()
    }

    fn add_lights_to_scene(&self, scene: &mut Scene) -> Status {
        let extensions = match self.gltf_model.json.extensions.as_ref() {
            Some(ext) => ext,
            None => return ok_status(),
        };
        let lights = match extensions.khr_lights_punctual.as_ref() {
            Some(lights) => &lights.lights,
            None => return ok_status(),
        };
        for light in lights {
            let light_index = scene.add_light();
            let scene_light = scene.light_mut(light_index);

            let light_type = match light.type_ {
                Checked::Valid(gltf::extensions::scene::khr_lights_punctual::Type::Directional) => {
                    LightType::Directional
                }
                Checked::Valid(gltf::extensions::scene::khr_lights_punctual::Type::Point) => {
                    LightType::Point
                }
                Checked::Valid(gltf::extensions::scene::khr_lights_punctual::Type::Spot) => {
                    LightType::Spot
                }
                Checked::Invalid => {
                    return Status::new(StatusCode::DracoError, "Light type is invalid.");
                }
            };
            scene_light.set_type(light_type);

            if light_type == LightType::Spot {
                if let Some(spot) = light.spot.as_ref() {
                    let inner = round_f32_to_f64(spot.inner_cone_angle);
                    let mut outer = round_f32_to_f64(spot.outer_cone_angle);
                    let default_outer = DRACO_PI / 4.0;
                    if (outer - default_outer).abs() < 1e-6 {
                        outer = default_outer;
                    }
                    scene_light.set_inner_cone_angle(inner);
                    scene_light.set_outer_cone_angle(outer);
                }
            }

            if let Some(name) = light.name.as_ref() {
                scene_light.set_name(name);
            }
            let color = light.color;
            if color.len() == 3 {
                scene_light.set_color(Vector3f::new3(color[0], color[1], color[2]));
            }
            scene_light.set_intensity(light.intensity as f64);
            if let Some(range) = light.range {
                if range < 0.0 {
                    return Status::new(StatusCode::DracoError, "Light range must be positive.");
                }
                scene_light.set_range(range as f64);
            }
        }
        ok_status()
    }

    fn add_materials_variants_names_to_scene(&self, scene: &mut Scene) -> Status {
        let extensions = match self.gltf_model.json.extensions.as_ref() {
            Some(ext) => ext,
            None => return ok_status(),
        };
        let variants = match extensions.khr_materials_variants.as_ref() {
            Some(ext) => &ext.variants,
            None => return ok_status(),
        };
        for variant in variants {
            scene
                .material_library_mut()
                .add_materials_variant(&variant.name);
        }
        ok_status()
    }

    fn add_structural_metadata_to_mesh(&mut self, mesh: &mut Mesh) -> Status {
        self.add_structural_metadata_to_geometry(mesh.structural_metadata_mut())
    }

    fn add_structural_metadata_to_scene(&mut self, scene: &mut Scene) -> Status {
        self.add_structural_metadata_to_geometry(scene.structural_metadata_mut())
    }

    fn add_structural_metadata_to_geometry(&mut self, metadata: &mut StructuralMetadata) -> Status {
        let extensions = match self.gltf_model.json.extensions.as_ref() {
            Some(ext) => ext,
            None => return ok_status(),
        };
        let ext_value = match extensions.others.get(EXT_STRUCTURAL_METADATA) {
            Some(value) => value,
            None => return ok_status(),
        };
        let object = match ext_value.as_object() {
            Some(obj) => obj,
            None => {
                return Status::new(
                    StatusCode::DracoError,
                    "Structural metadata extension is malformed.",
                )
            }
        };

        if let Err(status) = self.add_structural_metadata_schema_to_geometry(object, metadata) {
            return status;
        }
        if let Err(status) = self.add_property_tables_to_geometry(object, metadata) {
            return status;
        }
        if let Err(status) = self.add_property_attributes_to_geometry(object, metadata) {
            return status;
        }

        if metadata.num_property_tables() == 0 && metadata.num_property_attributes() == 0 {
            return Status::new(
                StatusCode::DracoError,
                "Structural metadata has no property tables, no property attributes.",
            );
        }
        ok_status()
    }

    fn add_structural_metadata_schema_to_geometry(
        &self,
        extension: &Map<String, Value>,
        metadata: &mut StructuralMetadata,
    ) -> Result<(), Status> {
        let schema_value = extension.get("schema").ok_or_else(|| {
            Status::new(
                StatusCode::DracoError,
                "Structural metadata extension has no schema.",
            )
        })?;
        let mut schema = StructuralMetadataSchema::new();
        parse_schema_value(schema_value, &mut schema.json)?;
        metadata.set_schema(schema);
        Ok(())
    }

    fn add_property_tables_to_geometry(
        &self,
        extension: &Map<String, Value>,
        metadata: &mut StructuralMetadata,
    ) -> Result<(), Status> {
        let tables_value = match extension.get("propertyTables") {
            Some(value) => value,
            None => return Ok(()),
        };
        let tables_array = tables_value.as_array().ok_or_else(|| {
            Status::new(
                StatusCode::DracoError,
                "Property tables array is malformed.",
            )
        })?;

        for table_value in tables_array {
            let table_obj = table_value.as_object().ok_or_else(|| {
                Status::new(StatusCode::DracoError, "Property table is malformed.")
            })?;

            let class_value = decode_string("class", table_obj)?.ok_or_else(|| {
                Status::new(StatusCode::DracoError, "Property class is malformed.")
            })?;
            let count_value = decode_int("count", table_obj)?.ok_or_else(|| {
                Status::new(StatusCode::DracoError, "Property count is malformed.")
            })?;

            let mut table = PropertyTable::new();
            table.set_class(&class_value);
            table.set_count(count_value);
            if let Some(name) = decode_string("name", table_obj)? {
                table.set_name(&name);
            }

            let properties_value = table_obj.get("properties").ok_or_else(|| {
                Status::new(StatusCode::DracoError, "Property table is malformed.")
            })?;
            let properties_obj = properties_value.as_object().ok_or_else(|| {
                Status::new(
                    StatusCode::DracoError,
                    "Property table properties property is malformed.",
                )
            })?;

            for (key, property_value) in properties_obj {
                let property_obj = property_value.as_object().ok_or_else(|| {
                    Status::new(StatusCode::DracoError, "Property entry is malformed.")
                })?;

                let mut property = PropertyTableProperty::new();
                property.set_name(key);

                let mut data = PropertyTableData::default();
                let values_ok = decode_property_table_data(
                    "values",
                    property_obj,
                    &self.gltf_model,
                    &mut data,
                )?;
                if !values_ok {
                    return Err(Status::new(
                        StatusCode::DracoError,
                        "Property values property is malformed.",
                    ));
                }
                *property.data_mut() = data;

                if let Some(type_name) = decode_string("stringOffsetType", property_obj)? {
                    property.string_offsets_mut().type_name = type_name;
                }
                if let Some(type_name) = decode_string("arrayOffsetType", property_obj)? {
                    property.array_offsets_mut().type_name = type_name;
                }

                let mut array_offsets = PropertyTableData::default();
                if decode_property_table_data(
                    "arrayOffsets",
                    property_obj,
                    &self.gltf_model,
                    &mut array_offsets,
                )? {
                    property.array_offsets_mut().data = array_offsets;
                }

                let mut string_offsets = PropertyTableData::default();
                if decode_property_table_data(
                    "stringOffsets",
                    property_obj,
                    &self.gltf_model,
                    &mut string_offsets,
                )? {
                    property.string_offsets_mut().data = string_offsets;
                }

                table.add_property(property);
            }

            metadata.add_property_table(table);
        }
        Ok(())
    }

    fn add_property_attributes_to_geometry(
        &self,
        extension: &Map<String, Value>,
        metadata: &mut StructuralMetadata,
    ) -> Result<(), Status> {
        let attributes_value = match extension.get("propertyAttributes") {
            Some(value) => value,
            None => return Ok(()),
        };
        let attributes_array = attributes_value.as_array().ok_or_else(|| {
            Status::new(
                StatusCode::DracoError,
                "Property attributes array is malformed.",
            )
        })?;

        for attribute_value in attributes_array {
            let attribute_obj = attribute_value.as_object().ok_or_else(|| {
                Status::new(StatusCode::DracoError, "Property attribute is malformed.")
            })?;

            let class_value = decode_string("class", attribute_obj)?.ok_or_else(|| {
                Status::new(StatusCode::DracoError, "Property class is malformed.")
            })?;

            let mut property_attribute = PropertyAttribute::new();
            property_attribute.set_class(&class_value);
            if let Some(name) = decode_string("name", attribute_obj)? {
                property_attribute.set_name(&name);
            }

            let properties_value = attribute_obj.get("properties").ok_or_else(|| {
                Status::new(StatusCode::DracoError, "Property attribute is malformed.")
            })?;
            let properties_obj = properties_value.as_object().ok_or_else(|| {
                Status::new(
                    StatusCode::DracoError,
                    "Property attribute properties property is malformed.",
                )
            })?;

            for (key, property_value) in properties_obj {
                let property_obj = property_value.as_object().ok_or_else(|| {
                    Status::new(StatusCode::DracoError, "Property entry is malformed.")
                })?;

                let mut property = PropertyAttributeProperty::new();
                property.set_name(key);

                let attribute_name =
                    decode_string("attribute", property_obj)?.ok_or_else(|| {
                        Status::new(StatusCode::DracoError, "Property attribute is malformed.")
                    })?;
                property.set_attribute_name(&attribute_name);

                property_attribute.add_property(property);
            }

            metadata.add_property_attribute(property_attribute);
        }
        Ok(())
    }

    fn add_animations_to_scene(&self, scene: &mut Scene) -> Status {
        for animation in &self.gltf_model.json.animations {
            let animation_index = scene.add_animation();
            let encoder_animation = scene.animation_mut(animation_index);
            if let Some(name) = animation.name.as_ref() {
                encoder_animation.set_name(name);
            }

            for channel in &animation.channels {
                let target_node = channel.target.node.value() as i32;
                let node_index = match self.gltf_node_to_scenenode_index.get(&target_node) {
                    Some(index) => *index,
                    None => {
                        return Status::new(
                            StatusCode::DracoError,
                            "Could not find Node in the scene.",
                        )
                    }
                };
                draco_core::draco_return_if_error!(TinyGltfUtils::add_channel_to_animation(
                    &self.gltf_model,
                    animation,
                    channel,
                    node_index.value() as i32,
                    encoder_animation,
                ));
            }
        }
        ok_status()
    }

    fn decode_node_for_scene(
        &mut self,
        node_index: i32,
        parent_index: SceneNodeIndex,
        scene: &mut Scene,
    ) -> Status {
        let mut scene_node_index = INVALID_SCENE_NODE_INDEX;
        let mut is_new_node = true;
        if self.gltf_scene_graph_mode == GltfSceneGraphMode::Dag {
            if let Some(existing) = self.gltf_node_to_scenenode_index.get(&node_index) {
                scene_node_index = *existing;
                is_new_node = false;
            }
        }

        if is_new_node {
            scene_node_index = scene.add_node();
            self.gltf_node_to_scenenode_index
                .insert(node_index, scene_node_index);
        }

        if parent_index != INVALID_SCENE_NODE_INDEX {
            let scene_node = scene.node_mut(scene_node_index);
            scene_node.add_parent_index(parent_index);
            let parent_node = scene.node_mut(parent_index);
            parent_node.add_child_index(scene_node_index);
        }

        if !is_new_node {
            return ok_status();
        }

        let (
            node_name,
            node_trs,
            skin_index,
            mesh_index,
            mesh_name,
            mesh_primitives,
            light_index,
            children,
        ) = {
            let node = &self.gltf_model.json.nodes[node_index as usize];
            let node_name = node.name.clone();
            let node_trs = get_node_trs_matrix(node);
            let skin_index = node.skin.as_ref().map(|skin| skin.value() as u32);
            let mesh_index = node.mesh.as_ref().map(|mesh| mesh.value());
            let mesh_name =
                mesh_index.and_then(|idx| self.gltf_model.json.meshes[idx].name.clone());
            let mesh_primitives =
                mesh_index.map(|idx| self.gltf_model.json.meshes[idx].primitives.clone());
            let light_index = node
                .extensions
                .as_ref()
                .and_then(|ext| ext.khr_lights_punctual.as_ref())
                .map(|khr| khr.light.value() as i32);
            let children: Vec<i32> = node
                .children
                .as_ref()
                .map(|c| c.iter().map(|child| child.value() as i32).collect())
                .unwrap_or_default();
            (
                node_name,
                node_trs,
                skin_index,
                mesh_index,
                mesh_name,
                mesh_primitives,
                light_index,
                children,
            )
        };

        {
            let scene_node = scene.node_mut(scene_node_index);
            if let Some(name) = node_name.as_ref() {
                scene_node.set_name(name);
            }
            scene_node.set_trs_matrix(&node_trs);
            if let Some(skin) = skin_index {
                scene_node.set_skin_index(SkinIndex::from(skin));
            }
        }

        let (mesh_group_index, decode_mesh) = if let Some(mesh_index) = mesh_index {
            if let Some(existing) = self.gltf_mesh_to_scene_mesh_group.get(&(mesh_index as i32)) {
                (Some(*existing), false)
            } else {
                let scene_mesh_group_index = scene.add_mesh_group();
                if let Some(name) = mesh_name.as_ref() {
                    scene.mesh_group_mut(scene_mesh_group_index).set_name(name);
                }
                self.gltf_mesh_to_scene_mesh_group
                    .insert(mesh_index as i32, scene_mesh_group_index);
                (Some(scene_mesh_group_index), true)
            }
        } else {
            (None, false)
        };

        if let Some(mesh_group_index) = mesh_group_index {
            scene
                .node_mut(scene_node_index)
                .set_mesh_group_index(mesh_group_index);
        }

        if decode_mesh {
            if let Some(primitives) = mesh_primitives.as_ref() {
                for primitive in primitives {
                    if let Some(mesh_group_index) = mesh_group_index {
                        let status =
                            self.decode_primitive_for_scene(primitive, mesh_group_index, scene);
                        if !status.is_ok() {
                            return status;
                        }
                    }
                }
            }
        }

        if let Some(light_index) = light_index {
            if light_index < 0 || light_index >= scene.num_lights() {
                return Status::new(StatusCode::DracoError, "Node light index is out of bounds.");
            }
            scene
                .node_mut(scene_node_index)
                .set_light_index(LightIndex::from(light_index as u32));
        }

        for child_index in children {
            draco_core::draco_return_if_error!(self.decode_node_for_scene(
                child_index,
                scene_node_index,
                scene
            ));
        }
        ok_status()
    }

    fn decode_primitive_for_scene(
        &mut self,
        primitive: &gltf::mesh::Primitive,
        mesh_group_index: MeshGroupIndex,
        scene: &mut Scene,
    ) -> Status {
        let mode = mode_to_gl_enum(&primitive.mode);
        if mode != gltf::mesh::Mode::Triangles.as_gl_enum()
            && mode != gltf::mesh::Mode::Points.as_gl_enum()
        {
            return Status::new(
                StatusCode::DracoError,
                "Primitive does not contain triangles or points.",
            );
        }

        let mut mappings: Vec<MaterialsVariantsMapping> = Vec::new();
        if let Some(ext) = primitive.extensions.as_ref() {
            if let Some(khr) = ext.khr_materials_variants.as_ref() {
                for mapping in &khr.mappings {
                    let material = mapping.material as i32;
                    let variants: Vec<i32> = mapping.variants.iter().map(|v| *v as i32).collect();
                    mappings.push(MaterialsVariantsMapping::new(material, &variants));
                }
            }
        }

        let signature = PrimitiveSignature::new(primitive);
        if let Some(existing) = self.gltf_primitive_to_draco_mesh_index.get(&signature) {
            let material_index = primitive
                .material
                .as_ref()
                .map(|m| m.value() as i32)
                .unwrap_or(-1);
            scene
                .mesh_group_mut(mesh_group_index)
                .add_mesh_instance(MeshInstance::with_variants(
                    *existing,
                    material_index,
                    &mappings,
                ));
            return ok_status();
        }

        let draco_extension = get_draco_extension(primitive);
        let mut draco_decoded: Option<DecodedDracoPrimitive> = None;
        if let Some(extension) = draco_extension.as_ref() {
            match decode_draco_primitive(&self.gltf_model, extension) {
                Ok(decoded) => draco_decoded = Some(decoded),
                Err(status) => return status,
            }
        }

        let (indices_data, number_of_faces, number_of_points) =
            if let Some(decoded) = draco_decoded.as_ref() {
                (
                    decoded.indices_data.clone(),
                    decoded.number_of_faces,
                    decoded.number_of_points,
                )
            } else {
                let indices_or = self.decode_primitive_indices(primitive);
                if !indices_or.is_ok() {
                    return Status::new(StatusCode::DracoError, "Could not convert indices.");
                }
                let indices = indices_or.into_value();
                let number_of_faces = (indices.len() / 3) as i32;
                let number_of_points = indices.len() as i32;
                (indices, number_of_faces, number_of_points)
            };

        let mut mb = TriangleSoupMeshBuilder::new();
        let mut pb = PointCloudBuilder::new();
        if mode == gltf::mesh::Mode::Triangles.as_gl_enum() {
            mb.start(number_of_faces);
        } else {
            pb.start(number_of_points as u32);
        }

        self.feature_id_attribute_indices.clear();
        let mut normalized_attributes: HashSet<i32> = HashSet::new();

        let mut attribute_entries: Vec<(String, usize)> = primitive
            .attributes
            .iter()
            .map(|(semantic, accessor_index)| {
                (semantic_to_string(semantic), accessor_index.value())
            })
            .collect();
        attribute_entries.sort_by(|a, b| a.0.cmp(&b.0));
        for (attribute_name, accessor_index) in attribute_entries {
            let accessor = self.gltf_model.json.accessors[accessor_index].clone();
            let component_type = match checked_component_type(&accessor) {
                Ok(component_type) => component_type,
                Err(status) => return status,
            };
            let accessor_type = match checked_accessor_type(&accessor) {
                Ok(accessor_type) => accessor_type,
                Err(status) => return status,
            };
            let normalized = accessor.normalized;
            let att_id = if mode == gltf::mesh::Mode::Triangles.as_gl_enum() {
                match self.add_attribute_for_builder_mesh(
                    &mut mb,
                    &attribute_name,
                    component_type,
                    accessor_type,
                ) {
                    Ok(att_id) => att_id,
                    Err(status) => return status,
                }
            } else {
                match self.add_attribute_for_builder_point(
                    &mut pb,
                    &attribute_name,
                    component_type,
                    accessor_type,
                ) {
                    Ok(att_id) => att_id,
                    Err(status) => return status,
                }
            };
            if att_id == -1 {
                continue;
            }
            if let Err(status) = self.register_feature_id_attribute(&attribute_name, att_id) {
                return status;
            }
            if normalized {
                normalized_attributes.insert(att_id);
            }

            if let Some(decoded) = draco_decoded.as_ref() {
                if let Some(unique_id) = decoded.attribute_unique_ids.get(&attribute_name) {
                    match &decoded.geometry {
                        DracoGeometry::Mesh(mesh) => {
                            let attribute =
                                match find_attribute_by_unique_id_mesh(mesh.as_ref(), *unique_id) {
                                    Ok(attribute) => attribute,
                                    Err(status) => return status,
                                };
                            if mode == gltf::mesh::Mode::Triangles.as_gl_enum() {
                                if let Err(status) = add_attribute_values_from_draco_mesh_builder(
                                    &mut mb,
                                    &attribute_name,
                                    &accessor,
                                    &indices_data,
                                    att_id,
                                    number_of_faces,
                                    mesh.as_ref(),
                                    attribute,
                                ) {
                                    return status;
                                }
                            } else if let Err(status) =
                                add_attribute_values_from_draco_point_builder(
                                    &mut pb,
                                    &attribute_name,
                                    &accessor,
                                    &indices_data,
                                    att_id,
                                    number_of_points,
                                    mesh.as_ref(),
                                    attribute,
                                )
                            {
                                return status;
                            }
                            continue;
                        }
                        DracoGeometry::PointCloud(pc) => {
                            let attribute =
                                match find_attribute_by_unique_id_point(pc.as_ref(), *unique_id) {
                                    Ok(attribute) => attribute,
                                    Err(status) => return status,
                                };
                            if let Err(status) = add_attribute_values_from_draco_point_builder(
                                &mut pb,
                                &attribute_name,
                                &accessor,
                                &indices_data,
                                att_id,
                                number_of_points,
                                pc.as_ref(),
                                attribute,
                            ) {
                                return status;
                            }
                            continue;
                        }
                    }
                }
            }

            if mode == gltf::mesh::Mode::Triangles.as_gl_enum() {
                if let Err(status) = add_attribute_values_from_accessor_mesh_builder(
                    &self.gltf_model,
                    &mut mb,
                    &attribute_name,
                    &accessor,
                    &indices_data,
                    att_id,
                    number_of_faces,
                ) {
                    return status;
                }
            } else if let Err(status) = add_attribute_values_from_accessor_point_builder(
                &self.gltf_model,
                &mut pb,
                &attribute_name,
                &accessor,
                &indices_data,
                att_id,
                number_of_points,
            ) {
                return status;
            }
        }

        let mesh_or = build_mesh_from_builder_local(
            mode == gltf::mesh::Mode::Triangles.as_gl_enum(),
            &mut mb,
            &mut pb,
            self.deduplicate_vertices,
        );
        if !mesh_or.is_ok() {
            return Status::new(
                StatusCode::DracoError,
                "Failed to build Draco mesh from glTF data.",
            );
        }
        let mut mesh = mesh_or.into_value();

        for att_id in normalized_attributes {
            if let Some(att) = mesh.attribute_mut(att_id) {
                att.set_normalized(true);
            }
        }

        let texture_library_ptr = {
            let library = scene.material_library_mut().texture_library_mut();
            std::ptr::NonNull::from(library)
        };
        let status = self.add_primitive_extensions_to_mesh_instance(
            primitive,
            texture_library_ptr,
            &mut mesh,
        );
        if !status.is_ok() {
            return status;
        }
        let status =
            self.maybe_generate_auto_tangents_for_primitive(primitive, &mappings, &mut mesh);
        if !status.is_ok() {
            return status;
        }
        let mesh_index = scene.add_mesh(mesh);
        if mesh_index == INVALID_MESH_INDEX {
            return Status::new(StatusCode::DracoError, "Could not add Draco mesh to scene.");
        }
        let material_index = primitive
            .material
            .as_ref()
            .map(|m| m.value() as i32)
            .unwrap_or(-1);
        scene
            .mesh_group_mut(mesh_group_index)
            .add_mesh_instance(MeshInstance::with_variants(
                mesh_index,
                material_index,
                &mappings,
            ));
        self.gltf_primitive_to_draco_mesh_index
            .insert(signature, mesh_index);
        ok_status()
    }

    fn add_attribute_for_builder_mesh(
        &self,
        builder: &mut TriangleSoupMeshBuilder,
        attribute_name: &str,
        component_type: gltf::accessor::ComponentType,
        accessor_type: gltf::accessor::Type,
    ) -> Result<i32, Status> {
        let draco_att_type = gltf_attribute_to_draco_attribute(attribute_name);
        if draco_att_type == GeometryAttributeType::Invalid {
            return Ok(-1);
        }
        let num_components = TinyGltfUtils::get_num_components_for_type(accessor_type);
        if num_components == 0 {
            return Err(Status::new(
                StatusCode::DracoError,
                "Could not add attribute with 0 components.",
            ));
        }
        let data_type = gltf_component_type_to_draco_type(component_type);
        if data_type == DataType::Invalid {
            return Err(Status::new(
                StatusCode::DracoError,
                "Could not add attribute with invalid type.",
            ));
        }
        let att_id = builder.add_attribute(draco_att_type, num_components as i8, data_type);
        if att_id < 0 {
            return Err(Status::new(
                StatusCode::DracoError,
                "Could not add attribute.",
            ));
        }
        Ok(att_id)
    }

    fn add_attribute_for_builder_point(
        &self,
        builder: &mut PointCloudBuilder,
        attribute_name: &str,
        component_type: gltf::accessor::ComponentType,
        accessor_type: gltf::accessor::Type,
    ) -> Result<i32, Status> {
        let draco_att_type = gltf_attribute_to_draco_attribute(attribute_name);
        if draco_att_type == GeometryAttributeType::Invalid {
            return Ok(-1);
        }
        let num_components = TinyGltfUtils::get_num_components_for_type(accessor_type);
        if num_components == 0 {
            return Err(Status::new(
                StatusCode::DracoError,
                "Could not add attribute with 0 components.",
            ));
        }
        let data_type = gltf_component_type_to_draco_type(component_type);
        if data_type == DataType::Invalid {
            return Err(Status::new(
                StatusCode::DracoError,
                "Could not add attribute with invalid type.",
            ));
        }
        let att_id = builder.add_attribute(draco_att_type, num_components as i8, data_type);
        if att_id < 0 {
            return Err(Status::new(
                StatusCode::DracoError,
                "Could not add attribute.",
            ));
        }
        Ok(att_id)
    }

    fn add_primitive_extensions_to_mesh(&mut self, mesh: &mut Mesh) -> Status {
        let node_indices: Vec<i32> = self
            .gltf_model
            .json
            .scenes
            .iter()
            .flat_map(|scene| scene.nodes.iter().map(|node| node.value() as i32))
            .collect();
        for node_index in node_indices {
            draco_core::draco_return_if_error!(
                self.add_primitive_extensions_to_mesh_node(node_index, mesh)
            );
        }
        ok_status()
    }

    fn add_primitive_extensions_to_mesh_node(
        &mut self,
        node_index: i32,
        mesh: &mut Mesh,
    ) -> Status {
        let (mesh_index, children) = {
            let node = &self.gltf_model.json.nodes[node_index as usize];
            let mesh_index = node.mesh.as_ref().map(|mesh| mesh.value());
            let children: Vec<i32> = node
                .children
                .as_ref()
                .map(|c| c.iter().map(|child| child.value() as i32).collect())
                .unwrap_or_default();
            (mesh_index, children)
        };
        if let Some(mesh_index) = mesh_index {
            let primitives = self.gltf_model.json.meshes[mesh_index].primitives.clone();
            for primitive in primitives {
                let texture_library_ptr = {
                    let library = mesh.material_library_mut().texture_library_mut();
                    std::ptr::NonNull::from(library)
                };
                let status = self.add_primitive_extensions_to_mesh_instance(
                    &primitive,
                    texture_library_ptr,
                    mesh,
                );
                if !status.is_ok() {
                    return status;
                }
            }
        }
        for child_index in children {
            draco_core::draco_return_if_error!(
                self.add_primitive_extensions_to_mesh_node(child_index, mesh)
            );
        }
        ok_status()
    }

    fn add_primitive_extensions_to_mesh_instance(
        &mut self,
        primitive: &gltf::mesh::Primitive,
        texture_library: std::ptr::NonNull<TextureLibrary>,
        mesh: &mut Mesh,
    ) -> Status {
        draco_core::draco_return_if_error!(self.decode_mesh_features(
            primitive,
            texture_library,
            mesh
        ));
        draco_core::draco_return_if_error!(self.decode_structural_metadata(primitive, mesh));
        ok_status()
    }

    fn decode_mesh_features(
        &mut self,
        primitive: &gltf::mesh::Primitive,
        texture_library: std::ptr::NonNull<TextureLibrary>,
        mesh: &mut Mesh,
    ) -> Status {
        // In mesh-decode mode (not scene), feature_id_attribute_indices may
        // not have been populated yet, so rebuild it from the primitive.
        // In scene mode, decode_primitive_for_scene already populated it.
        if !self.decoding_scene {
            if let Err(status) = self.rebuild_feature_id_attribute_indices_for_primitive(primitive)
            {
                return status;
            }
        }
        let ext = match primitive.extensions.as_ref() {
            Some(ext) => ext,
            None => return ok_status(),
        };
        let value = match ext.others.get(EXT_MESH_FEATURES) {
            Some(value) => value,
            None => return ok_status(),
        };
        let object = match value.as_object() {
            Some(object) => object,
            None => {
                return Status::new(
                    StatusCode::DracoError,
                    "Mesh features extension is malformed.",
                )
            }
        };
        let mut mesh_features: Vec<MeshFeatures> = Vec::new();
        if let Err(status) =
            self.decode_mesh_features_object(object, texture_library, &mut mesh_features)
        {
            return status;
        }
        for features in mesh_features {
            let mfi = mesh.add_mesh_features(Box::new(features));
            if self.scene.is_none() {
                let material_index = primitive
                    .material
                    .as_ref()
                    .map(|m| m.value() as i32)
                    .unwrap_or(-1);
                if let Some(mapped) = self
                    .gltf_primitive_material_to_draco_material
                    .get(&material_index)
                {
                    mesh.add_mesh_features_material_mask(mfi, *mapped);
                }
            }
        }
        ok_status()
    }

    fn rebuild_feature_id_attribute_indices_for_primitive(
        &mut self,
        primitive: &gltf::mesh::Primitive,
    ) -> Result<(), Status> {
        self.feature_id_attribute_indices.clear();
        for (semantic, _) in primitive.attributes.iter() {
            let attribute_name = semantic_to_string(semantic);
            if !attribute_name.starts_with("_FEATURE_ID_") {
                continue;
            }
            let index = get_index_from_feature_id_attribute_name(&attribute_name)?;
            if let Some(att_id) = self
                .attribute_name_to_draco_mesh_attribute_id
                .get(&attribute_name)
            {
                if *att_id >= 0 {
                    self.feature_id_attribute_indices.insert(index, *att_id);
                }
            }
        }
        Ok(())
    }

    fn register_feature_id_attribute(
        &mut self,
        attribute_name: &str,
        att_id: i32,
    ) -> Result<(), Status> {
        if !attribute_name.starts_with("_FEATURE_ID_") {
            return Ok(());
        }
        let index = get_index_from_feature_id_attribute_name(attribute_name)?;
        self.feature_id_attribute_indices.insert(index, att_id);
        Ok(())
    }

    fn decode_structural_metadata(
        &mut self,
        primitive: &gltf::mesh::Primitive,
        mesh: &mut Mesh,
    ) -> Status {
        let ext = match primitive.extensions.as_ref() {
            Some(ext) => ext,
            None => return ok_status(),
        };
        let value = match ext.others.get(EXT_STRUCTURAL_METADATA) {
            Some(value) => value,
            None => return ok_status(),
        };
        let object = match value.as_object() {
            Some(object) => object,
            None => {
                return Status::new(
                    StatusCode::DracoError,
                    "Property attributes array is malformed.",
                )
            }
        };
        let mut property_attributes_indices: Vec<i32> = Vec::new();
        if let Err(status) =
            self.decode_structural_metadata_object(object, &mut property_attributes_indices)
        {
            return status;
        }
        for pai in property_attributes_indices {
            let index = mesh.add_property_attributes_index(pai);
            if self.scene.is_none() {
                let material_index = primitive
                    .material
                    .as_ref()
                    .map(|m| m.value() as i32)
                    .unwrap_or(-1);
                if let Some(mapped) = self
                    .gltf_primitive_material_to_draco_material
                    .get(&material_index)
                {
                    mesh.add_property_attributes_index_material_mask(index, *mapped);
                }
            }
        }
        ok_status()
    }

    fn decode_mesh_features_object(
        &mut self,
        extension: &Map<String, Value>,
        texture_library: std::ptr::NonNull<TextureLibrary>,
        mesh_features: &mut Vec<MeshFeatures>,
    ) -> Result<(), Status> {
        let feature_ids_value = extension.get("featureIds").ok_or_else(|| {
            Status::new(
                StatusCode::DracoError,
                "Mesh features extension is malformed.",
            )
        })?;
        let array = feature_ids_value.as_array().ok_or_else(|| {
            Status::new(StatusCode::DracoError, "Mesh features array is malformed.")
        })?;

        for entry in array {
            let object = entry.as_object().ok_or_else(|| {
                Status::new(
                    StatusCode::DracoError,
                    "Mesh features array entry is malformed.",
                )
            })?;
            let mut features = MeshFeatures::new();

            let feature_count = decode_int("featureCount", object)?.ok_or_else(|| {
                Status::new(StatusCode::DracoError, "Mesh features is malformed.")
            })?;
            features.set_feature_count(feature_count);

            if let Some(null_feature_id) = decode_int("nullFeatureId", object)? {
                features.set_null_feature_id(null_feature_id);
            }
            if let Some(label) = decode_string("label", object)? {
                features.set_label(&label);
            }
            if let Some(attribute_index) = decode_int("attribute", object)? {
                // C++ uses unordered_map::operator[] which returns 0 for missing keys.
                let index = attribute_index as usize;
                let att_id = *self.feature_id_attribute_indices.get(&index).unwrap_or(&0);
                features.set_attribute_index(att_id);
            }
            if let Some(texture_value) = object.get("texture") {
                let texture_object = texture_value.as_object().ok_or_else(|| {
                    Status::new(StatusCode::DracoError, "Texture property is malformed.")
                })?;

                let mut material = Material::with_texture_library(Some(texture_library));
                self.decode_texture(
                    "texture",
                    TextureMapType::Generic,
                    texture_object,
                    &mut material,
                )?;
                if let Some(map) = material.texture_map_by_type(TextureMapType::Generic) {
                    features.set_texture_map(map);
                }

                let mut channels = vec![0];
                if let Some(channels_value) = texture_object.get("channels") {
                    let array = channels_value.as_array().ok_or_else(|| {
                        Status::new(StatusCode::DracoError, "Channels property is malformed.")
                    })?;
                    channels.clear();
                    for value in array {
                        let channel = value.as_i64().ok_or_else(|| {
                            Status::new(StatusCode::DracoError, "Channels value is malformed.")
                        })?;
                        channels.push(channel as i32);
                    }
                }
                features.set_texture_channels(channels);
            }
            if let Some(property_table) = decode_int("propertyTable", object)? {
                features.set_property_table_index(property_table);
            }

            mesh_features.push(features);
        }
        Ok(())
    }

    fn decode_structural_metadata_object(
        &self,
        extension: &Map<String, Value>,
        property_attributes: &mut Vec<i32>,
    ) -> Result<(), Status> {
        let attrs_value = match extension.get("propertyAttributes") {
            Some(value) => value,
            None => return Ok(()),
        };
        let array = attrs_value.as_array().ok_or_else(|| {
            Status::new(
                StatusCode::DracoError,
                "Property attributes array is malformed.",
            )
        })?;
        for value in array {
            let index = value.as_i64().ok_or_else(|| {
                Status::new(
                    StatusCode::DracoError,
                    "Property attributes array entry is malformed.",
                )
            })?;
            property_attributes.push(index as i32);
        }
        Ok(())
    }

    fn decode_texture(
        &mut self,
        name: &str,
        map_type: TextureMapType,
        object: &Map<String, Value>,
        material: &mut Material,
    ) -> Result<(), Status> {
        let info = parse_texture_info(name, object)?;
        if let Some(info) = info {
            let status = self.check_and_add_texture_to_draco_material(
                info.index,
                info.tex_coord,
                info.extensions.as_ref(),
                material,
                map_type,
            );
            if !status.is_ok() {
                return Err(status);
            }
        }
        Ok(())
    }

    fn add_gltf_material(
        &mut self,
        input_material_index: i32,
        output_material: &mut Material,
    ) -> Status {
        let input_material = self.gltf_model.json.materials[input_material_index as usize].clone();

        if let Some(name) = input_material.name.as_ref() {
            output_material.set_name(name);
        }
        output_material.set_transparency_mode(TinyGltfUtils::text_to_material_mode(
            &input_material.alpha_mode,
        ));
        let alpha_cutoff = input_material.alpha_cutoff.unwrap_or_default().0;
        output_material.set_alpha_cutoff(alpha_cutoff);
        let emissive = input_material.emissive_factor.0;
        output_material.set_emissive_factor(Vector3f::new3(emissive[0], emissive[1], emissive[2]));

        let pbr = &input_material.pbr_metallic_roughness;
        let base_color = pbr.base_color_factor.0;
        output_material.set_color_factor(Vector4f::new4(
            base_color[0],
            base_color[1],
            base_color[2],
            base_color[3],
        ));
        output_material.set_metallic_factor(pbr.metallic_factor.0);
        output_material.set_roughness_factor(pbr.roughness_factor.0);
        output_material.set_double_sided(input_material.double_sided);

        if let Some(base_color_tex) = pbr.base_color_texture.as_ref() {
            let status = self.check_and_add_texture_to_draco_material(
                base_color_tex.index.value() as i32,
                base_color_tex.tex_coord as i32,
                base_color_tex.extensions.as_ref(),
                output_material,
                TextureMapType::Color,
            );
            if !status.is_ok() {
                return status;
            }
        }
        if let Some(metallic_tex) = pbr.metallic_roughness_texture.as_ref() {
            let status = self.check_and_add_texture_to_draco_material(
                metallic_tex.index.value() as i32,
                metallic_tex.tex_coord as i32,
                metallic_tex.extensions.as_ref(),
                output_material,
                TextureMapType::MetallicRoughness,
            );
            if !status.is_ok() {
                return status;
            }
        }

        if let Some(normal_tex) = input_material.normal_texture.as_ref() {
            let normal_ext = match normal_tex.extensions.as_ref() {
                Some(ext) => match parse_texture_extensions_from_material_map(&ext.others) {
                    Ok(info) => info,
                    Err(status) => return status,
                },
                None => None,
            };
            let status = self.check_and_add_texture_to_draco_material(
                normal_tex.index.value() as i32,
                normal_tex.tex_coord as i32,
                normal_ext.as_ref(),
                output_material,
                TextureMapType::NormalTangentSpace,
            );
            if !status.is_ok() {
                return status;
            }
            if normal_tex.scale != 1.0 {
                output_material.set_normal_texture_scale(normal_tex.scale);
            }
        }
        if let Some(occlusion_tex) = input_material.occlusion_texture.as_ref() {
            let occlusion_ext = match occlusion_tex.extensions.as_ref() {
                Some(ext) => match parse_texture_extensions_from_material_map(&ext.others) {
                    Ok(info) => info,
                    Err(status) => return status,
                },
                None => None,
            };
            let status = self.check_and_add_texture_to_draco_material(
                occlusion_tex.index.value() as i32,
                occlusion_tex.tex_coord as i32,
                occlusion_ext.as_ref(),
                output_material,
                TextureMapType::AmbientOcclusion,
            );
            if !status.is_ok() {
                return status;
            }
        }
        if let Some(emissive_tex) = input_material.emissive_texture.as_ref() {
            let status = self.check_and_add_texture_to_draco_material(
                emissive_tex.index.value() as i32,
                emissive_tex.tex_coord as i32,
                emissive_tex.extensions.as_ref(),
                output_material,
                TextureMapType::Emissive,
            );
            if !status.is_ok() {
                return status;
            }
        }

        self.decode_material_unlit_extension(&input_material, output_material);
        let status = self.decode_material_sheen_extension(&input_material, output_material);
        if !status.is_ok() {
            return status;
        }
        let status = self.decode_material_transmission_extension(&input_material, output_material);
        if !status.is_ok() {
            return status;
        }
        let status = self.decode_material_clearcoat_extension(&input_material, output_material);
        if !status.is_ok() {
            return status;
        }
        let status = self.decode_material_volume_extension(
            &input_material,
            input_material_index,
            output_material,
        );
        if !status.is_ok() {
            return status;
        }
        let status = self.decode_material_ior_extension(&input_material, output_material);
        if !status.is_ok() {
            return status;
        }
        let status = self.decode_material_specular_extension(&input_material, output_material);
        if !status.is_ok() {
            return status;
        }

        ok_status()
    }

    fn decode_material_unlit_extension(
        &self,
        input_material: &gltf::Material,
        output_material: &mut Material,
    ) {
        if let Some(extensions) = input_material.extensions.as_ref() {
            if extensions.unlit.is_some() {
                output_material.set_unlit(true);
            }
            if extensions.others.contains_key(KHR_MATERIALS_UNLIT) {
                output_material.set_unlit(true);
            }
        }
    }

    fn decode_material_sheen_extension(
        &mut self,
        input_material: &gltf::Material,
        output_material: &mut Material,
    ) -> Status {
        let extensions = match input_material.extensions.as_ref() {
            Some(ext) => ext,
            None => return ok_status(),
        };
        let value = match extensions.others.get("KHR_materials_sheen") {
            Some(value) => value,
            None => return ok_status(),
        };
        let object = match value.as_object() {
            Some(object) => object,
            None => {
                return Status::new(StatusCode::DracoError, "KHR_materials_sheen is malformed.")
            }
        };

        output_material.set_has_sheen(true);
        let vector = match decode_vector3f("sheenColorFactor", object) {
            Ok(vector) => vector,
            Err(status) => return status,
        };
        if let Some(vector) = vector {
            output_material.set_sheen_color_factor(vector);
        }
        let value = match decode_float("sheenRoughnessFactor", object) {
            Ok(value) => value,
            Err(status) => return status,
        };
        if let Some(value) = value {
            output_material.set_sheen_roughness_factor(value);
        }
        if let Err(status) = self.decode_texture(
            "sheenColorTexture",
            TextureMapType::SheenColor,
            object,
            output_material,
        ) {
            return status;
        }
        if let Err(status) = self.decode_texture(
            "sheenRoughnessTexture",
            TextureMapType::SheenRoughness,
            object,
            output_material,
        ) {
            return status;
        }
        ok_status()
    }

    fn decode_material_transmission_extension(
        &mut self,
        input_material: &gltf::Material,
        output_material: &mut Material,
    ) -> Status {
        let extensions = match input_material.extensions.as_ref() {
            Some(ext) => ext,
            None => return ok_status(),
        };
        let value = if let Some(ext) = extensions.transmission.as_ref() {
            match serde_json::to_value(ext) {
                Ok(value) => value,
                Err(_) => {
                    return Status::new(
                        StatusCode::DracoError,
                        "KHR_materials_transmission is malformed.",
                    )
                }
            }
        } else if let Some(value) = extensions.others.get("KHR_materials_transmission") {
            value.clone()
        } else {
            return ok_status();
        };

        output_material.set_has_transmission(true);
        let object = match value.as_object() {
            Some(object) => object,
            None => {
                return Status::new(
                    StatusCode::DracoError,
                    "KHR_materials_transmission is malformed.",
                )
            }
        };

        let transmission = match decode_float("transmissionFactor", object) {
            Ok(value) => value,
            Err(status) => return status,
        };
        if let Some(value) = transmission {
            output_material.set_transmission_factor(value);
        }
        if let Err(status) = self.decode_texture(
            "transmissionTexture",
            TextureMapType::Transmission,
            object,
            output_material,
        ) {
            return status;
        }
        ok_status()
    }

    fn decode_material_clearcoat_extension(
        &mut self,
        input_material: &gltf::Material,
        output_material: &mut Material,
    ) -> Status {
        let extensions = match input_material.extensions.as_ref() {
            Some(ext) => ext,
            None => return ok_status(),
        };
        let value = match extensions.others.get("KHR_materials_clearcoat") {
            Some(value) => value,
            None => return ok_status(),
        };
        let object = match value.as_object() {
            Some(object) => object,
            None => {
                return Status::new(
                    StatusCode::DracoError,
                    "KHR_materials_clearcoat is malformed.",
                )
            }
        };

        output_material.set_has_clearcoat(true);
        let clearcoat_factor = match decode_float("clearcoatFactor", object) {
            Ok(value) => value,
            Err(status) => return status,
        };
        if let Some(value) = clearcoat_factor {
            output_material.set_clearcoat_factor(value);
        }
        let roughness_factor = match decode_float("clearcoatRoughnessFactor", object) {
            Ok(value) => value,
            Err(status) => return status,
        };
        if let Some(value) = roughness_factor {
            output_material.set_clearcoat_roughness_factor(value);
        }
        if let Err(status) = self.decode_texture(
            "clearcoatTexture",
            TextureMapType::Clearcoat,
            object,
            output_material,
        ) {
            return status;
        }
        if let Err(status) = self.decode_texture(
            "clearcoatRoughnessTexture",
            TextureMapType::ClearcoatRoughness,
            object,
            output_material,
        ) {
            return status;
        }
        if let Err(status) = self.decode_texture(
            "clearcoatNormalTexture",
            TextureMapType::ClearcoatNormal,
            object,
            output_material,
        ) {
            return status;
        }
        ok_status()
    }

    fn decode_material_volume_extension(
        &mut self,
        input_material: &gltf::Material,
        input_material_index: i32,
        output_material: &mut Material,
    ) -> Status {
        let extensions = match input_material.extensions.as_ref() {
            Some(ext) => ext,
            None => return ok_status(),
        };
        let value = if let Some(ext) = extensions.volume.as_ref() {
            match serde_json::to_value(ext) {
                Ok(value) => value,
                Err(_) => {
                    return Status::new(
                        StatusCode::DracoError,
                        "KHR_materials_volume is malformed.",
                    )
                }
            }
        } else if let Some(value) = extensions.others.get("KHR_materials_volume") {
            value.clone()
        } else {
            return ok_status();
        };
        output_material.set_has_volume(true);
        let object = match value.as_object() {
            Some(object) => object,
            None => {
                return Status::new(StatusCode::DracoError, "KHR_materials_volume is malformed.")
            }
        };

        let thickness = match decode_float("thicknessFactor", object) {
            Ok(value) => value,
            Err(status) => return status,
        };
        if let Some(value) = thickness {
            let mut scale = 1.0f32;
            if self.scene.is_none() {
                if let Some(scales) = self
                    .gltf_primitive_material_to_scales
                    .get(&input_material_index)
                {
                    if !scales.is_empty() {
                        scale = scales[0];
                        for s in scales.iter().skip(1) {
                            if *s != scale {
                                return Status::new(
                                    StatusCode::DracoError,
                                    "Cannot represent volume thickness in a mesh.",
                                );
                            }
                        }
                    }
                }
            }
            output_material.set_thickness_factor(scale * value);
        }
        let attenuation_distance = match decode_float("attenuationDistance", object) {
            Ok(value) => value,
            Err(status) => return status,
        };
        if let Some(value) = attenuation_distance {
            output_material.set_attenuation_distance(value);
        }
        let attenuation_color = match decode_vector3f("attenuationColor", object) {
            Ok(value) => value,
            Err(status) => return status,
        };
        if let Some(vector) = attenuation_color {
            output_material.set_attenuation_color(vector);
        }
        if let Err(status) = self.decode_texture(
            "thicknessTexture",
            TextureMapType::Thickness,
            object,
            output_material,
        ) {
            return status;
        }
        ok_status()
    }

    fn decode_material_ior_extension(
        &mut self,
        input_material: &gltf::Material,
        output_material: &mut Material,
    ) -> Status {
        let extensions = match input_material.extensions.as_ref() {
            Some(ext) => ext,
            None => return ok_status(),
        };
        let value = if let Some(ext) = extensions.ior.as_ref() {
            match serde_json::to_value(ext) {
                Ok(value) => value,
                Err(_) => {
                    return Status::new(StatusCode::DracoError, "KHR_materials_ior is malformed.")
                }
            }
        } else if let Some(value) = extensions.others.get("KHR_materials_ior") {
            value.clone()
        } else {
            return ok_status();
        };
        output_material.set_has_ior(true);
        let object = match value.as_object() {
            Some(object) => object,
            None => return Status::new(StatusCode::DracoError, "KHR_materials_ior is malformed."),
        };
        let ior = match decode_float("ior", object) {
            Ok(value) => value,
            Err(status) => return status,
        };
        if let Some(value) = ior {
            output_material.set_ior(value);
        }
        ok_status()
    }

    fn decode_material_specular_extension(
        &mut self,
        input_material: &gltf::Material,
        output_material: &mut Material,
    ) -> Status {
        let extensions = match input_material.extensions.as_ref() {
            Some(ext) => ext,
            None => return ok_status(),
        };
        let value = if let Some(ext) = extensions.specular.as_ref() {
            match serde_json::to_value(ext) {
                Ok(value) => value,
                Err(_) => {
                    return Status::new(
                        StatusCode::DracoError,
                        "KHR_materials_specular is malformed.",
                    )
                }
            }
        } else if let Some(value) = extensions.others.get("KHR_materials_specular") {
            value.clone()
        } else {
            return ok_status();
        };
        output_material.set_has_specular(true);
        let object = match value.as_object() {
            Some(object) => object,
            None => {
                return Status::new(
                    StatusCode::DracoError,
                    "KHR_materials_specular is malformed.",
                )
            }
        };
        let specular_factor = match decode_float("specularFactor", object) {
            Ok(value) => value,
            Err(status) => return status,
        };
        if let Some(value) = specular_factor {
            output_material.set_specular_factor(value);
        }
        let specular_color = match decode_vector3f("specularColorFactor", object) {
            Ok(value) => value,
            Err(status) => return status,
        };
        if let Some(vector) = specular_color {
            output_material.set_specular_color_factor(vector);
        }
        if let Err(status) = self.decode_texture(
            "specularTexture",
            TextureMapType::Specular,
            object,
            output_material,
        ) {
            return status;
        }
        if let Err(status) = self.decode_texture(
            "specularColorTexture",
            TextureMapType::SpecularColor,
            object,
            output_material,
        ) {
            return status;
        }
        ok_status()
    }

    fn add_materials_to_scene(&mut self, scene: &mut Scene) -> Status {
        for input_material_index in 0..self.gltf_model.json.materials.len() {
            let output_material = scene
                .material_library_mut()
                .mutable_material(input_material_index as i32)
                .expect("material");
            draco_core::draco_return_if_error!(
                self.add_gltf_material(input_material_index as i32, output_material)
            );
        }

        let default_material_index = scene.material_library().num_materials() as i32;
        let mut default_material_needed = false;
        for mgi in 0..scene.num_mesh_groups() {
            let mesh_group = scene.mesh_group_mut(MeshGroupIndex::from(mgi as u32));
            for mi in 0..mesh_group.num_mesh_instances() {
                let instance = mesh_group.mesh_instance_mut(mi);
                if instance.material_index == -1 {
                    instance.material_index = default_material_index;
                    default_material_needed = true;
                }
            }
        }
        if default_material_needed {
            let _ = scene
                .material_library_mut()
                .mutable_material(default_material_index);
        }
        ok_status()
    }

    fn add_skins_to_scene(&self, scene: &mut Scene) -> Status {
        for source_skin_index in 0..self.gltf_model.json.skins.len() {
            let skin = &self.gltf_model.json.skins[source_skin_index];
            let skin_index = scene.add_skin();
            let new_skin = scene.skin_mut(skin_index);

            if skin_index.value() as usize != source_skin_index {
                return Status::new(StatusCode::DracoError, "Skin indices are mismatched.");
            }

            if let Some(accessor_index) = skin.inverse_bind_matrices.as_ref() {
                let accessor = &self.gltf_model.json.accessors[accessor_index.value()];
                draco_core::draco_return_if_error!(TinyGltfUtils::add_accessor_to_animation_data(
                    &self.gltf_model,
                    accessor,
                    new_skin.inverse_bind_matrices_mut(),
                ));
            }

            if let Some(skeleton) = skin.skeleton.as_ref() {
                let node_index = skeleton.value() as i32;
                let mapped = match self.gltf_node_to_scenenode_index.get(&node_index) {
                    Some(mapped) => mapped,
                    None => {
                        return Status::new(
                            StatusCode::DracoError,
                            "Could not find skeleton in the skin.",
                        )
                    }
                };
                new_skin.set_joint_root(*mapped);
            }

            for joint in &skin.joints {
                let node_index = joint.value() as i32;
                let mapped = match self.gltf_node_to_scenenode_index.get(&node_index) {
                    Some(mapped) => mapped,
                    None => {
                        return Status::new(
                            StatusCode::DracoError,
                            "Could not find skeleton in the skin.",
                        )
                    }
                };
                new_skin.add_joint(*mapped);
            }
        }
        ok_status()
    }

    fn move_non_material_textures_mesh(&self, mesh: &mut Mesh) {
        let mut non_material_textures: HashSet<*const Texture> = HashSet::new();
        for i in 0..mesh.num_mesh_features() {
            let features = mesh.mesh_features(MeshFeaturesIndex::from(i as u32));
            if let Some(texture) = features.texture_map().texture() {
                non_material_textures.insert(texture as *const Texture);
            }
        }
        let (material_tl, non_material_tl) = mesh.texture_libraries_for_move_mut();
        move_non_material_textures(&non_material_textures, material_tl, non_material_tl);
    }

    fn move_non_material_textures_scene(&self, scene: &mut Scene) {
        let mut non_material_textures: HashSet<*const Texture> = HashSet::new();
        for i in 0..scene.num_meshes() {
            let mesh = scene.mesh(MeshIndex::from(i as u32));
            for j in 0..mesh.num_mesh_features() {
                let features = mesh.mesh_features(MeshFeaturesIndex::from(j as u32));
                if let Some(texture) = features.texture_map().texture() {
                    non_material_textures.insert(texture as *const Texture);
                }
            }
        }
        let (material_tl, non_material_tl) = scene.texture_libraries_for_move_mut();
        move_non_material_textures(&non_material_textures, material_tl, non_material_tl);
    }

    /// Adds auto-generated tangents when glTF normal maps require them.
    ///
    /// Why: glTF normal maps are tangent-space; Draco C++ auto-generates missing tangents.
    fn maybe_generate_auto_tangents_for_mesh(&self, mesh: &mut Mesh) -> Status {
        if mesh.get_named_attribute_id(GeometryAttributeType::Tangent) != -1 {
            return ok_status();
        }
        let mut needs_tangents = false;
        for material_index in self.gltf_primitive_material_to_draco_material.keys() {
            if *material_index < 0 {
                continue;
            }
            if self.gltf_material_has_normal_map(*material_index) {
                needs_tangents = true;
                break;
            }
        }
        if !needs_tangents {
            return ok_status();
        }
        self.generate_auto_tangents(mesh)
    }

    /// Adds auto-generated tangents for a scene primitive when normal maps require them.
    fn maybe_generate_auto_tangents_for_primitive(
        &self,
        primitive: &gltf::mesh::Primitive,
        mappings: &[MaterialsVariantsMapping],
        mesh: &mut Mesh,
    ) -> Status {
        if mesh.get_named_attribute_id(GeometryAttributeType::Tangent) != -1 {
            return ok_status();
        }
        if !self.primitive_requires_tangents(primitive, mappings) {
            return ok_status();
        }
        self.generate_auto_tangents(mesh)
    }

    fn primitive_requires_tangents(
        &self,
        primitive: &gltf::mesh::Primitive,
        mappings: &[MaterialsVariantsMapping],
    ) -> bool {
        let mut material_indices: Vec<i32> = Vec::new();
        if let Some(material) = primitive.material.as_ref() {
            material_indices.push(material.value() as i32);
        }
        for mapping in mappings {
            material_indices.push(mapping.material);
        }
        for material_index in material_indices {
            if self.gltf_material_has_normal_map(material_index) {
                return true;
            }
        }
        false
    }

    fn gltf_material_has_normal_map(&self, material_index: i32) -> bool {
        if material_index < 0 {
            return false;
        }
        let index = material_index as usize;
        if index >= self.gltf_model.json.materials.len() {
            return false;
        }
        self.gltf_model.json.materials[index]
            .normal_texture
            .as_ref()
            .is_some()
    }

    fn generate_auto_tangents(&self, mesh: &mut Mesh) -> Status {
        if mesh.num_faces() == 0 || mesh.num_points() == 0 {
            return ok_status();
        }
        let pos_att = match mesh.get_named_attribute(GeometryAttributeType::Position) {
            Some(att) => att,
            None => return ok_status(),
        };
        let norm_att = match mesh.get_named_attribute(GeometryAttributeType::Normal) {
            Some(att) => att,
            None => return ok_status(),
        };
        let tex_att = match mesh.get_named_attribute(GeometryAttributeType::TexCoord) {
            Some(att) => att,
            None => return ok_status(),
        };

        let num_points = mesh.num_points() as usize;
        let mut tan1 = vec![[0.0f32; 3]; num_points];
        let mut tan2 = vec![[0.0f32; 3]; num_points];

        for fi in 0..mesh.num_faces() {
            let face = mesh.face(FaceIndex::from(fi));
            let p0 = match attribute_vec3_f32(pos_att, face[0]) {
                Some(v) => v,
                None => continue,
            };
            let p1 = match attribute_vec3_f32(pos_att, face[1]) {
                Some(v) => v,
                None => continue,
            };
            let p2 = match attribute_vec3_f32(pos_att, face[2]) {
                Some(v) => v,
                None => continue,
            };
            let uv0 = match attribute_vec2_f32(tex_att, face[0]) {
                Some(v) => v,
                None => continue,
            };
            let uv1 = match attribute_vec2_f32(tex_att, face[1]) {
                Some(v) => v,
                None => continue,
            };
            let uv2 = match attribute_vec2_f32(tex_att, face[2]) {
                Some(v) => v,
                None => continue,
            };

            let delta_pos1 = vec3_sub(p1, p0);
            let delta_pos2 = vec3_sub(p2, p0);
            let delta_uv1 = [uv1[0] - uv0[0], uv1[1] - uv0[1]];
            let delta_uv2 = [uv2[0] - uv0[0], uv2[1] - uv0[1]];
            let denom = delta_uv1[0] * delta_uv2[1] - delta_uv1[1] * delta_uv2[0];
            if denom.abs() < 1e-12f32 {
                continue;
            }
            let r = 1.0f32 / denom;
            let sdir = vec3_scale(
                vec3_sub(
                    vec3_scale(delta_pos1, delta_uv2[1]),
                    vec3_scale(delta_pos2, delta_uv1[1]),
                ),
                r,
            );
            let tdir = vec3_scale(
                vec3_sub(
                    vec3_scale(delta_pos2, delta_uv1[0]),
                    vec3_scale(delta_pos1, delta_uv2[0]),
                ),
                r,
            );

            let p0_index = face[0].value() as usize;
            let p1_index = face[1].value() as usize;
            let p2_index = face[2].value() as usize;
            if p0_index < num_points {
                tan1[p0_index] = vec3_add(tan1[p0_index], sdir);
                tan2[p0_index] = vec3_add(tan2[p0_index], tdir);
            }
            if p1_index < num_points {
                tan1[p1_index] = vec3_add(tan1[p1_index], sdir);
                tan2[p1_index] = vec3_add(tan2[p1_index], tdir);
            }
            if p2_index < num_points {
                tan1[p2_index] = vec3_add(tan1[p2_index], sdir);
                tan2[p2_index] = vec3_add(tan2[p2_index], tdir);
            }
        }

        let mut tang_att = PointAttribute::new();
        tang_att.init(
            GeometryAttributeType::Tangent,
            4,
            DataType::Float32,
            false,
            num_points,
        );
        for i in 0..num_points {
            let point_index = PointIndex::from(i as u32);
            let normal = match attribute_vec3_f32(norm_att, point_index) {
                Some(v) => v,
                None => [0.0f32; 3],
            };
            let mut tangent = tan1[i];
            let n_dot_t = vec3_dot(normal, tangent);
            tangent = vec3_sub(tangent, vec3_scale(normal, n_dot_t));
            tangent = vec3_normalize(tangent);
            let bitangent = tan2[i];
            let handedness = if vec3_dot(vec3_cross(normal, tangent), bitangent) < 0.0 {
                -1.0f32
            } else {
                1.0f32
            };
            let value = [tangent[0], tangent[1], tangent[2], handedness];
            tang_att.set_attribute_value_array(AttributeValueIndex::from(i as u32), &value);
        }

        let tangent_att_id = mesh.add_attribute(tang_att);
        if tangent_att_id < 0 {
            return ok_status();
        }
        mesh.set_attribute_element_type(
            tangent_att_id,
            MeshAttributeElementType::MeshVertexAttribute,
        );
        let mut metadata = AttributeMetadata::new();
        metadata.add_entry_int("auto_generated", 1);
        mesh.add_attribute_metadata(tangent_att_id, metadata);
        ok_status()
    }

    fn add_asset_metadata_to_scene(&self, scene: &mut Scene) -> Status {
        self.add_asset_metadata(scene.metadata_mut())
    }

    fn add_asset_metadata_to_mesh(&self, mesh: &mut Mesh) -> Status {
        if let Some(metadata) = mesh.metadata_mut() {
            return self.add_asset_metadata(metadata);
        }
        let mut metadata = draco_core::metadata::geometry_metadata::GeometryMetadata::new();
        let status = self.add_asset_metadata(&mut metadata);
        if status.is_ok() && metadata.num_entries() > 0 {
            mesh.add_metadata(metadata);
        }
        status
    }

    fn add_asset_metadata(&self, metadata: &mut Metadata) -> Status {
        if let Some(copyright) = self.gltf_model.json.asset.copyright.as_ref() {
            metadata.add_entry_string("copyright", copyright);
        }
        ok_status()
    }

    fn decode_cesium_rtc(&self, scene: &mut Scene) -> Result<(), Status> {
        let extensions = match self.gltf_model.json.extensions.as_ref() {
            Some(ext) => ext,
            None => return Ok(()),
        };
        let value = match extensions.others.get("CESIUM_RTC") {
            Some(value) => value,
            None => return Ok(()),
        };
        let center = value
            .get("center")
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                Status::new(StatusCode::DracoError, "CESIUM_RTC center is malformed.")
            })?;
        if center.len() != 3 {
            return Err(Status::new(
                StatusCode::DracoError,
                "CESIUM_RTC center is malformed.",
            ));
        }
        let mut rtc = Vec::with_capacity(3);
        for v in center {
            let value = v.as_f64().ok_or_else(|| {
                Status::new(StatusCode::DracoError, "CESIUM_RTC center is malformed.")
            })?;
            rtc.push(value);
        }
        scene.set_cesium_rtc(rtc);
        Ok(())
    }

    fn build_mesh_from_builder(&mut self) -> StatusOr<Box<Mesh>> {
        build_mesh_from_builder_local(
            self.total_face_indices_count > 0,
            &mut self.mb,
            &mut self.pb,
            self.deduplicate_vertices,
        )
    }
}

impl Default for GltfDecoder {
    fn default() -> Self {
        Self::new()
    }
}

// --- Helper functions below ---

fn gltf_component_type_to_draco_type(component_type: gltf::accessor::ComponentType) -> DataType {
    match component_type {
        gltf::accessor::ComponentType::I8 => DataType::Int8,
        gltf::accessor::ComponentType::U8 => DataType::Uint8,
        gltf::accessor::ComponentType::I16 => DataType::Int16,
        gltf::accessor::ComponentType::U16 => DataType::Uint16,
        gltf::accessor::ComponentType::U32 => DataType::Uint32,
        gltf::accessor::ComponentType::F32 => DataType::Float32,
    }
}

fn gltf_attribute_to_draco_attribute(attribute_name: &str) -> GeometryAttributeType {
    match attribute_name {
        "POSITION" => GeometryAttributeType::Position,
        "NORMAL" => GeometryAttributeType::Normal,
        "TEXCOORD_0" => GeometryAttributeType::TexCoord,
        "TEXCOORD_1" => GeometryAttributeType::TexCoord,
        "TANGENT" => GeometryAttributeType::Tangent,
        "COLOR_0" => GeometryAttributeType::Color,
        "JOINTS_0" => GeometryAttributeType::Joints,
        "WEIGHTS_0" => GeometryAttributeType::Weights,
        _ if attribute_name.starts_with("_FEATURE_ID_") => GeometryAttributeType::Generic,
        _ if attribute_name.starts_with('_') => GeometryAttributeType::Generic,
        _ => GeometryAttributeType::Invalid,
    }
}

fn checked_component_type(
    accessor: &gltf::Accessor,
) -> Result<gltf::accessor::ComponentType, Status> {
    match accessor.component_type {
        Checked::Valid(gltf::accessor::GenericComponentType(component_type)) => Ok(component_type),
        Checked::Invalid => Err(Status::new(
            StatusCode::DracoError,
            "Invalid accessor component type.",
        )),
    }
}

fn checked_accessor_type(accessor: &gltf::Accessor) -> Result<gltf::accessor::Type, Status> {
    match accessor.type_ {
        Checked::Valid(accessor_type) => Ok(accessor_type),
        Checked::Invalid => Err(Status::new(
            StatusCode::DracoError,
            "Invalid accessor type.",
        )),
    }
}

fn copy_data_from_buffer_view(
    model: &GltfModel,
    buffer_view_index: i32,
    data: &mut Vec<u8>,
) -> Result<(), Status> {
    if buffer_view_index < 0 {
        return Err(Status::new(
            StatusCode::DracoError,
            "Error CopyDataFromBufferView() bufferView < 0.",
        ));
    }
    let buffer_view_index = buffer_view_index as usize;
    if buffer_view_index >= model.json.buffer_views.len() {
        return Err(Status::new(
            StatusCode::DracoError,
            "Error CopyDataFromBufferView() bufferView out of range.",
        ));
    }
    let buffer_view = &model.json.buffer_views[buffer_view_index];
    if buffer_view.byte_stride.is_some() {
        return Err(Status::new(
            StatusCode::DracoError,
            "Error buffer view byteStride != 0.",
        ));
    }
    let buffer = model
        .buffers
        .get(buffer_view.buffer.value())
        .ok_or_else(|| {
            Status::new(
                StatusCode::DracoError,
                "Error CopyDataFromBufferView() buffer < 0.",
            )
        })?;

    let view_offset = buffer_view.byte_offset.map(|v| v.0).unwrap_or(0) as usize;
    let byte_length = buffer_view.byte_length.0 as usize;
    let start = view_offset;
    let end = start + byte_length;
    if end > buffer.len() {
        return Err(Status::new(
            StatusCode::DracoError,
            "Buffer view out of range.",
        ));
    }
    data.clear();
    data.extend_from_slice(&buffer[start..end]);
    Ok(())
}

fn accessor_raw_bytes(model: &GltfModel, accessor: &gltf::Accessor) -> Result<AccessorRaw, Status> {
    let component_type = checked_component_type(accessor)?;
    let accessor_type = checked_accessor_type(accessor)?;
    let buffer_view_index = accessor
        .buffer_view
        .as_ref()
        .ok_or_else(|| Status::new(StatusCode::DracoError, "Error CopyDataAs() bufferView < 0."))?;
    let buffer_view_index = buffer_view_index.value();
    if buffer_view_index >= model.json.buffer_views.len() {
        return Err(Status::new(
            StatusCode::DracoError,
            "Error CopyDataAs() bufferView out of range.",
        ));
    }
    let buffer_view = &model.json.buffer_views[buffer_view_index];
    let buffer = model
        .buffers
        .get(buffer_view.buffer.value())
        .ok_or_else(|| Status::new(StatusCode::DracoError, "Error CopyDataAs() buffer < 0."))?;

    let component_size = component_size_bytes(component_type);
    let num_components = TinyGltfUtils::get_num_components_for_type(accessor_type) as usize;
    let element_size = component_size * num_components;
    if element_size == 0 {
        return Err(Status::new(
            StatusCode::DracoError,
            "Accessor element size is zero.",
        ));
    }

    let view_offset = buffer_view.byte_offset.map(|v| v.0).unwrap_or(0) as usize;
    let accessor_offset = accessor.byte_offset.map(|v| v.0).unwrap_or(0) as usize;
    let byte_offset = view_offset + accessor_offset;
    let stride_default = element_size;
    let byte_stride = buffer_view
        .byte_stride
        .map(|stride| stride.0 as usize)
        .unwrap_or(stride_default);
    if byte_stride < element_size {
        return Err(Status::new(
            StatusCode::DracoError,
            "Accessor stride smaller than element size.",
        ));
    }

    let count = usize::try_from(accessor.count.0)
        .map_err(|_| Status::new(StatusCode::DracoError, "Accessor count exceeds usize."))?;
    let mut out = vec![0u8; count * element_size];
    let mut offset = byte_offset;
    for i in 0..count {
        let start = offset;
        let end = start + element_size;
        if end > buffer.len() {
            return Err(Status::new(
                StatusCode::DracoError,
                "Accessor data out of range.",
            ));
        }
        let dst_start = i * element_size;
        out[dst_start..dst_start + element_size].copy_from_slice(&buffer[start..end]);
        offset += byte_stride;
    }

    Ok(AccessorRaw {
        data: out,
        element_size,
    })
}

fn copy_data_as_uint32(model: &GltfModel, accessor: &gltf::Accessor) -> StatusOr<Vec<u32>> {
    let component_type = match checked_component_type(accessor) {
        Ok(component_type) => component_type,
        Err(status) => return StatusOr::new_status(status),
    };
    match component_type {
        gltf::accessor::ComponentType::U8
        | gltf::accessor::ComponentType::U16
        | gltf::accessor::ComponentType::U32 => {}
        _ => {
            return StatusOr::new_status(Status::new(
                StatusCode::DracoError,
                "Accessor data cannot be converted to Uint32.",
            ))
        }
    }
    let raw = match accessor_raw_bytes(model, accessor) {
        Ok(raw) => raw,
        Err(status) => return StatusOr::new_status(status),
    };
    let num_components = match checked_accessor_type(accessor) {
        Ok(accessor_type) => TinyGltfUtils::get_num_components_for_type(accessor_type),
        Err(status) => return StatusOr::new_status(status),
    };
    let count = usize::try_from(accessor.count.0)
        .map_err(|_| Status::new(StatusCode::DracoError, "Accessor count exceeds usize."));
    let count = match count {
        Ok(v) => v,
        Err(status) => return StatusOr::new_status(status),
    };
    let num_elements = count * num_components as usize;
    let mut output = Vec::with_capacity(num_elements);

    let component_size = component_size_bytes(component_type);
    for i in 0..num_elements {
        let start = i * component_size;
        let end = start + component_size;
        if end > raw.data.len() {
            return StatusOr::new_status(Status::new(
                StatusCode::DracoError,
                "Accessor data out of range.",
            ));
        }
        let value = match component_type {
            gltf::accessor::ComponentType::U8 => raw.data[start] as u32,
            gltf::accessor::ComponentType::U16 => {
                let bytes = [raw.data[start], raw.data[start + 1]];
                u16::from_le_bytes(bytes) as u32
            }
            gltf::accessor::ComponentType::U32 => {
                let bytes = [
                    raw.data[start],
                    raw.data[start + 1],
                    raw.data[start + 2],
                    raw.data[start + 3],
                ];
                u32::from_le_bytes(bytes)
            }
            _ => 0,
        };
        output.push(value);
    }
    StatusOr::new_value(output)
}

fn copy_data_as_float_checked<T: FloatPack>(
    model: &GltfModel,
    accessor: &gltf::Accessor,
) -> Result<Vec<T>, Status> {
    let data_or = TinyGltfUtils::copy_data_as_float::<T>(model, accessor);
    if !data_or.is_ok() {
        return Err(data_or.status().clone());
    }
    Ok(data_or.into_value())
}

fn component_size_bytes(component_type: gltf::accessor::ComponentType) -> usize {
    match component_type {
        gltf::accessor::ComponentType::I8 => 1,
        gltf::accessor::ComponentType::U8 => 1,
        gltf::accessor::ComponentType::I16 => 2,
        gltf::accessor::ComponentType::U16 => 2,
        gltf::accessor::ComponentType::U32 => 4,
        gltf::accessor::ComponentType::F32 => 4,
    }
}

fn get_node_trs_matrix(node: &gltf::scene::Node) -> TrsMatrix {
    let mut trsm = TrsMatrix::new();
    if let Some(matrix) = node.matrix.as_ref() {
        if matrix.len() == 16 {
            let mut transformation = Matrix4d::identity();
            transformation.m = [
                [
                    matrix[0] as f64,
                    matrix[4] as f64,
                    matrix[8] as f64,
                    matrix[12] as f64,
                ],
                [
                    matrix[1] as f64,
                    matrix[5] as f64,
                    matrix[9] as f64,
                    matrix[13] as f64,
                ],
                [
                    matrix[2] as f64,
                    matrix[6] as f64,
                    matrix[10] as f64,
                    matrix[14] as f64,
                ],
                [
                    matrix[3] as f64,
                    matrix[7] as f64,
                    matrix[11] as f64,
                    matrix[15] as f64,
                ],
            ];
            if transformation != Matrix4d::identity() {
                trsm.set_matrix(transformation);
            }
        }
    }

    if let Some(translation) = node.translation.as_ref() {
        if translation.len() == 3 {
            let default_translation = [0.0f64, 0.0f64, 0.0f64];
            let node_translation = [
                translation[0] as f64,
                translation[1] as f64,
                translation[2] as f64,
            ];
            if node_translation != default_translation {
                trsm.set_translation(Vector3d::new(
                    node_translation[0],
                    node_translation[1],
                    node_translation[2],
                ));
            }
        }
    }

    if let Some(scale) = node.scale.as_ref() {
        if scale.len() == 3 {
            let default_scale = [1.0f64, 1.0f64, 1.0f64];
            let node_scale = [scale[0] as f64, scale[1] as f64, scale[2] as f64];
            if node_scale != default_scale {
                trsm.set_scale(Vector3d::new(node_scale[0], node_scale[1], node_scale[2]));
            }
        }
    }

    if let Some(rotation) = node.rotation.as_ref() {
        if rotation.0.len() == 4 {
            let default_rotation = [0.0f64, 0.0f64, 0.0f64, 1.0f64];
            let node_rotation = [
                rotation.0[0] as f64,
                rotation.0[1] as f64,
                rotation.0[2] as f64,
                rotation.0[3] as f64,
            ];
            if node_rotation != default_rotation {
                trsm.set_rotation(Quaterniond::new(
                    node_rotation[3],
                    node_rotation[0],
                    node_rotation[1],
                    node_rotation[2],
                ));
            }
        }
    }

    trsm
}

fn update_matrix_for_normals(transform_matrix: &Matrix4d) -> Matrix4d {
    let mat3 = transform_matrix.block_3x3().inverse().transpose();
    let mut mat4 = Matrix4d::identity();
    mat4.set_block_3x3(mat3);
    mat4
}

fn determinant(transform_matrix: &Matrix4d) -> f64 {
    let m = transform_matrix.block_3x3().m;
    m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
}

fn normalize_vec3(v: &mut [f64; 3]) {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len != 0.0 {
        v[0] /= len;
        v[1] /= len;
        v[2] /= len;
    }
}

fn round_f32_to_f64(value: f32) -> f64 {
    let v = value as f64;
    (v * 1_000_000.0).round() / 1_000_000.0
}

fn semantic_to_string(semantic: &Checked<gltf::mesh::Semantic>) -> String {
    match semantic {
        Checked::Valid(semantic) => match semantic {
            gltf::mesh::Semantic::Positions => "POSITION".to_string(),
            gltf::mesh::Semantic::Normals => "NORMAL".to_string(),
            gltf::mesh::Semantic::Tangents => "TANGENT".to_string(),
            gltf::mesh::Semantic::TexCoords(index) => format!("TEXCOORD_{}", index),
            gltf::mesh::Semantic::Colors(index) => format!("COLOR_{}", index),
            gltf::mesh::Semantic::Joints(index) => format!("JOINTS_{}", index),
            gltf::mesh::Semantic::Weights(index) => format!("WEIGHTS_{}", index),
            gltf::mesh::Semantic::Extras(name) => format!("_{}", name),
        },
        Checked::Invalid => "".to_string(),
    }
}

fn mode_to_gl_enum(mode: &Checked<gltf::mesh::Mode>) -> u32 {
    match mode {
        Checked::Valid(mode) => mode.as_gl_enum(),
        Checked::Invalid => 0,
    }
}

fn tiny_gltf_to_draco_axis_wrapping_mode(
    mode: &Checked<gltf::texture::WrappingMode>,
) -> Result<TextureMapAxisWrappingMode, Status> {
    match mode {
        Checked::Valid(gltf::texture::WrappingMode::ClampToEdge) => {
            Ok(TextureMapAxisWrappingMode::ClampToEdge)
        }
        Checked::Valid(gltf::texture::WrappingMode::MirroredRepeat) => {
            Ok(TextureMapAxisWrappingMode::MirroredRepeat)
        }
        Checked::Valid(gltf::texture::WrappingMode::Repeat) => {
            Ok(TextureMapAxisWrappingMode::Repeat)
        }
        Checked::Invalid => Err(Status::new(
            StatusCode::UnsupportedFeature,
            "Unsupported wrapping mode.",
        )),
    }
}

fn tiny_gltf_to_draco_min_filter_type(
    filter: Option<&Checked<gltf::texture::MinFilter>>,
) -> Result<TextureMapFilterType, Status> {
    match filter {
        None => Ok(TextureMapFilterType::Unspecified),
        Some(Checked::Valid(gltf::texture::MinFilter::Nearest)) => {
            Ok(TextureMapFilterType::Nearest)
        }
        Some(Checked::Valid(gltf::texture::MinFilter::Linear)) => Ok(TextureMapFilterType::Linear),
        Some(Checked::Valid(gltf::texture::MinFilter::NearestMipmapNearest)) => {
            Ok(TextureMapFilterType::NearestMipmapNearest)
        }
        Some(Checked::Valid(gltf::texture::MinFilter::LinearMipmapNearest)) => {
            Ok(TextureMapFilterType::LinearMipmapNearest)
        }
        Some(Checked::Valid(gltf::texture::MinFilter::NearestMipmapLinear)) => {
            Ok(TextureMapFilterType::NearestMipmapLinear)
        }
        Some(Checked::Valid(gltf::texture::MinFilter::LinearMipmapLinear)) => {
            Ok(TextureMapFilterType::LinearMipmapLinear)
        }
        Some(Checked::Invalid) => Err(Status::new(
            StatusCode::DracoError,
            "Unsupported texture filter type.",
        )),
    }
}

fn tiny_gltf_to_draco_mag_filter_type(
    filter: Option<&Checked<gltf::texture::MagFilter>>,
) -> Result<TextureMapFilterType, Status> {
    match filter {
        None => Ok(TextureMapFilterType::Unspecified),
        Some(Checked::Valid(gltf::texture::MagFilter::Nearest)) => {
            Ok(TextureMapFilterType::Nearest)
        }
        Some(Checked::Valid(gltf::texture::MagFilter::Linear)) => Ok(TextureMapFilterType::Linear),
        Some(Checked::Invalid) => Err(Status::new(
            StatusCode::DracoError,
            "Unsupported texture filter type.",
        )),
    }
}

fn check_khr_texture_transform(
    ext: Option<&gltf::extensions::texture::Info>,
    transform: &mut TextureTransform,
) -> Result<bool, Status> {
    let ext = match ext.and_then(|e| e.texture_transform.as_ref()) {
        Some(ext) => ext,
        None => return Ok(false),
    };

    let mut transform_set = false;
    let scale = ext.scale.0;
    if scale != [1.0, 1.0] {
        transform.set_scale([scale[0] as f64, scale[1] as f64]);
        transform_set = true;
    }
    if ext.rotation.0 != 0.0 {
        transform.set_rotation(ext.rotation.0 as f64);
        transform_set = true;
    }
    let offset = ext.offset.0;
    if offset != [0.0, 0.0] {
        transform.set_offset([offset[0] as f64, offset[1] as f64]);
        transform_set = true;
    }
    if let Some(tex_coord) = ext.tex_coord {
        transform.set_tex_coord(tex_coord as i32);
        transform_set = true;
    }
    Ok(transform_set)
}

fn decode_float(name: &str, object: &Map<String, Value>) -> Result<Option<f32>, Status> {
    match object.get(name) {
        Some(value) => {
            if value.is_null() {
                return Ok(None);
            }
            value
                .as_f64()
                .map(|v| Some(v as f32))
                .ok_or_else(|| Status::new(StatusCode::DracoError, &format!("Invalid {}.", name)))
        }
        None => Ok(None),
    }
}

fn decode_int(name: &str, object: &Map<String, Value>) -> Result<Option<i32>, Status> {
    match object.get(name) {
        Some(value) => value
            .as_i64()
            .map(|v| Some(v as i32))
            .ok_or_else(|| Status::new(StatusCode::DracoError, &format!("Invalid {}.", name))),
        None => Ok(None),
    }
}

fn decode_string(name: &str, object: &Map<String, Value>) -> Result<Option<String>, Status> {
    match object.get(name) {
        Some(value) => value
            .as_str()
            .map(|v| Some(v.to_string()))
            .ok_or_else(|| Status::new(StatusCode::DracoError, &format!("Invalid {}.", name))),
        None => Ok(None),
    }
}

fn decode_vector3f(name: &str, object: &Map<String, Value>) -> Result<Option<Vector3f>, Status> {
    match object.get(name) {
        Some(value) => {
            let array = value.as_array().ok_or_else(|| {
                Status::new(StatusCode::DracoError, &format!("Invalid {}.", name))
            })?;
            if array.len() != 3 {
                return Err(Status::new(
                    StatusCode::DracoError,
                    &format!("Invalid {}.", name),
                ));
            }
            let mut out = Vector3f::new3(0.0, 0.0, 0.0);
            for (i, entry) in array.iter().enumerate() {
                let v = entry.as_f64().ok_or_else(|| {
                    Status::new(StatusCode::DracoError, &format!("Invalid {}.", name))
                })?;
                out[i] = v as f32;
            }
            Ok(Some(out))
        }
        None => Ok(None),
    }
}

fn parse_texture_info(
    texture_name: &str,
    container_object: &Map<String, Value>,
) -> Result<Option<ParsedTextureInfo>, Status> {
    let texture_object = if let Some(value) = container_object.get(texture_name) {
        value
            .as_object()
            .ok_or_else(|| Status::new(StatusCode::DracoError, "Invalid texture object."))?
    } else if container_object.contains_key("index")
        || container_object.contains_key("texCoord")
        || container_object.contains_key("extensions")
    {
        container_object
    } else {
        return Ok(None);
    };

    let mut index = -1;
    if let Some(value) = texture_object.get("index") {
        let num = value
            .as_i64()
            .ok_or_else(|| Status::new(StatusCode::DracoError, "Invalid texture index."))?;
        index = num as i32;
    }

    let mut tex_coord = 0;
    if let Some(value) = texture_object.get("texCoord") {
        let num = value
            .as_i64()
            .ok_or_else(|| Status::new(StatusCode::DracoError, "Invalid texture texCoord."))?;
        tex_coord = num as i32;
    }

    let extensions = if let Some(value) = texture_object.get("extensions") {
        let obj = value
            .as_object()
            .ok_or_else(|| Status::new(StatusCode::DracoError, "Invalid extension."))?;
        let info: gltf::extensions::texture::Info =
            serde_json::from_value(Value::Object(obj.clone()))
                .map_err(|_| Status::new(StatusCode::DracoError, "Invalid extension."))?;
        Some(info)
    } else {
        None
    };

    Ok(Some(ParsedTextureInfo {
        index,
        tex_coord,
        extensions,
    }))
}

fn parse_texture_extensions_from_material_map(
    map: &Map<String, Value>,
) -> Result<Option<gltf::extensions::texture::Info>, Status> {
    if map.is_empty() {
        return Ok(None);
    }
    let info: gltf::extensions::texture::Info = serde_json::from_value(Value::Object(map.clone()))
        .map_err(|_| Status::new(StatusCode::DracoError, "Invalid texture extension."))?;
    Ok(Some(info))
}

fn decode_property_table_data(
    name: &str,
    object: &Map<String, Value>,
    model: &GltfModel,
    data: &mut PropertyTableData,
) -> Result<bool, Status> {
    let buffer_view_index = match decode_int(name, object)? {
        Some(index) => index,
        None => return Ok(false),
    };
    copy_data_from_buffer_view(model, buffer_view_index, &mut data.data)?;
    let buffer_view = &model.json.buffer_views[buffer_view_index as usize];
    let target = match buffer_view.target {
        Some(Checked::Valid(gltf::buffer::Target::ArrayBuffer)) => {
            gltf::buffer::ARRAY_BUFFER as i32
        }
        Some(Checked::Valid(gltf::buffer::Target::ElementArrayBuffer)) => {
            gltf::buffer::ELEMENT_ARRAY_BUFFER as i32
        }
        _ => 0,
    };
    data.target = target;
    Ok(true)
}

fn extras_to_value(extras: &gltf::Extras) -> Option<Value> {
    extras
        .as_ref()
        .and_then(|raw| serde_json::from_str(raw.get()).ok())
}

fn is_data_uri(uri: &str) -> bool {
    uri.starts_with("data:")
}

fn decode_data_uri(uri: &str) -> Result<(Vec<u8>, Option<String>), Status> {
    let data = uri
        .strip_prefix("data:")
        .ok_or_else(|| Status::new(StatusCode::DracoError, "Invalid data URI."))?;
    let mut parts = data.splitn(2, ',');
    let header = parts.next().unwrap_or("");
    let payload = parts.next().unwrap_or("");
    let is_base64 = header.contains(";base64");
    let mime = header
        .split(';')
        .next()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    if !is_base64 {
        return Err(Status::new(
            StatusCode::DracoError,
            "Non-base64 data URIs are unsupported.",
        ));
    }
    let decoded = STANDARD
        .decode(payload)
        .map_err(|_| Status::new(StatusCode::DracoError, "Failed to decode data URI."))?;
    Ok((decoded, mime))
}

fn white_color_bytes(data_type: DataType, num_components: usize) -> Vec<u8> {
    let element_size = data_type_length(data_type);
    if element_size <= 0 {
        return Vec::new();
    }
    let element_size = element_size as usize;
    let mut out = vec![0u8; num_components * element_size];
    match data_type {
        DataType::Float32 => {
            let value = 1.0f32.to_le_bytes();
            for i in 0..num_components {
                let start = i * 4;
                out[start..start + 4].copy_from_slice(&value);
            }
        }
        DataType::Uint8 => {
            for i in 0..num_components {
                out[i] = 1u8;
            }
        }
        DataType::Uint16 => {
            let value = 1u16.to_le_bytes();
            for i in 0..num_components {
                let start = i * 2;
                out[start..start + 2].copy_from_slice(&value);
            }
        }
        _ => {}
    }
    out
}

fn get_index_from_feature_id_attribute_name(name: &str) -> Result<usize, Status> {
    let prefix = "_FEATURE_ID_";
    if !name.starts_with(prefix) {
        return Err(Status::new(
            StatusCode::DracoError,
            "Invalid feature ID name.",
        ));
    }
    let suffix = &name[prefix.len()..];
    suffix
        .parse::<usize>()
        .map_err(|_| Status::new(StatusCode::DracoError, "Invalid feature ID attribute name."))
}

// Accessor raw storage
struct AccessorRaw {
    data: Vec<u8>,
    element_size: usize,
}

fn set_values_for_mesh<const N: usize>(
    builder: &mut TriangleSoupMeshBuilder,
    indices_data: &[u32],
    att_id: i32,
    number_of_faces: i32,
    data: &[[f32; N]],
    reverse_winding: bool,
    face_offset: i32,
) {
    for f in 0..number_of_faces {
        let base_corner = (f * 3) as usize;
        let next_offset = if reverse_winding { 2 } else { 1 };
        let prev_offset = if reverse_winding { 1 } else { 2 };
        let v_id = indices_data[base_corner] as usize;
        let v_next = indices_data[base_corner + next_offset] as usize;
        let v_prev = indices_data[base_corner + prev_offset] as usize;
        let face_index = FaceIndex::from((f + face_offset) as u32);
        builder.set_attribute_values_for_face(
            att_id,
            face_index,
            &data[v_id],
            &data[v_next],
            &data[v_prev],
        );
    }
}

fn set_values_for_point<const N: usize>(
    builder: &mut PointCloudBuilder,
    indices_data: &[u32],
    att_id: i32,
    number_of_points: i32,
    data: &[[f32; N]],
    point_offset: i32,
) {
    let mut buf = vec![0u8; N * 4];
    for i in 0..number_of_points {
        let v_id = indices_data[i as usize] as usize;
        let point_index = PointIndex::from((i + point_offset) as u32);
        for (j, f) in data[v_id].iter().enumerate() {
            buf[j * 4..(j + 1) * 4].copy_from_slice(&f.to_le_bytes());
        }
        builder.set_attribute_value_for_point(att_id, point_index, &buf);
    }
}

fn set_values_for_mesh_bytes(
    builder: &mut TriangleSoupMeshBuilder,
    indices_data: &[u32],
    att_id: i32,
    number_of_faces: i32,
    data: &AccessorRaw,
    reverse_winding: bool,
    face_offset: i32,
) {
    for f in 0..number_of_faces {
        let base_corner = (f * 3) as usize;
        let next_offset = if reverse_winding { 2 } else { 1 };
        let prev_offset = if reverse_winding { 1 } else { 2 };
        let v_id = indices_data[base_corner] as usize;
        let v_next = indices_data[base_corner + next_offset] as usize;
        let v_prev = indices_data[base_corner + prev_offset] as usize;
        let face_index = FaceIndex::from((f + face_offset) as u32);
        builder.set_attribute_values_for_face_bytes(
            att_id,
            face_index,
            data.element(v_id),
            data.element(v_next),
            data.element(v_prev),
        );
    }
}

fn set_values_for_point_bytes(
    builder: &mut PointCloudBuilder,
    indices_data: &[u32],
    att_id: i32,
    number_of_points: i32,
    data: &AccessorRaw,
    point_offset: i32,
) {
    for i in 0..number_of_points {
        let v_id = indices_data[i as usize] as usize;
        let point_index = PointIndex::from((i + point_offset) as u32);
        builder.set_attribute_value_for_point(att_id, point_index, data.element(v_id));
    }
}

impl AccessorRaw {
    fn element(&self, index: usize) -> &[u8] {
        let start = index * self.element_size;
        &self.data[start..start + self.element_size]
    }
}

// --- Draco KHR_mesh_compression decode ---

enum DracoGeometry {
    Mesh(Box<Mesh>),
    PointCloud(Box<PointCloud>),
}

struct DecodedDracoPrimitive {
    geometry: DracoGeometry,
    attribute_unique_ids: HashMap<String, u32>,
    indices_data: Vec<u32>,
    number_of_faces: i32,
    number_of_points: i32,
}

fn get_draco_extension(primitive: &gltf::mesh::Primitive) -> Option<Map<String, Value>> {
    primitive
        .extensions
        .as_ref()
        .and_then(|ext| ext.others.get(KHR_DRACO_MESH_COMPRESSION))
        .and_then(|value| value.as_object())
        .cloned()
}

fn decode_draco_primitive(
    model: &GltfModel,
    extension: &Map<String, Value>,
) -> Result<DecodedDracoPrimitive, Status> {
    let buffer_view_index = extension
        .get("bufferView")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| Status::new(StatusCode::DracoError, "KHR_draco bufferView is malformed."))?
        as i32;

    let attributes_value = extension
        .get("attributes")
        .and_then(|v| v.as_object())
        .ok_or_else(|| Status::new(StatusCode::DracoError, "KHR_draco attributes is malformed."))?;

    let mut attribute_unique_ids = HashMap::new();
    for (key, value) in attributes_value {
        let id = value.as_i64().ok_or_else(|| {
            Status::new(
                StatusCode::DracoError,
                "KHR_draco attribute index is malformed.",
            )
        })?;
        attribute_unique_ids.insert(key.clone(), id as u32);
    }

    let mut data = Vec::new();
    copy_data_from_buffer_view(model, buffer_view_index, &mut data)?;
    let mut decoder_buffer = DecoderBuffer::new();
    decoder_buffer.init(&data);
    let mut decoder = Decoder::new();
    let decoded_or = decoder.decode_point_cloud_from_buffer(&mut decoder_buffer);
    if !decoded_or.is_ok() {
        return Err(decoded_or.status().clone());
    }
    let decoded = decoded_or.into_value();

    match decoded {
        draco_bitstream::compression::decode::DecodedPointCloud::Mesh(mesh) => {
            let mut indices_data = Vec::with_capacity((mesh.num_faces() as usize) * 3);
            for f in 0..mesh.num_faces() {
                let face = mesh.face(FaceIndex::from(f));
                indices_data.push(face[0].value());
                indices_data.push(face[1].value());
                indices_data.push(face[2].value());
            }
            let number_of_faces = mesh.num_faces() as i32;
            let number_of_points = indices_data.len() as i32;
            Ok(DecodedDracoPrimitive {
                geometry: DracoGeometry::Mesh(mesh),
                attribute_unique_ids,
                indices_data,
                number_of_faces,
                number_of_points,
            })
        }
        draco_bitstream::compression::decode::DecodedPointCloud::PointCloud(pc) => {
            let number_of_points = pc.num_points() as i32;
            let mut indices_data = Vec::with_capacity(number_of_points as usize);
            for i in 0..number_of_points {
                indices_data.push(i as u32);
            }
            Ok(DecodedDracoPrimitive {
                geometry: DracoGeometry::PointCloud(pc),
                attribute_unique_ids,
                indices_data,
                number_of_faces: 0,
                number_of_points,
            })
        }
    }
}

fn find_attribute_by_unique_id_mesh(
    mesh: &Mesh,
    unique_id: u32,
) -> Result<&PointAttribute, Status> {
    for att_id in 0..mesh.num_attributes() {
        if let Some(att) = mesh.attribute(att_id) {
            if att.unique_id() == unique_id {
                return Ok(att);
            }
        }
    }
    Err(Status::new(
        StatusCode::DracoError,
        "Draco attribute not found.",
    ))
}

fn find_attribute_by_unique_id_point(
    pc: &PointCloud,
    unique_id: u32,
) -> Result<&PointAttribute, Status> {
    for att_id in 0..pc.num_attributes() {
        if let Some(att) = pc.attribute(att_id) {
            if att.unique_id() == unique_id {
                return Ok(att);
            }
        }
    }
    Err(Status::new(
        StatusCode::DracoError,
        "Draco attribute not found.",
    ))
}

fn add_attribute_values_from_draco_mesh_builder(
    builder: &mut TriangleSoupMeshBuilder,
    attribute_name: &str,
    accessor: &gltf::Accessor,
    indices_data: &[u32],
    att_id: i32,
    number_of_faces: i32,
    mesh: &Mesh,
    attribute: &PointAttribute,
) -> Result<(), Status> {
    let reverse_winding = false;
    if attribute_name == "TEXCOORD_0" || attribute_name == "TEXCOORD_1" {
        return add_tex_coord_from_draco_mesh_builder(
            builder,
            accessor,
            indices_data,
            att_id,
            number_of_faces,
            reverse_winding,
            mesh,
            attribute,
        );
    }
    if attribute_name == "TANGENT" {
        return add_tangent_from_draco_mesh_builder(
            builder,
            accessor,
            indices_data,
            att_id,
            number_of_faces,
            reverse_winding,
            mesh,
            attribute,
        );
    }
    if attribute_name == "POSITION" || attribute_name == "NORMAL" {
        let normalize = attribute_name == "NORMAL";
        return add_transformed_from_draco_mesh_builder(
            builder,
            accessor,
            indices_data,
            att_id,
            number_of_faces,
            normalize,
            reverse_winding,
            mesh,
            attribute,
        );
    }
    if attribute_name.starts_with("_FEATURE_ID_") {
        check_feature_id_accessor_basic(accessor)?;
        return add_attribute_data_from_draco_mesh_builder(
            builder,
            accessor,
            indices_data,
            att_id,
            number_of_faces,
            reverse_winding,
            mesh,
            attribute,
        );
    }
    if attribute_name.starts_with('_') {
        add_attribute_data_from_draco_mesh_builder(
            builder,
            accessor,
            indices_data,
            att_id,
            number_of_faces,
            reverse_winding,
            mesh,
            attribute,
        )?;
        builder.set_attribute_name(att_id, attribute_name);
        return Ok(());
    }
    add_attribute_data_from_draco_mesh_builder(
        builder,
        accessor,
        indices_data,
        att_id,
        number_of_faces,
        reverse_winding,
        mesh,
        attribute,
    )
}

fn add_attribute_values_from_draco_point_builder(
    builder: &mut PointCloudBuilder,
    attribute_name: &str,
    accessor: &gltf::Accessor,
    indices_data: &[u32],
    att_id: i32,
    number_of_points: i32,
    pc: &PointCloud,
    attribute: &PointAttribute,
) -> Result<(), Status> {
    let reverse_winding = false;
    if attribute_name == "TEXCOORD_0" || attribute_name == "TEXCOORD_1" {
        return add_tex_coord_from_draco_point_builder(
            builder,
            accessor,
            indices_data,
            att_id,
            number_of_points,
            reverse_winding,
            pc,
            attribute,
        );
    }
    if attribute_name == "TANGENT" {
        return add_tangent_from_draco_point_builder(
            builder,
            accessor,
            indices_data,
            att_id,
            number_of_points,
            reverse_winding,
            pc,
            attribute,
        );
    }
    if attribute_name == "POSITION" || attribute_name == "NORMAL" {
        let normalize = attribute_name == "NORMAL";
        return add_transformed_from_draco_point_builder(
            builder,
            accessor,
            indices_data,
            att_id,
            number_of_points,
            normalize,
            reverse_winding,
            pc,
            attribute,
        );
    }
    if attribute_name.starts_with("_FEATURE_ID_") {
        check_feature_id_accessor_basic(accessor)?;
        return add_attribute_data_from_draco_point_builder(
            builder,
            accessor,
            indices_data,
            att_id,
            number_of_points,
            reverse_winding,
            pc,
            attribute,
        );
    }
    if attribute_name.starts_with('_') {
        add_attribute_data_from_draco_point_builder(
            builder,
            accessor,
            indices_data,
            att_id,
            number_of_points,
            reverse_winding,
            pc,
            attribute,
        )?;
        builder.set_attribute_name(att_id, attribute_name);
        return Ok(());
    }
    add_attribute_data_from_draco_point_builder(
        builder,
        accessor,
        indices_data,
        att_id,
        number_of_points,
        reverse_winding,
        pc,
        attribute,
    )
}

fn add_attribute_values_from_accessor_mesh_builder(
    model: &GltfModel,
    builder: &mut TriangleSoupMeshBuilder,
    attribute_name: &str,
    accessor: &gltf::Accessor,
    indices_data: &[u32],
    att_id: i32,
    number_of_faces: i32,
) -> Result<(), Status> {
    let reverse_winding = false;
    if attribute_name.starts_with("_FEATURE_ID_") {
        check_feature_id_accessor_basic(accessor)?;
    }
    if attribute_name == "TEXCOORD_0" || attribute_name == "TEXCOORD_1" {
        let mut data = copy_data_as_float_checked::<[f32; 2]>(model, accessor)?;
        for uv in &mut data {
            uv[1] = 1.0 - uv[1];
        }
        set_values_for_mesh(
            builder,
            indices_data,
            att_id,
            number_of_faces,
            &data,
            reverse_winding,
            0,
        );
        return Ok(());
    }
    if attribute_name == "TANGENT" {
        let data = copy_data_as_float_checked::<[f32; 4]>(model, accessor)?;
        set_values_for_mesh(
            builder,
            indices_data,
            att_id,
            number_of_faces,
            &data,
            reverse_winding,
            0,
        );
        return Ok(());
    }
    if attribute_name == "POSITION" || attribute_name == "NORMAL" {
        let data = copy_data_as_float_checked::<[f32; 3]>(model, accessor)?;
        set_values_for_mesh(
            builder,
            indices_data,
            att_id,
            number_of_faces,
            &data,
            reverse_winding,
            0,
        );
        return Ok(());
    }
    let raw = accessor_raw_bytes(model, accessor)?;
    set_values_for_mesh_bytes(
        builder,
        indices_data,
        att_id,
        number_of_faces,
        &raw,
        reverse_winding,
        0,
    );
    if attribute_name.starts_with('_') {
        builder.set_attribute_name(att_id, attribute_name);
    }
    Ok(())
}

fn add_attribute_values_from_accessor_point_builder(
    model: &GltfModel,
    builder: &mut PointCloudBuilder,
    attribute_name: &str,
    accessor: &gltf::Accessor,
    indices_data: &[u32],
    att_id: i32,
    number_of_points: i32,
) -> Result<(), Status> {
    if attribute_name.starts_with("_FEATURE_ID_") {
        check_feature_id_accessor_basic(accessor)?;
    }
    if attribute_name == "TEXCOORD_0" || attribute_name == "TEXCOORD_1" {
        let mut data = copy_data_as_float_checked::<[f32; 2]>(model, accessor)?;
        for uv in &mut data {
            uv[1] = 1.0 - uv[1];
        }
        set_values_for_point(builder, indices_data, att_id, number_of_points, &data, 0);
        return Ok(());
    }
    if attribute_name == "TANGENT" {
        let data = copy_data_as_float_checked::<[f32; 4]>(model, accessor)?;
        set_values_for_point(builder, indices_data, att_id, number_of_points, &data, 0);
        return Ok(());
    }
    if attribute_name == "POSITION" || attribute_name == "NORMAL" {
        let data = copy_data_as_float_checked::<[f32; 3]>(model, accessor)?;
        set_values_for_point(builder, indices_data, att_id, number_of_points, &data, 0);
        return Ok(());
    }
    let raw = accessor_raw_bytes(model, accessor)?;
    set_values_for_point_bytes(builder, indices_data, att_id, number_of_points, &raw, 0);
    if attribute_name.starts_with('_') {
        builder.set_attribute_name(att_id, attribute_name);
    }
    Ok(())
}

fn check_feature_id_accessor_basic(accessor: &gltf::Accessor) -> Result<(), Status> {
    let num_components =
        TinyGltfUtils::get_num_components_for_type(checked_accessor_type(accessor)?);
    if num_components != 1 {
        return Err(Status::new(
            StatusCode::DracoError,
            "Invalid feature ID attribute type.",
        ));
    }
    let draco_component_type = gltf_component_type_to_draco_type(checked_component_type(accessor)?);
    if draco_component_type != DataType::Uint8
        && draco_component_type != DataType::Uint16
        && draco_component_type != DataType::Float32
    {
        return Err(Status::new(
            StatusCode::DracoError,
            "Invalid feature ID attribute component type.",
        ));
    }
    Ok(())
}

fn add_attribute_data_from_draco_mesh_builder(
    builder: &mut TriangleSoupMeshBuilder,
    _accessor: &gltf::Accessor,
    indices_data: &[u32],
    att_id: i32,
    number_of_faces: i32,
    reverse_winding: bool,
    mesh: &Mesh,
    attribute: &PointAttribute,
) -> Result<(), Status> {
    let raw = attribute_raw_bytes(mesh.num_points() as usize, attribute);
    set_values_for_mesh_bytes(
        builder,
        indices_data,
        att_id,
        number_of_faces,
        &raw,
        reverse_winding,
        0,
    );
    Ok(())
}

fn add_attribute_data_from_draco_point_builder(
    builder: &mut PointCloudBuilder,
    accessor: &gltf::Accessor,
    indices_data: &[u32],
    att_id: i32,
    number_of_points: i32,
    reverse_winding: bool,
    pc: &PointCloud,
    attribute: &PointAttribute,
) -> Result<(), Status> {
    let raw = attribute_raw_bytes(pc.num_points() as usize, attribute);
    let _ = accessor;
    let _ = reverse_winding;
    set_values_for_point_bytes(builder, indices_data, att_id, number_of_points, &raw, 0);
    Ok(())
}

fn add_tex_coord_from_draco_mesh_builder(
    builder: &mut TriangleSoupMeshBuilder,
    accessor: &gltf::Accessor,
    indices_data: &[u32],
    att_id: i32,
    number_of_faces: i32,
    reverse_winding: bool,
    mesh: &Mesh,
    attribute: &PointAttribute,
) -> Result<(), Status> {
    check_accessor_float_2(accessor, attribute)?;
    let mut data = attribute_vec2_float(mesh.num_points() as usize, attribute)?;
    for uv in &mut data {
        uv[1] = 1.0 - uv[1];
    }
    set_values_for_mesh(
        builder,
        indices_data,
        att_id,
        number_of_faces,
        &data,
        reverse_winding,
        0,
    );
    Ok(())
}

fn add_tex_coord_from_draco_point_builder(
    builder: &mut PointCloudBuilder,
    accessor: &gltf::Accessor,
    indices_data: &[u32],
    att_id: i32,
    number_of_points: i32,
    reverse_winding: bool,
    pc: &PointCloud,
    attribute: &PointAttribute,
) -> Result<(), Status> {
    check_accessor_float_2(accessor, attribute)?;
    let mut data = attribute_vec2_float(pc.num_points() as usize, attribute)?;
    for uv in &mut data {
        uv[1] = 1.0 - uv[1];
    }
    let _ = reverse_winding;
    set_values_for_point(builder, indices_data, att_id, number_of_points, &data, 0);
    Ok(())
}

fn add_tangent_from_draco_mesh_builder(
    builder: &mut TriangleSoupMeshBuilder,
    accessor: &gltf::Accessor,
    indices_data: &[u32],
    att_id: i32,
    number_of_faces: i32,
    reverse_winding: bool,
    mesh: &Mesh,
    attribute: &PointAttribute,
) -> Result<(), Status> {
    check_accessor_float_4(accessor, attribute)?;
    let data = attribute_vec4_float(mesh.num_points() as usize, attribute)?;
    set_values_for_mesh(
        builder,
        indices_data,
        att_id,
        number_of_faces,
        &data,
        reverse_winding,
        0,
    );
    Ok(())
}

fn add_tangent_from_draco_point_builder(
    builder: &mut PointCloudBuilder,
    accessor: &gltf::Accessor,
    indices_data: &[u32],
    att_id: i32,
    number_of_points: i32,
    reverse_winding: bool,
    pc: &PointCloud,
    attribute: &PointAttribute,
) -> Result<(), Status> {
    check_accessor_float_4(accessor, attribute)?;
    let data = attribute_vec4_float(pc.num_points() as usize, attribute)?;
    let _ = reverse_winding;
    set_values_for_point(builder, indices_data, att_id, number_of_points, &data, 0);
    Ok(())
}

fn add_transformed_from_draco_mesh_builder(
    builder: &mut TriangleSoupMeshBuilder,
    accessor: &gltf::Accessor,
    indices_data: &[u32],
    att_id: i32,
    number_of_faces: i32,
    normalize: bool,
    reverse_winding: bool,
    mesh: &Mesh,
    attribute: &PointAttribute,
) -> Result<(), Status> {
    check_accessor_float_3(accessor, attribute)?;
    let data = attribute_vec3_float(mesh.num_points() as usize, attribute)?;
    let mut transformed = Vec::with_capacity(data.len());
    for v in data {
        let mut vec = [v[0] as f64, v[1] as f64, v[2] as f64];
        if normalize {
            normalize_vec3(&mut vec);
        }
        transformed.push([vec[0] as f32, vec[1] as f32, vec[2] as f32]);
    }
    set_values_for_mesh(
        builder,
        indices_data,
        att_id,
        number_of_faces,
        &transformed,
        reverse_winding,
        0,
    );
    Ok(())
}

fn add_transformed_from_draco_point_builder(
    builder: &mut PointCloudBuilder,
    accessor: &gltf::Accessor,
    indices_data: &[u32],
    att_id: i32,
    number_of_points: i32,
    normalize: bool,
    reverse_winding: bool,
    pc: &PointCloud,
    attribute: &PointAttribute,
) -> Result<(), Status> {
    check_accessor_float_3(accessor, attribute)?;
    let data = attribute_vec3_float(pc.num_points() as usize, attribute)?;
    let mut transformed = Vec::with_capacity(data.len());
    for v in data {
        let mut vec = [v[0] as f64, v[1] as f64, v[2] as f64];
        if normalize {
            normalize_vec3(&mut vec);
        }
        transformed.push([vec[0] as f32, vec[1] as f32, vec[2] as f32]);
    }
    let _ = reverse_winding;
    set_values_for_point(
        builder,
        indices_data,
        att_id,
        number_of_points,
        &transformed,
        0,
    );
    Ok(())
}

fn check_accessor_float_2(
    accessor: &gltf::Accessor,
    attribute: &PointAttribute,
) -> Result<(), Status> {
    check_accessor_component(accessor, attribute, 2)
}

fn check_accessor_float_3(
    accessor: &gltf::Accessor,
    attribute: &PointAttribute,
) -> Result<(), Status> {
    check_accessor_component(accessor, attribute, 3)
}

fn check_accessor_float_4(
    accessor: &gltf::Accessor,
    attribute: &PointAttribute,
) -> Result<(), Status> {
    check_accessor_component(accessor, attribute, 4)
}

fn check_accessor_component(
    accessor: &gltf::Accessor,
    attribute: &PointAttribute,
    components: u8,
) -> Result<(), Status> {
    let component_type = checked_component_type(accessor)?;
    if component_type != gltf::accessor::ComponentType::F32 {
        return Err(Status::new(
            StatusCode::DracoError,
            "Accessor type mismatch.",
        ));
    }
    if attribute.data_type() != DataType::Float32 || attribute.num_components() != components {
        return Err(Status::new(
            StatusCode::DracoError,
            "Draco attribute type mismatch.",
        ));
    }
    Ok(())
}

fn attribute_vec2_f32(attribute: &PointAttribute, point_index: PointIndex) -> Option<[f32; 2]> {
    let mut out = [0.0f32; 2];
    if attribute.convert_value(attribute.mapped_index(point_index), 2, &mut out) {
        Some(out)
    } else {
        None
    }
}

fn attribute_vec3_f32(attribute: &PointAttribute, point_index: PointIndex) -> Option<[f32; 3]> {
    let mut out = [0.0f32; 3];
    if attribute.convert_value(attribute.mapped_index(point_index), 3, &mut out) {
        Some(out)
    } else {
        None
    }
}

fn vec3_add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn vec3_sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn vec3_scale(v: [f32; 3], s: f32) -> [f32; 3] {
    [v[0] * s, v[1] * s, v[2] * s]
}

fn vec3_dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn vec3_cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn vec3_normalize(v: [f32; 3]) -> [f32; 3] {
    let len_sq = vec3_dot(v, v);
    if len_sq > 1e-12f32 {
        vec3_scale(v, 1.0f32 / len_sq.sqrt())
    } else {
        [0.0f32; 3]
    }
}

fn attribute_vec2_float(
    num_points: usize,
    attribute: &PointAttribute,
) -> Result<Vec<[f32; 2]>, Status> {
    let mut out = vec![[0.0f32; 2]; num_points];
    for i in 0..num_points {
        let avi = attribute.mapped_index(PointIndex::from(i as u32));
        out[i] = attribute.get_value_array::<f32, 2>(avi);
    }
    Ok(out)
}

fn attribute_vec3_float(
    num_points: usize,
    attribute: &PointAttribute,
) -> Result<Vec<[f32; 3]>, Status> {
    let mut out = vec![[0.0f32; 3]; num_points];
    for i in 0..num_points {
        let avi = attribute.mapped_index(PointIndex::from(i as u32));
        out[i] = attribute.get_value_array::<f32, 3>(avi);
    }
    Ok(out)
}

fn attribute_vec4_float(
    num_points: usize,
    attribute: &PointAttribute,
) -> Result<Vec<[f32; 4]>, Status> {
    let mut out = vec![[0.0f32; 4]; num_points];
    for i in 0..num_points {
        let avi = attribute.mapped_index(PointIndex::from(i as u32));
        out[i] = attribute.get_value_array::<f32, 4>(avi);
    }
    Ok(out)
}

fn attribute_raw_bytes(num_points: usize, attribute: &PointAttribute) -> AccessorRaw {
    let element_size = attribute.byte_stride() as usize;
    let mut data = vec![0u8; num_points * element_size];
    for i in 0..num_points {
        let avi = attribute.mapped_index(PointIndex::from(i as u32));
        let start = i * element_size;
        let end = start + element_size;
        attribute.get_value_bytes(avi, &mut data[start..end]);
    }
    AccessorRaw { data, element_size }
}

fn parse_schema_value(value: &Value, object: &mut StructuralMetadataObject) -> Result<(), Status> {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                let mut child = StructuralMetadataObject::with_name(key);
                parse_schema_value(value, &mut child)?;
                object.set_objects().push(child);
            }
        }
        Value::Array(array) => {
            for value in array {
                let mut child = StructuralMetadataObject::new();
                parse_schema_value(value, &mut child)?;
                object.set_array().push(child);
            }
        }
        Value::String(value) => object.set_string(value),
        Value::Number(num) => {
            let value = num.as_i64().ok_or_else(|| {
                Status::new(StatusCode::DracoError, "Unsupported JSON type in schema.")
            })?;
            object.set_integer(value as i32);
        }
        Value::Bool(value) => object.set_boolean(*value),
        _ => {
            return Err(Status::new(
                StatusCode::DracoError,
                "Unsupported JSON type in schema.",
            ))
        }
    }
    Ok(())
}

fn move_non_material_textures(
    non_material_textures: &HashSet<*const Texture>,
    material_tl: &mut TextureLibrary,
    non_material_tl: &mut TextureLibrary,
) {
    let mut i = 0;
    while i < material_tl.num_textures() {
        let texture = material_tl.texture(i as i32).unwrap();
        if non_material_textures.contains(&(texture as *const Texture)) {
            let texture = material_tl.remove_texture(i as i32).unwrap();
            non_material_tl.push_texture(texture);
        } else {
            i += 1;
        }
    }
}

fn build_mesh_from_builder_local(
    use_mesh_builder: bool,
    mb: &mut TriangleSoupMeshBuilder,
    pb: &mut PointCloudBuilder,
    deduplicate_vertices: bool,
) -> StatusOr<Box<Mesh>> {
    if use_mesh_builder {
        if let Some(mesh) = mb.finalize() {
            return StatusOr::new_value(Box::new(mesh));
        }
        return StatusOr::new_status(Status::new(
            StatusCode::DracoError,
            "Failed to build Draco mesh from glTF data.",
        ));
    }
    let pc = pb.finalize(deduplicate_vertices);
    if let Some(pc) = pc {
        let mut mesh = Mesh::new();
        mesh.copy(&pc);
        return StatusOr::new_value(Box::new(mesh));
    }
    StatusOr::new_status(Status::new(
        StatusCode::DracoError,
        "Failed to build Draco mesh from glTF data.",
    ))
}
