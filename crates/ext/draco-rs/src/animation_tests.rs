//! Animation tests ported from Draco C++ reference.
//!
//! What: Covers `animation_test.cc` and `keyframe_animation_*_test.cc` parity.
//! Why: Validates animation copy behavior and keyframe encode/decode paths.
//! Where used: `cargo test -p draco-rs animation_`.

use crate::animation::{
    Animation, AnimationChannel, AnimationSampler, ChannelTransformation, KeyframeAnimation,
    KeyframeAnimationDecoder, KeyframeAnimationEncoder, SamplerInterpolation,
};
use draco_bitstream::compression::config::decoder_options::DecoderOptions;
use draco_bitstream::compression::config::encoder_options::EncoderOptions;
use draco_core::attributes::geometry_indices::AttributeValueIndex;
use draco_core::core::decoder_buffer::DecoderBuffer;
use draco_core::core::draco_types::DataType;
use draco_core::core::encoder_buffer::EncoderBuffer;

/// Asserts that a Status-like value is OK (test-only helper).
macro_rules! draco_assert_ok {
    ($expression:expr) => {{
        let _local_status = $expression;
        assert!(
            _local_status.is_ok(),
            "{}",
            _local_status.error_msg_string()
        );
    }};
}

struct KeyframeAnimationTestHarness {
    animation: KeyframeAnimation,
    timestamps: Vec<f32>,
    animation_data: Vec<f32>,
}

impl KeyframeAnimationTestHarness {
    fn new() -> Self {
        Self {
            animation: KeyframeAnimation::new(),
            timestamps: Vec::new(),
            animation_data: Vec::new(),
        }
    }

    fn create_and_add_timestamps(&mut self, num_frames: i32) -> bool {
        self.timestamps.clear();
        self.timestamps.resize(num_frames as usize, 0.0);
        for i in 0..self.timestamps.len() {
            self.timestamps[i] = i as f32;
        }
        // Timestamp attribute must be unique and always uses id 0.
        self.animation.set_timestamps(&self.timestamps)
    }

    fn create_and_add_animation_data(&mut self, num_frames: i32, num_components: u32) -> i32 {
        self.animation_data.clear();
        self.animation_data
            .resize((num_frames as usize) * (num_components as usize), 0.0);
        for i in 0..self.animation_data.len() {
            self.animation_data[i] = i as f32;
        }
        // Keyframe data is stored as a PointAttribute with the requested component count.
        self.animation
            .add_keyframes(DataType::Float32, num_components, &self.animation_data)
    }

    fn compare_animation_data<const N: usize>(&self) {
        // Compare timestamps via the attribute accessors (mirrors C++ GetValue).
        let timestamp_att = self
            .animation
            .timestamps()
            .expect("timestamps attribute missing");
        for i in 0..self.timestamps.len() {
            let mut att_value = [0.0f32; 1];
            assert!(timestamp_att.get_value_array_into::<f32, 1>(
                AttributeValueIndex::from(i as u32),
                &mut att_value
            ));
            assert_eq!(att_value[0], i as f32);
        }

        // Compare keyframe data for the first animation attribute (id = 1).
        let keyframe_att = self
            .animation
            .keyframes(1)
            .expect("keyframe attribute missing");
        for i in 0..(self.animation_data.len() / N) {
            let mut att_value = [0.0f32; N];
            assert!(keyframe_att.get_value_array_into::<f32, N>(
                AttributeValueIndex::from(i as u32),
                &mut att_value
            ));
            for j in 0..N {
                let expected = (i * N + j) as f32;
                assert_eq!(att_value[j], expected);
            }
        }
    }

    fn test_keyframe_animation<const N: usize>(&mut self, num_frames: i32) {
        assert!(self.create_and_add_timestamps(num_frames));
        assert_eq!(self.create_and_add_animation_data(num_frames, N as u32), 1);
        self.compare_animation_data::<N>();
    }
}

struct KeyframeAnimationEncodingHarness {
    animation: KeyframeAnimation,
    timestamps: Vec<f32>,
    animation_data: Vec<f32>,
}

impl KeyframeAnimationEncodingHarness {
    fn new() -> Self {
        Self {
            animation: KeyframeAnimation::new(),
            timestamps: Vec::new(),
            animation_data: Vec::new(),
        }
    }

    fn create_and_add_timestamps(&mut self, num_frames: i32) -> bool {
        self.timestamps.clear();
        self.timestamps.resize(num_frames as usize, 0.0);
        for i in 0..self.timestamps.len() {
            self.timestamps[i] = i as f32;
        }
        self.animation.set_timestamps(&self.timestamps)
    }

    fn create_and_add_animation_data(&mut self, num_frames: i32, num_components: u32) -> i32 {
        self.animation_data.clear();
        self.animation_data
            .resize((num_frames as usize) * (num_components as usize), 0.0);
        for i in 0..self.animation_data.len() {
            self.animation_data[i] = i as f32;
        }
        self.animation
            .add_keyframes(DataType::Float32, num_components, &self.animation_data)
    }

    fn compare_animation_data<const N: usize>(
        &self,
        animation0: &KeyframeAnimation,
        animation1: &KeyframeAnimation,
        quantized: bool,
    ) {
        assert_eq!(animation0.num_frames(), animation1.num_frames());
        assert_eq!(animation0.num_animations(), animation1.num_animations());

        if quantized {
            // Quantization introduces slight value differences; reference test skips value checks.
            return;
        }

        // Compare timestamps using attribute accessors (reference test uses animation0 for both).
        let timestamp_att0 = animation0
            .timestamps()
            .expect("timestamps attribute missing");
        let timestamp_att1 = animation0
            .timestamps()
            .expect("timestamps attribute missing");
        for i in 0..animation0.num_frames() {
            let mut att_value0 = [0.0f32; 1];
            let mut att_value1 = [0.0f32; 1];
            assert!(timestamp_att0.get_value_array_into::<f32, 1>(
                AttributeValueIndex::from(i as u32),
                &mut att_value0
            ));
            assert!(timestamp_att1.get_value_array_into::<f32, 1>(
                AttributeValueIndex::from(i as u32),
                &mut att_value1
            ));
            assert_eq!(att_value0[0], att_value1[0]);
        }

        for animation_id in 1..animation0.num_animations() {
            let keyframe_att0 = animation0
                .keyframes(animation_id)
                .expect("keyframe attribute missing");
            let keyframe_att1 = animation1
                .keyframes(animation_id)
                .expect("keyframe attribute missing");
            assert_eq!(
                keyframe_att0.num_components(),
                keyframe_att1.num_components()
            );
            for i in 0..animation0.num_frames() {
                let mut att_value0 = [0.0f32; N];
                let mut att_value1 = [0.0f32; N];
                assert!(keyframe_att0.get_value_array_into::<f32, N>(
                    AttributeValueIndex::from(i as u32),
                    &mut att_value0
                ));
                assert!(keyframe_att1.get_value_array_into::<f32, N>(
                    AttributeValueIndex::from(i as u32),
                    &mut att_value1
                ));
                for j in 0..att_value0.len() {
                    assert_eq!(att_value0[j], att_value1[j]);
                }
            }
        }
    }

    fn test_keyframe_animation_encoding<const N: usize>(&self, quantized: bool) {
        let mut buffer = EncoderBuffer::new();
        let mut encoder = KeyframeAnimationEncoder::new();
        let mut options = EncoderOptions::create_default_options();

        if quantized {
            // Match reference: set quantization for timestamps and each keyframe attribute.
            options.set_attribute_int(&0, "quantization_bits", 20);
            for i in 1..=self.animation.num_animations() {
                options.set_attribute_int(&i, "quantization_bits", 20);
            }
        }

        draco_assert_ok!(encoder.encode_keyframe_animation(&self.animation, &options, &mut buffer));

        let mut decoder = KeyframeAnimationDecoder::new();
        let mut dec_buffer = DecoderBuffer::new();
        dec_buffer.init(buffer.data());
        let dec_options = DecoderOptions::default();

        let mut decoded_animation = KeyframeAnimation::new();
        draco_assert_ok!(decoder.decode(&dec_options, &mut dec_buffer, &mut decoded_animation));

        self.compare_animation_data::<N>(&self.animation, &decoded_animation, quantized);
    }
}

#[test]
fn animation_copy() {
    let mut src_anim = Animation::new();
    assert!(src_anim.name().is_empty());
    src_anim.set_name("Walking");
    assert_eq!(src_anim.name(), "Walking");

    let mut src_sampler_0 = Box::new(AnimationSampler::default());
    src_sampler_0.interpolation_type = SamplerInterpolation::CubicSpline;

    let mut src_sampler_1 = Box::new(AnimationSampler::default());
    src_sampler_1.copy_from(&src_sampler_0);
    assert_eq!(
        src_sampler_0.interpolation_type,
        src_sampler_1.interpolation_type
    );

    src_sampler_1.interpolation_type = SamplerInterpolation::Step;

    src_anim.add_sampler(src_sampler_0);
    src_anim.add_sampler(src_sampler_1);
    assert_eq!(src_anim.num_samplers(), 2);

    let mut src_channel = Box::new(AnimationChannel::default());
    src_channel.transformation_type = ChannelTransformation::Weights;
    src_anim.add_channel(src_channel);
    assert_eq!(src_anim.num_channels(), 1);

    let mut dst_anim = Animation::new();
    dst_anim.copy_from(&src_anim);

    assert_eq!(dst_anim.name(), src_anim.name());
    assert_eq!(dst_anim.num_samplers(), 2);
    assert_eq!(dst_anim.num_channels(), 1);

    assert_eq!(
        dst_anim.sampler(0).unwrap().interpolation_type,
        src_anim.sampler(0).unwrap().interpolation_type
    );
    assert_eq!(
        dst_anim.sampler(1).unwrap().interpolation_type,
        src_anim.sampler(1).unwrap().interpolation_type
    );
    assert_eq!(
        dst_anim.channel(0).unwrap().transformation_type,
        src_anim.channel(0).unwrap().transformation_type
    );
}

#[test]
fn keyframe_animation_one_component() {
    let mut harness = KeyframeAnimationTestHarness::new();
    harness.test_keyframe_animation::<1>(10);
}

#[test]
fn keyframe_animation_four_component() {
    let mut harness = KeyframeAnimationTestHarness::new();
    harness.test_keyframe_animation::<4>(10);
}

#[test]
fn keyframe_animation_adding_animation_first() {
    let mut harness = KeyframeAnimationTestHarness::new();
    assert_eq!(harness.create_and_add_animation_data(5, 1), 1);
    assert!(harness.create_and_add_timestamps(5));
}

#[test]
fn keyframe_animation_error_adding_timestamps_twice() {
    let mut harness = KeyframeAnimationTestHarness::new();
    assert!(harness.create_and_add_timestamps(5));
    assert!(!harness.create_and_add_timestamps(5));
}

#[test]
fn keyframe_animation_multiple_animation_data() {
    let mut harness = KeyframeAnimationTestHarness::new();
    let num_frames = 5;
    assert!(harness.create_and_add_timestamps(num_frames));
    assert_eq!(harness.create_and_add_animation_data(num_frames, 1), 1);
    assert_eq!(harness.create_and_add_animation_data(num_frames, 2), 2);
}

#[test]
fn keyframe_animation_encoding_one_component() {
    let mut harness = KeyframeAnimationEncodingHarness::new();
    let num_frames = 1;
    assert!(harness.create_and_add_timestamps(num_frames));
    assert_eq!(harness.create_and_add_animation_data(num_frames, 1), 1);
    harness.test_keyframe_animation_encoding::<1>(false);
}

#[test]
fn keyframe_animation_encoding_many_components() {
    let mut harness = KeyframeAnimationEncodingHarness::new();
    let num_frames = 100;
    assert!(harness.create_and_add_timestamps(num_frames));
    assert_eq!(harness.create_and_add_animation_data(num_frames, 100), 1);
    harness.test_keyframe_animation_encoding::<100>(false);
}

#[test]
fn keyframe_animation_encoding_many_components_with_quantization() {
    let mut harness = KeyframeAnimationEncodingHarness::new();
    let num_frames = 100;
    assert!(harness.create_and_add_timestamps(num_frames));
    assert_eq!(harness.create_and_add_animation_data(num_frames, 4), 1);
    harness.test_keyframe_animation_encoding::<4>(true);
}

#[test]
fn keyframe_animation_encoding_multiple_animations() {
    let mut harness = KeyframeAnimationEncodingHarness::new();
    let num_frames = 5;
    assert!(harness.create_and_add_timestamps(num_frames));
    assert_eq!(harness.create_and_add_animation_data(num_frames, 3), 1);
    assert_eq!(harness.create_and_add_animation_data(num_frames, 3), 2);
    harness.test_keyframe_animation_encoding::<3>(false);
}
