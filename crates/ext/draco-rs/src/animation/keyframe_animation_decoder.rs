//! Keyframe animation decoder.
//!
//! What: Decodes keyframe animations from Draco bitstreams.
//! Why: Allows animation payloads to be stored as Draco point clouds.
//! How: Wraps `PointCloudSequentialDecoder`.
//! Where used: Transcoder tools and animation tests.

use draco_bitstream::compression::config::decoder_options::DecoderOptions;
use draco_bitstream::compression::point_cloud::{PointCloudDecoder, PointCloudSequentialDecoder};
use draco_core::core::decoder_buffer::DecoderBuffer;
use draco_core::core::status::Status;

use crate::animation::keyframe_animation::KeyframeAnimation;

/// Decoder for keyframe animations.
#[derive(Default)]
pub struct KeyframeAnimationDecoder {
    decoder: PointCloudSequentialDecoder,
}

impl KeyframeAnimationDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn decode(
        &mut self,
        options: &DecoderOptions,
        in_buffer: &mut DecoderBuffer,
        animation: &mut KeyframeAnimation,
    ) -> Status {
        self.decoder
            .decode(options, in_buffer, animation.point_cloud_mut())
    }
}
