//! glTF encoder.
//!
//! What: Encodes Draco meshes/scenes into glTF 2.0 (.gltf/.glb) with Draco
//!       compression, materials, textures, mesh features, and structural metadata.
//! Why: Provides parity with Draco C++ glTF transcoder output logic.
//! How: Builds a glTF asset graph, streams JSON via JsonWriter, and writes
//!      binary buffers/images with proper extension metadata.
//! Where used: Scene IO, mesh IO, and glTF transcoder tools.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::animation::{
    AnimationChannel, AnimationSampler, NodeAnimationData, NodeAnimationDataHash,
};
use crate::io::file_utils;
use crate::io::file_writer_factory::FileWriterFactory;
use crate::io::gltf_utils::{GltfValue, JsonWriter, Mode as JsonMode};
use crate::io::texture_io::{write_texture_to_buffer, write_texture_to_file};
use crate::scene::MaterialsVariantsMapping;
use crate::scene::{
    AnimationIndex, InstanceArrayIndex, Light, LightIndex, LightType, MeshGroupIndex, MeshIndex,
    Scene, SceneNodeIndex, SkinIndex, INVALID_MESH_GROUP_INDEX, INVALID_MESH_INDEX,
};

use draco_bitstream::compression::expert_encode::ExpertEncoder;
use draco_core::attributes::geometry_attribute::GeometryAttributeType;
use draco_core::attributes::geometry_indices::{AttributeValueIndex, FaceIndex, PointIndex};
use draco_core::attributes::point_attribute::PointAttribute;
use draco_core::core::draco_index_type_vector::IndexTypeVector;
use draco_core::core::draco_types::DataType;
use draco_core::core::encoder_buffer::EncoderBuffer;
use draco_core::core::status::{ok_status, Status, StatusCode};
use draco_core::core::status_or::StatusOr;
use draco_core::core::vector_d::{Vector3f, Vector4f};
use draco_core::material::material::{Material, MaterialTransparencyMode};
use draco_core::material::material_library::MaterialLibrary;
use draco_core::mesh::mesh_features::MeshFeatures;
use draco_core::mesh::mesh_indices::MeshFeaturesIndex;
use draco_core::mesh::mesh_splitter::MeshSplitter;
use draco_core::mesh::mesh_utils::{Matrix4d, MeshUtils};
use draco_core::mesh::Mesh;
use draco_core::metadata::metadata::MetadataString;
use draco_core::metadata::structural_metadata::StructuralMetadata;
use draco_core::metadata::structural_metadata_schema::{
    Object as StructuralMetadataObject, ObjectType as StructuralMetadataObjectType,
};
use draco_core::texture::texture::Texture;
use draco_core::texture::texture_map::{
    TextureMap, TextureMapAxisWrappingMode, TextureMapFilterType, TextureMapType,
    TextureMapWrappingMode,
};
use draco_core::texture::texture_transform::TextureTransform;
use draco_core::texture::texture_utils::TextureUtils;

/// Types of output modes for the glTF data encoder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputType {
    Compact,
    Verbose,
}

/// Class for encoding Draco meshes/scenes into glTF 2.0.
pub struct GltfEncoder {
    output_type: OutputType,
    copyright: String,
}

impl GltfEncoder {
    pub const DRACO_METADATA_GLTF_ATTRIBUTE_NAME: &'static str =
        "//GLTF/ApplicationSpecificAttributeName";

    pub fn new() -> Self {
        Self {
            output_type: OutputType::Compact,
            copyright: String::new(),
        }
    }

    pub fn set_output_type(&mut self, output_type: OutputType) {
        self.output_type = output_type;
    }

    pub fn output_type(&self) -> OutputType {
        self.output_type
    }

    pub fn set_copyright(&mut self, copyright: &str) {
        self.copyright = copyright.to_string();
    }

    pub fn copyright(&self) -> &str {
        &self.copyright
    }

    /// Encodes geometry and saves to file using explicit base directory.
    pub fn encode_to_file<'g, T: GltfEncodeGeometry<'g>>(
        &mut self,
        geometry: &'g T,
        file_name: &str,
        base_dir: &str,
    ) -> bool {
        if base_dir.is_empty() {
            return false;
        }
        let buffer_name = format!("{}/buffer0.bin", base_dir);
        self.encode_file_full(geometry, file_name, &buffer_name, base_dir)
            .is_ok()
    }

    /// Encodes geometry to a glTF file, deriving bin filename from output path.
    pub fn encode_file<'g, T: GltfEncodeGeometry<'g>>(
        &mut self,
        geometry: &'g T,
        filename: &str,
    ) -> Status {
        if filename.is_empty() {
            return Status::new(StatusCode::DracoError, "Output parameter is empty.");
        }
        let mut dir_path = String::new();
        let mut basename = String::new();
        file_utils::split_path(filename, &mut dir_path, &mut basename);
        let bin_basename = file_utils::replace_file_extension(&basename, "bin");
        let bin_filename = format!("{}/{}", dir_path, bin_basename);
        self.encode_file_full(geometry, filename, &bin_filename, &dir_path)
    }

    /// Encodes geometry to a glTF file with explicit bin filename.
    pub fn encode_file_with_bin<'g, T: GltfEncodeGeometry<'g>>(
        &mut self,
        geometry: &'g T,
        filename: &str,
        bin_filename: &str,
    ) -> Status {
        if filename.is_empty() {
            return Status::new(StatusCode::DracoError, "Output parameter is empty.");
        }
        let mut dir_path = String::new();
        let mut _basename = String::new();
        file_utils::split_path(filename, &mut dir_path, &mut _basename);
        self.encode_file_full(geometry, filename, bin_filename, &dir_path)
    }

    /// Encodes geometry to a glTF file with explicit bin filename and resource dir.
    pub fn encode_file_full<'g, T: GltfEncodeGeometry<'g>>(
        &mut self,
        geometry: &'g T,
        filename: &str,
        bin_filename: &str,
        resource_dir: &str,
    ) -> Status {
        if filename.is_empty() || bin_filename.is_empty() || resource_dir.is_empty() {
            return Status::new(StatusCode::DracoError, "Output parameter is empty.");
        }
        let extension = file_utils::lowercase_file_extension(filename);
        if extension != "gltf" && extension != "glb" {
            return Status::new(
                StatusCode::DracoError,
                "gltf_encoder only supports .gltf or .glb output.",
            );
        }

        let mut gltf_asset = GltfAsset::new();
        gltf_asset.set_copyright(&self.copyright);
        gltf_asset.set_output_type(self.output_type);

        if extension == "gltf" {
            let mut bin_path = String::new();
            let mut bin_basename = String::new();
            file_utils::split_path(bin_filename, &mut bin_path, &mut bin_basename);
            gltf_asset.set_buffer_name(&bin_basename);
        } else {
            gltf_asset.set_buffer_name("");
            gltf_asset.set_add_images_to_buffer(true);
        }

        let mut buffer = EncoderBuffer::new();
        let status = self.encode_to_buffer_internal(geometry, &mut gltf_asset, &mut buffer);
        if !status.is_ok() {
            return status;
        }

        if extension == "glb" {
            return self.write_glb_file(&gltf_asset, &buffer, filename);
        }
        self.write_gltf_files(&gltf_asset, &buffer, filename, bin_filename, resource_dir)
    }

    /// Encodes geometry into a GLB buffer (json + binary chunk).
    pub fn encode_to_buffer<'g, T: GltfEncodeGeometry<'g>>(
        &mut self,
        geometry: &'g T,
        out_buffer: &mut EncoderBuffer,
    ) -> Status {
        let mut gltf_asset = GltfAsset::new();
        gltf_asset.set_output_type(self.output_type);
        gltf_asset.set_buffer_name("");
        gltf_asset.set_add_images_to_buffer(true);
        gltf_asset.set_copyright(&self.copyright);

        let mut json_buffer = EncoderBuffer::new();
        let status = self.encode_to_buffer_internal(geometry, &mut gltf_asset, &mut json_buffer);
        if !status.is_ok() {
            return status;
        }

        let mut encode_chunk = |chunk: &EncoderBuffer| -> Status {
            if !out_buffer.encode_bytes(chunk.data()) {
                return Status::new(StatusCode::DracoError, "Error writing to buffer.");
            }
            ok_status()
        };
        self.process_glb_file_chunks(&gltf_asset, &json_buffer, &mut encode_chunk)
    }

    fn encode_to_buffer_internal<'g, T: GltfEncodeGeometry<'g>>(
        &mut self,
        geometry: &'g T,
        gltf_asset: &mut GltfAsset<'g>,
        out_buffer: &mut EncoderBuffer,
    ) -> Status {
        Self::set_json_writer_mode(gltf_asset);
        let status = T::add_to_asset(gltf_asset, geometry);
        if !status.is_ok() {
            return status;
        }
        gltf_asset.output(out_buffer)
    }

    fn set_json_writer_mode(gltf_asset: &mut GltfAsset<'_>) {
        if gltf_asset.output_type() == OutputType::Compact && gltf_asset.add_images_to_buffer() {
            gltf_asset.set_json_output_mode(JsonMode::Compact);
        } else {
            gltf_asset.set_json_output_mode(JsonMode::Readable);
        }
    }

    fn write_gltf_files(
        &self,
        gltf_asset: &GltfAsset<'_>,
        buffer: &EncoderBuffer,
        filename: &str,
        bin_filename: &str,
        resource_dir: &str,
    ) -> Status {
        let mut file = match FileWriterFactory::open_writer(filename) {
            Some(file) => file,
            None => {
                return Status::new(
                    StatusCode::DracoError,
                    "Output glTF file could not be opened.",
                )
            }
        };
        let mut bin_file = match FileWriterFactory::open_writer(bin_filename) {
            Some(file) => file,
            None => {
                return Status::new(
                    StatusCode::DracoError,
                    "Output glTF bin file could not be opened.",
                )
            }
        };

        if !file.write(buffer.data()) {
            return Status::new(StatusCode::DracoError, "Error writing to glTF file.");
        }
        if !bin_file.write(gltf_asset.buffer().data()) {
            return Status::new(StatusCode::DracoError, "Error writing to glTF bin file.");
        }

        for i in 0..gltf_asset.num_images() {
            let name = format!("{}/{}", resource_dir, gltf_asset.image_name(i));
            let image = match gltf_asset.get_image(i as i32) {
                Some(image) => image,
                None => return Status::new(StatusCode::DracoError, "Error getting glTF image."),
            };
            let status = write_texture_to_file(&name, image.texture_source.as_ref());
            if !status.is_ok() {
                return status;
            }
        }
        ok_status()
    }

    fn write_glb_file(
        &self,
        gltf_asset: &GltfAsset<'_>,
        json_data: &EncoderBuffer,
        filename: &str,
    ) -> Status {
        let mut file = match FileWriterFactory::open_writer(filename) {
            Some(file) => file,
            None => {
                return Status::new(
                    StatusCode::DracoError,
                    "Output glb file could not be opened.",
                )
            }
        };

        let mut write_chunk = |chunk: &EncoderBuffer| -> Status {
            if !file.write(chunk.data()) {
                return Status::new(StatusCode::DracoError, "Error writing to glb file.");
            }
            ok_status()
        };

        self.process_glb_file_chunks(gltf_asset, json_data, &mut write_chunk)
    }

    fn process_glb_file_chunks<F>(
        &self,
        gltf_asset: &GltfAsset<'_>,
        json_data: &EncoderBuffer,
        process_chunk: &mut F,
    ) -> Status
    where
        F: FnMut(&EncoderBuffer) -> Status,
    {
        let json_pad_length = if json_data.size() % 4 != 0 {
            4 - (json_data.size() % 4)
        } else {
            0
        } as u32;
        let json_length = json_data.size() as u32 + json_pad_length;
        let total_length = 12u32 + 8u32 + json_length + 8u32 + gltf_asset.buffer().size() as u32;

        let mut header = EncoderBuffer::new();
        if !header.encode_bytes(b"glTF") {
            return Status::new(StatusCode::DracoError, "Error writing to glb file.");
        }
        if !encode_u32_le(&mut header, 2) {
            return Status::new(StatusCode::DracoError, "Error writing to glb file.");
        }
        if !encode_u32_le(&mut header, total_length) {
            return Status::new(StatusCode::DracoError, "Error writing to glb file.");
        }

        // JSON chunk header.
        let json_chunk_type = 0x4E4F534Au32;
        if !encode_u32_le(&mut header, json_length) {
            return Status::new(StatusCode::DracoError, "Error writing to glb file.");
        }
        if !encode_u32_le(&mut header, json_chunk_type) {
            return Status::new(StatusCode::DracoError, "Error writing to glb file.");
        }
        let status = process_chunk(&header);
        if !status.is_ok() {
            return status;
        }
        let status = process_chunk(json_data);
        if !status.is_ok() {
            return status;
        }

        // Pad JSON data.
        header.clear();
        if json_pad_length > 0 {
            let padding = vec![b' '; json_pad_length as usize];
            if !header.encode_bytes(&padding) {
                return Status::new(StatusCode::DracoError, "Error writing to glb file.");
            }
            let status = process_chunk(&header);
            if !status.is_ok() {
                return status;
            }
        }

        // Binary chunk.
        header.clear();
        let bin_chunk_type = 0x004E4942u32;
        let gltf_bin_size = gltf_asset.buffer().size() as u32;
        if !encode_u32_le(&mut header, gltf_bin_size) {
            return Status::new(StatusCode::DracoError, "Error writing to glb file.");
        }
        if !encode_u32_le(&mut header, bin_chunk_type) {
            return Status::new(StatusCode::DracoError, "Error writing to glb file.");
        }
        let status = process_chunk(&header);
        if !status.is_ok() {
            return status;
        }
        process_chunk(gltf_asset.buffer())
    }
}

pub trait GltfEncodeGeometry<'a> {
    fn add_to_asset(asset: &mut GltfAsset<'a>, geometry: &'a Self) -> Status;
}

impl<'a> GltfEncodeGeometry<'a> for Mesh {
    fn add_to_asset(asset: &mut GltfAsset<'a>, geometry: &'a Self) -> Status {
        if !asset.add_draco_mesh(geometry) {
            return Status::new(StatusCode::DracoError, "Error adding Draco mesh.");
        }
        ok_status()
    }
}

impl<'a> GltfEncodeGeometry<'a> for Scene {
    fn add_to_asset(asset: &mut GltfAsset<'a>, geometry: &'a Self) -> Status {
        asset.add_scene(geometry)
    }
}

#[derive(Clone, Debug, Default)]
struct GltfScene {
    // Parity: present in C++ but currently unused in output generation.
    #[allow(dead_code)]
    node_indices: Vec<i32>,
}

#[derive(Clone, Debug)]
struct GltfNode {
    name: String,
    children_indices: Vec<i32>,
    mesh_index: i32,
    skin_index: i32,
    light_index: i32,
    instance_array_index: i32,
    root_node: bool,
    trs_matrix: crate::scene::TrsMatrix,
}

impl Default for GltfNode {
    fn default() -> Self {
        Self {
            name: String::new(),
            children_indices: Vec::new(),
            mesh_index: -1,
            skin_index: -1,
            light_index: -1,
            instance_array_index: -1,
            root_node: false,
            trs_matrix: crate::scene::TrsMatrix::new(),
        }
    }
}

/// Texture reference: either borrowed from scene/mesh or owned (e.g. from file).
#[derive(Debug)]
enum TextureSource<'a> {
    Borrowed(&'a Texture),
    Owned(Box<Texture>),
}

impl TextureSource<'_> {
    fn as_ref(&self) -> &Texture {
        match self {
            TextureSource::Borrowed(r) => r,
            TextureSource::Owned(b) => b.as_ref(),
        }
    }
}

#[derive(Debug)]
struct GltfImage<'a> {
    image_name: String,
    texture_source: TextureSource<'a>,
    num_components: i32,
    buffer_view: i32,
    mime_type: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TextureSampler {
    min_filter: TextureMapFilterType,
    mag_filter: TextureMapFilterType,
    wrapping_mode: TextureMapWrappingMode,
}

impl TextureSampler {
    fn new(
        min: TextureMapFilterType,
        mag: TextureMapFilterType,
        mode: TextureMapWrappingMode,
    ) -> Self {
        Self {
            min_filter: min,
            mag_filter: mag,
            wrapping_mode: mode,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GltfTexture {
    image_index: i32,
    sampler_index: i32,
}

#[derive(Clone, Debug)]
struct GltfAccessor {
    buffer_view_index: i32,
    // Parity: exists in C++ but unused in current encoder path.
    #[allow(dead_code)]
    byte_stride: i32,
    component_type: i32,
    count: i64,
    max: Vec<GltfValue>,
    min: Vec<GltfValue>,
    type_name: String,
    normalized: bool,
}

impl Default for GltfAccessor {
    fn default() -> Self {
        Self {
            buffer_view_index: -1,
            byte_stride: 0,
            component_type: -1,
            count: 0,
            max: Vec::new(),
            min: Vec::new(),
            type_name: String::new(),
            normalized: false,
        }
    }
}

#[derive(Clone, Debug)]
struct GltfBufferView {
    buffer_byte_offset: i64,
    byte_length: i64,
    target: i32,
}

impl Default for GltfBufferView {
    fn default() -> Self {
        Self {
            buffer_byte_offset: -1,
            byte_length: 0,
            target: 0,
        }
    }
}

#[derive(Clone, Debug)]
struct GltfDracoCompressedMesh {
    buffer_view_index: i32,
    attributes: BTreeMap<String, i32>,
}

impl Default for GltfDracoCompressedMesh {
    fn default() -> Self {
        Self {
            buffer_view_index: -1,
            attributes: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug)]
struct GltfPrimitive<'a> {
    indices: i32,
    mode: i32,
    material: i32,
    material_variants_mappings: Vec<MaterialsVariantsMapping>,
    mesh_features: Vec<&'a MeshFeatures>,
    property_attributes: Vec<i32>,
    attributes: BTreeMap<String, i32>,
    compressed_mesh_info: GltfDracoCompressedMesh,
    feature_id_name_indices: HashMap<i32, i32>,
}

impl<'a> Default for GltfPrimitive<'a> {
    fn default() -> Self {
        Self {
            indices: -1,
            mode: 4,
            material: 0,
            material_variants_mappings: Vec::new(),
            mesh_features: Vec::new(),
            property_attributes: Vec::new(),
            attributes: BTreeMap::new(),
            compressed_mesh_info: GltfDracoCompressedMesh::default(),
            feature_id_name_indices: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug, Default)]
struct GltfMesh<'a> {
    name: String,
    primitives: Vec<GltfPrimitive<'a>>,
}

#[derive(Clone, Debug, Default)]
struct EncoderAnimation {
    name: String,
    samplers: Vec<Box<AnimationSampler>>,
    channels: Vec<Box<AnimationChannel>>,
}

#[derive(Clone, Debug)]
struct EncoderSkin {
    inverse_bind_matrices_index: i32,
    joints: Vec<i32>,
    skeleton_index: i32,
}

impl Default for EncoderSkin {
    fn default() -> Self {
        Self {
            inverse_bind_matrices_index: -1,
            joints: Vec::new(),
            skeleton_index: -1,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct EncoderInstanceArray {
    translation: i32,
    rotation: i32,
    scale: i32,
}

impl Default for EncoderInstanceArray {
    fn default() -> Self {
        Self {
            translation: -1,
            rotation: -1,
            scale: -1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
enum ComponentType {
    Byte = 5120,
    UnsignedByte = 5121,
    Short = 5122,
    UnsignedShort = 5123,
    UnsignedInt = 5125,
    Float = 5126,
}

impl ComponentType {
    fn as_i32(self) -> i32 {
        self as i32
    }
}

#[doc(hidden)]
pub struct GltfAsset<'a> {
    copyright: String,
    generator: String,
    version: String,
    scenes: Vec<GltfScene>,
    scene_index: i32,
    nodes: Vec<GltfNode>,
    accessors: Vec<GltfAccessor>,
    buffer_views: Vec<GltfBufferView>,
    meshes: Vec<GltfMesh<'a>>,
    material_library: MaterialLibrary,
    images: Vec<GltfImage<'a>>,
    textures: Vec<GltfTexture>,
    buffer_name: String,
    buffer: EncoderBuffer,
    gltf_json: JsonWriter,
    mesh_group_index_to_gltf_mesh: HashMap<MeshGroupIndex, i32>,
    mesh_index_to_gltf_mesh_primitive: HashMap<MeshIndex, (i32, i32)>,
    base_mesh_transforms: IndexTypeVector<MeshIndex, Matrix4d>,
    animations: Vec<EncoderAnimation>,
    skins: Vec<EncoderSkin>,
    lights: Vec<Light>,
    materials_variants_names: Vec<String>,
    instance_arrays: Vec<EncoderInstanceArray>,
    structural_metadata: Option<&'a StructuralMetadata>,
    draco_compression_used: bool,
    mesh_features_used: bool,
    structural_metadata_used: bool,
    mesh_features_texture_index: i32,
    add_images_to_buffer: bool,
    extensions_used: BTreeSet<String>,
    extensions_required: BTreeSet<String>,
    texture_samplers: Vec<TextureSampler>,
    output_type: OutputType,
    local_meshes: Vec<Box<Mesh>>,
    cesium_rtc: Vec<f64>,
}

impl<'a> GltfAsset<'a> {
    fn new() -> Self {
        Self {
            copyright: String::new(),
            generator: "draco_decoder".to_string(),
            version: "2.0".to_string(),
            scenes: Vec::new(),
            scene_index: -1,
            nodes: Vec::new(),
            accessors: Vec::new(),
            buffer_views: Vec::new(),
            meshes: Vec::new(),
            material_library: MaterialLibrary::new(),
            images: Vec::new(),
            textures: Vec::new(),
            buffer_name: "buffer0.bin".to_string(),
            buffer: EncoderBuffer::new(),
            gltf_json: JsonWriter::new(),
            mesh_group_index_to_gltf_mesh: HashMap::new(),
            mesh_index_to_gltf_mesh_primitive: HashMap::new(),
            base_mesh_transforms: IndexTypeVector::new(),
            animations: Vec::new(),
            skins: Vec::new(),
            lights: Vec::new(),
            materials_variants_names: Vec::new(),
            instance_arrays: Vec::new(),
            structural_metadata: None,
            draco_compression_used: false,
            mesh_features_used: false,
            structural_metadata_used: false,
            mesh_features_texture_index: 0,
            add_images_to_buffer: false,
            extensions_used: BTreeSet::new(),
            extensions_required: BTreeSet::new(),
            texture_samplers: Vec::new(),
            output_type: OutputType::Compact,
            local_meshes: Vec::new(),
            cesium_rtc: Vec::new(),
        }
    }

    fn set_copyright(&mut self, copyright: &str) {
        self.copyright = copyright.to_string();
    }

    fn set_output_type(&mut self, output_type: OutputType) {
        self.output_type = output_type;
    }

    fn output_type(&self) -> OutputType {
        self.output_type
    }

    fn set_json_output_mode(&mut self, mode: JsonMode) {
        self.gltf_json.set_mode(mode);
    }

    // Parity: getter mirrors C++ API, currently unused in Rust flow.
    #[allow(dead_code)]
    fn buffer_name(&self) -> &str {
        &self.buffer_name
    }

    fn set_buffer_name(&mut self, name: &str) {
        self.buffer_name = name.to_string();
    }

    fn buffer(&self) -> &EncoderBuffer {
        &self.buffer
    }

    fn set_add_images_to_buffer(&mut self, flag: bool) {
        self.add_images_to_buffer = flag;
    }

    fn add_images_to_buffer(&self) -> bool {
        self.add_images_to_buffer
    }

    fn num_images(&self) -> usize {
        self.images.len()
    }

    fn image_name(&self, index: usize) -> &str {
        &self.images[index].image_name
    }

    fn get_image(&self, index: i32) -> Option<&GltfImage<'_>> {
        if index < 0 {
            return None;
        }
        self.images.get(index as usize)
    }

    fn unsigned_int_component_size(max_value: u32) -> usize {
        if max_value < 0xff {
            1
        } else if max_value < 0xffff {
            2
        } else {
            4
        }
    }

    fn unsigned_int_component_type(max_value: u32) -> ComponentType {
        if max_value < 0xff {
            ComponentType::UnsignedByte
        } else if max_value < 0xffff {
            ComponentType::UnsignedShort
        } else {
            ComponentType::UnsignedInt
        }
    }

    fn add_scene_internal(&mut self) -> i32 {
        self.scenes.push(GltfScene::default());
        let scene_index = (self.scenes.len() - 1) as i32;
        if self.scene_index == -1 {
            self.scene_index = scene_index;
        }
        scene_index
    }

    fn add_draco_mesh(&mut self, mesh: &'a Mesh) -> bool {
        let scene_index = self.add_scene_internal();
        if scene_index < 0 {
            return false;
        }
        self.add_materials_from_mesh(mesh);

        self.meshes.push(GltfMesh::default());

        self.add_structural_metadata_mesh(mesh);
        if self.copyright.is_empty() {
            self.set_copyright_from_mesh(mesh);
        }

        let material_att_id = mesh.get_named_attribute_id(GeometryAttributeType::Material);
        if material_att_id == -1 {
            if !self.add_draco_mesh_with_material(mesh, 0, &[], &Matrix4d::identity()) {
                return false;
            }
        } else {
            let mat_att = match mesh.get_named_attribute(GeometryAttributeType::Material) {
                Some(att) => att,
                None => return false,
            };

            let mut splitter = MeshSplitter::new();
            let split_maybe = splitter.split_mesh(mesh, material_att_id as u32);
            if !split_maybe.is_ok() {
                return false;
            }
            let mut split_meshes = split_maybe.into_value();
            for i in 0..split_meshes.len() {
                let Some(split_mesh) = split_meshes[i].take() else {
                    continue;
                };

                let mut mat_index: u32 = 0;
                let mut tmp = [0u32; 1];
                if mat_att.get_value_array_into(AttributeValueIndex::from(i as u32), &mut tmp) {
                    mat_index = tmp[0];
                }

                let mut split_mesh = split_mesh;
                Mesh::copy_mesh_features_for_material(mesh, &mut split_mesh, mat_index as i32);
                Mesh::copy_property_attributes_indices_for_material(
                    mesh,
                    &mut split_mesh,
                    mat_index as i32,
                );

                self.local_meshes.push(split_mesh);
                // SAFETY: mesh_ptr points to mesh we just pushed; no mutation of local_meshes during
                // add_draco_mesh_with_material; ptr valid for this scope only (borrow-split pattern).
                let mesh_ptr = self.local_meshes.last().unwrap().as_ref() as *const Mesh;
                if !self.add_draco_mesh_with_material(
                    unsafe { &*mesh_ptr },
                    mat_index as i32,
                    &[],
                    &Matrix4d::identity(),
                ) {
                    return false;
                }
            }
        }

        // Currently output only one mesh.
        let mut mesh_node = GltfNode::default();
        mesh_node.mesh_index = 0;
        mesh_node.root_node = true;
        self.nodes.push(mesh_node);
        true
    }

    fn output(&mut self, buf_out: &mut EncoderBuffer) -> Status {
        self.gltf_json.begin_object();
        if !self.encode_asset_property(buf_out) {
            return Status::new(StatusCode::DracoError, "Failed encoding asset.");
        }
        if !self.encode_scenes_property(buf_out) {
            return Status::new(StatusCode::DracoError, "Failed encoding scenes.");
        }
        if !self.encode_initial_scene_property(buf_out) {
            return Status::new(StatusCode::DracoError, "Failed encoding initial scene.");
        }
        if !self.encode_nodes_property(buf_out) {
            return Status::new(StatusCode::DracoError, "Failed encoding nodes.");
        }
        draco_core::draco_return_if_error!(self.encode_meshes_property(buf_out));
        draco_core::draco_return_if_error!(self.encode_materials(buf_out));
        if !self.encode_accessors_property(buf_out) {
            return Status::new(StatusCode::DracoError, "Failed encoding accessors.");
        }
        draco_core::draco_return_if_error!(self.encode_animations_property(buf_out));
        draco_core::draco_return_if_error!(self.encode_skins_property(buf_out));
        draco_core::draco_return_if_error!(self.encode_top_level_extensions_property(buf_out));
        if !self.encode_buffer_views_property(buf_out) {
            return Status::new(StatusCode::DracoError, "Failed encoding buffer views.");
        }
        if !self.encode_buffers_property(buf_out) {
            return Status::new(StatusCode::DracoError, "Failed encoding buffers.");
        }
        draco_core::draco_return_if_error!(self.encode_extensions_properties(buf_out));
        self.gltf_json.end_object();

        let asset_str = self.gltf_json.move_data();
        if !buf_out.encode_bytes(asset_str.as_bytes()) {
            return Status::new(StatusCode::DracoError, "Failed encoding json data.");
        }
        if !buf_out.encode_bytes(b"\n") {
            return Status::new(StatusCode::DracoError, "Failed encoding json data.");
        }
        ok_status()
    }

    fn pad_buffer(&mut self) -> bool {
        if self.buffer.size() % 4 != 0 {
            let pad_bytes = 4 - self.buffer.size() % 4;
            let pad = vec![0u8; pad_bytes];
            if !self.buffer.encode_bytes(&pad) {
                return false;
            }
        }
        true
    }

    fn add_attribute_to_draco_extension(
        &mut self,
        mesh: &Mesh,
        attr_type: GeometryAttributeType,
        index: i32,
        name: &str,
        compressed_mesh_info: &mut GltfDracoCompressedMesh,
    ) {
        if mesh.is_compression_enabled() {
            if let Some(att) = mesh.get_named_attribute_by_index(attr_type, index) {
                compressed_mesh_info
                    .attributes
                    .insert(name.to_string(), att.unique_id() as i32);
            }
        }
    }

    fn compress_mesh_with_draco(
        &mut self,
        mesh: &Mesh,
        transform: &Matrix4d,
        primitive: &mut GltfPrimitive,
        num_encoded_points: &mut i64,
        num_encoded_faces: &mut i64,
    ) -> Status {
        let mut compression_options = mesh.compression_options().clone();
        let status = compression_options.check();
        if !status.is_ok() {
            return status;
        }

        let mut mesh_copy = Box::new(Mesh::new());
        mesh_copy.copy_from(mesh);

        // Delete auto-generated tangents.
        if MeshUtils::has_auto_generated_tangents(&mesh_copy) {
            loop {
                let tangent_att_id =
                    mesh_copy.get_named_attribute_id(GeometryAttributeType::Tangent);
                if tangent_att_id == -1 {
                    break;
                }
                mesh_copy.delete_attribute(tangent_att_id);
            }
        }

        if mesh_copy.num_faces() <= 0 {
            return Status::new(
                StatusCode::DracoError,
                "Draco compression is not supported for glTF point clouds.",
            );
        }

        let speed = 10 - compression_options.compression_level;
        let mut quant_settings: Vec<(i32, i32)> = Vec::new();

        for i in 0..mesh_copy.num_attributes() {
            let att = match mesh_copy.attribute(i) {
                Some(att) => att,
                None => continue,
            };
            if att.attribute_type() == GeometryAttributeType::Position
                && !compression_options
                    .quantization_position
                    .are_quantization_bits_defined()
            {
                let global_spacing = compression_options.quantization_position.spacing();
                let max_scale = matrix_max_scale(transform);
                let local_spacing = global_spacing / max_scale;
                compression_options
                    .quantization_position
                    .set_grid(local_spacing);
            } else {
                let mut num_quantization_bits: Option<i32> = None;
                match att.attribute_type() {
                    GeometryAttributeType::Position => {
                        let bits = compression_options
                            .quantization_position
                            .quantization_bits();
                        if bits > 0 {
                            num_quantization_bits = Some(bits);
                        }
                    }
                    GeometryAttributeType::Normal => {
                        let bits = compression_options.quantization_bits_normal;
                        if bits > 0 {
                            num_quantization_bits = Some(bits);
                        }
                    }
                    GeometryAttributeType::TexCoord => {
                        let bits = compression_options.quantization_bits_tex_coord;
                        if bits > 0 {
                            num_quantization_bits = Some(bits);
                        }
                    }
                    GeometryAttributeType::Tangent => {
                        let bits = compression_options.quantization_bits_tangent;
                        if bits > 0 {
                            num_quantization_bits = Some(bits);
                        }
                    }
                    GeometryAttributeType::Weights => {
                        let bits = compression_options.quantization_bits_weight;
                        if bits > 0 {
                            num_quantization_bits = Some(bits);
                        }
                    }
                    GeometryAttributeType::Generic => {
                        if !is_feature_id_attribute(i, &mesh_copy) {
                            let bits = compression_options.quantization_bits_generic;
                            if bits > 0 {
                                num_quantization_bits = Some(bits);
                            }
                        } else {
                            num_quantization_bits = Some(-1);
                        }
                    }
                    _ => {}
                }
                if let Some(bits) = num_quantization_bits {
                    quant_settings.push((i, bits));
                }
            }
        }

        for i in 0..mesh_copy.num_attributes() {
            let att = match mesh_copy.attribute_mut(i) {
                Some(att) => att,
                None => continue,
            };
            if att.attribute_type() == GeometryAttributeType::TexCoord {
                if !MeshUtils::flip_texture_uv_values(false, true, att) {
                    return Status::new(
                        StatusCode::DracoError,
                        "Could not flip texture UV values.",
                    );
                }
            }
        }

        for i in 0..mesh_copy.num_attributes() {
            let att = match mesh_copy.attribute_mut(i) {
                Some(att) => att,
                None => continue,
            };
            if att.attribute_type() == GeometryAttributeType::Tangent
                || att.attribute_type() == GeometryAttributeType::Joints
                || att.attribute_type() == GeometryAttributeType::Weights
            {
                att.set_attribute_type(GeometryAttributeType::Generic);
            }
        }

        mesh_copy.set_compression_options(compression_options);

        let mut encoder = ExpertEncoder::new_mesh(&mesh_copy);
        encoder.set_track_encoded_properties(true);
        encoder.set_speed_options(speed, speed);
        for (att_id, bits) in quant_settings {
            encoder.set_attribute_quantization(att_id, bits);
        }
        let mut buffer = EncoderBuffer::new();
        let status = encoder.encode_to_buffer(&mut buffer);
        if !status.is_ok() {
            return status;
        }
        *num_encoded_points = encoder.num_encoded_points() as i64;
        *num_encoded_faces = encoder.num_encoded_faces() as i64;

        let buffer_start_offset = self.buffer.size();
        if !self.buffer.encode_bytes(buffer.data()) {
            return Status::new(
                StatusCode::DracoError,
                "Could not copy Draco compressed data.",
            );
        }
        if !self.pad_buffer() {
            return Status::new(StatusCode::DracoError, "Could not pad glTF buffer.");
        }

        let mut buffer_view = GltfBufferView::default();
        buffer_view.buffer_byte_offset = buffer_start_offset as i64;
        buffer_view.byte_length = (self.buffer.size() - buffer_start_offset) as i64;
        self.buffer_views.push(buffer_view);
        primitive.compressed_mesh_info.buffer_view_index = (self.buffer_views.len() - 1) as i32;
        ok_status()
    }

    fn add_draco_mesh_with_material(
        &mut self,
        mesh: &'a Mesh,
        material_id: i32,
        material_variants_mappings: &[MaterialsVariantsMapping],
        transform: &Matrix4d,
    ) -> bool {
        let mut primitive = GltfPrimitive::default();
        let mut num_encoded_points = mesh.num_points() as i64;
        let mut num_encoded_faces = mesh.num_faces() as i64;
        if num_encoded_faces > 0 && mesh.is_compression_enabled() {
            let status = self.compress_mesh_with_draco(
                mesh,
                transform,
                &mut primitive,
                &mut num_encoded_points,
                &mut num_encoded_faces,
            );
            if !status.is_ok() {
                return false;
            }
            self.draco_compression_used = true;
        }

        let mut indices_index = -1;
        if num_encoded_faces > 0 {
            indices_index = self.add_draco_indices(mesh, num_encoded_faces);
            if indices_index < 0 {
                return false;
            }
        }
        let position_index = self.add_draco_positions(mesh, num_encoded_points as i32);
        if position_index < 0 {
            return false;
        }

        let tex_coord_order = match check_and_get_tex_coord_attribute_order(mesh) {
            Some(order) => order,
            None => return false,
        };

        let normals_accessor_index = self.add_draco_normals(mesh, num_encoded_points as i32);
        let colors_accessor_index = self.add_draco_colors(mesh, num_encoded_points as i32);
        let texture0_accessor_index =
            self.add_draco_texture(mesh, tex_coord_order[0], num_encoded_points as i32);
        let texture1_accessor_index =
            self.add_draco_texture(mesh, tex_coord_order[1], num_encoded_points as i32);
        let tangent_accessor_index = self.add_draco_tangents(mesh, num_encoded_points as i32);
        let joints_accessor_index = self.add_draco_joints(mesh, num_encoded_points as i32);
        let weights_accessor_index = self.add_draco_weights(mesh, num_encoded_points as i32);
        let generics_accessors = self.add_draco_generics(
            mesh,
            num_encoded_points as i32,
            &mut primitive.feature_id_name_indices,
        );

        if num_encoded_faces == 0 {
            primitive.mode = 0; // POINTS
        }
        primitive.material = material_id;
        primitive.material_variants_mappings = material_variants_mappings.to_vec();
        primitive.mesh_features.reserve(mesh.num_mesh_features());
        for i in 0..mesh.num_mesh_features() {
            let idx = MeshFeaturesIndex::from(i as u32);
            primitive.mesh_features.push(mesh.mesh_features(idx));
        }
        primitive
            .property_attributes
            .reserve(mesh.num_property_attributes_indices() as usize);
        for i in 0..mesh.num_property_attributes_indices() {
            primitive
                .property_attributes
                .push(mesh.property_attributes_index(i));
        }
        primitive.indices = indices_index;
        primitive
            .attributes
            .insert("POSITION".to_string(), position_index);
        self.add_attribute_to_draco_extension(
            mesh,
            GeometryAttributeType::Position,
            0,
            "POSITION",
            &mut primitive.compressed_mesh_info,
        );
        if normals_accessor_index > 0 {
            primitive
                .attributes
                .insert("NORMAL".to_string(), normals_accessor_index);
            self.add_attribute_to_draco_extension(
                mesh,
                GeometryAttributeType::Normal,
                0,
                "NORMAL",
                &mut primitive.compressed_mesh_info,
            );
        }
        if colors_accessor_index > 0 {
            primitive
                .attributes
                .insert("COLOR_0".to_string(), colors_accessor_index);
            self.add_attribute_to_draco_extension(
                mesh,
                GeometryAttributeType::Color,
                0,
                "COLOR_0",
                &mut primitive.compressed_mesh_info,
            );
        }
        if texture0_accessor_index > 0 {
            primitive
                .attributes
                .insert("TEXCOORD_0".to_string(), texture0_accessor_index);
            self.add_attribute_to_draco_extension(
                mesh,
                GeometryAttributeType::TexCoord,
                0,
                "TEXCOORD_0",
                &mut primitive.compressed_mesh_info,
            );
        }
        if texture1_accessor_index > 0 {
            primitive
                .attributes
                .insert("TEXCOORD_1".to_string(), texture1_accessor_index);
            self.add_attribute_to_draco_extension(
                mesh,
                GeometryAttributeType::TexCoord,
                1,
                "TEXCOORD_1",
                &mut primitive.compressed_mesh_info,
            );
        }
        if tangent_accessor_index > 0 {
            primitive
                .attributes
                .insert("TANGENT".to_string(), tangent_accessor_index);
            self.add_attribute_to_draco_extension(
                mesh,
                GeometryAttributeType::Tangent,
                0,
                "TANGENT",
                &mut primitive.compressed_mesh_info,
            );
        }
        if joints_accessor_index > 0 {
            primitive
                .attributes
                .insert("JOINTS_0".to_string(), joints_accessor_index);
            self.add_attribute_to_draco_extension(
                mesh,
                GeometryAttributeType::Joints,
                0,
                "JOINTS_0",
                &mut primitive.compressed_mesh_info,
            );
        }
        if weights_accessor_index > 0 {
            primitive
                .attributes
                .insert("WEIGHTS_0".to_string(), weights_accessor_index);
            self.add_attribute_to_draco_extension(
                mesh,
                GeometryAttributeType::Weights,
                0,
                "WEIGHTS_0",
                &mut primitive.compressed_mesh_info,
            );
        }
        for (att_index, (attribute_name, accessor)) in generics_accessors.iter().enumerate() {
            if !attribute_name.is_empty() {
                primitive
                    .attributes
                    .insert(attribute_name.clone(), *accessor);
                self.add_attribute_to_draco_extension(
                    mesh,
                    GeometryAttributeType::Generic,
                    att_index as i32,
                    attribute_name,
                    &mut primitive.compressed_mesh_info,
                );
            }
        }

        if let Some(last) = self.meshes.last_mut() {
            last.primitives.push(primitive);
        }
        true
    }

    fn add_draco_indices(&mut self, mesh: &Mesh, num_encoded_faces: i64) -> i32 {
        let mut min_index: u32 = 0xffffffff;
        let mut max_index: u32 = 0;
        for i in 0..mesh.num_faces() {
            let f = mesh.face(FaceIndex::from(i as u32));
            for j in 0..3 {
                let value = f[j].value();
                if value < min_index {
                    min_index = value;
                }
                if value > max_index {
                    max_index = value;
                }
            }
        }

        let component_size = GltfAsset::unsigned_int_component_size(max_index);
        let mut accessor = GltfAccessor::default();
        if !mesh.is_compression_enabled() {
            let buffer_start_offset = self.buffer.size();
            for i in 0..mesh.num_faces() {
                let f = mesh.face(FaceIndex::from(i as u32));
                for j in 0..3 {
                    let index = f[j].value();
                    if !encode_u32_with_size(&mut self.buffer, index, component_size) {
                        return -1;
                    }
                }
            }
            if !self.pad_buffer() {
                return -1;
            }
            let mut buffer_view = GltfBufferView::default();
            buffer_view.buffer_byte_offset = buffer_start_offset as i64;
            buffer_view.byte_length = (self.buffer.size() - buffer_start_offset) as i64;
            self.buffer_views.push(buffer_view);
            accessor.buffer_view_index = (self.buffer_views.len() - 1) as i32;
        }

        accessor.component_type = GltfAsset::unsigned_int_component_type(max_index).as_i32();
        accessor.count = num_encoded_faces * 3;
        if self.output_type == OutputType::Verbose {
            accessor.max.push(GltfValue::from_u32(max_index));
            accessor.min.push(GltfValue::from_u32(min_index));
        }
        accessor.type_name = "SCALAR".to_string();
        self.accessors.push(accessor);
        (self.accessors.len() - 1) as i32
    }

    fn add_draco_positions(&mut self, mesh: &Mesh, num_encoded_points: i32) -> i32 {
        let att = match mesh.get_named_attribute(GeometryAttributeType::Position) {
            Some(att) => att,
            None => return -1,
        };
        if !Self::check_draco_attribute(att, &[DataType::Float32], &[3]) {
            return -1;
        }
        self.add_attribute_typed::<f32>(
            att,
            mesh.num_points() as i32,
            num_encoded_points,
            mesh.is_compression_enabled(),
        )
    }

    fn add_draco_normals(&mut self, mesh: &Mesh, num_encoded_points: i32) -> i32 {
        let att = match mesh.get_named_attribute(GeometryAttributeType::Normal) {
            Some(att) => att,
            None => return -1,
        };
        if !Self::check_draco_attribute(att, &[DataType::Float32], &[3]) {
            return -1;
        }
        self.add_attribute_typed::<f32>(
            att,
            mesh.num_points() as i32,
            num_encoded_points,
            mesh.is_compression_enabled(),
        )
    }

    fn add_draco_colors(&mut self, mesh: &Mesh, num_encoded_points: i32) -> i32 {
        let att = match mesh.get_named_attribute(GeometryAttributeType::Color) {
            Some(att) => att,
            None => return -1,
        };
        if !Self::check_draco_attribute(
            att,
            &[DataType::Uint8, DataType::Uint16, DataType::Float32],
            &[3, 4],
        ) {
            return -1;
        }
        match att.data_type() {
            DataType::Uint16 => self.add_attribute_typed::<u16>(
                att,
                mesh.num_points() as i32,
                num_encoded_points,
                mesh.is_compression_enabled(),
            ),
            DataType::Float32 => self.add_attribute_typed::<f32>(
                att,
                mesh.num_points() as i32,
                num_encoded_points,
                mesh.is_compression_enabled(),
            ),
            _ => self.add_attribute_typed::<u8>(
                att,
                mesh.num_points() as i32,
                num_encoded_points,
                mesh.is_compression_enabled(),
            ),
        }
    }

    fn add_draco_texture(
        &mut self,
        mesh: &Mesh,
        tex_coord_index: i32,
        num_encoded_points: i32,
    ) -> i32 {
        let att = match mesh
            .get_named_attribute_by_index(GeometryAttributeType::TexCoord, tex_coord_index)
        {
            Some(att) => att,
            None => return -1,
        };
        if !Self::check_draco_attribute(att, &[DataType::Float32], &[2]) {
            return -1;
        }

        let mut ta = PointAttribute::new();
        ta.init(
            GeometryAttributeType::TexCoord,
            2,
            att.data_type(),
            false,
            mesh.num_points() as usize,
        );

        let mut value = [0.0f32; 2];
        for v in 0..mesh.num_points() {
            let point = PointIndex::from(v as u32);
            let mapped = att.mapped_index(point);
            if !att.get_value_array_into(mapped, &mut value) {
                return -1;
            }
            let flipped = [value[0], 1.0 - value[1]];
            ta.set_attribute_value_array(AttributeValueIndex::from(v as u32), &flipped);
        }

        self.add_attribute_typed::<f32>(
            &ta,
            mesh.num_points() as i32,
            num_encoded_points,
            mesh.is_compression_enabled(),
        )
    }

    fn add_draco_tangents(&mut self, mesh: &Mesh, num_encoded_points: i32) -> i32 {
        let att = match mesh.get_named_attribute(GeometryAttributeType::Tangent) {
            Some(att) => att,
            None => return -1,
        };
        if !Self::check_draco_attribute(att, &[DataType::Float32], &[3, 4]) {
            return -1;
        }
        if MeshUtils::has_auto_generated_tangents(mesh) {
            return -1;
        }
        if att.num_components() == 4 {
            return self.add_attribute_typed::<f32>(
                att,
                mesh.num_points() as i32,
                num_encoded_points,
                mesh.is_compression_enabled(),
            );
        }

        let mut ta = PointAttribute::new();
        ta.init(
            GeometryAttributeType::Tangent,
            4,
            DataType::Float32,
            false,
            mesh.num_points() as usize,
        );

        let mut value = [0.0f32; 3];
        for v in 0..mesh.num_points() {
            let point = PointIndex::from(v as u32);
            let mapped = att.mapped_index(point);
            if !att.get_value_array_into(mapped, &mut value) {
                return -1;
            }
            let tangent = [value[0], value[1], value[2], 1.0];
            ta.set_attribute_value_array(AttributeValueIndex::from(v as u32), &tangent);
        }

        self.add_attribute_typed::<f32>(
            &ta,
            mesh.num_points() as i32,
            num_encoded_points,
            mesh.is_compression_enabled(),
        )
    }

    fn add_draco_joints(&mut self, mesh: &Mesh, num_encoded_points: i32) -> i32 {
        let att = match mesh.get_named_attribute(GeometryAttributeType::Joints) {
            Some(att) => att,
            None => return -1,
        };
        if !Self::check_draco_attribute(att, &[DataType::Uint8, DataType::Uint16], &[4]) {
            return -1;
        }
        if att.data_type() == DataType::Uint16 {
            return self.add_attribute_typed::<u16>(
                att,
                mesh.num_points() as i32,
                num_encoded_points,
                mesh.is_compression_enabled(),
            );
        }
        self.add_attribute_typed::<u8>(
            att,
            mesh.num_points() as i32,
            num_encoded_points,
            mesh.is_compression_enabled(),
        )
    }

    fn add_draco_weights(&mut self, mesh: &Mesh, num_encoded_points: i32) -> i32 {
        let att = match mesh.get_named_attribute(GeometryAttributeType::Weights) {
            Some(att) => att,
            None => return -1,
        };
        if !Self::check_draco_attribute(att, &[DataType::Float32], &[4]) {
            return -1;
        }
        self.add_attribute_typed::<f32>(
            att,
            mesh.num_points() as i32,
            num_encoded_points,
            mesh.is_compression_enabled(),
        )
    }

    fn add_draco_generics(
        &mut self,
        mesh: &Mesh,
        num_encoded_points: i32,
        feature_id_name_indices: &mut HashMap<i32, i32>,
    ) -> Vec<(String, i32)> {
        let num_generic_attributes = mesh.num_named_attributes(GeometryAttributeType::Generic);
        let mut attrs = Vec::new();
        let mut feature_id_count = 0;
        for i in 0..num_generic_attributes {
            let att_index = mesh.get_named_attribute_id_by_index(GeometryAttributeType::Generic, i);
            let att = match mesh.attribute(att_index) {
                Some(att) => att,
                None => continue,
            };
            let mut attr_name = String::new();
            let mut accessor = -1;

            if let Some(metadata) = mesh.get_attribute_metadata_by_attribute_id(att_index) {
                let mut name_value = MetadataString::default();
                if metadata.get_entry_string(
                    GltfEncoder::DRACO_METADATA_GLTF_ATTRIBUTE_NAME,
                    &mut name_value,
                ) {
                    attr_name = name_value.to_utf8_lossy().into_owned();
                    if att.data_type() == DataType::Float32 {
                        accessor = self.add_attribute_typed::<f32>(
                            att,
                            mesh.num_points() as i32,
                            num_encoded_points,
                            mesh.is_compression_enabled(),
                        );
                    }
                }
            } else {
                if is_feature_id_attribute(att_index, mesh) && att.num_components() == 1 {
                    accessor = self.add_attribute(
                        att,
                        mesh.num_points() as i32,
                        num_encoded_points,
                        mesh.is_compression_enabled(),
                    );
                    attr_name = format!("_FEATURE_ID_{}", feature_id_count);
                    feature_id_name_indices.insert(att_index, feature_id_count);
                    feature_id_count += 1;
                } else if self.structural_metadata.is_some()
                    && is_property_attribute(att_index, mesh, self.structural_metadata.unwrap())
                {
                    accessor = self.add_attribute(
                        att,
                        mesh.num_points() as i32,
                        num_encoded_points,
                        mesh.is_compression_enabled(),
                    );
                    attr_name = att.name().to_string();
                }
            }
            if accessor != -1 && !attr_name.is_empty() {
                attrs.push((attr_name, accessor));
            }
        }
        attrs
    }

    fn add_materials_from_mesh(&mut self, mesh: &Mesh) {
        if mesh.material_library().num_materials() > 0 {
            self.material_library.copy_from(mesh.material_library());
        }
    }

    fn add_materials_from_scene(&mut self, scene: &Scene) {
        if scene.material_library().num_materials() > 0 {
            self.material_library.copy_from(scene.material_library());
        }
    }

    fn check_draco_attribute(
        attribute: &PointAttribute,
        data_types: &[DataType],
        num_components: &[i32],
    ) -> bool {
        if attribute.size() == 0 {
            return false;
        }
        if !data_types.contains(&attribute.data_type()) {
            return false;
        }
        let num_comp = attribute.num_components() as i32;
        if !num_components.contains(&num_comp) {
            return false;
        }
        true
    }

    fn add_image(
        &mut self,
        image_stem: &str,
        texture: &'a Texture,
        num_components: i32,
    ) -> StatusOr<i32> {
        self.add_image_with_owned(image_stem, texture, None, num_components)
    }

    fn find_existing_image_index(&self, texture: &Texture) -> Option<usize> {
        self.images.iter().enumerate().find_map(|(i, img)| {
            if std::ptr::eq(img.texture_source.as_ref(), texture) {
                Some(i)
            } else {
                None
            }
        })
    }

    fn add_image_with_owned(
        &mut self,
        image_stem: &str,
        texture: &'a Texture,
        owned_texture: Option<Box<Texture>>,
        num_components: i32,
    ) -> StatusOr<i32> {
        if let Some(index) = self.find_existing_image_index(texture) {
            let image = &mut self.images[index];
            if image.num_components < num_components {
                image.num_components = num_components;
            }
            return StatusOr::new_value(index as i32);
        }

        let (texture_source, extension, mime_type) = match owned_texture {
            Some(owned) => {
                let t = owned.as_ref();
                let ext = TextureUtils::get_target_extension(t);
                let ext = if ext.is_empty() {
                    file_utils::lowercase_file_extension(t.source_image().filename())
                } else {
                    ext
                };
                let mime = TextureUtils::get_target_mime_type(t);
                (TextureSource::Owned(owned), ext, mime)
            }
            None => {
                let ext = TextureUtils::get_target_extension(texture);
                let ext = if ext.is_empty() {
                    file_utils::lowercase_file_extension(texture.source_image().filename())
                } else {
                    ext
                };
                let mime = TextureUtils::get_target_mime_type(texture);
                (TextureSource::Borrowed(texture), ext, mime)
            }
        };

        if extension == "ktx2" {
            self.extensions_used
                .insert("KHR_texture_basisu".to_string());
            self.extensions_required
                .insert("KHR_texture_basisu".to_string());
        }
        if extension == "webp" {
            self.extensions_used.insert("EXT_texture_webp".to_string());
            self.extensions_required
                .insert("EXT_texture_webp".to_string());
        }

        let image = GltfImage {
            image_name: format!("{}.{}", image_stem, extension),
            texture_source,
            num_components,
            buffer_view: -1,
            mime_type,
        };

        let index = self.images.len();
        self.images.push(image);
        StatusOr::new_value(index as i32)
    }

    fn save_image_to_buffer(&mut self, image_index: usize) -> Status {
        let texture = self.images[image_index].texture_source.as_ref();
        let mut buffer = Vec::new();
        let status = write_texture_to_buffer(texture, &mut buffer);
        if !status.is_ok() {
            return status;
        }
        let buffer_start_offset = self.buffer.size();
        self.buffer.encode_bytes(&buffer);
        if !self.pad_buffer() {
            return Status::new(
                StatusCode::DracoError,
                "Could not pad buffer in SaveImageToBuffer.",
            );
        }
        let mut buffer_view = GltfBufferView::default();
        buffer_view.buffer_byte_offset = buffer_start_offset as i64;
        buffer_view.byte_length = (self.buffer.size() - buffer_start_offset) as i64;
        self.buffer_views.push(buffer_view);
        let buffer_view_index = (self.buffer_views.len() - 1) as i32;
        self.images[image_index].buffer_view = buffer_view_index;
        ok_status()
    }

    fn add_texture_sampler(&mut self, sampler: TextureSampler) -> StatusOr<i32> {
        if sampler.min_filter == TextureMapFilterType::Unspecified
            && sampler.mag_filter == TextureMapFilterType::Unspecified
            && sampler.wrapping_mode.s == TextureMapAxisWrappingMode::Repeat
            && sampler.wrapping_mode.t == TextureMapAxisWrappingMode::Repeat
        {
            return StatusOr::new_value(-1);
        }

        if let Some(index) = self
            .texture_samplers
            .iter()
            .position(|existing| existing == &sampler)
        {
            return StatusOr::new_value(index as i32);
        }

        self.texture_samplers.push(sampler);
        StatusOr::new_value((self.texture_samplers.len() - 1) as i32)
    }

    fn add_scene(&mut self, scene: &'a Scene) -> Status {
        let scene_index = self.add_scene_internal();
        if scene_index < 0 {
            return Status::new(StatusCode::DracoError, "Error creating a new scene.");
        }
        self.add_materials_from_scene(scene);
        self.add_structural_metadata_scene(scene);

        self.base_mesh_transforms = find_largest_base_mesh_transforms(scene);
        for i in 0..scene.num_nodes() {
            draco_core::draco_return_if_error!(
                self.add_scene_node(scene, SceneNodeIndex::from(i as u32))
            );
        }
        for i in 0..scene.num_root_nodes() {
            let root_index = scene.root_node_index(i);
            let idx = root_index.value() as usize;
            if let Some(node) = self.nodes.get_mut(idx) {
                node.root_node = true;
            }
        }

        draco_core::draco_return_if_error!(self.add_animations(scene));
        draco_core::draco_return_if_error!(self.add_skins(scene));
        draco_core::draco_return_if_error!(self.add_lights(scene));
        draco_core::draco_return_if_error!(self.add_materials_variants_names(scene));
        draco_core::draco_return_if_error!(self.add_instance_arrays(scene));
        if self.copyright.is_empty() {
            self.set_copyright_from_scene(scene);
        }

        self.cesium_rtc = scene.cesium_rtc().clone();
        if !self.cesium_rtc.is_empty() {
            self.extensions_used.insert("CESIUM_RTC".to_string());
        }

        ok_status()
    }

    fn add_scene_node(&mut self, scene: &'a Scene, scene_node_index: SceneNodeIndex) -> Status {
        let scene_node = scene.node(scene_node_index);
        let mut node = GltfNode::default();
        node.name = scene_node.name().to_string();
        node.trs_matrix.copy_from(scene_node.trs_matrix());
        for child in scene_node.children() {
            node.children_indices.push(child.value() as i32);
        }

        let mesh_group_index = scene_node.mesh_group_index();
        if mesh_group_index != INVALID_MESH_GROUP_INDEX {
            if !self
                .mesh_group_index_to_gltf_mesh
                .contains_key(&mesh_group_index)
            {
                let mesh_group = scene.mesh_group(mesh_group_index);
                let mut gltf_mesh = GltfMesh::default();
                if !mesh_group.name().is_empty() {
                    gltf_mesh.name = mesh_group.name().to_string();
                }
                self.meshes.push(gltf_mesh);

                for i in 0..mesh_group.num_mesh_instances() {
                    let instance = mesh_group.mesh_instance(i);
                    let mesh_index = instance.mesh_index;
                    if mesh_index == INVALID_MESH_INDEX {
                        continue;
                    }
                    if !self
                        .mesh_index_to_gltf_mesh_primitive
                        .contains_key(&mesh_index)
                    {
                        let mesh = scene.mesh(mesh_index);
                        let transform = self.base_mesh_transforms[mesh_index];
                        if !self.add_draco_mesh_with_material(
                            mesh,
                            instance.material_index,
                            &instance.materials_variants_mappings,
                            &transform,
                        ) {
                            return Status::new(
                                StatusCode::DracoError,
                                "Adding a Draco mesh failed.",
                            );
                        }
                        let gltf_mesh_index = (self.meshes.len() - 1) as i32;
                        let gltf_primitive_index =
                            (self.meshes.last().unwrap().primitives.len() - 1) as i32;
                        self.mesh_index_to_gltf_mesh_primitive
                            .insert(mesh_index, (gltf_mesh_index, gltf_primitive_index));
                    } else {
                        let (gltf_mesh_index, gltf_primitive_index) =
                            self.mesh_index_to_gltf_mesh_primitive[&mesh_index];
                        let mut primitive = self.meshes[gltf_mesh_index as usize].primitives
                            [gltf_primitive_index as usize]
                            .clone();
                        primitive.material = instance.material_index;
                        primitive.material_variants_mappings =
                            instance.materials_variants_mappings.clone();
                        let mesh = scene.mesh(mesh_index);
                        primitive.mesh_features.clear();
                        primitive.mesh_features.reserve(mesh.num_mesh_features());
                        for j in 0..mesh.num_mesh_features() {
                            let idx = MeshFeaturesIndex::from(j as u32);
                            primitive.mesh_features.push(mesh.mesh_features(idx));
                        }
                        primitive
                            .property_attributes
                            .reserve(mesh.num_property_attributes_indices() as usize);
                        for j in 0..mesh.num_property_attributes_indices() {
                            primitive
                                .property_attributes
                                .push(mesh.property_attributes_index(j));
                        }
                        self.meshes
                            .last_mut()
                            .expect("mesh list empty")
                            .primitives
                            .push(primitive);
                    }
                }
                self.mesh_group_index_to_gltf_mesh
                    .insert(mesh_group_index, (self.meshes.len() - 1) as i32);
            }
            node.mesh_index = self.mesh_group_index_to_gltf_mesh[&mesh_group_index];
        }

        node.skin_index = scene_node.skin_index().value() as i32;
        node.light_index = scene_node.light_index().value() as i32;
        node.instance_array_index = scene_node.instance_array_index().value() as i32;
        self.nodes.push(node);
        ok_status()
    }

    fn add_animations(&mut self, scene: &Scene) -> Status {
        if scene.num_animations() == 0 {
            return ok_status();
        }

        let mut node_animation_data_to_accessor: HashMap<(i32, i32), i32> = HashMap::new();
        let mut data_to_index_map: HashMap<NodeAnimationDataHash, i32> = HashMap::new();

        for i in 0..scene.num_animations() {
            let animation = scene.animation(AnimationIndex::from(i as u32));
            for j in 0..animation.num_node_animation_data() {
                let node_animation_data = animation.node_animation_data(j).unwrap();
                let nadh = NodeAnimationDataHash::new(node_animation_data);
                let index = if let Some(existing) = data_to_index_map.get(&nadh) {
                    *existing
                } else {
                    let status_or = self.add_node_animation_data(nadh.node_animation_data());
                    if !status_or.is_ok() {
                        return status_or.status().clone();
                    }
                    let added_index = status_or.into_value();
                    data_to_index_map.insert(nadh, added_index);
                    added_index
                };
                node_animation_data_to_accessor.insert((i as i32, j), index);
            }
        }

        for i in 0..scene.num_animations() {
            let animation = scene.animation(AnimationIndex::from(i as u32));
            let mut new_animation = EncoderAnimation::default();
            new_animation.name = animation.name().to_string();

            for j in 0..animation.num_samplers() {
                let sampler = animation.sampler(j).unwrap();
                let input_key = (i as i32, sampler.input_index);
                let input_index = match node_animation_data_to_accessor.get(&input_key) {
                    Some(index) => *index,
                    None => {
                        return Status::new(
                            StatusCode::DracoError,
                            "Could not find animation accessor input index.",
                        )
                    }
                };
                let output_key = (i as i32, sampler.output_index);
                let output_index = match node_animation_data_to_accessor.get(&output_key) {
                    Some(index) => *index,
                    None => {
                        return Status::new(
                            StatusCode::DracoError,
                            "Could not find animation accessor output index.",
                        )
                    }
                };

                let mut new_sampler = Box::new(AnimationSampler::default());
                new_sampler.input_index = input_index;
                new_sampler.output_index = output_index;
                new_sampler.interpolation_type = sampler.interpolation_type;

                if self.output_type == OutputType::Compact {
                    let idx = new_sampler.output_index as usize;
                    if idx < self.accessors.len() {
                        self.accessors[idx].min.clear();
                        self.accessors[idx].max.clear();
                    }
                }

                new_animation.samplers.push(new_sampler);
            }

            for j in 0..animation.num_channels() {
                let channel = animation.channel(j).unwrap();
                let mut new_channel = Box::new(AnimationChannel::default());
                new_channel.copy_from(channel);
                new_animation.channels.push(new_channel);
            }

            self.animations.push(new_animation);
        }

        ok_status()
    }

    fn add_node_animation_data(
        &mut self,
        node_animation_data: &NodeAnimationData,
    ) -> StatusOr<i32> {
        let buffer_start_offset = self.buffer.size();
        let component_size = node_animation_data.component_size() as usize;
        let num_components = node_animation_data.num_components() as usize;
        let data = node_animation_data.data();
        if data.is_empty() {
            return StatusOr::new_status(Status::new(
                StatusCode::DracoError,
                "NodeAnimationData is empty.",
            ));
        }

        let mut min_values = vec![data[0]; num_components];
        let mut max_values = min_values.clone();

        for i in 0..node_animation_data.count() as usize {
            for j in 0..num_components {
                let value = data[i * num_components + j];
                if value < min_values[j] {
                    min_values[j] = value;
                }
                if value > max_values[j] {
                    max_values[j] = value;
                }
                let _ = component_size;
                self.buffer.encode(value);
            }
        }

        if !self.pad_buffer() {
            return StatusOr::new_status(Status::new(
                StatusCode::DracoError,
                "AddNodeAnimationData: PadBuffer returned DRACO_ERROR.",
            ));
        }

        let mut buffer_view = GltfBufferView::default();
        buffer_view.buffer_byte_offset = buffer_start_offset as i64;
        buffer_view.byte_length = (self.buffer.size() - buffer_start_offset) as i64;
        self.buffer_views.push(buffer_view);

        let mut accessor = GltfAccessor::default();
        accessor.buffer_view_index = (self.buffer_views.len() - 1) as i32;
        accessor.component_type = ComponentType::Float.as_i32();
        accessor.count = node_animation_data.count() as i64;
        for j in 0..num_components {
            accessor.max.push(GltfValue::from_f32(max_values[j]));
            accessor.min.push(GltfValue::from_f32(min_values[j]));
        }
        accessor.type_name = node_animation_data.type_as_string().to_string();
        accessor.normalized = node_animation_data.normalized();
        self.accessors.push(accessor);
        StatusOr::new_value((self.accessors.len() - 1) as i32)
    }

    fn add_skins(&mut self, scene: &Scene) -> Status {
        if scene.num_skins() == 0 {
            return ok_status();
        }
        for i in 0..scene.num_skins() {
            let skin = scene.skin(SkinIndex::from(i as u32));
            let status_or = self.add_node_animation_data(skin.inverse_bind_matrices());
            if !status_or.is_ok() {
                return status_or.status().clone();
            }
            let output_accessor_index = status_or.into_value();
            let mut encoder_skin = EncoderSkin::default();
            encoder_skin.inverse_bind_matrices_index = output_accessor_index;
            encoder_skin.joints.reserve(skin.num_joints() as usize);
            for j in 0..skin.num_joints() {
                encoder_skin.joints.push(skin.joint(j).value() as i32);
            }
            encoder_skin.skeleton_index = skin.joint_root().value() as i32;
            self.skins.push(encoder_skin);
        }
        ok_status()
    }

    fn add_lights(&mut self, scene: &Scene) -> Status {
        if scene.num_lights() == 0 {
            return ok_status();
        }
        for i in 0..scene.num_lights() {
            let mut light = Light::new();
            light.copy_from(scene.light(LightIndex::from(i as u32)));
            self.lights.push(light);
        }
        ok_status()
    }

    fn add_materials_variants_names(&mut self, scene: &Scene) -> Status {
        let library = scene.material_library();
        for i in 0..library.num_materials_variants() {
            self.materials_variants_names
                .push(library.materials_variant_name(i).to_string());
        }
        ok_status()
    }

    fn add_instance_arrays(&mut self, scene: &Scene) -> Status {
        if scene.num_instance_arrays() == 0 {
            return ok_status();
        }

        let mut t_data = Vec::<f32>::new();
        let mut r_data = Vec::<f32>::new();
        let mut s_data = Vec::<f32>::new();

        for i in 0..scene.num_instance_arrays() {
            let array = scene.instance_array(InstanceArrayIndex::from(i as u32));
            let mut is_t_set = false;
            let mut is_r_set = false;
            let mut is_s_set = false;
            for j in 0..array.num_instances() {
                let instance = array.instance(j);
                if instance.trs.translation_set() {
                    is_t_set = true;
                }
                if instance.trs.rotation_set() {
                    is_r_set = true;
                }
                if instance.trs.scale_set() {
                    is_s_set = true;
                }
            }

            t_data.clear();
            r_data.clear();
            s_data.clear();
            if is_t_set {
                t_data.reserve((array.num_instances() * 3) as usize);
            }
            if is_r_set {
                r_data.reserve((array.num_instances() * 4) as usize);
            }
            if is_s_set {
                s_data.reserve((array.num_instances() * 3) as usize);
            }

            for j in 0..array.num_instances() {
                let instance = array.instance(j);
                if is_t_set {
                    let t_vec_or = instance.trs.translation();
                    if !t_vec_or.is_ok() {
                        return t_vec_or.status().clone();
                    }
                    let t_vec = t_vec_or.into_value();
                    t_data.push(t_vec.x as f32);
                    t_data.push(t_vec.y as f32);
                    t_data.push(t_vec.z as f32);
                }
                if is_r_set {
                    let r_vec_or = instance.trs.rotation();
                    if !r_vec_or.is_ok() {
                        return r_vec_or.status().clone();
                    }
                    let r_vec = r_vec_or.into_value();
                    r_data.push(r_vec.x as f32);
                    r_data.push(r_vec.y as f32);
                    r_data.push(r_vec.z as f32);
                    r_data.push(r_vec.w as f32);
                }
                if is_s_set {
                    let s_vec_or = instance.trs.scale();
                    if !s_vec_or.is_ok() {
                        return s_vec_or.status().clone();
                    }
                    let s_vec = s_vec_or.into_value();
                    s_data.push(s_vec.x as f32);
                    s_data.push(s_vec.y as f32);
                    s_data.push(s_vec.z as f32);
                }
            }

            let mut accessors = EncoderInstanceArray::default();
            if is_t_set {
                let status_or = self.add_data(&t_data, 3);
                if !status_or.is_ok() {
                    return status_or.status().clone();
                }
                accessors.translation = status_or.into_value();
            }
            if is_r_set {
                let status_or = self.add_data(&r_data, 4);
                if !status_or.is_ok() {
                    return status_or.status().clone();
                }
                accessors.rotation = status_or.into_value();
            }
            if is_s_set {
                let status_or = self.add_data(&s_data, 3);
                if !status_or.is_ok() {
                    return status_or.status().clone();
                }
                accessors.scale = status_or.into_value();
            }

            self.instance_arrays.push(accessors);
        }
        ok_status()
    }

    fn add_structural_metadata_mesh(&mut self, mesh: &'a Mesh) {
        self.structural_metadata = Some(mesh.structural_metadata());
    }

    fn add_structural_metadata_scene(&mut self, scene: &'a Scene) {
        self.structural_metadata = Some(scene.structural_metadata());
    }

    fn add_data(&mut self, data: &[f32], num_components: i32) -> StatusOr<i32> {
        let type_name = match num_components {
            3 => "VEC3",
            4 => "VEC4",
            _ => {
                return StatusOr::new_status(Status::new(
                    StatusCode::DracoError,
                    "Unsupported number of components.",
                ))
            }
        };

        let buffer_start_offset = self.buffer.size();
        let num_components = num_components as usize;
        if data.is_empty() {
            return StatusOr::new_status(Status::new(
                StatusCode::DracoError,
                "AddData: empty data.",
            ));
        }

        let mut min_values = vec![data[0]; num_components];
        let mut max_values = min_values.clone();
        let count = data.len() / num_components;
        for i in 0..count {
            for j in 0..num_components {
                let value = data[i * num_components + j];
                if value < min_values[j] {
                    min_values[j] = value;
                }
                if value > max_values[j] {
                    max_values[j] = value;
                }
                self.buffer.encode(value);
            }
        }

        if !self.pad_buffer() {
            return StatusOr::new_status(Status::new(
                StatusCode::DracoError,
                "AddArray: PadBuffer returned DRACO_ERROR.",
            ));
        }

        let mut buffer_view = GltfBufferView::default();
        buffer_view.buffer_byte_offset = buffer_start_offset as i64;
        buffer_view.byte_length = (self.buffer.size() - buffer_start_offset) as i64;
        self.buffer_views.push(buffer_view);

        let mut accessor = GltfAccessor::default();
        accessor.buffer_view_index = (self.buffer_views.len() - 1) as i32;
        accessor.component_type = ComponentType::Float.as_i32();
        accessor.count = count as i64;
        for j in 0..num_components {
            accessor.max.push(GltfValue::from_f32(max_values[j]));
            accessor.min.push(GltfValue::from_f32(min_values[j]));
        }
        accessor.type_name = type_name.to_string();
        accessor.normalized = false;
        self.accessors.push(accessor);
        StatusOr::new_value((self.accessors.len() - 1) as i32)
    }

    fn add_buffer_view(
        &mut self,
        data: &draco_core::metadata::property_table::Data,
    ) -> StatusOr<i32> {
        let buffer_start_offset = self.buffer.size();
        self.buffer.encode_bytes(&data.data);
        if !self.pad_buffer() {
            return StatusOr::new_status(Status::new(
                StatusCode::DracoError,
                "AddBufferView: PadBuffer returned DRACO_ERROR.",
            ));
        }
        let mut buffer_view = GltfBufferView::default();
        buffer_view.buffer_byte_offset = buffer_start_offset as i64;
        buffer_view.byte_length = (self.buffer.size() - buffer_start_offset) as i64;
        buffer_view.target = data.target;
        self.buffer_views.push(buffer_view);
        StatusOr::new_value((self.buffer_views.len() - 1) as i32)
    }

    fn encode_asset_property(&mut self, buf_out: &mut EncoderBuffer) -> bool {
        self.gltf_json.begin_object_named("asset");
        self.gltf_json.output_named_string("version", &self.version);
        self.gltf_json
            .output_named_string("generator", &self.generator);
        if !self.copyright.is_empty() {
            self.gltf_json
                .output_named_string("copyright", &self.copyright);
        }
        self.gltf_json.end_object();
        let asset_str = self.gltf_json.move_data();
        buf_out.encode_bytes(asset_str.as_bytes())
    }

    fn encode_scenes_property(&mut self, buf_out: &mut EncoderBuffer) -> bool {
        self.gltf_json.begin_array_named("scenes");
        self.gltf_json.begin_object();
        self.gltf_json.begin_array_named("nodes");
        for i in 0..self.nodes.len() {
            if self.nodes[i].root_node {
                self.gltf_json.output_value(i as i32);
            }
        }
        self.gltf_json.end_array();
        self.gltf_json.end_object();
        self.gltf_json.end_array();
        let asset_str = self.gltf_json.move_data();
        buf_out.encode_bytes(asset_str.as_bytes())
    }

    fn encode_initial_scene_property(&mut self, buf_out: &mut EncoderBuffer) -> bool {
        self.gltf_json.output_named_value("scene", self.scene_index);
        let asset_str = self.gltf_json.move_data();
        buf_out.encode_bytes(asset_str.as_bytes())
    }

    fn encode_nodes_property(&mut self, buf_out: &mut EncoderBuffer) -> bool {
        self.gltf_json.begin_array_named("nodes");
        for node in &self.nodes {
            self.gltf_json.begin_object();
            if !node.name.is_empty() {
                self.gltf_json.output_named_string("name", &node.name);
            }
            if node.mesh_index >= 0 {
                self.gltf_json.output_named_value("mesh", node.mesh_index);
            }
            if node.skin_index >= 0 {
                self.gltf_json.output_named_value("skin", node.skin_index);
            }
            if node.instance_array_index >= 0 || node.light_index >= 0 {
                self.gltf_json.begin_object_named("extensions");
                if node.instance_array_index >= 0 {
                    self.gltf_json.begin_object_named("EXT_mesh_gpu_instancing");
                    self.gltf_json.begin_object_named("attributes");
                    let accessors = self.instance_arrays[node.instance_array_index as usize];
                    if accessors.translation != -1 {
                        self.gltf_json
                            .output_named_value("TRANSLATION", accessors.translation);
                    }
                    if accessors.rotation != -1 {
                        self.gltf_json
                            .output_named_value("ROTATION", accessors.rotation);
                    }
                    if accessors.scale != -1 {
                        self.gltf_json.output_named_value("SCALE", accessors.scale);
                    }
                    self.gltf_json.end_object();
                    self.gltf_json.end_object();
                }
                if node.light_index >= 0 {
                    self.gltf_json.begin_object_named("KHR_lights_punctual");
                    self.gltf_json.output_named_value("light", node.light_index);
                    self.gltf_json.end_object();
                }
                self.gltf_json.end_object();
            }

            if !node.children_indices.is_empty() {
                self.gltf_json.begin_array_named("children");
                for child in &node.children_indices {
                    self.gltf_json.output_value(*child);
                }
                self.gltf_json.end_array();
            }

            if !node.trs_matrix.is_matrix_identity() {
                if node.trs_matrix.is_matrix_translation_only() {
                    let matrix = node.trs_matrix.matrix().into_value();
                    self.gltf_json.begin_array_named("translation");
                    self.gltf_json.output_value(matrix.m[0][3]);
                    self.gltf_json.output_value(matrix.m[1][3]);
                    self.gltf_json.output_value(matrix.m[2][3]);
                    self.gltf_json.end_array();
                } else {
                    let matrix = node.trs_matrix.matrix().into_value();
                    self.gltf_json.begin_array_named("matrix");
                    for j in 0..4 {
                        for k in 0..4 {
                            self.gltf_json.output_value(matrix.m[k][j]);
                        }
                    }
                    self.gltf_json.end_array();
                }
            } else {
                if node.trs_matrix.translation_set() {
                    let translation = node.trs_matrix.translation().into_value();
                    self.gltf_json.begin_array_named("translation");
                    self.gltf_json.output_value(translation.x);
                    self.gltf_json.output_value(translation.y);
                    self.gltf_json.output_value(translation.z);
                    self.gltf_json.end_array();
                }
                if node.trs_matrix.rotation_set() {
                    let rotation = node.trs_matrix.rotation().into_value();
                    self.gltf_json.begin_array_named("rotation");
                    self.gltf_json.output_value(rotation.x);
                    self.gltf_json.output_value(rotation.y);
                    self.gltf_json.output_value(rotation.z);
                    self.gltf_json.output_value(rotation.w);
                    self.gltf_json.end_array();
                }
                if node.trs_matrix.scale_set() {
                    let scale = node.trs_matrix.scale().into_value();
                    self.gltf_json.begin_array_named("scale");
                    self.gltf_json.output_value(scale.x);
                    self.gltf_json.output_value(scale.y);
                    self.gltf_json.output_value(scale.z);
                    self.gltf_json.end_array();
                }
            }
            self.gltf_json.end_object();
        }

        self.gltf_json.end_array();
        let asset_str = self.gltf_json.move_data();
        buf_out.encode_bytes(asset_str.as_bytes())
    }

    fn encode_meshes_property(&mut self, buf_out: &mut EncoderBuffer) -> Status {
        self.mesh_features_texture_index = 0;
        self.gltf_json.begin_array_named("meshes");
        let meshes = self.meshes.clone();
        for mesh in &meshes {
            self.gltf_json.begin_object();
            if !mesh.name.is_empty() {
                self.gltf_json.output_named_string("name", &mesh.name);
            }
            if !mesh.primitives.is_empty() {
                self.gltf_json.begin_array_named("primitives");
                for primitive in &mesh.primitives {
                    self.gltf_json.begin_object();
                    self.gltf_json.begin_object_named("attributes");
                    for (name, index) in &primitive.attributes {
                        self.gltf_json.output_named_value(name, *index);
                    }
                    self.gltf_json.end_object();

                    if primitive.indices >= 0 {
                        self.gltf_json
                            .output_named_value("indices", primitive.indices);
                    }
                    self.gltf_json.output_named_value("mode", primitive.mode);
                    if primitive.material >= 0 {
                        self.gltf_json
                            .output_named_value("material", primitive.material);
                    }
                    draco_core::draco_return_if_error!(
                        self.encode_primitive_extensions_property(primitive)
                    );
                    self.gltf_json.end_object();
                }
                self.gltf_json.end_array();
            }
            self.gltf_json.end_object();
        }
        self.gltf_json.end_array();

        let asset_str = self.gltf_json.move_data();
        if !buf_out.encode_bytes(asset_str.as_bytes()) {
            return Status::new(StatusCode::DracoError, "Failed encoding meshes.");
        }
        ok_status()
    }

    fn encode_primitive_extensions_property(&mut self, primitive: &GltfPrimitive<'a>) -> Status {
        let has_draco_mesh_compression = primitive.compressed_mesh_info.buffer_view_index >= 0;
        let has_materials_variants = !primitive.material_variants_mappings.is_empty();
        let has_structural_metadata = !primitive.property_attributes.is_empty();
        let has_mesh_features = !primitive.mesh_features.is_empty();
        if !has_draco_mesh_compression
            && !has_materials_variants
            && !has_mesh_features
            && !has_structural_metadata
        {
            return ok_status();
        }

        self.gltf_json.begin_object_named("extensions");
        if has_draco_mesh_compression {
            self.gltf_json
                .begin_object_named("KHR_draco_mesh_compression");
            self.gltf_json.output_named_value(
                "bufferView",
                primitive.compressed_mesh_info.buffer_view_index,
            );
            self.gltf_json.begin_object_named("attributes");
            for (name, value) in &primitive.compressed_mesh_info.attributes {
                self.gltf_json.output_named_value(name, *value);
            }
            self.gltf_json.end_object();
            self.gltf_json.end_object();
        }
        if has_materials_variants {
            self.gltf_json.begin_object_named("KHR_materials_variants");
            self.gltf_json.begin_array_named("mappings");
            for mapping in &primitive.material_variants_mappings {
                self.gltf_json.begin_object();
                self.gltf_json
                    .output_named_value("material", mapping.material);
                self.gltf_json.begin_array_named("variants");
                for variant in &mapping.variants {
                    self.gltf_json.output_value(*variant);
                }
                self.gltf_json.end_array();
                self.gltf_json.end_object();
            }
            self.gltf_json.end_array();
            self.gltf_json.end_object();
        }
        if has_mesh_features {
            self.gltf_json.begin_object_named("EXT_mesh_features");
            self.gltf_json.begin_array_named("featureIds");
            for features in &primitive.mesh_features {
                self.gltf_json.begin_object();
                if !features.label().is_empty() {
                    self.gltf_json
                        .output_named_string("label", features.label());
                }
                self.gltf_json
                    .output_named_value("featureCount", features.feature_count());
                if features.attribute_index() != -1 {
                    let index = primitive
                        .feature_id_name_indices
                        .get(&features.attribute_index())
                        .cloned()
                        .ok_or_else(|| {
                            Status::new(
                                StatusCode::DracoError,
                                "Missing feature ID attribute mapping.",
                            )
                        });
                    let index = match index {
                        Ok(value) => value,
                        Err(status) => return status,
                    };
                    self.gltf_json.output_named_value("attribute", index);
                }
                if features.property_table_index() != -1 {
                    self.gltf_json
                        .output_named_value("propertyTable", features.property_table_index());
                }
                if features.texture_map().tex_coord_index() != -1 {
                    let texture_map = features.texture_map();
                    let texture = match texture_map.texture() {
                        Some(texture) => texture,
                        None => {
                            return Status::new(
                                StatusCode::DracoError,
                                "Missing mesh feature texture.",
                            )
                        }
                    };
                    let texture_stem = TextureUtils::get_or_generate_target_stem(
                        texture,
                        self.mesh_features_texture_index,
                        "_MeshFeatures",
                    );
                    self.mesh_features_texture_index += 1;

                    let channels = features.texture_channels();
                    let num_channels = if channels.iter().any(|c| *c == 3) {
                        4
                    } else {
                        3
                    };

                    let status_or = self.add_image(&texture_stem, texture, num_channels);
                    if !status_or.is_ok() {
                        return status_or.status().clone();
                    }
                    let image_index = status_or.into_value();
                    let tex_coord_index = texture_map.tex_coord_index();
                    let status = self.encode_texture_map(
                        "texture",
                        image_index,
                        tex_coord_index,
                        None,
                        texture_map,
                        channels,
                    );
                    if !status.is_ok() {
                        return status;
                    }
                }
                if features.null_feature_id() != -1 {
                    self.gltf_json
                        .output_named_value("nullFeatureId", features.null_feature_id());
                }
                self.gltf_json.end_object();
                self.mesh_features_used = true;
            }
            self.gltf_json.end_array();
            self.gltf_json.end_object();
        }
        if has_structural_metadata {
            self.structural_metadata_used = true;
            self.gltf_json.begin_object_named("EXT_structural_metadata");
            self.gltf_json.begin_array_named("propertyAttributes");
            for index in &primitive.property_attributes {
                self.gltf_json.output_value(*index);
            }
            self.gltf_json.end_array();
            self.gltf_json.end_object();
        }
        self.gltf_json.end_object();
        ok_status()
    }

    fn encode_materials(&mut self, buf_out: &mut EncoderBuffer) -> Status {
        if self.material_library.num_materials() == 0 {
            return self.encode_default_material(buf_out);
        }
        self.encode_materials_property(buf_out)
    }

    fn encode_color_material(
        &mut self,
        red: f32,
        green: f32,
        blue: f32,
        alpha: f32,
        metallic: f32,
    ) {
        self.gltf_json.begin_object_named("pbrMetallicRoughness");
        self.gltf_json.begin_array_named("baseColorFactor");
        self.gltf_json.output_value(red);
        self.gltf_json.output_value(green);
        self.gltf_json.output_value(blue);
        self.gltf_json.output_value(alpha);
        self.gltf_json.end_array();
        self.gltf_json
            .output_named_value("metallicFactor", metallic);
        self.gltf_json.end_object();
    }

    fn encode_default_material(&mut self, buf_out: &mut EncoderBuffer) -> Status {
        self.gltf_json.begin_array_named("materials");
        self.gltf_json.begin_object();
        self.encode_color_material(0.75, 0.75, 0.75, 1.0, 0.0);
        self.gltf_json.end_object();
        self.gltf_json.end_array();
        let asset_str = self.gltf_json.move_data();
        if !buf_out.encode_bytes(asset_str.as_bytes()) {
            return Status::new(StatusCode::DracoError, "Error encoding default material.");
        }
        ok_status()
    }

    fn encode_texture_map(
        &mut self,
        object_name: &str,
        image_index: i32,
        tex_coord_index: i32,
        normal_scale: Option<f32>,
        texture_map: &TextureMap,
        channels: &[i32],
    ) -> Status {
        let sampler = TextureSampler::new(
            texture_map.min_filter(),
            texture_map.mag_filter(),
            texture_map.wrapping_mode(),
        );
        let sampler_index = self.add_texture_sampler(sampler).into_value();

        let texture = GltfTexture {
            image_index,
            sampler_index,
        };
        let texture_index = if let Some(index) = self.textures.iter().position(|t| t == &texture) {
            index as i32
        } else {
            self.textures.push(texture);
            (self.textures.len() - 1) as i32
        };

        self.gltf_json.begin_object_named(object_name);
        self.gltf_json.output_named_value("index", texture_index);
        self.gltf_json
            .output_named_value("texCoord", tex_coord_index);
        if object_name == "normalTexture" {
            let scale = normal_scale.unwrap_or(1.0);
            if scale != 1.0 {
                self.gltf_json.output_named_value("scale", scale);
            }
        }

        if object_name == "texture" && !channels.is_empty() {
            self.gltf_json.begin_array_named("channels");
            for channel in channels {
                self.gltf_json.output_value(*channel);
            }
            self.gltf_json.end_array();
        }

        if !TextureTransform::is_default(texture_map.texture_transform()) {
            self.gltf_json.begin_object_named("extensions");
            self.gltf_json.begin_object_named("KHR_texture_transform");
            if texture_map.texture_transform().is_offset_set() {
                let offset = texture_map.texture_transform().offset();
                self.gltf_json.begin_array_named("offset");
                self.gltf_json.output_value(offset[0]);
                self.gltf_json.output_value(offset[1]);
                self.gltf_json.end_array();
            }
            if texture_map.texture_transform().is_rotation_set() {
                self.gltf_json
                    .output_named_value("rotation", texture_map.texture_transform().rotation());
            }
            if texture_map.texture_transform().is_scale_set() {
                let scale = texture_map.texture_transform().scale();
                self.gltf_json.begin_array_named("scale");
                self.gltf_json.output_value(scale[0]);
                self.gltf_json.output_value(scale[1]);
                self.gltf_json.end_array();
            }
            if texture_map.texture_transform().is_tex_coord_set() {
                self.gltf_json
                    .output_named_value("texCoord", texture_map.texture_transform().tex_coord());
            } else {
                self.extensions_required
                    .insert("KHR_texture_transform".to_string());
            }
            self.gltf_json.end_object();
            self.gltf_json.end_object();
            self.extensions_used
                .insert("KHR_texture_transform".to_string());
        }

        self.gltf_json.end_object();
        ok_status()
    }

    fn encode_materials_property(&mut self, buf_out: &mut EncoderBuffer) -> Status {
        self.gltf_json.begin_array_named("materials");
        // SAFETY: Borrow-split — material_library is not modified by any method called below.
        // We read from material_library while mutating other fields (gltf_json, images, textures,
        // texture_samplers, extensions_*). This is the Rust equivalent of C++ disjoint member access.
        // self lives for 'a, so the returned references are valid for 'a.
        let material_library: &'a MaterialLibrary =
            unsafe { &*(&self.material_library as *const MaterialLibrary) };
        for i in 0..material_library.num_materials() {
            let material: &'a Material = match material_library.material(i as i32) {
                Some(m) => m,
                None => return Status::new(StatusCode::DracoError, "Error getting material."),
            };

            let color = material.texture_map_by_type(TextureMapType::Color);
            let metallic = material.texture_map_by_type(TextureMapType::MetallicRoughness);
            let normal = material.texture_map_by_type(TextureMapType::NormalTangentSpace);
            let occlusion = material.texture_map_by_type(TextureMapType::AmbientOcclusion);
            let emissive = material.texture_map_by_type(TextureMapType::Emissive);

            if material.unlit()
                && (color.is_none()
                    || metallic.is_some()
                    || normal.is_some()
                    || occlusion.is_some()
                    || emissive.is_some()
                    || material.metallic_factor() != 0.0
                    || material.roughness_factor() <= 0.5
                    || material.emissive_factor() != Vector3f::new3(0.0, 0.0, 0.0))
            {
                self.extensions_required
                    .insert("KHR_materials_unlit".to_string());
            }

            let mut occlusion_metallic_roughness_image_index = -1;

            self.gltf_json.begin_object();
            self.gltf_json.begin_object_named("pbrMetallicRoughness");
            if let Some(color) = color {
                let texture_stem = TextureUtils::get_or_generate_target_stem(
                    color.texture().unwrap(),
                    i as i32,
                    "_BaseColor",
                );
                let status_or = self.add_image(&texture_stem, color.texture().unwrap(), 4);
                if !status_or.is_ok() {
                    return status_or.status().clone();
                }
                let color_image_index = status_or.into_value();
                draco_core::draco_return_if_error!(self.encode_texture_map(
                    "baseColorTexture",
                    color_image_index,
                    color.tex_coord_index(),
                    None,
                    color,
                    &[],
                ));
            }

            if let (Some(metallic), Some(occlusion)) = (metallic, occlusion) {
                if metallic.tex_coord_index() == occlusion.tex_coord_index() {
                    if std::ptr::eq(metallic.texture().unwrap(), occlusion.texture().unwrap()) {
                        let texture_stem = TextureUtils::get_or_generate_target_stem(
                            metallic.texture().unwrap(),
                            i as i32,
                            "_OcclusionMetallicRoughness",
                        );
                        let status_or =
                            self.add_image(&texture_stem, metallic.texture().unwrap(), 3);
                        if !status_or.is_ok() {
                            return status_or.status().clone();
                        }
                        occlusion_metallic_roughness_image_index = status_or.into_value();
                    }
                    if occlusion_metallic_roughness_image_index != -1 {
                        draco_core::draco_return_if_error!(self.encode_texture_map(
                            "metallicRoughnessTexture",
                            occlusion_metallic_roughness_image_index,
                            metallic.tex_coord_index(),
                            None,
                            metallic,
                            &[],
                        ));
                    }
                }
            }

            if let Some(metallic) = metallic {
                if occlusion_metallic_roughness_image_index == -1 {
                    let texture_stem = TextureUtils::get_or_generate_target_stem(
                        metallic.texture().unwrap(),
                        i as i32,
                        "_MetallicRoughness",
                    );
                    let status_or = self.add_image(&texture_stem, metallic.texture().unwrap(), 3);
                    if !status_or.is_ok() {
                        return status_or.status().clone();
                    }
                    let metallic_roughness_image_index = status_or.into_value();
                    draco_core::draco_return_if_error!(self.encode_texture_map(
                        "metallicRoughnessTexture",
                        metallic_roughness_image_index,
                        metallic.tex_coord_index(),
                        None,
                        metallic,
                        &[],
                    ));
                }
            }

            self.encode_vector4("baseColorFactor", material.color_factor());
            self.gltf_json
                .output_named_value("metallicFactor", material.metallic_factor());
            self.gltf_json
                .output_named_value("roughnessFactor", material.roughness_factor());
            self.gltf_json.end_object();

            if let Some(normal) = normal {
                let texture_stem = TextureUtils::get_or_generate_target_stem(
                    normal.texture().unwrap(),
                    i as i32,
                    "_Normal",
                );
                let status_or = self.add_image(&texture_stem, normal.texture().unwrap(), 3);
                if !status_or.is_ok() {
                    return status_or.status().clone();
                }
                let normal_image_index = status_or.into_value();
                draco_core::draco_return_if_error!(self.encode_texture_map(
                    "normalTexture",
                    normal_image_index,
                    normal.tex_coord_index(),
                    Some(material.normal_texture_scale()),
                    normal,
                    &[],
                ));
            }

            if occlusion_metallic_roughness_image_index != -1 {
                if let Some(metallic) = metallic {
                    draco_core::draco_return_if_error!(self.encode_texture_map(
                        "occlusionTexture",
                        occlusion_metallic_roughness_image_index,
                        metallic.tex_coord_index(),
                        None,
                        metallic,
                        &[],
                    ));
                }
            } else if let Some(occlusion) = occlusion {
                let num_components = TextureUtils::compute_required_num_channels(
                    occlusion.texture().unwrap(),
                    material_library,
                );
                let suffix = if num_components == 1 {
                    "_Occlusion"
                } else {
                    "_OcclusionMetallicRoughness"
                };
                let texture_stem = TextureUtils::get_or_generate_target_stem(
                    occlusion.texture().unwrap(),
                    i as i32,
                    suffix,
                );
                let status_or =
                    self.add_image(&texture_stem, occlusion.texture().unwrap(), num_components);
                if !status_or.is_ok() {
                    return status_or.status().clone();
                }
                let occlusion_image_index = status_or.into_value();
                draco_core::draco_return_if_error!(self.encode_texture_map(
                    "occlusionTexture",
                    occlusion_image_index,
                    occlusion.tex_coord_index(),
                    None,
                    occlusion,
                    &[],
                ));
            }

            if let Some(emissive) = emissive {
                let texture_stem = TextureUtils::get_or_generate_target_stem(
                    emissive.texture().unwrap(),
                    i as i32,
                    "_Emissive",
                );
                let status_or = self.add_image(&texture_stem, emissive.texture().unwrap(), 3);
                if !status_or.is_ok() {
                    return status_or.status().clone();
                }
                let emissive_image_index = status_or.into_value();
                draco_core::draco_return_if_error!(self.encode_texture_map(
                    "emissiveTexture",
                    emissive_image_index,
                    emissive.tex_coord_index(),
                    None,
                    emissive,
                    &[],
                ));
            }

            self.encode_vector3("emissiveFactor", material.emissive_factor());

            match material.transparency_mode() {
                MaterialTransparencyMode::Mask => {
                    self.gltf_json.output_named_string("alphaMode", "MASK");
                    self.gltf_json
                        .output_named_value("alphaCutoff", material.alpha_cutoff());
                }
                MaterialTransparencyMode::Blend => {
                    self.gltf_json.output_named_string("alphaMode", "BLEND");
                }
                _ => {
                    self.gltf_json.output_named_string("alphaMode", "OPAQUE");
                }
            }
            if !material.name().is_empty() {
                self.gltf_json.output_named_string("name", material.name());
            }
            if material.double_sided() {
                self.gltf_json
                    .output_named_bool("doubleSided", material.double_sided());
            }

            if material.unlit()
                || material.has_sheen()
                || material.has_transmission()
                || material.has_clearcoat()
                || material.has_volume()
                || material.has_ior()
                || material.has_specular()
            {
                self.gltf_json.begin_object_named("extensions");
                if material.unlit() {
                    self.encode_material_unlit_extension();
                } else {
                    let defaults = Material::new();
                    if material.has_sheen() {
                        draco_core::draco_return_if_error!(
                            self.encode_material_sheen_extension(material, &defaults, i as i32,)
                        );
                    }
                    if material.has_transmission() {
                        draco_core::draco_return_if_error!(
                            self.encode_material_transmission_extension(
                                material, &defaults, i as i32,
                            )
                        );
                    }
                    if material.has_clearcoat() {
                        draco_core::draco_return_if_error!(self
                            .encode_material_clearcoat_extension(material, &defaults, i as i32,));
                    }
                    if material.has_volume() {
                        draco_core::draco_return_if_error!(
                            self.encode_material_volume_extension(material, &defaults, i as i32,)
                        );
                    }
                    if material.has_ior() {
                        draco_core::draco_return_if_error!(
                            self.encode_material_ior_extension(material, &defaults)
                        );
                    }
                    if material.has_specular() {
                        draco_core::draco_return_if_error!(
                            self.encode_material_specular_extension(material, &defaults, i as i32,)
                        );
                    }
                }
                self.gltf_json.end_object();
            }

            self.gltf_json.end_object();
        }

        self.gltf_json.end_array();

        if !self.textures.is_empty() {
            self.gltf_json.begin_array_named("textures");
            let textures = self.textures.clone();
            for texture in &textures {
                let image_index = texture.image_index;
                let image_index_usize = image_index as usize;
                let sampler_index = texture.sampler_index;
                let mime_type = self.images[image_index_usize].mime_type.clone();
                self.gltf_json.begin_object();
                if mime_type == "image/webp" {
                    self.gltf_json.begin_object_named("extensions");
                    self.gltf_json.begin_object_named("EXT_texture_webp");
                    self.gltf_json.output_named_value("source", image_index);
                    self.gltf_json.end_object();
                    self.gltf_json.end_object();
                } else if mime_type == "image/ktx2" {
                    self.gltf_json.begin_object_named("extensions");
                    self.gltf_json.begin_object_named("KHR_texture_basisu");
                    self.gltf_json.output_named_value("source", image_index);
                    self.gltf_json.end_object();
                    self.gltf_json.end_object();
                } else {
                    self.gltf_json.output_named_value("source", image_index);
                }
                if sampler_index >= 0 {
                    self.gltf_json.output_named_value("sampler", sampler_index);
                }
                self.gltf_json.end_object();
            }
            self.gltf_json.end_array();
        }

        if !self.texture_samplers.is_empty() {
            self.gltf_json.begin_array_named("samplers");
            let samplers = self.texture_samplers.clone();
            for sampler in &samplers {
                self.gltf_json.begin_object();
                let mode_s = texture_axis_wrapping_mode_to_gltf_value(sampler.wrapping_mode.s);
                let mode_t = texture_axis_wrapping_mode_to_gltf_value(sampler.wrapping_mode.t);
                self.gltf_json.output_named_value("wrapS", mode_s);
                self.gltf_json.output_named_value("wrapT", mode_t);
                if sampler.min_filter != TextureMapFilterType::Unspecified {
                    self.gltf_json.output_named_value(
                        "minFilter",
                        texture_filter_type_to_gltf_value(sampler.min_filter),
                    );
                }
                if sampler.mag_filter != TextureMapFilterType::Unspecified {
                    self.gltf_json.output_named_value(
                        "magFilter",
                        texture_filter_type_to_gltf_value(sampler.mag_filter),
                    );
                }
                self.gltf_json.end_object();
            }
            self.gltf_json.end_array();
        }

        if !self.images.is_empty() {
            self.gltf_json.begin_array_named("images");
            for i in 0..self.images.len() {
                if self.add_images_to_buffer {
                    draco_core::draco_return_if_error!(self.save_image_to_buffer(i));
                }
                let (buffer_view, mime_type, image_name) = {
                    let image = &self.images[i];
                    (
                        image.buffer_view,
                        image.mime_type.clone(),
                        image.image_name.clone(),
                    )
                };
                self.gltf_json.begin_object();
                if buffer_view >= 0 {
                    self.gltf_json.output_named_value("bufferView", buffer_view);
                    self.gltf_json.output_named_string("mimeType", &mime_type);
                } else {
                    self.gltf_json.output_named_string("uri", &image_name);
                }
                self.gltf_json.end_object();
            }
            self.gltf_json.end_array();
        }

        let asset_str = self.gltf_json.move_data();
        if !buf_out.encode_bytes(asset_str.as_bytes()) {
            return Status::new(StatusCode::DracoError, "Error encoding materials.");
        }
        ok_status()
    }

    fn encode_material_unlit_extension(&mut self) {
        self.extensions_used
            .insert("KHR_materials_unlit".to_string());
        self.gltf_json.begin_object_named("KHR_materials_unlit");
        self.gltf_json.end_object();
    }

    fn encode_material_sheen_extension(
        &mut self,
        material: &'a Material,
        defaults: &Material,
        material_index: i32,
    ) -> Status {
        self.extensions_used
            .insert("KHR_materials_sheen".to_string());
        self.gltf_json.begin_object_named("KHR_materials_sheen");

        if material.sheen_color_factor() != defaults.sheen_color_factor() {
            self.encode_vector3("sheenColorFactor", material.sheen_color_factor());
        }
        if material.sheen_roughness_factor() != defaults.sheen_roughness_factor() {
            self.gltf_json
                .output_named_value("sheenRoughnessFactor", material.sheen_roughness_factor());
        }

        draco_core::draco_return_if_error!(self.encode_texture(
            "sheenColorTexture",
            "_SheenColor",
            TextureMapType::SheenColor,
            -1,
            material,
            material_index,
        ));
        draco_core::draco_return_if_error!(self.encode_texture(
            "sheenRoughnessTexture",
            "_SheenRoughness",
            TextureMapType::SheenRoughness,
            4,
            material,
            material_index,
        ));

        self.gltf_json.end_object();
        ok_status()
    }

    fn encode_material_transmission_extension(
        &mut self,
        material: &'a Material,
        defaults: &Material,
        material_index: i32,
    ) -> Status {
        self.extensions_used
            .insert("KHR_materials_transmission".to_string());
        self.gltf_json
            .begin_object_named("KHR_materials_transmission");

        if material.transmission_factor() != defaults.transmission_factor() {
            self.gltf_json
                .output_named_value("transmissionFactor", material.transmission_factor());
        }

        draco_core::draco_return_if_error!(self.encode_texture(
            "transmissionTexture",
            "_Transmission",
            TextureMapType::Transmission,
            3,
            material,
            material_index,
        ));

        self.gltf_json.end_object();
        ok_status()
    }

    fn encode_material_clearcoat_extension(
        &mut self,
        material: &'a Material,
        defaults: &Material,
        material_index: i32,
    ) -> Status {
        self.extensions_used
            .insert("KHR_materials_clearcoat".to_string());
        self.gltf_json.begin_object_named("KHR_materials_clearcoat");

        if material.clearcoat_factor() != defaults.clearcoat_factor() {
            self.gltf_json
                .output_named_value("clearcoatFactor", material.clearcoat_factor());
        }
        if material.clearcoat_roughness_factor() != defaults.clearcoat_roughness_factor() {
            self.gltf_json.output_named_value(
                "clearcoatRoughnessFactor",
                material.clearcoat_roughness_factor(),
            );
        }

        draco_core::draco_return_if_error!(self.encode_texture(
            "clearcoatTexture",
            "_Clearcoat",
            TextureMapType::Clearcoat,
            3,
            material,
            material_index,
        ));
        draco_core::draco_return_if_error!(self.encode_texture(
            "clearcoatRoughnessTexture",
            "_ClearcoatRoughness",
            TextureMapType::ClearcoatRoughness,
            3,
            material,
            material_index,
        ));
        draco_core::draco_return_if_error!(self.encode_texture(
            "clearcoatNormalTexture",
            "_ClearcoatNormal",
            TextureMapType::ClearcoatNormal,
            3,
            material,
            material_index,
        ));

        self.gltf_json.end_object();
        ok_status()
    }

    fn encode_material_volume_extension(
        &mut self,
        material: &'a Material,
        defaults: &Material,
        material_index: i32,
    ) -> Status {
        self.extensions_used
            .insert("KHR_materials_volume".to_string());
        self.gltf_json.begin_object_named("KHR_materials_volume");

        if material.thickness_factor() != defaults.thickness_factor() {
            self.gltf_json
                .output_named_value("thicknessFactor", material.thickness_factor());
        }
        if material.attenuation_distance() != defaults.attenuation_distance() {
            self.gltf_json
                .output_named_value("attenuationDistance", material.attenuation_distance());
        }
        if material.attenuation_color() != defaults.attenuation_color() {
            self.encode_vector3("attenuationColor", material.attenuation_color());
        }

        draco_core::draco_return_if_error!(self.encode_texture(
            "thicknessTexture",
            "_Thickness",
            TextureMapType::Thickness,
            3,
            material,
            material_index,
        ));

        self.gltf_json.end_object();
        ok_status()
    }

    fn encode_material_ior_extension(
        &mut self,
        material: &Material,
        defaults: &Material,
    ) -> Status {
        self.extensions_used.insert("KHR_materials_ior".to_string());
        self.gltf_json.begin_object_named("KHR_materials_ior");
        if material.ior() != defaults.ior() {
            self.gltf_json.output_named_value("ior", material.ior());
        }
        self.gltf_json.end_object();
        ok_status()
    }

    fn encode_material_specular_extension(
        &mut self,
        material: &'a Material,
        defaults: &Material,
        material_index: i32,
    ) -> Status {
        self.extensions_used
            .insert("KHR_materials_specular".to_string());
        self.gltf_json.begin_object_named("KHR_materials_specular");

        if material.specular_factor() != defaults.specular_factor() {
            self.gltf_json
                .output_named_value("specularFactor", material.specular_factor());
        }
        if material.specular_color_factor() != defaults.specular_color_factor() {
            self.encode_vector3("specularColorFactor", material.specular_color_factor());
        }

        draco_core::draco_return_if_error!(self.encode_texture(
            "specularTexture",
            "_Specular",
            TextureMapType::Specular,
            4,
            material,
            material_index,
        ));
        draco_core::draco_return_if_error!(self.encode_texture(
            "specularColorTexture",
            "_SpecularColor",
            TextureMapType::SpecularColor,
            -1,
            material,
            material_index,
        ));

        self.gltf_json.end_object();
        ok_status()
    }

    fn encode_texture(
        &mut self,
        name: &str,
        stem_suffix: &str,
        texture_type: TextureMapType,
        mut num_components: i32,
        material: &'a Material,
        material_index: i32,
    ) -> Status {
        let texture_map = material.texture_map_by_type(texture_type);
        if let Some(texture_map) = texture_map {
            if num_components == -1 {
                num_components = 4;
            }
            let texture_stem = TextureUtils::get_or_generate_target_stem(
                texture_map.texture().unwrap(),
                material_index,
                stem_suffix,
            );
            let status_or = self.add_image(
                &texture_stem,
                texture_map.texture().unwrap(),
                num_components,
            );
            if !status_or.is_ok() {
                return status_or.status().clone();
            }
            let image_index = status_or.into_value();
            draco_core::draco_return_if_error!(self.encode_texture_map(
                name,
                image_index,
                texture_map.tex_coord_index(),
                if name == "normalTexture" {
                    Some(material.normal_texture_scale())
                } else {
                    None
                },
                texture_map,
                &[],
            ));
        }
        ok_status()
    }

    fn encode_animations_property(&mut self, buf_out: &mut EncoderBuffer) -> Status {
        if self.animations.is_empty() {
            return ok_status();
        }
        self.gltf_json.begin_array_named("animations");
        for animation in &self.animations {
            self.gltf_json.begin_object();
            if !animation.name.is_empty() {
                self.gltf_json.output_named_string("name", &animation.name);
            }
            self.gltf_json.begin_array_named("samplers");
            for sampler in &animation.samplers {
                self.gltf_json.begin_object();
                self.gltf_json
                    .output_named_value("input", sampler.input_index);
                self.gltf_json
                    .output_named_string("interpolation", sampler.interpolation_type.to_string());
                self.gltf_json
                    .output_named_value("output", sampler.output_index);
                self.gltf_json.end_object();
            }
            self.gltf_json.end_array();

            self.gltf_json.begin_array_named("channels");
            for channel in &animation.channels {
                self.gltf_json.begin_object();
                self.gltf_json
                    .output_named_value("sampler", channel.sampler_index);
                self.gltf_json.begin_object_named("target");
                self.gltf_json
                    .output_named_value("node", channel.target_index);
                self.gltf_json
                    .output_named_string("path", channel.transformation_type.to_string());
                self.gltf_json.end_object();
                self.gltf_json.end_object();
            }
            self.gltf_json.end_array();

            self.gltf_json.end_object();
        }
        self.gltf_json.end_array();
        let asset_str = self.gltf_json.move_data();
        if !buf_out.encode_bytes(asset_str.as_bytes()) {
            return Status::new(StatusCode::DracoError, "Could not encode animations.");
        }
        ok_status()
    }

    fn encode_skins_property(&mut self, buf_out: &mut EncoderBuffer) -> Status {
        if self.skins.is_empty() {
            return ok_status();
        }
        self.gltf_json.begin_array_named("skins");
        for skin in &self.skins {
            self.gltf_json.begin_object();
            if skin.inverse_bind_matrices_index >= 0 {
                self.gltf_json
                    .output_named_value("inverseBindMatrices", skin.inverse_bind_matrices_index);
            }
            if skin.skeleton_index >= 0 {
                self.gltf_json
                    .output_named_value("skeleton", skin.skeleton_index);
            }
            if !skin.joints.is_empty() {
                self.gltf_json.begin_array_named("joints");
                for joint in &skin.joints {
                    self.gltf_json.output_value(*joint);
                }
                self.gltf_json.end_array();
            }
            self.gltf_json.end_object();
        }
        self.gltf_json.end_array();
        let asset_str = self.gltf_json.move_data();
        if !buf_out.encode_bytes(asset_str.as_bytes()) {
            return Status::new(StatusCode::DracoError, "Could not encode animations.");
        }
        ok_status()
    }

    fn encode_top_level_extensions_property(&mut self, _buf_out: &mut EncoderBuffer) -> Status {
        let structural_metadata = match self.structural_metadata {
            Some(meta) => meta,
            None => {
                if self.lights.is_empty()
                    && self.materials_variants_names.is_empty()
                    && self.cesium_rtc.is_empty()
                {
                    return ok_status();
                }
                self.gltf_json.begin_object_named("extensions");
                draco_core::draco_return_if_error!(self.encode_lights_property());
                draco_core::draco_return_if_error!(self.encode_materials_variants_names_property());
                draco_core::draco_return_if_error!(self.encode_cesium_rtc_property());
                self.gltf_json.end_object();
                return ok_status();
            }
        };

        if self.lights.is_empty()
            && self.materials_variants_names.is_empty()
            && structural_metadata.num_property_tables() == 0
            && structural_metadata.num_property_attributes() == 0
            && self.cesium_rtc.is_empty()
        {
            return ok_status();
        }

        self.gltf_json.begin_object_named("extensions");
        draco_core::draco_return_if_error!(self.encode_lights_property());
        draco_core::draco_return_if_error!(self.encode_materials_variants_names_property());
        draco_core::draco_return_if_error!(self.encode_structural_metadata_property());
        draco_core::draco_return_if_error!(self.encode_cesium_rtc_property());
        self.gltf_json.end_object();
        ok_status()
    }

    fn encode_lights_property(&mut self) -> Status {
        if self.lights.is_empty() {
            return ok_status();
        }
        self.gltf_json.begin_object_named("KHR_lights_punctual");
        self.gltf_json.begin_array_named("lights");
        let defaults = Light::new();
        for light in &self.lights {
            self.gltf_json.begin_object();
            if light.name() != defaults.name() {
                self.gltf_json.output_named_string("name", light.name());
            }
            if light.color() != defaults.color() {
                self.gltf_json.begin_array_named("color");
                self.gltf_json.output_value(light.color()[0]);
                self.gltf_json.output_value(light.color()[1]);
                self.gltf_json.output_value(light.color()[2]);
                self.gltf_json.end_array();
            }
            if light.intensity() != defaults.intensity() {
                self.gltf_json
                    .output_named_value("intensity", light.intensity());
            }
            match light.light_type() {
                LightType::Directional => {
                    self.gltf_json.output_named_string("type", "directional");
                }
                LightType::Point => {
                    self.gltf_json.output_named_string("type", "point");
                }
                LightType::Spot => {
                    self.gltf_json.output_named_string("type", "spot");
                }
            }
            if light.range() != defaults.range() {
                self.gltf_json.output_named_value("range", light.range());
            }
            if light.light_type() == LightType::Spot {
                self.gltf_json.begin_object_named("spot");
                if light.inner_cone_angle() != defaults.inner_cone_angle() {
                    self.gltf_json
                        .output_named_value("innerConeAngle", light.inner_cone_angle());
                }
                if light.outer_cone_angle() != defaults.outer_cone_angle() {
                    self.gltf_json
                        .output_named_value("outerConeAngle", light.outer_cone_angle());
                }
                self.gltf_json.end_object();
            }
            self.gltf_json.end_object();
        }
        self.gltf_json.end_array();
        self.gltf_json.end_object();
        ok_status()
    }

    fn encode_materials_variants_names_property(&mut self) -> Status {
        if self.materials_variants_names.is_empty() {
            return ok_status();
        }
        self.gltf_json.begin_object_named("KHR_materials_variants");
        self.gltf_json.begin_array_named("variants");
        for name in &self.materials_variants_names {
            self.gltf_json.begin_object();
            self.gltf_json.output_named_string("name", name);
            self.gltf_json.end_object();
        }
        self.gltf_json.end_array();
        self.gltf_json.end_object();
        ok_status()
    }

    fn encode_cesium_rtc_property(&mut self) -> Status {
        if self.cesium_rtc.is_empty() {
            return ok_status();
        }
        self.gltf_json.begin_object_named("CESIUM_RTC");
        self.gltf_json.begin_array_named("center");
        for value in &self.cesium_rtc {
            self.gltf_json.output_value(*value);
        }
        self.gltf_json.end_array();
        self.gltf_json.end_object();
        ok_status()
    }

    fn encode_structural_metadata_property(&mut self) -> Status {
        let structural_metadata = match self.structural_metadata {
            Some(meta) => meta,
            None => return ok_status(),
        };

        if structural_metadata.schema().is_empty() {
            return ok_status();
        }

        self.structural_metadata_used = true;
        self.gltf_json.begin_object_named("EXT_structural_metadata");

        fn write_schema(obj: &StructuralMetadataObject, json: &mut JsonWriter) {
            match obj.object_type() {
                StructuralMetadataObjectType::Object => {
                    if obj.name().is_empty() {
                        json.begin_object();
                    } else {
                        json.begin_object_named(obj.name());
                    }
                    for child in obj.objects() {
                        write_schema(child, json);
                    }
                    json.end_object();
                }
                StructuralMetadataObjectType::Array => {
                    if obj.name().is_empty() {
                        json.begin_array();
                    } else {
                        json.begin_array_named(obj.name());
                    }
                    for child in obj.array() {
                        write_schema(child, json);
                    }
                    json.end_array();
                }
                StructuralMetadataObjectType::String => {
                    json.output_named_string(obj.name(), obj.string());
                }
                StructuralMetadataObjectType::Integer => {
                    json.output_named_value(obj.name(), obj.integer());
                }
                StructuralMetadataObjectType::Boolean => {
                    json.output_named_bool(obj.name(), obj.boolean());
                }
            }
        }

        write_schema(&structural_metadata.schema().json, &mut self.gltf_json);

        self.gltf_json.begin_array_named("propertyTables");
        for i in 0..structural_metadata.num_property_tables() {
            let table = structural_metadata.property_table(i);
            self.gltf_json.begin_object();
            if !table.name().is_empty() {
                self.gltf_json.output_named_string("name", table.name());
            }
            if !table.class().is_empty() {
                self.gltf_json.output_named_string("class", table.class());
            }
            self.gltf_json.output_named_value("count", table.count());
            self.gltf_json.begin_object_named("properties");
            for j in 0..table.num_properties() {
                let property = table.property(j);
                self.gltf_json.begin_object_named(property.name());
                let values_index = self.add_buffer_view(property.data());
                if !values_index.is_ok() {
                    return values_index.status().clone();
                }
                self.gltf_json
                    .output_named_value("values", values_index.into_value());

                if !property.array_offsets().data.data.is_empty() {
                    if !property.array_offsets().type_name.is_empty() {
                        self.gltf_json.output_named_string(
                            "arrayOffsetType",
                            &property.array_offsets().type_name,
                        );
                    }
                    let offset_index = self.add_buffer_view(&property.array_offsets().data);
                    if !offset_index.is_ok() {
                        return offset_index.status().clone();
                    }
                    self.gltf_json
                        .output_named_value("arrayOffsets", offset_index.into_value());
                }

                if !property.string_offsets().data.data.is_empty() {
                    if !property.string_offsets().type_name.is_empty() {
                        self.gltf_json.output_named_string(
                            "stringOffsetType",
                            &property.string_offsets().type_name,
                        );
                    }
                    let offset_index = self.add_buffer_view(&property.string_offsets().data);
                    if !offset_index.is_ok() {
                        return offset_index.status().clone();
                    }
                    self.gltf_json
                        .output_named_value("stringOffsets", offset_index.into_value());
                }
                self.gltf_json.end_object();
            }
            self.gltf_json.end_object();
            self.gltf_json.end_object();
        }
        self.gltf_json.end_array();

        self.gltf_json.begin_array_named("propertyAttributes");
        for i in 0..structural_metadata.num_property_attributes() {
            let attribute = structural_metadata.property_attribute(i);
            self.gltf_json.begin_object();
            if !attribute.name().is_empty() {
                self.gltf_json.output_named_string("name", attribute.name());
            }
            if !attribute.class().is_empty() {
                self.gltf_json
                    .output_named_string("class", attribute.class());
            }
            self.gltf_json.begin_object_named("properties");
            for j in 0..attribute.num_properties() {
                let property = attribute.property(j);
                self.gltf_json.begin_object_named(property.name());
                self.gltf_json
                    .output_named_string("attribute", property.attribute_name());
                self.gltf_json.end_object();
            }
            self.gltf_json.end_object();
            self.gltf_json.end_object();
        }
        self.gltf_json.end_array();

        self.gltf_json.end_object();
        ok_status()
    }

    fn encode_accessors_property(&mut self, buf_out: &mut EncoderBuffer) -> bool {
        self.gltf_json.begin_array_named("accessors");
        for accessor in &self.accessors {
            self.gltf_json.begin_object();
            if accessor.buffer_view_index >= 0 {
                self.gltf_json
                    .output_named_value("bufferView", accessor.buffer_view_index);
                if self.output_type == OutputType::Verbose {
                    self.gltf_json.output_named_value("byteOffset", 0);
                }
            }
            self.gltf_json
                .output_named_value("componentType", accessor.component_type);
            self.gltf_json.output_named_value("count", accessor.count);
            if accessor.normalized {
                self.gltf_json
                    .output_named_bool("normalized", accessor.normalized);
            }
            if !accessor.max.is_empty() {
                self.gltf_json.begin_array_named("max");
                for value in &accessor.max {
                    self.gltf_json.output_value(*value);
                }
                self.gltf_json.end_array();
            }
            if !accessor.min.is_empty() {
                self.gltf_json.begin_array_named("min");
                for value in &accessor.min {
                    self.gltf_json.output_value(*value);
                }
                self.gltf_json.end_array();
            }
            self.gltf_json
                .output_named_string("type", &accessor.type_name);
            self.gltf_json.end_object();
        }
        self.gltf_json.end_array();
        let asset_str = self.gltf_json.move_data();
        buf_out.encode_bytes(asset_str.as_bytes())
    }

    fn encode_buffer_views_property(&mut self, buf_out: &mut EncoderBuffer) -> bool {
        self.gltf_json.begin_array_named("bufferViews");
        for view in &self.buffer_views {
            self.gltf_json.begin_object();
            self.gltf_json.output_named_value("buffer", 0);
            self.gltf_json
                .output_named_value("byteOffset", view.buffer_byte_offset);
            self.gltf_json
                .output_named_value("byteLength", view.byte_length);
            if view.target != 0 {
                self.gltf_json.output_named_value("target", view.target);
            }
            self.gltf_json.end_object();
        }
        self.gltf_json.end_array();
        let asset_str = self.gltf_json.move_data();
        buf_out.encode_bytes(asset_str.as_bytes())
    }

    fn encode_buffers_property(&mut self, buf_out: &mut EncoderBuffer) -> bool {
        if self.buffer.size() == 0 {
            return true;
        }
        self.gltf_json.begin_array_named("buffers");
        self.gltf_json.begin_object();
        self.gltf_json
            .output_named_value("byteLength", self.buffer.size());
        if !self.buffer_name.is_empty() {
            self.gltf_json.output_named_string("uri", &self.buffer_name);
        }
        self.gltf_json.end_object();
        self.gltf_json.end_array();
        let asset_str = self.gltf_json.move_data();
        buf_out.encode_bytes(asset_str.as_bytes())
    }

    fn encode_extensions_properties(&mut self, buf_out: &mut EncoderBuffer) -> Status {
        if self.draco_compression_used {
            self.extensions_used
                .insert("KHR_draco_mesh_compression".to_string());
            self.extensions_required
                .insert("KHR_draco_mesh_compression".to_string());
        }
        if !self.lights.is_empty() {
            self.extensions_used
                .insert("KHR_lights_punctual".to_string());
        }
        if !self.materials_variants_names.is_empty() {
            self.extensions_used
                .insert("KHR_materials_variants".to_string());
        }
        if !self.instance_arrays.is_empty() {
            self.extensions_used
                .insert("EXT_mesh_gpu_instancing".to_string());
            self.extensions_required
                .insert("EXT_mesh_gpu_instancing".to_string());
        }
        if self.mesh_features_used {
            self.extensions_used.insert("EXT_mesh_features".to_string());
        }
        if self.structural_metadata_used {
            self.extensions_used
                .insert("EXT_structural_metadata".to_string());
        }

        if !self.extensions_required.is_empty() {
            self.gltf_json.begin_array_named("extensionsRequired");
            for extension in &self.extensions_required {
                self.gltf_json.output_string(extension);
            }
            self.gltf_json.end_array();
        }
        if !self.extensions_used.is_empty() {
            self.gltf_json.begin_array_named("extensionsUsed");
            for extension in &self.extensions_used {
                self.gltf_json.output_string(extension);
            }
            self.gltf_json.end_array();
        }

        let asset_str = self.gltf_json.move_data();
        if !asset_str.is_empty() {
            if !buf_out.encode_bytes(asset_str.as_bytes()) {
                return Status::new(StatusCode::DracoError, "Could not encode extensions.");
            }
        }
        ok_status()
    }

    fn add_attribute(
        &mut self,
        att: &PointAttribute,
        num_points: i32,
        num_encoded_points: i32,
        compress: bool,
    ) -> i32 {
        match att.data_type() {
            DataType::Uint8 => {
                self.add_attribute_typed::<u8>(att, num_points, num_encoded_points, compress)
            }
            DataType::Uint16 => {
                self.add_attribute_typed::<u16>(att, num_points, num_encoded_points, compress)
            }
            DataType::Float32 => {
                self.add_attribute_typed::<f32>(att, num_points, num_encoded_points, compress)
            }
            _ => -1,
        }
    }

    fn add_attribute_typed<
        T: Copy
            + Default
            + PartialOrd
            + ToGltfValue
            + GltfComponentType
            + 'static
            + draco_core::attributes::draco_numeric::DracoNumeric,
    >(
        &mut self,
        att: &PointAttribute,
        num_points: i32,
        num_encoded_points: i32,
        compress: bool,
    ) -> i32 {
        let num_components = att.num_components() as i32;
        match num_components {
            1 => self.add_attribute_fixed::<1, T>(
                att,
                num_points,
                num_encoded_points,
                "SCALAR",
                compress,
            ),
            2 => self.add_attribute_fixed::<2, T>(
                att,
                num_points,
                num_encoded_points,
                "VEC2",
                compress,
            ),
            3 => self.add_attribute_fixed::<3, T>(
                att,
                num_points,
                num_encoded_points,
                "VEC3",
                compress,
            ),
            4 => self.add_attribute_fixed::<4, T>(
                att,
                num_points,
                num_encoded_points,
                "VEC4",
                compress,
            ),
            _ => -1,
        }
    }

    fn add_attribute_fixed<
        const N: usize,
        T: Copy
            + Default
            + PartialOrd
            + ToGltfValue
            + GltfComponentType
            + 'static
            + draco_core::attributes::draco_numeric::DracoNumeric,
    >(
        &mut self,
        att: &PointAttribute,
        num_points: i32,
        num_encoded_points: i32,
        type_name: &str,
        compress: bool,
    ) -> i32 {
        if att.size() == 0 {
            return -1;
        }

        let mut value = [T::default(); N];
        let mut min_values = [T::default(); N];
        if !att.convert_value(AttributeValueIndex::from(0u32), N as i8, &mut min_values) {
            return -1;
        }
        let mut max_values = min_values;

        let min_max_required = self.output_type == OutputType::Verbose
            || att.attribute_type() == GeometryAttributeType::Position;
        if min_max_required {
            for i in 1..att.size() {
                if !att.convert_value(AttributeValueIndex::from(i as u32), N as i8, &mut value) {
                    return -1;
                }
                for j in 0..N {
                    if value[j] < min_values[j] {
                        min_values[j] = value[j];
                    }
                    if value[j] > max_values[j] {
                        max_values[j] = value[j];
                    }
                }
            }
        }

        let mut accessor = GltfAccessor::default();
        if !compress {
            let buffer_start_offset = self.buffer.size();
            for v in 0..num_points {
                let point = PointIndex::from(v as u32);
                let mapped = att.mapped_index(point);
                if !att.convert_value(mapped, N as i8, &mut value) {
                    return -1;
                }
                for j in 0..N {
                    self.buffer.encode(value[j]);
                }
            }
            if !self.pad_buffer() {
                return -1;
            }
            let mut buffer_view = GltfBufferView::default();
            buffer_view.buffer_byte_offset = buffer_start_offset as i64;
            buffer_view.byte_length = (self.buffer.size() - buffer_start_offset) as i64;
            self.buffer_views.push(buffer_view);
            accessor.buffer_view_index = (self.buffer_views.len() - 1) as i32;
        }

        accessor.component_type = T::component_type().as_i32();
        accessor.count = num_encoded_points as i64;
        if min_max_required {
            for j in 0..N {
                accessor.max.push(max_values[j].to_gltf_value());
                accessor.min.push(min_values[j].to_gltf_value());
            }
        }
        accessor.type_name = type_name.to_string();
        accessor.normalized = att.attribute_type() == GeometryAttributeType::Color
            && att.data_type() != DataType::Float32;
        self.accessors.push(accessor);
        (self.accessors.len() - 1) as i32
    }

    fn set_copyright_from_scene(&mut self, scene: &Scene) {
        let mut copyright = MetadataString::default();
        scene
            .metadata()
            .get_entry_string("copyright", &mut copyright);
        self.set_copyright(&copyright.to_utf8_lossy());
    }

    fn set_copyright_from_mesh(&mut self, mesh: &Mesh) {
        if let Some(metadata) = mesh.get_metadata() {
            let mut copyright = MetadataString::default();
            metadata.get_entry_string("copyright", &mut copyright);
            self.set_copyright(&copyright.to_utf8_lossy());
        }
    }

    fn encode_vector3(&mut self, array_name: &str, vec: Vector3f) {
        self.gltf_json.begin_array_named(array_name);
        for i in 0..Vector3f::DIMENSION {
            self.gltf_json.output_value(vec[i]);
        }
        self.gltf_json.end_array();
    }

    fn encode_vector4(&mut self, array_name: &str, vec: Vector4f) {
        self.gltf_json.begin_array_named(array_name);
        for i in 0..Vector4f::DIMENSION {
            self.gltf_json.output_value(vec[i]);
        }
        self.gltf_json.end_array();
    }
}

trait GltfComponentType {
    fn component_type() -> ComponentType;
}

impl GltfComponentType for u8 {
    fn component_type() -> ComponentType {
        ComponentType::UnsignedByte
    }
}

impl GltfComponentType for u16 {
    fn component_type() -> ComponentType {
        ComponentType::UnsignedShort
    }
}

impl GltfComponentType for u32 {
    fn component_type() -> ComponentType {
        ComponentType::UnsignedInt
    }
}

impl GltfComponentType for i8 {
    fn component_type() -> ComponentType {
        ComponentType::Byte
    }
}

impl GltfComponentType for i16 {
    fn component_type() -> ComponentType {
        ComponentType::Short
    }
}

impl GltfComponentType for f32 {
    fn component_type() -> ComponentType {
        ComponentType::Float
    }
}

trait ToGltfValue {
    fn to_gltf_value(self) -> GltfValue;
}

impl ToGltfValue for u8 {
    fn to_gltf_value(self) -> GltfValue {
        GltfValue::from_u8(self)
    }
}

impl ToGltfValue for u16 {
    fn to_gltf_value(self) -> GltfValue {
        GltfValue::from_u16(self)
    }
}

impl ToGltfValue for u32 {
    fn to_gltf_value(self) -> GltfValue {
        GltfValue::from_u32(self)
    }
}

impl ToGltfValue for i8 {
    fn to_gltf_value(self) -> GltfValue {
        GltfValue::from_i8(self)
    }
}

impl ToGltfValue for i16 {
    fn to_gltf_value(self) -> GltfValue {
        GltfValue::from_i16(self)
    }
}

impl ToGltfValue for f32 {
    fn to_gltf_value(self) -> GltfValue {
        GltfValue::from_f32(self)
    }
}

fn texture_filter_type_to_gltf_value(filter_type: TextureMapFilterType) -> i32 {
    match filter_type {
        TextureMapFilterType::Nearest => 9728,
        TextureMapFilterType::Linear => 9729,
        TextureMapFilterType::NearestMipmapNearest => 9984,
        TextureMapFilterType::LinearMipmapNearest => 9985,
        TextureMapFilterType::NearestMipmapLinear => 9986,
        TextureMapFilterType::LinearMipmapLinear => 9987,
        _ => -1,
    }
}

fn texture_axis_wrapping_mode_to_gltf_value(mode: TextureMapAxisWrappingMode) -> i32 {
    match mode {
        TextureMapAxisWrappingMode::ClampToEdge => 33071,
        TextureMapAxisWrappingMode::MirroredRepeat => 33648,
        TextureMapAxisWrappingMode::Repeat => 10497,
    }
}

fn is_feature_id_attribute(att_index: i32, mesh: &Mesh) -> bool {
    for i in 0..mesh.num_mesh_features() {
        let idx = MeshFeaturesIndex::from(i as u32);
        if mesh.mesh_features(idx).attribute_index() == att_index {
            return true;
        }
    }
    false
}

fn is_property_attribute(
    att_index: i32,
    mesh: &Mesh,
    structural_metadata: &StructuralMetadata,
) -> bool {
    if structural_metadata.num_property_attributes() == 0 {
        return false;
    }
    let attribute_name = match mesh.attribute(att_index) {
        Some(att) => att.name().to_string(),
        None => return false,
    };
    if !attribute_name.starts_with('_') {
        return false;
    }
    for i in 0..mesh.num_property_attributes_indices() {
        let property_attribute_index = mesh.property_attributes_index(i);
        let attribute = structural_metadata.property_attribute(property_attribute_index);
        for j in 0..attribute.num_properties() {
            let property = attribute.property(j);
            if property.attribute_name() == attribute_name {
                return true;
            }
        }
    }
    false
}

fn check_and_get_tex_coord_attribute_order(mesh: &Mesh) -> Option<[i32; 2]> {
    let mut tex_coord_order = [0, 1];
    let num_attributes = std::cmp::min(
        mesh.num_named_attributes(GeometryAttributeType::TexCoord),
        2,
    );
    if num_attributes == 0 {
        return Some(tex_coord_order);
    }

    let mut names = vec![MetadataString::default(); num_attributes as usize];
    for i in 0..num_attributes {
        let att_id = mesh.get_named_attribute_id_by_index(GeometryAttributeType::TexCoord, i);
        if let Some(metadata) = mesh.get_attribute_metadata_by_attribute_id(att_id) {
            let mut attribute_name = MetadataString::default();
            if metadata.get_entry_string("attribute_name", &mut attribute_name) {
                names[i as usize] = attribute_name;
            }
        }
    }

    if names.iter().all(|name| name.is_empty()) {
        return Some(tex_coord_order);
    }

    let unique: BTreeSet<MetadataString> = names.iter().cloned().collect();
    if unique.len() != names.len() {
        return None;
    }
    if names.iter().any(|name| {
        !name.is_empty() && name.as_bytes() != b"TEXCOORD_0" && name.as_bytes() != b"TEXCOORD_1"
    }) {
        return None;
    }
    if names[0].as_bytes() == b"TEXCOORD_1" {
        tex_coord_order = [1, 0];
    }
    Some(tex_coord_order)
}

struct SceneMeshInstance {
    mesh_index: MeshIndex,
    transform: Matrix4d,
}

fn compute_all_instances(scene: &Scene) -> Vec<SceneMeshInstance> {
    let mut instances = Vec::new();
    for i in 0..scene.num_root_nodes() {
        let root_index = scene.root_node_index(i);
        let mut stack = vec![SceneNodeStackItem {
            scene_node_index: root_index,
            transform: Matrix4d::identity(),
        }];
        while let Some(node) = stack.pop() {
            let scene_node = scene.node(node.scene_node_index);
            let local = scene_node.trs_matrix().compute_transformation_matrix();
            let combined = node.transform.mul(&local);

            let mesh_group_index = scene_node.mesh_group_index();
            if mesh_group_index != INVALID_MESH_GROUP_INDEX {
                let mesh_group = scene.mesh_group(mesh_group_index);
                for i in 0..mesh_group.num_mesh_instances() {
                    let instance = mesh_group.mesh_instance(i);
                    if instance.mesh_index != INVALID_MESH_INDEX {
                        instances.push(SceneMeshInstance {
                            mesh_index: instance.mesh_index,
                            transform: combined,
                        });
                    }
                }
            }

            for child in scene_node.children() {
                stack.push(SceneNodeStackItem {
                    scene_node_index: *child,
                    transform: combined,
                });
            }
        }
    }
    instances
}

struct SceneNodeStackItem {
    scene_node_index: SceneNodeIndex,
    transform: Matrix4d,
}

fn find_largest_base_mesh_transforms(scene: &Scene) -> IndexTypeVector<MeshIndex, Matrix4d> {
    let mut transforms =
        IndexTypeVector::with_size_value(scene.num_meshes() as usize, Matrix4d::identity());
    let mut transform_scale = IndexTypeVector::with_size_value(scene.num_meshes() as usize, 0.0f32);

    let instances = compute_all_instances(scene);
    for instance in instances {
        let max_scale = matrix_max_scale(&instance.transform);
        if transform_scale[instance.mesh_index] < max_scale {
            transform_scale[instance.mesh_index] = max_scale;
            transforms[instance.mesh_index] = instance.transform;
        }
    }
    transforms
}

fn matrix_max_scale(transform: &Matrix4d) -> f32 {
    let mut max_scale = 0.0f64;
    for col in 0..3 {
        let x = transform.m[0][col];
        let y = transform.m[1][col];
        let z = transform.m[2][col];
        let scale = (x * x + y * y + z * z).sqrt();
        if scale > max_scale {
            max_scale = scale;
        }
    }
    max_scale as f32
}

fn encode_u32_with_size(buffer: &mut EncoderBuffer, value: u32, size: usize) -> bool {
    let bytes = value.to_le_bytes();
    buffer.encode_bytes(&bytes[..size])
}

fn encode_u32_le(buffer: &mut EncoderBuffer, value: u32) -> bool {
    buffer.encode_bytes(&value.to_le_bytes())
}
