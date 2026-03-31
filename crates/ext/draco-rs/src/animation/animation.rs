//! Animation container types.
//!
//! What: Represents glTF-style animation samplers and channels.
//! Why: Used by scene graphs and glTF IO to store animation data.
//! How: Mirrors Draco C++ structs with explicit copy helpers.
//! Where used: `tiny_gltf_utils`, scene loading, and scene serialization.

use crate::animation::node_animation_data::NodeAnimationData;

/// Animation sampler settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SamplerInterpolation {
    Linear,
    Step,
    CubicSpline,
}

impl SamplerInterpolation {
    pub fn to_string(self) -> &'static str {
        match self {
            SamplerInterpolation::Step => "STEP",
            SamplerInterpolation::CubicSpline => "CUBICSPLINE",
            SamplerInterpolation::Linear => "LINEAR",
        }
    }
}

/// Animation sampler definition.
#[derive(Clone, Debug)]
pub struct AnimationSampler {
    pub input_index: i32,
    pub interpolation_type: SamplerInterpolation,
    pub output_index: i32,
}

impl Default for AnimationSampler {
    fn default() -> Self {
        Self {
            input_index: -1,
            interpolation_type: SamplerInterpolation::Linear,
            output_index: -1,
        }
    }
}

impl AnimationSampler {
    pub fn copy_from(&mut self, src: &AnimationSampler) {
        self.input_index = src.input_index;
        self.interpolation_type = src.interpolation_type;
        self.output_index = src.output_index;
    }
}

/// Channel transformation target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelTransformation {
    Translation,
    Rotation,
    Scale,
    Weights,
}

impl ChannelTransformation {
    pub fn to_string(self) -> &'static str {
        match self {
            ChannelTransformation::Rotation => "rotation",
            ChannelTransformation::Scale => "scale",
            ChannelTransformation::Weights => "weights",
            ChannelTransformation::Translation => "translation",
        }
    }
}

/// Animation channel definition.
#[derive(Clone, Debug)]
pub struct AnimationChannel {
    pub target_index: i32,
    pub transformation_type: ChannelTransformation,
    pub sampler_index: i32,
}

impl Default for AnimationChannel {
    fn default() -> Self {
        Self {
            target_index: -1,
            transformation_type: ChannelTransformation::Translation,
            sampler_index: -1,
        }
    }
}

impl AnimationChannel {
    pub fn copy_from(&mut self, src: &AnimationChannel) {
        self.target_index = src.target_index;
        self.transformation_type = src.transformation_type;
        self.sampler_index = src.sampler_index;
    }
}

/// Animation data container.
#[derive(Clone, Debug, Default)]
pub struct Animation {
    name: String,
    samplers: Vec<Box<AnimationSampler>>,
    channels: Vec<Box<AnimationChannel>>,
    node_animation_data: Vec<Box<NodeAnimationData>>,
}

impl Animation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn copy_from(&mut self, src: &Animation) {
        self.name = src.name.clone();
        self.channels.clear();
        for channel in &src.channels {
            let mut new_channel = Box::new(AnimationChannel::default());
            new_channel.copy_from(channel);
            self.channels.push(new_channel);
        }

        self.samplers.clear();
        for sampler in &src.samplers {
            let mut new_sampler = Box::new(AnimationSampler::default());
            new_sampler.copy_from(sampler);
            self.samplers.push(new_sampler);
        }

        self.node_animation_data.clear();
        for data in &src.node_animation_data {
            let mut new_data = Box::new(NodeAnimationData::default());
            new_data.copy_from(data);
            self.node_animation_data.push(new_data);
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    pub fn num_channels(&self) -> i32 {
        self.channels.len() as i32
    }

    pub fn num_samplers(&self) -> i32 {
        self.samplers.len() as i32
    }

    pub fn num_node_animation_data(&self) -> i32 {
        self.node_animation_data.len() as i32
    }

    pub fn channel(&self, index: i32) -> Option<&AnimationChannel> {
        self.channels.get(index as usize).map(|c| c.as_ref())
    }

    pub fn channel_mut(&mut self, index: i32) -> Option<&mut AnimationChannel> {
        self.channels.get_mut(index as usize).map(|c| c.as_mut())
    }

    pub fn sampler(&self, index: i32) -> Option<&AnimationSampler> {
        self.samplers.get(index as usize).map(|s| s.as_ref())
    }

    pub fn sampler_mut(&mut self, index: i32) -> Option<&mut AnimationSampler> {
        self.samplers.get_mut(index as usize).map(|s| s.as_mut())
    }

    pub fn node_animation_data(&self, index: i32) -> Option<&NodeAnimationData> {
        self.node_animation_data
            .get(index as usize)
            .map(|d| d.as_ref())
    }

    pub fn node_animation_data_mut(&mut self, index: i32) -> Option<&mut NodeAnimationData> {
        self.node_animation_data
            .get_mut(index as usize)
            .map(|d| d.as_mut())
    }

    pub fn add_node_animation_data(&mut self, data: Box<NodeAnimationData>) {
        self.node_animation_data.push(data);
    }

    pub fn add_sampler(&mut self, sampler: Box<AnimationSampler>) {
        self.samplers.push(sampler);
    }

    pub fn add_channel(&mut self, channel: Box<AnimationChannel>) {
        self.channels.push(channel);
    }
}
