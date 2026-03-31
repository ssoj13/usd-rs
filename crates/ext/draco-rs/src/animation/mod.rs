//! Animation module.
//!
//! What: Rust port of Draco animation types and keyframe encoders.
//! Why: glTF animations and skins are stored in these structures.
//! How: Mirrors C++ API with plain structs and explicit copy helpers.
//! Where used: glTF IO, scene graphs, and transcoder tools.

mod animation;
mod keyframe_animation;
mod keyframe_animation_decoder;
mod keyframe_animation_encoder;
mod node_animation_data;
mod skin;

pub use animation::{
    Animation, AnimationChannel, AnimationSampler, ChannelTransformation, SamplerInterpolation,
};
pub use keyframe_animation::KeyframeAnimation;
pub use keyframe_animation_decoder::KeyframeAnimationDecoder;
pub use keyframe_animation_encoder::KeyframeAnimationEncoder;
pub use node_animation_data::{NodeAnimationData, NodeAnimationDataHash, NodeAnimationDataType};
pub use skin::Skin;
