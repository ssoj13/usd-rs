//! Tiny glTF utilities.
//!
//! What: Helper functions for mapping glTF JSON types to Draco types.
//! Why: Shared by glTF decoder/encoder for animations and materials.
//! How: Ported from Draco `tiny_gltf_utils` using `gltf-json` types.
//! Where used: `gltf_decoder`, `gltf_encoder`, and glTF tests.

use gltf::validation::{Checked, USize64};
use gltf_json as gltf;

use crate::animation::{
    Animation, AnimationChannel, AnimationSampler, ChannelTransformation, NodeAnimationData,
    NodeAnimationDataType, SamplerInterpolation,
};
use draco_core::core::status::{ok_status, Status, StatusCode};
use draco_core::core::status_or::StatusOr;
use draco_core::core::vector_d::{Vector3f, Vector4f};
use draco_core::material::material::MaterialTransparencyMode;

/// glTF model with JSON and decoded buffers.
#[derive(Debug, Default)]
pub struct GltfModel {
    pub json: gltf::Root,
    pub buffers: Vec<Vec<u8>>,
}

pub struct TinyGltfUtils;

impl TinyGltfUtils {
    /// Returns the number of components for the glTF accessor type.
    pub fn get_num_components_for_type(accessor_type: gltf::accessor::Type) -> i32 {
        match accessor_type {
            gltf::accessor::Type::Scalar => 1,
            gltf::accessor::Type::Vec2 => 2,
            gltf::accessor::Type::Vec3 => 3,
            gltf::accessor::Type::Vec4 => 4,
            gltf::accessor::Type::Mat2 => 4,
            gltf::accessor::Type::Mat3 => 9,
            gltf::accessor::Type::Mat4 => 16,
        }
    }

    /// Converts glTF material alpha mode to Draco transparency mode.
    pub fn text_to_material_mode(
        mode: &Checked<gltf::material::AlphaMode>,
    ) -> MaterialTransparencyMode {
        match mode {
            Checked::Valid(gltf::material::AlphaMode::Mask) => MaterialTransparencyMode::Mask,
            Checked::Valid(gltf::material::AlphaMode::Blend) => MaterialTransparencyMode::Blend,
            _ => MaterialTransparencyMode::Opaque,
        }
    }

    /// Converts glTF sampler interpolation to Draco enum.
    pub fn text_to_sampler_interpolation(
        interpolation: &Checked<gltf::animation::Interpolation>,
    ) -> SamplerInterpolation {
        match interpolation {
            Checked::Valid(gltf::animation::Interpolation::Step) => SamplerInterpolation::Step,
            Checked::Valid(gltf::animation::Interpolation::CubicSpline) => {
                SamplerInterpolation::CubicSpline
            }
            _ => SamplerInterpolation::Linear,
        }
    }

    /// Converts glTF channel path to Draco transformation type.
    pub fn text_to_channel_transformation(
        path: &Checked<gltf::animation::Property>,
    ) -> ChannelTransformation {
        match path {
            Checked::Valid(gltf::animation::Property::Rotation) => ChannelTransformation::Rotation,
            Checked::Valid(gltf::animation::Property::Scale) => ChannelTransformation::Scale,
            Checked::Valid(gltf::animation::Property::MorphTargetWeights) => {
                ChannelTransformation::Weights
            }
            _ => ChannelTransformation::Translation,
        }
    }

    /// Adds a glTF channel and its sampler/accessor data to a Draco animation.
    pub fn add_channel_to_animation(
        model: &GltfModel,
        input_animation: &gltf::animation::Animation,
        channel: &gltf::animation::Channel,
        node_index: i32,
        animation: &mut Animation,
    ) -> Status {
        let sampler_index = channel.sampler.value();
        let sampler = &input_animation.samplers[sampler_index];
        draco_core::draco_return_if_error!(Self::add_sampler_to_animation(
            model, sampler, animation
        ));
        let mut new_channel = AnimationChannel::default();
        new_channel.sampler_index = animation.num_samplers() - 1;
        new_channel.target_index = node_index;
        new_channel.transformation_type =
            Self::text_to_channel_transformation(&channel.target.path);
        animation.add_channel(Box::new(new_channel));
        ok_status()
    }

    /// Adds a glTF sampler and its input/output accessors to a Draco animation.
    pub fn add_sampler_to_animation(
        model: &GltfModel,
        sampler: &gltf::animation::Sampler,
        animation: &mut Animation,
    ) -> Status {
        let mut input_data = Box::new(NodeAnimationData::default());
        // NOTE: Matches C++ behavior: we always copy accessor data, even if shared.
        let input_accessor = &model.json.accessors[sampler.input.value()];
        draco_core::draco_return_if_error!(Self::add_accessor_to_animation_data(
            model,
            input_accessor,
            &mut input_data,
        ));
        animation.add_node_animation_data(input_data);

        let mut new_sampler = AnimationSampler::default();
        new_sampler.input_index = animation.num_node_animation_data() - 1;

        let mut output_data = Box::new(NodeAnimationData::default());
        let output_accessor = &model.json.accessors[sampler.output.value()];
        draco_core::draco_return_if_error!(Self::add_accessor_to_animation_data(
            model,
            output_accessor,
            &mut output_data,
        ));
        animation.add_node_animation_data(output_data);
        new_sampler.output_index = animation.num_node_animation_data() - 1;

        new_sampler.interpolation_type =
            Self::text_to_sampler_interpolation(&sampler.interpolation);
        animation.add_sampler(Box::new(new_sampler));
        ok_status()
    }

    /// Converts accessor data into NodeAnimationData.
    pub fn add_accessor_to_animation_data(
        model: &GltfModel,
        accessor: &gltf::Accessor,
        node_animation_data: &mut NodeAnimationData,
    ) -> Status {
        let component_type = match Self::checked_component_type(accessor) {
            Ok(component_type) => component_type,
            Err(status) => return status,
        };
        if component_type != gltf::accessor::ComponentType::F32 {
            return Status::new(
                StatusCode::DracoError,
                "Unsupported ComponentType for NodeAnimationData.",
            );
        }

        let accessor_type = match Self::checked_accessor_type(accessor) {
            Ok(accessor_type) => accessor_type,
            Err(status) => return status,
        };

        let dest_data = node_animation_data.data_mut();
        match accessor_type {
            gltf::accessor::Type::Scalar => {
                let data: Vec<f32>;
                draco_core::draco_assign_or_return!(
                    data,
                    Self::copy_data_as_float::<f32>(model, accessor)
                );
                dest_data.extend_from_slice(&data);
                node_animation_data.set_type(NodeAnimationDataType::Scalar);
            }
            gltf::accessor::Type::Vec3 => {
                let data: Vec<Vector3f>;
                draco_core::draco_assign_or_return!(
                    data,
                    Self::copy_data_as_float::<Vector3f>(model, accessor)
                );
                for v in data {
                    dest_data.push(v[0]);
                    dest_data.push(v[1]);
                    dest_data.push(v[2]);
                }
                node_animation_data.set_type(NodeAnimationDataType::Vec3);
            }
            gltf::accessor::Type::Vec4 => {
                let data: Vec<Vector4f>;
                draco_core::draco_assign_or_return!(
                    data,
                    Self::copy_data_as_float::<Vector4f>(model, accessor)
                );
                for v in data {
                    dest_data.push(v[0]);
                    dest_data.push(v[1]);
                    dest_data.push(v[2]);
                    dest_data.push(v[3]);
                }
                node_animation_data.set_type(NodeAnimationDataType::Vec4);
            }
            gltf::accessor::Type::Mat4 => {
                let data: Vec<[f32; 16]>;
                draco_core::draco_assign_or_return!(
                    data,
                    Self::copy_data_as_float::<[f32; 16]>(model, accessor)
                );
                for m in data {
                    dest_data.extend_from_slice(&m);
                }
                node_animation_data.set_type(NodeAnimationDataType::Mat4);
            }
            _ => {
                return Status::new(
                    StatusCode::DracoError,
                    "Unsupported Type for GltfNodeAnimationData.",
                );
            }
        }

        let count_i32 = match i32::try_from(accessor.count.0) {
            Ok(count) => count,
            Err(_) => {
                return Status::new(
                    StatusCode::DracoError,
                    "Accessor count exceeds i32 for NodeAnimationData.",
                )
            }
        };
        node_animation_data.set_count(count_i32);
        node_animation_data.set_normalized(accessor.normalized);
        ok_status()
    }

    /// Returns accessor data as floats (generic over output type).
    pub(crate) fn copy_data_as_float<T: FloatPack>(
        model: &GltfModel,
        accessor: &gltf::Accessor,
    ) -> StatusOr<Vec<T>> {
        let accessor_type = match Self::checked_accessor_type(accessor) {
            Ok(accessor_type) => accessor_type,
            Err(status) => return StatusOr::new_status(status),
        };
        let num_components = Self::get_num_components_for_type(accessor_type) as usize;
        if num_components != T::COMPONENTS {
            return StatusOr::new_status(Status::new(
                StatusCode::DracoError,
                "Dimension does not equal num components.",
            ));
        }
        Self::copy_data_as_float_impl::<T>(model, accessor, accessor_type)
    }

    fn copy_data_as_float_impl<T: FloatPack>(
        model: &GltfModel,
        accessor: &gltf::Accessor,
        accessor_type: gltf::accessor::Type,
    ) -> StatusOr<Vec<T>> {
        let component_type = match Self::checked_component_type(accessor) {
            Ok(component_type) => component_type,
            Err(status) => return StatusOr::new_status(status),
        };
        if component_type != gltf::accessor::ComponentType::F32 {
            return StatusOr::new_status(Status::new(
                StatusCode::DracoError,
                "Non-float data is not supported by CopyDataAsFloat().",
            ));
        }

        let buffer_view_index = match accessor.buffer_view {
            Some(view) => view.value(),
            None => {
                return StatusOr::new_status(Status::new(
                    StatusCode::DracoError,
                    "Error CopyDataAsFloat() bufferView < 0.",
                ))
            }
        };
        if model.json.buffer_views.is_empty() {
            return StatusOr::new_status(Status::new(
                StatusCode::DracoError,
                "glTF bufferViews array is empty.",
            ));
        }
        if buffer_view_index >= model.json.buffer_views.len() {
            return StatusOr::new_status(Status::new(
                StatusCode::DracoError,
                "glTF bufferView index is out of range.",
            ));
        }
        let buffer_view = &model.json.buffer_views[buffer_view_index];
        let buffer_index = buffer_view.buffer.value();
        let buffer = match model.buffers.get(buffer_index) {
            Some(buf) => buf,
            None => {
                return StatusOr::new_status(Status::new(
                    StatusCode::DracoError,
                    "Error CopyDataAsFloat() buffer < 0.",
                ))
            }
        };

        let view_offset = match Self::optional_usize(
            buffer_view.byte_offset,
            "BufferView byteOffset exceeds usize.",
        ) {
            Ok(value) => value,
            Err(status) => return StatusOr::new_status(status),
        };
        let accessor_offset = match Self::optional_usize(
            accessor.byte_offset,
            "Accessor byteOffset exceeds usize.",
        ) {
            Ok(value) => value,
            Err(status) => return StatusOr::new_status(status),
        };
        let byte_offset = view_offset + accessor_offset;

        let component_size = Self::component_size_bytes(component_type);
        let num_components = Self::get_num_components_for_type(accessor_type) as usize;
        let stride_default = match component_size.checked_mul(num_components) {
            Some(value) => value,
            None => {
                return StatusOr::new_status(Status::new(
                    StatusCode::DracoError,
                    "Accessor stride overflow.",
                ))
            }
        };
        let byte_stride = buffer_view
            .byte_stride
            .map(|stride| stride.0)
            .unwrap_or(stride_default);

        let count = match Self::usize_from_u64(accessor.count, "Accessor count exceeds usize.") {
            Ok(value) => value,
            Err(status) => return StatusOr::new_status(status),
        };

        let mut output = Vec::with_capacity(count);
        let mut offset = byte_offset;
        for _ in 0..count {
            let mut values = T::default();
            for c in 0..num_components {
                let component_offset = match c
                    .checked_mul(component_size)
                    .and_then(|delta| offset.checked_add(delta))
                {
                    Some(value) => value,
                    None => {
                        return StatusOr::new_status(Status::new(
                            StatusCode::DracoError,
                            "Accessor offset overflow.",
                        ))
                    }
                };
                let end = match component_offset.checked_add(component_size) {
                    Some(value) => value,
                    None => {
                        return StatusOr::new_status(Status::new(
                            StatusCode::DracoError,
                            "Accessor offset overflow.",
                        ))
                    }
                };
                if end > buffer.len() {
                    return StatusOr::new_status(Status::new(
                        StatusCode::DracoError,
                        "Accessor data out of range.",
                    ));
                }
                let mut bytes = [0u8; 4];
                bytes.copy_from_slice(&buffer[component_offset..end]);
                let value = f32::from_le_bytes(bytes);
                values.set_component(c, value);
            }
            output.push(values);
            offset = match offset.checked_add(byte_stride) {
                Some(value) => value,
                None => {
                    return StatusOr::new_status(Status::new(
                        StatusCode::DracoError,
                        "Accessor stride overflow.",
                    ))
                }
            };
        }
        StatusOr::new_value(output)
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

    fn checked_component_type(
        accessor: &gltf::Accessor,
    ) -> Result<gltf::accessor::ComponentType, Status> {
        match accessor.component_type {
            Checked::Valid(gltf::accessor::GenericComponentType(component_type)) => {
                Ok(component_type)
            }
            Checked::Invalid => Err(Status::new(
                StatusCode::DracoError,
                "Invalid accessor component type.",
            )),
        }
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

    fn usize_from_u64(value: USize64, message: &str) -> Result<usize, Status> {
        usize::try_from(value.0).map_err(|_| Status::new(StatusCode::DracoError, message))
    }

    fn optional_usize(value: Option<USize64>, message: &str) -> Result<usize, Status> {
        match value {
            Some(value) => Self::usize_from_u64(value, message),
            None => Ok(0),
        }
    }
}

pub(crate) trait FloatPack: Copy + Default {
    const COMPONENTS: usize;
    fn set_component(&mut self, index: usize, value: f32);
}

impl FloatPack for f32 {
    const COMPONENTS: usize = 1;

    fn set_component(&mut self, _index: usize, value: f32) {
        *self = value;
    }
}

impl FloatPack for [f32; 2] {
    const COMPONENTS: usize = 2;

    fn set_component(&mut self, index: usize, value: f32) {
        self[index] = value;
    }
}

impl FloatPack for [f32; 3] {
    const COMPONENTS: usize = 3;

    fn set_component(&mut self, index: usize, value: f32) {
        self[index] = value;
    }
}

impl FloatPack for [f32; 4] {
    const COMPONENTS: usize = 4;

    fn set_component(&mut self, index: usize, value: f32) {
        self[index] = value;
    }
}

impl FloatPack for Vector3f {
    const COMPONENTS: usize = 3;

    fn set_component(&mut self, index: usize, value: f32) {
        self[index] = value;
    }
}

impl FloatPack for Vector4f {
    const COMPONENTS: usize = 4;

    fn set_component(&mut self, index: usize, value: f32) {
        self[index] = value;
    }
}

impl FloatPack for [f32; 16] {
    const COMPONENTS: usize = 16;

    fn set_component(&mut self, index: usize, value: f32) {
        self[index] = value;
    }
}
