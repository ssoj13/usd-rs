//! Keyframe animation encoder.
//!
//! What: Encodes keyframe animations using point cloud compression.
//! Why: Keeps animation compression separate from mesh compression APIs.
//! How: Wraps `PointCloudSequentialEncoder`.
//! Where used: Transcoder tools and animation tests.

use draco_bitstream::compression::config::encoder_options::EncoderOptions;
use draco_bitstream::compression::point_cloud::{PointCloudEncoder, PointCloudSequentialEncoder};
use draco_core::core::encoder_buffer::EncoderBuffer;
use draco_core::core::status::Status;

use crate::animation::keyframe_animation::KeyframeAnimation;

/// Encoder for keyframe animations.
#[derive(Default)]
pub struct KeyframeAnimationEncoder {
    encoder: PointCloudSequentialEncoder,
}

impl KeyframeAnimationEncoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn encode_keyframe_animation(
        &mut self,
        animation: &KeyframeAnimation,
        options: &EncoderOptions,
        out_buffer: &mut EncoderBuffer,
    ) -> Status {
        self.encoder.set_point_cloud(animation.point_cloud());
        self.encoder.encode(options, out_buffer)
    }
}
