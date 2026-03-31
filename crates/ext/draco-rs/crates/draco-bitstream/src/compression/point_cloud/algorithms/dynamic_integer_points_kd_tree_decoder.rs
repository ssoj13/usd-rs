//! Dynamic integer point cloud kD-tree decoder.
//! Reference: `_ref/draco/src/draco/compression/point_cloud/algorithms/dynamic_integer_points_kd_tree_decoder.h`.
//!
//! Decodes integer point clouds encoded by DynamicIntegerPointsKdTreeEncoder.

use crate::compression::bit_coders::direct_bit_decoder::DirectBitDecoder;
use crate::compression::bit_coders::folded_integer_bit_decoder::FoldedBit32Decoder;
use crate::compression::bit_coders::rans_bit_decoder::RAnsBitDecoder;
use crate::compression::point_cloud::algorithms::quantize_points_3::PointOutput;
use draco_core::core::bit_utils::most_significant_bit;
use draco_core::core::decoder_buffer::DecoderBuffer;
use draco_core::core::math_utils::increment_mod;
use draco_core::draco_dcheck_eq;

/// Minimal bit-decoder API used by kd-tree algorithms.
pub(crate) trait BitDecoderLike {
    fn start_decoding(&mut self, source_buffer: &mut DecoderBuffer) -> bool;
    fn decode_next_bit(&mut self) -> bool;
    fn decode_lsb32(&mut self, nbits: i32, value: &mut u32) -> bool;
    fn end_decoding(&mut self);
}

impl BitDecoderLike for DirectBitDecoder {
    fn start_decoding(&mut self, source_buffer: &mut DecoderBuffer) -> bool {
        self.start_decoding(source_buffer)
    }

    fn decode_next_bit(&mut self) -> bool {
        self.decode_next_bit()
    }

    fn decode_lsb32(&mut self, nbits: i32, value: &mut u32) -> bool {
        self.decode_least_significant_bits32(nbits, value)
    }

    fn end_decoding(&mut self) {
        // Direct decoder has no explicit end state.
    }
}

impl BitDecoderLike for RAnsBitDecoder {
    fn start_decoding(&mut self, source_buffer: &mut DecoderBuffer) -> bool {
        self.start_decoding(source_buffer)
    }

    fn decode_next_bit(&mut self) -> bool {
        self.decode_next_bit()
    }

    fn decode_lsb32(&mut self, nbits: i32, value: &mut u32) -> bool {
        self.decode_least_significant_bits32(nbits, value);
        true
    }

    fn end_decoding(&mut self) {
        // rANS decoder has no explicit end state.
    }
}

impl<T: crate::compression::bit_coders::BitDecoder + Default> BitDecoderLike
    for FoldedBit32Decoder<T>
{
    fn start_decoding(&mut self, source_buffer: &mut DecoderBuffer) -> bool {
        self.start_decoding(source_buffer)
    }

    fn decode_next_bit(&mut self) -> bool {
        self.decode_next_bit()
    }

    fn decode_lsb32(&mut self, nbits: i32, value: &mut u32) -> bool {
        self.decode_least_significant_bits32(nbits, value);
        true
    }

    fn end_decoding(&mut self) {
        self.end_decoding();
    }
}

/// Compression policy selection (0..6).
pub(crate) trait DynamicDecoderPolicy {
    type NumbersDecoder: BitDecoderLike + Default;
    type AxisDecoder: BitDecoderLike + Default;
    type HalfDecoder: BitDecoderLike + Default;
    type RemainingBitsDecoder: BitDecoderLike + Default;
    const SELECT_AXIS: bool;
}

pub(crate) struct DecoderPolicy<const LEVEL: usize>;

impl DynamicDecoderPolicy for DecoderPolicy<0> {
    type NumbersDecoder = DirectBitDecoder;
    type AxisDecoder = DirectBitDecoder;
    type HalfDecoder = DirectBitDecoder;
    type RemainingBitsDecoder = DirectBitDecoder;
    const SELECT_AXIS: bool = false;
}

impl DynamicDecoderPolicy for DecoderPolicy<1> {
    type NumbersDecoder = DirectBitDecoder;
    type AxisDecoder = DirectBitDecoder;
    type HalfDecoder = DirectBitDecoder;
    type RemainingBitsDecoder = DirectBitDecoder;
    const SELECT_AXIS: bool = false;
}

impl DynamicDecoderPolicy for DecoderPolicy<2> {
    type NumbersDecoder = RAnsBitDecoder;
    type AxisDecoder = DirectBitDecoder;
    type HalfDecoder = DirectBitDecoder;
    type RemainingBitsDecoder = DirectBitDecoder;
    const SELECT_AXIS: bool = false;
}

impl DynamicDecoderPolicy for DecoderPolicy<3> {
    type NumbersDecoder = RAnsBitDecoder;
    type AxisDecoder = DirectBitDecoder;
    type HalfDecoder = DirectBitDecoder;
    type RemainingBitsDecoder = DirectBitDecoder;
    const SELECT_AXIS: bool = false;
}

impl DynamicDecoderPolicy for DecoderPolicy<4> {
    type NumbersDecoder = FoldedBit32Decoder<RAnsBitDecoder>;
    type AxisDecoder = DirectBitDecoder;
    type HalfDecoder = DirectBitDecoder;
    type RemainingBitsDecoder = DirectBitDecoder;
    const SELECT_AXIS: bool = false;
}

impl DynamicDecoderPolicy for DecoderPolicy<5> {
    type NumbersDecoder = FoldedBit32Decoder<RAnsBitDecoder>;
    type AxisDecoder = DirectBitDecoder;
    type HalfDecoder = DirectBitDecoder;
    type RemainingBitsDecoder = DirectBitDecoder;
    const SELECT_AXIS: bool = false;
}

impl DynamicDecoderPolicy for DecoderPolicy<6> {
    type NumbersDecoder = FoldedBit32Decoder<RAnsBitDecoder>;
    type AxisDecoder = DirectBitDecoder;
    type HalfDecoder = DirectBitDecoder;
    type RemainingBitsDecoder = DirectBitDecoder;
    const SELECT_AXIS: bool = true;
}

/// Dynamic integer kD-tree decoder (0..6).
pub(crate) struct DynamicIntegerPointsKdTreeDecoder<const LEVEL: usize>
where
    DecoderPolicy<LEVEL>: DynamicDecoderPolicy,
{
    bit_length: u32,
    num_points: u32,
    num_decoded_points: u32,
    dimension: u32,
    numbers_decoder: <DecoderPolicy<LEVEL> as DynamicDecoderPolicy>::NumbersDecoder,
    remaining_bits_decoder: <DecoderPolicy<LEVEL> as DynamicDecoderPolicy>::RemainingBitsDecoder,
    axis_decoder: <DecoderPolicy<LEVEL> as DynamicDecoderPolicy>::AxisDecoder,
    half_decoder: <DecoderPolicy<LEVEL> as DynamicDecoderPolicy>::HalfDecoder,
    p: Vec<u32>,
    axes: Vec<u32>,
    base_stack: Vec<Vec<u32>>,
    levels_stack: Vec<Vec<u32>>,
}

impl<const LEVEL: usize> DynamicIntegerPointsKdTreeDecoder<LEVEL>
where
    DecoderPolicy<LEVEL>: DynamicDecoderPolicy,
{
    pub fn new(dimension: u32) -> Self {
        let stack_len = 32usize.saturating_mul(dimension as usize).saturating_add(1);
        Self {
            bit_length: 0,
            num_points: 0,
            num_decoded_points: 0,
            dimension,
            numbers_decoder: Default::default(),
            remaining_bits_decoder: Default::default(),
            axis_decoder: Default::default(),
            half_decoder: Default::default(),
            p: vec![0; dimension as usize],
            axes: vec![0; dimension as usize],
            base_stack: vec![vec![0; dimension as usize]; stack_len],
            levels_stack: vec![vec![0; dimension as usize]; stack_len],
        }
    }

    pub fn num_decoded_points(&self) -> u32 {
        self.num_decoded_points
    }

    pub fn decode_points<O: PointOutput<u32>>(
        &mut self,
        buffer: &mut DecoderBuffer,
        out: &mut O,
        max_points: u32,
    ) -> bool {
        if !buffer.decode(&mut self.bit_length) {
            return false;
        }
        if self.bit_length > 32 {
            return false;
        }
        if !buffer.decode(&mut self.num_points) {
            return false;
        }
        if self.num_points == 0 {
            return true;
        }
        if self.num_points > max_points {
            return false;
        }
        self.num_decoded_points = 0;

        if !self.numbers_decoder.start_decoding(buffer) {
            return false;
        }
        if !self.remaining_bits_decoder.start_decoding(buffer) {
            return false;
        }
        if !self.axis_decoder.start_decoding(buffer) {
            return false;
        }
        if !self.half_decoder.start_decoding(buffer) {
            return false;
        }

        if !self.decode_internal(self.num_points as usize, out) {
            return false;
        }

        self.numbers_decoder.end_decoding();
        self.remaining_bits_decoder.end_decoding();
        self.axis_decoder.end_decoding();
        self.half_decoder.end_decoding();
        true
    }

    fn decode_internal<O: PointOutput<u32>>(&mut self, num_points: usize, out: &mut O) -> bool {
        #[derive(Clone, Copy)]
        struct Status {
            num_remaining_points: usize,
            last_axis: u32,
            stack_pos: usize,
        }

        self.base_stack[0].fill(0);
        self.levels_stack[0].fill(0);
        let mut stack = Vec::new();
        stack.push(Status {
            num_remaining_points: num_points,
            last_axis: 0,
            stack_pos: 0,
        });

        while let Some(status) = stack.pop() {
            let num_remaining_points = status.num_remaining_points;
            let last_axis = status.last_axis;
            let stack_pos = status.stack_pos;
            let old_base = self.base_stack[stack_pos].clone();
            let levels_snapshot = self.levels_stack[stack_pos].clone();

            if num_remaining_points > num_points {
                return false;
            }

            let axis = self.get_axis(num_remaining_points, &levels_snapshot, last_axis);
            if axis as usize >= self.dimension as usize {
                return false;
            }
            let level = levels_snapshot[axis as usize];

            if (self.bit_length - level) == 0 {
                for _ in 0..num_remaining_points {
                    out.write_point(&old_base);
                    self.num_decoded_points += 1;
                }
                continue;
            }

            draco_dcheck_eq!(true, num_remaining_points != 0);

            if num_remaining_points <= 2 {
                self.axes[0] = axis;
                for i in 1..self.dimension as usize {
                    self.axes[i] = increment_mod(self.axes[i - 1], self.dimension);
                }
                for _ in 0..num_remaining_points {
                    for j in 0..self.dimension as usize {
                        self.p[self.axes[j] as usize] = 0;
                        let remaining = self.bit_length - levels_snapshot[self.axes[j] as usize];
                        if remaining > 0 {
                            if !self
                                .remaining_bits_decoder
                                .decode_lsb32(remaining as i32, &mut self.p[self.axes[j] as usize])
                            {
                                return false;
                            }
                        }
                        self.p[self.axes[j] as usize] |= old_base[self.axes[j] as usize];
                    }
                    out.write_point(&self.p);
                    self.num_decoded_points += 1;
                }
                continue;
            }

            if self.num_decoded_points > self.num_points {
                return false;
            }

            let num_remaining_bits = self.bit_length - level;
            let modifier = 1u32 << (num_remaining_bits - 1);
            self.base_stack[stack_pos] = old_base.clone();
            self.base_stack[stack_pos + 1] = old_base.clone();
            self.base_stack[stack_pos + 1][axis as usize] += modifier;

            let incoming_bits = most_significant_bit(num_remaining_points as u32);
            let mut number = 0u32;
            if !self
                .numbers_decoder
                .decode_lsb32(incoming_bits, &mut number)
            {
                return false;
            }

            let mut first_half = num_remaining_points / 2;
            if first_half < number as usize {
                return false;
            }
            first_half -= number as usize;
            let mut second_half = num_remaining_points - first_half;

            if first_half != second_half {
                if !self.half_decoder.decode_next_bit() {
                    std::mem::swap(&mut first_half, &mut second_half);
                }
            }

            self.levels_stack[stack_pos] = levels_snapshot.clone();
            self.levels_stack[stack_pos][axis as usize] += 1;
            self.levels_stack[stack_pos + 1] = self.levels_stack[stack_pos].clone();
            if first_half > 0 {
                stack.push(Status {
                    num_remaining_points: first_half,
                    last_axis: axis,
                    stack_pos,
                });
            }
            if second_half > 0 {
                stack.push(Status {
                    num_remaining_points: second_half,
                    last_axis: axis,
                    stack_pos: stack_pos + 1,
                });
            }
        }
        true
    }

    fn get_axis(&mut self, num_remaining_points: usize, levels: &[u32], last_axis: u32) -> u32 {
        if !<DecoderPolicy<LEVEL> as DynamicDecoderPolicy>::SELECT_AXIS {
            return increment_mod(last_axis, self.dimension);
        }

        let mut best_axis = 0u32;
        if num_remaining_points < 64 {
            for axis in 1..self.dimension as usize {
                if levels[best_axis as usize] > levels[axis] {
                    best_axis = axis as u32;
                }
            }
        } else {
            let mut axis = 0u32;
            self.axis_decoder.decode_lsb32(4, &mut axis);
            best_axis = axis;
        }
        best_axis
    }
}
