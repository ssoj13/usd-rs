//! Integer point cloud kD-tree decoder (legacy).
//! Reference: `_ref/draco/src/draco/compression/point_cloud/algorithms/integer_points_kd_tree_decoder.h`.
//!
//! Decodes integer point clouds encoded by IntegerPointsKdTreeEncoder.

#![allow(dead_code)]

use std::cmp::Ordering;

use crate::compression::bit_coders::adaptive_rans_bit_decoder::AdaptiveRAnsBitDecoder;
use crate::compression::bit_coders::direct_bit_decoder::DirectBitDecoder;
use crate::compression::bit_coders::folded_integer_bit_decoder::FoldedBit32Decoder;
use crate::compression::bit_coders::rans_bit_decoder::RAnsBitDecoder;
use crate::compression::point_cloud::algorithms::point_cloud_types::PointTraits;
use crate::compression::point_cloud::algorithms::quantize_points_3::PointOutput;
use crate::compression::point_cloud::algorithms::queuing_policy::{PriorityQueue, Queue, Stack};
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

/// Queue policy abstraction.
pub(crate) trait QueueLike<T> {
    fn empty(&self) -> bool;
    fn push(&mut self, value: T);
    fn pop(&mut self);
    fn front(&self) -> &T;
}

impl<T> QueueLike<T> for Queue<T> {
    fn empty(&self) -> bool {
        self.empty()
    }

    fn push(&mut self, value: T) {
        self.push(value);
    }

    fn pop(&mut self) {
        self.pop();
    }

    fn front(&self) -> &T {
        self.front()
    }
}

impl<T> QueueLike<T> for Stack<T> {
    fn empty(&self) -> bool {
        self.empty()
    }

    fn push(&mut self, value: T) {
        self.push(value);
    }

    fn pop(&mut self) {
        self.pop();
    }

    fn front(&self) -> &T {
        self.front()
    }
}

impl<T: Ord> QueueLike<T> for PriorityQueue<T> {
    fn empty(&self) -> bool {
        self.empty()
    }

    fn push(&mut self, value: T) {
        self.push(value);
    }

    fn pop(&mut self) {
        self.pop();
    }

    fn front(&self) -> &T {
        self.front()
    }
}

/// Compression policy selection (0..10).
pub(crate) trait IntegerDecoderPolicy<PointT: Copy> {
    type NumbersDecoder: BitDecoderLike + Default;
    type AxisDecoder: BitDecoderLike + Default;
    type HalfDecoder: BitDecoderLike + Default;
    type RemainingBitsDecoder: BitDecoderLike + Default;
    const SELECT_AXIS: bool;
    type Queue: QueueLike<DecodingStatus<PointT>> + Default;
}

pub(crate) struct DecoderPolicy<const LEVEL: usize>;

impl<PointT: Copy> IntegerDecoderPolicy<PointT> for DecoderPolicy<0> {
    type NumbersDecoder = DirectBitDecoder;
    type AxisDecoder = DirectBitDecoder;
    type HalfDecoder = DirectBitDecoder;
    type RemainingBitsDecoder = DirectBitDecoder;
    const SELECT_AXIS: bool = false;
    type Queue = Stack<DecodingStatus<PointT>>;
}

impl<PointT: Copy> IntegerDecoderPolicy<PointT> for DecoderPolicy<1> {
    type NumbersDecoder = DirectBitDecoder;
    type AxisDecoder = DirectBitDecoder;
    type HalfDecoder = DirectBitDecoder;
    type RemainingBitsDecoder = DirectBitDecoder;
    const SELECT_AXIS: bool = false;
    type Queue = Stack<DecodingStatus<PointT>>;
}

impl<PointT: Copy> IntegerDecoderPolicy<PointT> for DecoderPolicy<2> {
    type NumbersDecoder = RAnsBitDecoder;
    type AxisDecoder = DirectBitDecoder;
    type HalfDecoder = DirectBitDecoder;
    type RemainingBitsDecoder = DirectBitDecoder;
    const SELECT_AXIS: bool = false;
    type Queue = Stack<DecodingStatus<PointT>>;
}

impl<PointT: Copy> IntegerDecoderPolicy<PointT> for DecoderPolicy<3> {
    type NumbersDecoder = RAnsBitDecoder;
    type AxisDecoder = DirectBitDecoder;
    type HalfDecoder = DirectBitDecoder;
    type RemainingBitsDecoder = DirectBitDecoder;
    const SELECT_AXIS: bool = false;
    type Queue = Stack<DecodingStatus<PointT>>;
}

impl<PointT: Copy> IntegerDecoderPolicy<PointT> for DecoderPolicy<4> {
    type NumbersDecoder = FoldedBit32Decoder<RAnsBitDecoder>;
    type AxisDecoder = DirectBitDecoder;
    type HalfDecoder = DirectBitDecoder;
    type RemainingBitsDecoder = DirectBitDecoder;
    const SELECT_AXIS: bool = false;
    type Queue = Stack<DecodingStatus<PointT>>;
}

impl<PointT: Copy> IntegerDecoderPolicy<PointT> for DecoderPolicy<5> {
    type NumbersDecoder = FoldedBit32Decoder<RAnsBitDecoder>;
    type AxisDecoder = DirectBitDecoder;
    type HalfDecoder = DirectBitDecoder;
    type RemainingBitsDecoder = DirectBitDecoder;
    const SELECT_AXIS: bool = false;
    type Queue = Stack<DecodingStatus<PointT>>;
}

impl<PointT: Copy> IntegerDecoderPolicy<PointT> for DecoderPolicy<6> {
    type NumbersDecoder = FoldedBit32Decoder<RAnsBitDecoder>;
    type AxisDecoder = DirectBitDecoder;
    type HalfDecoder = DirectBitDecoder;
    type RemainingBitsDecoder = DirectBitDecoder;
    const SELECT_AXIS: bool = true;
    type Queue = Stack<DecodingStatus<PointT>>;
}

impl<PointT: Copy> IntegerDecoderPolicy<PointT> for DecoderPolicy<7> {
    type NumbersDecoder = FoldedBit32Decoder<RAnsBitDecoder>;
    type AxisDecoder = DirectBitDecoder;
    type HalfDecoder = DirectBitDecoder;
    type RemainingBitsDecoder = DirectBitDecoder;
    const SELECT_AXIS: bool = true;
    type Queue = Stack<DecodingStatus<PointT>>;
}

impl<PointT: Copy> IntegerDecoderPolicy<PointT> for DecoderPolicy<8> {
    type NumbersDecoder = FoldedBit32Decoder<AdaptiveRAnsBitDecoder>;
    type AxisDecoder = DirectBitDecoder;
    type HalfDecoder = DirectBitDecoder;
    type RemainingBitsDecoder = DirectBitDecoder;
    const SELECT_AXIS: bool = true;
    type Queue = Queue<DecodingStatus<PointT>>;
}

impl<PointT: Copy> IntegerDecoderPolicy<PointT> for DecoderPolicy<9> {
    type NumbersDecoder = FoldedBit32Decoder<AdaptiveRAnsBitDecoder>;
    type AxisDecoder = DirectBitDecoder;
    type HalfDecoder = DirectBitDecoder;
    type RemainingBitsDecoder = DirectBitDecoder;
    const SELECT_AXIS: bool = true;
    type Queue = Queue<DecodingStatus<PointT>>;
}

impl<PointT: Copy> IntegerDecoderPolicy<PointT> for DecoderPolicy<10> {
    type NumbersDecoder = FoldedBit32Decoder<AdaptiveRAnsBitDecoder>;
    type AxisDecoder = DirectBitDecoder;
    type HalfDecoder = DirectBitDecoder;
    type RemainingBitsDecoder = DirectBitDecoder;
    const SELECT_AXIS: bool = true;
    type Queue = PriorityQueue<DecodingStatus<PointT>>;
}

#[derive(Clone)]
pub(crate) struct DecodingStatus<PointT: Copy> {
    num_remaining_points: usize,
    old_base: PointT,
    levels: Vec<u32>,
    last_axis: u32,
}

impl<PointT: Copy> DecodingStatus<PointT> {
    fn new(
        num_remaining_points: usize,
        old_base: PointT,
        levels: Vec<u32>,
        last_axis: u32,
    ) -> Self {
        Self {
            num_remaining_points,
            old_base,
            levels,
            last_axis,
        }
    }
}

impl<PointT: Copy> PartialEq for DecodingStatus<PointT> {
    fn eq(&self, other: &Self) -> bool {
        self.num_remaining_points == other.num_remaining_points
    }
}

impl<PointT: Copy> Eq for DecodingStatus<PointT> {}

impl<PointT: Copy> PartialOrd for DecodingStatus<PointT> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<PointT: Copy> Ord for DecodingStatus<PointT> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.num_remaining_points.cmp(&other.num_remaining_points)
    }
}

/// Integer kD-tree decoder.
pub(crate) struct IntegerPointsKdTreeDecoder<PointT: Copy + Default, const LEVEL: usize>
where
    DecoderPolicy<LEVEL>: IntegerDecoderPolicy<PointT>,
{
    bit_length: u32,
    num_points: u32,
    numbers_decoder: <DecoderPolicy<LEVEL> as IntegerDecoderPolicy<PointT>>::NumbersDecoder,
    remaining_bits_decoder:
        <DecoderPolicy<LEVEL> as IntegerDecoderPolicy<PointT>>::RemainingBitsDecoder,
    axis_decoder: <DecoderPolicy<LEVEL> as IntegerDecoderPolicy<PointT>>::AxisDecoder,
    half_decoder: <DecoderPolicy<LEVEL> as IntegerDecoderPolicy<PointT>>::HalfDecoder,
    _marker: std::marker::PhantomData<PointT>,
}

impl<PointT: Copy + Default, const LEVEL: usize> IntegerPointsKdTreeDecoder<PointT, LEVEL>
where
    DecoderPolicy<LEVEL>: IntegerDecoderPolicy<PointT>,
    PointT: std::ops::Index<usize, Output = u32> + std::ops::IndexMut<usize>,
    PointT: PointTraits<Point = PointT>,
{
    pub fn new() -> Self {
        Self {
            bit_length: 0,
            num_points: 0,
            numbers_decoder: Default::default(),
            remaining_bits_decoder: Default::default(),
            axis_decoder: Default::default(),
            half_decoder: Default::default(),
            _marker: std::marker::PhantomData,
        }
    }

    pub fn decode_points<O: PointOutput<u32>>(
        &mut self,
        buffer: &mut DecoderBuffer,
        out: &mut O,
    ) -> bool {
        if !buffer.decode(&mut self.bit_length) {
            return false;
        }
        if !buffer.decode(&mut self.num_points) {
            return false;
        }
        if self.num_points == 0 {
            return true;
        }

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

        let levels = <PointT as PointTraits>::zero_levels();
        self.decode_internal(
            self.num_points as usize,
            <PointT as PointTraits>::origin(),
            levels,
            0,
            out,
        );

        self.numbers_decoder.end_decoding();
        self.remaining_bits_decoder.end_decoding();
        self.axis_decoder.end_decoding();
        self.half_decoder.end_decoding();
        true
    }

    fn decode_internal<O: PointOutput<u32>>(
        &mut self,
        num_remaining_points: usize,
        old_base: PointT,
        levels: Vec<u32>,
        last_axis: u32,
        out: &mut O,
    ) {
        let mut status_q: <DecoderPolicy<LEVEL> as IntegerDecoderPolicy<PointT>>::Queue =
            Default::default();
        status_q.push(DecodingStatus::new(
            num_remaining_points,
            old_base,
            levels,
            last_axis,
        ));

        while !status_q.empty() {
            let status = status_q.front().clone();
            status_q.pop();

            let num_remaining_points = status.num_remaining_points;
            let old_base = status.old_base;
            let levels = status.levels;
            let last_axis = status.last_axis;
            let dimension = <PointT as PointTraits>::DIMENSION;

            let axis = self.get_axis(num_remaining_points, &old_base, &levels, last_axis);
            let level = levels[axis as usize];

            if (self.bit_length - level) == 0 {
                for _ in 0..num_remaining_points {
                    let mut coords = vec![0u32; dimension];
                    for i in 0..dimension {
                        coords[i] = old_base[i];
                    }
                    out.write_point(&coords);
                }
                continue;
            }

            draco_dcheck_eq!(true, num_remaining_points != 0);

            if num_remaining_points <= 2 {
                let mut axes = vec![0u32; dimension];
                axes[0] = axis;
                for i in 1..dimension {
                    axes[i] = increment_mod(axes[i - 1], dimension as u32);
                }
                for _ in 0..num_remaining_points {
                    let mut coords = vec![0u32; dimension];
                    for j in 0..dimension {
                        let mut val = 0u32;
                        let remaining = self.bit_length - levels[axes[j] as usize];
                        if remaining > 0 {
                            let _ = self
                                .remaining_bits_decoder
                                .decode_lsb32(remaining as i32, &mut val);
                        }
                        coords[axes[j] as usize] = old_base[axes[j] as usize] | val;
                    }
                    out.write_point(&coords);
                }
                continue;
            }

            let num_remaining_bits = self.bit_length - level;
            let modifier = 1u32 << (num_remaining_bits - 1);
            let mut new_base = old_base;
            new_base[axis as usize] += modifier;

            let incoming_bits = most_significant_bit(num_remaining_points as u32);
            let mut number = 0u32;
            let _ = self
                .numbers_decoder
                .decode_lsb32(incoming_bits, &mut number);

            let mut first_half = num_remaining_points / 2;
            if first_half < number as usize {
                return;
            }
            first_half -= number as usize;
            let mut second_half = num_remaining_points - first_half;

            if first_half != second_half {
                if !self.half_decoder.decode_next_bit() {
                    std::mem::swap(&mut first_half, &mut second_half);
                }
            }

            let mut next_levels = levels;
            next_levels[axis as usize] += 1;
            if first_half > 0 {
                status_q.push(DecodingStatus::new(
                    first_half,
                    old_base,
                    next_levels.clone(),
                    axis,
                ));
            }
            if second_half > 0 {
                status_q.push(DecodingStatus::new(
                    second_half,
                    new_base,
                    next_levels,
                    axis,
                ));
            }
        }
    }

    fn get_axis(
        &mut self,
        num_remaining_points: usize,
        _base: &PointT,
        levels: &[u32],
        last_axis: u32,
    ) -> u32 {
        if !<DecoderPolicy<LEVEL> as IntegerDecoderPolicy<PointT>>::SELECT_AXIS {
            return increment_mod(last_axis, <PointT as PointTraits>::DIMENSION as u32);
        }

        let mut best_axis = 0u32;
        if num_remaining_points < 64 {
            for axis in 1..levels.len() {
                if levels[best_axis as usize] > levels[axis] {
                    best_axis = axis as u32;
                }
            }
        } else {
            let mut axis = 0u32;
            let _ = self.axis_decoder.decode_lsb32(4, &mut axis);
            best_axis = axis;
        }
        best_axis
    }
}
