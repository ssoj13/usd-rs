//! Integer point cloud kD-tree encoder (legacy).
//! Reference: `_ref/draco/src/draco/compression/point_cloud/algorithms/integer_points_kd_tree_encoder.h`.
//!
//! Provides an alternative kD-tree encoding with multiple queueing strategies
//! and compression levels (0..10). Kept for parity with the reference.

#![allow(dead_code)]

use std::cmp::Ordering;

use crate::compression::bit_coders::adaptive_rans_bit_encoder::AdaptiveRAnsBitEncoder;
use crate::compression::bit_coders::direct_bit_encoder::DirectBitEncoder;
use crate::compression::bit_coders::folded_integer_bit_encoder::FoldedBit32Encoder;
use crate::compression::bit_coders::rans_bit_encoder::RAnsBitEncoder;
use crate::compression::point_cloud::algorithms::point_cloud_types::PointTraits;
use crate::compression::point_cloud::algorithms::queuing_policy::{PriorityQueue, Queue, Stack};
use draco_core::core::bit_utils::most_significant_bit;
use draco_core::core::encoder_buffer::EncoderBuffer;
use draco_core::core::math_utils::increment_mod;
use draco_core::draco_dcheck_eq;

/// Minimal bit-encoder API used by kd-tree algorithms.
pub(crate) trait BitEncoderLike {
    fn start_encoding(&mut self);
    fn encode_bit(&mut self, bit: bool);
    fn encode_lsb32(&mut self, nbits: i32, value: u32);
    fn end_encoding(&mut self, target_buffer: &mut EncoderBuffer);
}

impl BitEncoderLike for DirectBitEncoder {
    fn start_encoding(&mut self) {
        self.start_encoding();
    }

    fn encode_bit(&mut self, bit: bool) {
        self.encode_bit(bit);
    }

    fn encode_lsb32(&mut self, nbits: i32, value: u32) {
        self.encode_least_significant_bits32(nbits, value);
    }

    fn end_encoding(&mut self, target_buffer: &mut EncoderBuffer) {
        self.end_encoding(target_buffer);
    }
}

impl BitEncoderLike for RAnsBitEncoder {
    fn start_encoding(&mut self) {
        self.start_encoding();
    }

    fn encode_bit(&mut self, bit: bool) {
        self.encode_bit(bit);
    }

    fn encode_lsb32(&mut self, nbits: i32, value: u32) {
        self.encode_least_significant_bits32(nbits, value);
    }

    fn end_encoding(&mut self, target_buffer: &mut EncoderBuffer) {
        self.end_encoding(target_buffer);
    }
}

impl<T: crate::compression::bit_coders::BitEncoder + Default> BitEncoderLike
    for FoldedBit32Encoder<T>
{
    fn start_encoding(&mut self) {
        self.start_encoding();
    }

    fn encode_bit(&mut self, bit: bool) {
        self.encode_bit(bit);
    }

    fn encode_lsb32(&mut self, nbits: i32, value: u32) {
        self.encode_least_significant_bits32(nbits, value);
    }

    fn end_encoding(&mut self, target_buffer: &mut EncoderBuffer) {
        self.end_encoding(target_buffer);
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
pub(crate) trait IntegerEncoderPolicy<PointT: Copy> {
    type NumbersEncoder: BitEncoderLike + Default;
    type AxisEncoder: BitEncoderLike + Default;
    type HalfEncoder: BitEncoderLike + Default;
    type RemainingBitsEncoder: BitEncoderLike + Default;
    const SELECT_AXIS: bool;
    type Queue: QueueLike<EncodingStatus<PointT>> + Default;
}

pub(crate) struct EncoderPolicy<const LEVEL: usize>;

impl<PointT: Copy> IntegerEncoderPolicy<PointT> for EncoderPolicy<0> {
    type NumbersEncoder = DirectBitEncoder;
    type AxisEncoder = DirectBitEncoder;
    type HalfEncoder = DirectBitEncoder;
    type RemainingBitsEncoder = DirectBitEncoder;
    const SELECT_AXIS: bool = false;
    type Queue = Stack<EncodingStatus<PointT>>;
}

impl<PointT: Copy> IntegerEncoderPolicy<PointT> for EncoderPolicy<1> {
    type NumbersEncoder = DirectBitEncoder;
    type AxisEncoder = DirectBitEncoder;
    type HalfEncoder = DirectBitEncoder;
    type RemainingBitsEncoder = DirectBitEncoder;
    const SELECT_AXIS: bool = false;
    type Queue = Stack<EncodingStatus<PointT>>;
}

impl<PointT: Copy> IntegerEncoderPolicy<PointT> for EncoderPolicy<2> {
    type NumbersEncoder = RAnsBitEncoder;
    type AxisEncoder = DirectBitEncoder;
    type HalfEncoder = DirectBitEncoder;
    type RemainingBitsEncoder = DirectBitEncoder;
    const SELECT_AXIS: bool = false;
    type Queue = Stack<EncodingStatus<PointT>>;
}

impl<PointT: Copy> IntegerEncoderPolicy<PointT> for EncoderPolicy<3> {
    type NumbersEncoder = RAnsBitEncoder;
    type AxisEncoder = DirectBitEncoder;
    type HalfEncoder = DirectBitEncoder;
    type RemainingBitsEncoder = DirectBitEncoder;
    const SELECT_AXIS: bool = false;
    type Queue = Stack<EncodingStatus<PointT>>;
}

impl<PointT: Copy> IntegerEncoderPolicy<PointT> for EncoderPolicy<4> {
    type NumbersEncoder = FoldedBit32Encoder<RAnsBitEncoder>;
    type AxisEncoder = DirectBitEncoder;
    type HalfEncoder = DirectBitEncoder;
    type RemainingBitsEncoder = DirectBitEncoder;
    const SELECT_AXIS: bool = false;
    type Queue = Stack<EncodingStatus<PointT>>;
}

impl<PointT: Copy> IntegerEncoderPolicy<PointT> for EncoderPolicy<5> {
    type NumbersEncoder = FoldedBit32Encoder<RAnsBitEncoder>;
    type AxisEncoder = DirectBitEncoder;
    type HalfEncoder = DirectBitEncoder;
    type RemainingBitsEncoder = DirectBitEncoder;
    const SELECT_AXIS: bool = false;
    type Queue = Stack<EncodingStatus<PointT>>;
}

impl<PointT: Copy> IntegerEncoderPolicy<PointT> for EncoderPolicy<6> {
    type NumbersEncoder = FoldedBit32Encoder<RAnsBitEncoder>;
    type AxisEncoder = DirectBitEncoder;
    type HalfEncoder = DirectBitEncoder;
    type RemainingBitsEncoder = DirectBitEncoder;
    const SELECT_AXIS: bool = true;
    type Queue = Stack<EncodingStatus<PointT>>;
}

impl<PointT: Copy> IntegerEncoderPolicy<PointT> for EncoderPolicy<7> {
    type NumbersEncoder = FoldedBit32Encoder<RAnsBitEncoder>;
    type AxisEncoder = DirectBitEncoder;
    type HalfEncoder = DirectBitEncoder;
    type RemainingBitsEncoder = DirectBitEncoder;
    const SELECT_AXIS: bool = true;
    type Queue = Stack<EncodingStatus<PointT>>;
}

impl<PointT: Copy> IntegerEncoderPolicy<PointT> for EncoderPolicy<8> {
    type NumbersEncoder = FoldedBit32Encoder<AdaptiveRAnsBitEncoder>;
    type AxisEncoder = DirectBitEncoder;
    type HalfEncoder = DirectBitEncoder;
    type RemainingBitsEncoder = DirectBitEncoder;
    const SELECT_AXIS: bool = true;
    type Queue = Queue<EncodingStatus<PointT>>;
}

impl<PointT: Copy> IntegerEncoderPolicy<PointT> for EncoderPolicy<9> {
    type NumbersEncoder = FoldedBit32Encoder<AdaptiveRAnsBitEncoder>;
    type AxisEncoder = DirectBitEncoder;
    type HalfEncoder = DirectBitEncoder;
    type RemainingBitsEncoder = DirectBitEncoder;
    const SELECT_AXIS: bool = true;
    type Queue = Queue<EncodingStatus<PointT>>;
}

impl<PointT: Copy> IntegerEncoderPolicy<PointT> for EncoderPolicy<10> {
    type NumbersEncoder = FoldedBit32Encoder<AdaptiveRAnsBitEncoder>;
    type AxisEncoder = DirectBitEncoder;
    type HalfEncoder = DirectBitEncoder;
    type RemainingBitsEncoder = DirectBitEncoder;
    const SELECT_AXIS: bool = true;
    type Queue = PriorityQueue<EncodingStatus<PointT>>;
}

#[derive(Clone)]
pub(crate) struct EncodingStatus<PointT: Copy> {
    begin: usize,
    end: usize,
    old_base: PointT,
    levels: Vec<u32>,
    last_axis: u32,
    num_remaining_points: usize,
}

impl<PointT: Copy> EncodingStatus<PointT> {
    fn new(begin: usize, end: usize, old_base: PointT, levels: Vec<u32>, last_axis: u32) -> Self {
        Self {
            begin,
            end,
            old_base,
            levels,
            last_axis,
            num_remaining_points: end - begin,
        }
    }
}

impl<PointT: Copy> PartialEq for EncodingStatus<PointT> {
    fn eq(&self, other: &Self) -> bool {
        self.num_remaining_points == other.num_remaining_points
    }
}

impl<PointT: Copy> Eq for EncodingStatus<PointT> {}

impl<PointT: Copy> PartialOrd for EncodingStatus<PointT> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<PointT: Copy> Ord for EncodingStatus<PointT> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.num_remaining_points.cmp(&other.num_remaining_points)
    }
}

/// Integer kD-tree encoder.
pub(crate) struct IntegerPointsKdTreeEncoder<PointT: Copy + Default, const LEVEL: usize>
where
    EncoderPolicy<LEVEL>: IntegerEncoderPolicy<PointT>,
{
    bit_length: u32,
    num_points: u32,
    numbers_encoder: <EncoderPolicy<LEVEL> as IntegerEncoderPolicy<PointT>>::NumbersEncoder,
    remaining_bits_encoder:
        <EncoderPolicy<LEVEL> as IntegerEncoderPolicy<PointT>>::RemainingBitsEncoder,
    axis_encoder: <EncoderPolicy<LEVEL> as IntegerEncoderPolicy<PointT>>::AxisEncoder,
    half_encoder: <EncoderPolicy<LEVEL> as IntegerEncoderPolicy<PointT>>::HalfEncoder,
    _marker: std::marker::PhantomData<PointT>,
}

impl<PointT: Copy + Default, const LEVEL: usize> IntegerPointsKdTreeEncoder<PointT, LEVEL>
where
    EncoderPolicy<LEVEL>: IntegerEncoderPolicy<PointT>,
    PointT: std::ops::Index<usize, Output = u32> + std::ops::IndexMut<usize>,
    PointT: PointTraits<Point = PointT>,
{
    pub fn new() -> Self {
        Self {
            bit_length: 0,
            num_points: 0,
            numbers_encoder: Default::default(),
            remaining_bits_encoder: Default::default(),
            axis_encoder: Default::default(),
            half_encoder: Default::default(),
            _marker: std::marker::PhantomData,
        }
    }

    pub fn encode_points(
        &mut self,
        points: &mut [PointT],
        bit_length: u32,
        buffer: &mut EncoderBuffer,
    ) -> bool {
        self.bit_length = bit_length;
        self.num_points = points.len() as u32;

        buffer.encode(self.bit_length);
        buffer.encode(self.num_points);
        if self.num_points == 0 {
            return true;
        }

        self.numbers_encoder.start_encoding();
        self.remaining_bits_encoder.start_encoding();
        self.axis_encoder.start_encoding();
        self.half_encoder.start_encoding();

        let levels = <PointT as PointTraits>::zero_levels();
        self.encode_internal(
            points,
            0,
            points.len(),
            <PointT as PointTraits>::origin(),
            levels,
            0,
        );

        self.numbers_encoder.end_encoding(buffer);
        self.remaining_bits_encoder.end_encoding(buffer);
        self.axis_encoder.end_encoding(buffer);
        self.half_encoder.end_encoding(buffer);
        true
    }

    fn encode_internal(
        &mut self,
        points: &mut [PointT],
        begin: usize,
        end: usize,
        old_base: PointT,
        levels: Vec<u32>,
        last_axis: u32,
    ) {
        let mut status_q: <EncoderPolicy<LEVEL> as IntegerEncoderPolicy<PointT>>::Queue =
            Default::default();
        status_q.push(EncodingStatus::new(begin, end, old_base, levels, last_axis));

        while !status_q.empty() {
            let status = status_q.front().clone();
            status_q.pop();

            let begin = status.begin;
            let end = status.end;
            let old_base = status.old_base;
            let mut levels = status.levels;
            let last_axis = status.last_axis;
            let dimension = <PointT as PointTraits>::DIMENSION;

            let axis = self.get_axis(points, begin, end, &old_base, &levels, last_axis);
            let level = levels[axis as usize];
            let num_remaining_points = end - begin;

            if (self.bit_length - level) == 0 {
                continue;
            }

            if num_remaining_points <= 2 {
                let mut axes = vec![0u32; dimension];
                axes[0] = axis;
                for i in 1..dimension {
                    axes[i] = increment_mod(axes[i - 1], dimension as u32);
                }

                let mut num_remaining_bits = vec![0u32; dimension];
                for i in 0..dimension {
                    num_remaining_bits[i] = self.bit_length - levels[axes[i] as usize];
                }

                for i in 0..num_remaining_points {
                    let point_index = begin + i;
                    for j in 0..dimension {
                        if num_remaining_bits[j] > 0 {
                            self.remaining_bits_encoder.encode_lsb32(
                                num_remaining_bits[j] as i32,
                                points[point_index][axes[j] as usize],
                            );
                        }
                    }
                }
                continue;
            }

            let num_remaining_bits = self.bit_length - level;
            let modifier = 1u32 << (num_remaining_bits - 1);
            let mut new_base = old_base;
            new_base[axis as usize] += modifier;

            let split = partition_range(points, begin, end, axis as usize, new_base[axis as usize]);

            let required_bits = most_significant_bit(num_remaining_points as u32);
            let first_half = split - begin;
            let second_half = end - split;
            let left = first_half < second_half;

            if first_half != second_half {
                self.half_encoder.encode_bit(left);
            }

            if left {
                self.numbers_encoder.encode_lsb32(
                    required_bits,
                    (num_remaining_points / 2 - first_half) as u32,
                );
            } else {
                self.numbers_encoder.encode_lsb32(
                    required_bits,
                    (num_remaining_points / 2 - second_half) as u32,
                );
            }

            levels[axis as usize] += 1;
            if split != begin {
                status_q.push(EncodingStatus::new(
                    begin,
                    split,
                    old_base,
                    levels.clone(),
                    axis,
                ));
            }
            if split != end {
                status_q.push(EncodingStatus::new(split, end, new_base, levels, axis));
            }
        }
    }

    fn get_axis(
        &mut self,
        points: &[PointT],
        begin: usize,
        end: usize,
        old_base: &PointT,
        levels: &[u32],
        last_axis: u32,
    ) -> u32 {
        if !<EncoderPolicy<LEVEL> as IntegerEncoderPolicy<PointT>>::SELECT_AXIS {
            return increment_mod(last_axis, <PointT as PointTraits>::DIMENSION as u32);
        }

        draco_dcheck_eq!(true, end != begin);
        let mut best_axis = 0u32;
        if end - begin < 64 {
            for axis in 1..levels.len() {
                if levels[best_axis as usize] > levels[axis] {
                    best_axis = axis as u32;
                }
            }
        } else {
            let size = (end - begin) as u32;
            let mut num_remaining_bits = vec![0u32; levels.len()];
            for i in 0..levels.len() {
                num_remaining_bits[i] = self.bit_length - levels[i];
            }
            let mut split = *old_base;
            for i in 0..levels.len() {
                if num_remaining_bits[i] > 0 {
                    split[i] += 1u32 << (num_remaining_bits[i] - 1);
                }
            }

            let mut deviations = vec![0u32; levels.len()];
            for p in &points[begin..end] {
                for i in 0..levels.len() {
                    deviations[i] += (p[i] < split[i]) as u32;
                }
            }
            for i in 0..levels.len() {
                deviations[i] = std::cmp::max(size - deviations[i], deviations[i]);
            }

            let mut max_value = 0u32;
            best_axis = 0;
            for i in 0..levels.len() {
                if num_remaining_bits[i] > 0 && max_value < deviations[i] {
                    max_value = deviations[i];
                    best_axis = i as u32;
                }
            }
            self.axis_encoder.encode_lsb32(4, best_axis);
        }

        best_axis
    }
}

fn partition_range<PointT>(
    points: &mut [PointT],
    mut begin: usize,
    mut end: usize,
    axis: usize,
    split_value: u32,
) -> usize
where
    PointT: std::ops::Index<usize, Output = u32>,
{
    while begin < end {
        if points[begin][axis] < split_value {
            begin += 1;
        } else {
            end -= 1;
            points.swap(begin, end);
        }
    }
    begin
}
