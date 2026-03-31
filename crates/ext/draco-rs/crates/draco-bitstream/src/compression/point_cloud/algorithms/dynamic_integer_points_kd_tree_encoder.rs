//! Dynamic integer point cloud kD-tree encoder.
//! Reference: `_ref/draco/src/draco/compression/point_cloud/algorithms/dynamic_integer_points_kd_tree_encoder.h`.
//!
//! Encodes integer point clouds using a kD-tree split strategy with multiple
//! compression levels (0..6).

use crate::compression::attributes::point_d_vector::PointDVector;
use crate::compression::bit_coders::direct_bit_encoder::DirectBitEncoder;
use crate::compression::bit_coders::folded_integer_bit_encoder::FoldedBit32Encoder;
use crate::compression::bit_coders::rans_bit_encoder::RAnsBitEncoder;
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

/// Access abstraction for mutable point clouds.
pub trait PointCloudAccess {
    fn len(&self) -> usize;
    fn dimension(&self) -> usize;
    fn get_component(&self, point_index: usize, axis: usize) -> u32;
    fn swap_points(&mut self, a: usize, b: usize);
}

/// View over a mutable slice of fixed-dimension VectorD points.
pub struct PointSlice<'a, const N: usize> {
    points: &'a mut [draco_core::core::vector_d::VectorD<u32, N>],
}

impl<'a, const N: usize> PointSlice<'a, N> {
    pub fn new(points: &'a mut [draco_core::core::vector_d::VectorD<u32, N>]) -> Self {
        Self { points }
    }
}

impl<'a, const N: usize> PointCloudAccess for PointSlice<'a, N> {
    fn len(&self) -> usize {
        self.points.len()
    }

    fn dimension(&self) -> usize {
        N
    }

    fn get_component(&self, point_index: usize, axis: usize) -> u32 {
        self.points[point_index][axis]
    }

    fn swap_points(&mut self, a: usize, b: usize) {
        self.points.swap(a, b);
    }
}

impl PointCloudAccess for PointDVector<u32> {
    fn len(&self) -> usize {
        self.size()
    }

    fn dimension(&self) -> usize {
        self.dimensionality()
    }

    fn get_component(&self, point_index: usize, axis: usize) -> u32 {
        self.point(point_index)[axis]
    }

    fn swap_points(&mut self, a: usize, b: usize) {
        self.swap_points(a, b);
    }
}

/// Compression policy selection (0..6).
pub(crate) trait DynamicEncoderPolicy {
    type NumbersEncoder: BitEncoderLike + Default;
    type AxisEncoder: BitEncoderLike + Default;
    type HalfEncoder: BitEncoderLike + Default;
    type RemainingBitsEncoder: BitEncoderLike + Default;
    const SELECT_AXIS: bool;
}

pub(crate) struct EncoderPolicy<const LEVEL: usize>;

impl DynamicEncoderPolicy for EncoderPolicy<0> {
    type NumbersEncoder = DirectBitEncoder;
    type AxisEncoder = DirectBitEncoder;
    type HalfEncoder = DirectBitEncoder;
    type RemainingBitsEncoder = DirectBitEncoder;
    const SELECT_AXIS: bool = false;
}

impl DynamicEncoderPolicy for EncoderPolicy<1> {
    type NumbersEncoder = DirectBitEncoder;
    type AxisEncoder = DirectBitEncoder;
    type HalfEncoder = DirectBitEncoder;
    type RemainingBitsEncoder = DirectBitEncoder;
    const SELECT_AXIS: bool = false;
}

impl DynamicEncoderPolicy for EncoderPolicy<2> {
    type NumbersEncoder = RAnsBitEncoder;
    type AxisEncoder = DirectBitEncoder;
    type HalfEncoder = DirectBitEncoder;
    type RemainingBitsEncoder = DirectBitEncoder;
    const SELECT_AXIS: bool = false;
}

impl DynamicEncoderPolicy for EncoderPolicy<3> {
    type NumbersEncoder = RAnsBitEncoder;
    type AxisEncoder = DirectBitEncoder;
    type HalfEncoder = DirectBitEncoder;
    type RemainingBitsEncoder = DirectBitEncoder;
    const SELECT_AXIS: bool = false;
}

impl DynamicEncoderPolicy for EncoderPolicy<4> {
    type NumbersEncoder = FoldedBit32Encoder<RAnsBitEncoder>;
    type AxisEncoder = DirectBitEncoder;
    type HalfEncoder = DirectBitEncoder;
    type RemainingBitsEncoder = DirectBitEncoder;
    const SELECT_AXIS: bool = false;
}

impl DynamicEncoderPolicy for EncoderPolicy<5> {
    type NumbersEncoder = FoldedBit32Encoder<RAnsBitEncoder>;
    type AxisEncoder = DirectBitEncoder;
    type HalfEncoder = DirectBitEncoder;
    type RemainingBitsEncoder = DirectBitEncoder;
    const SELECT_AXIS: bool = false;
}

impl DynamicEncoderPolicy for EncoderPolicy<6> {
    type NumbersEncoder = FoldedBit32Encoder<RAnsBitEncoder>;
    type AxisEncoder = DirectBitEncoder;
    type HalfEncoder = DirectBitEncoder;
    type RemainingBitsEncoder = DirectBitEncoder;
    const SELECT_AXIS: bool = true;
}

/// Dynamic integer kD-tree encoder (0..6).
pub(crate) struct DynamicIntegerPointsKdTreeEncoder<const LEVEL: usize>
where
    EncoderPolicy<LEVEL>: DynamicEncoderPolicy,
{
    bit_length: u32,
    num_points: u32,
    dimension: u32,
    numbers_encoder: <EncoderPolicy<LEVEL> as DynamicEncoderPolicy>::NumbersEncoder,
    remaining_bits_encoder: <EncoderPolicy<LEVEL> as DynamicEncoderPolicy>::RemainingBitsEncoder,
    axis_encoder: <EncoderPolicy<LEVEL> as DynamicEncoderPolicy>::AxisEncoder,
    half_encoder: <EncoderPolicy<LEVEL> as DynamicEncoderPolicy>::HalfEncoder,
    deviations: Vec<u32>,
    num_remaining_bits: Vec<u32>,
    axes: Vec<u32>,
    base_stack: Vec<Vec<u32>>,
    levels_stack: Vec<Vec<u32>>,
}

impl<const LEVEL: usize> DynamicIntegerPointsKdTreeEncoder<LEVEL>
where
    EncoderPolicy<LEVEL>: DynamicEncoderPolicy,
{
    pub fn new(dimension: u32) -> Self {
        let stack_len = 32usize.saturating_mul(dimension as usize).saturating_add(1);
        Self {
            bit_length: 0,
            num_points: 0,
            dimension,
            numbers_encoder: Default::default(),
            remaining_bits_encoder: Default::default(),
            axis_encoder: Default::default(),
            half_encoder: Default::default(),
            deviations: vec![0; dimension as usize],
            num_remaining_bits: vec![0; dimension as usize],
            axes: vec![0; dimension as usize],
            base_stack: vec![vec![0; dimension as usize]; stack_len],
            levels_stack: vec![vec![0; dimension as usize]; stack_len],
        }
    }

    pub fn encode_points<A: PointCloudAccess>(
        &mut self,
        points: &mut A,
        bit_length: u32,
        buffer: &mut EncoderBuffer,
    ) -> bool {
        self.bit_length = bit_length;
        self.num_points = points.len() as u32;
        draco_dcheck_eq!(true, self.dimension as usize == points.dimension());

        // Header: bit length + number of points.
        buffer.encode(self.bit_length);
        buffer.encode(self.num_points);
        if self.num_points == 0 {
            return true;
        }

        self.numbers_encoder.start_encoding();
        self.remaining_bits_encoder.start_encoding();
        self.axis_encoder.start_encoding();
        self.half_encoder.start_encoding();

        self.encode_internal(points, 0, points.len(), 0, 0);

        self.numbers_encoder.end_encoding(buffer);
        self.remaining_bits_encoder.end_encoding(buffer);
        self.axis_encoder.end_encoding(buffer);
        self.half_encoder.end_encoding(buffer);
        true
    }

    fn encode_internal<A: PointCloudAccess>(
        &mut self,
        points: &mut A,
        mut begin: usize,
        mut end: usize,
        mut last_axis: u32,
        mut stack_pos: usize,
    ) {
        #[derive(Clone, Copy)]
        struct Status {
            begin: usize,
            end: usize,
            last_axis: u32,
            stack_pos: usize,
        }

        self.base_stack[0].fill(0);
        self.levels_stack[0].fill(0);
        let mut stack = Vec::new();
        stack.push(Status {
            begin,
            end,
            last_axis,
            stack_pos,
        });

        while let Some(status) = stack.pop() {
            begin = status.begin;
            end = status.end;
            last_axis = status.last_axis;
            stack_pos = status.stack_pos;

            let old_base = self.base_stack[stack_pos].clone();
            let levels_snapshot = self.levels_stack[stack_pos].clone();

            let axis = self.get_and_encode_axis(
                points,
                begin,
                end,
                &old_base,
                &levels_snapshot,
                last_axis,
            );
            let level = levels_snapshot[axis as usize];
            let num_remaining_points = end - begin;

            // All axes fully subdivided.
            if (self.bit_length - level) == 0 {
                continue;
            }

            if num_remaining_points <= 2 {
                // Fast-path for small leafs: encode remaining bits directly.
                self.axes[0] = axis;
                for i in 1..self.dimension as usize {
                    self.axes[i] = increment_mod(self.axes[i - 1], self.dimension);
                }
                for i in 0..num_remaining_points {
                    let point_index = begin + i;
                    for j in 0..self.dimension as usize {
                        let axis_j = self.axes[j] as usize;
                        let remaining = self.bit_length - levels_snapshot[axis_j];
                        if remaining > 0 {
                            let value = points.get_component(point_index, axis_j);
                            self.remaining_bits_encoder
                                .encode_lsb32(remaining as i32, value);
                        }
                    }
                }
                continue;
            }

            let num_remaining_bits = self.bit_length - level;
            let modifier = 1u32 << (num_remaining_bits - 1);
            self.base_stack[stack_pos] = old_base.clone();
            self.base_stack[stack_pos + 1] = old_base.clone();
            self.base_stack[stack_pos + 1][axis as usize] += modifier;
            let split_value = self.base_stack[stack_pos + 1][axis as usize];

            let split = partition_range(points, begin, end, axis as usize, split_value);

            // Encode number of points in the two halves.
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

            self.levels_stack[stack_pos] = levels_snapshot.clone();
            self.levels_stack[stack_pos][axis as usize] += 1;
            self.levels_stack[stack_pos + 1] = self.levels_stack[stack_pos].clone();
            if split != begin {
                stack.push(Status {
                    begin,
                    end: split,
                    last_axis: axis,
                    stack_pos,
                });
            }
            if split != end {
                stack.push(Status {
                    begin: split,
                    end,
                    last_axis: axis,
                    stack_pos: stack_pos + 1,
                });
            }
        }
    }

    fn get_and_encode_axis<A: PointCloudAccess>(
        &mut self,
        points: &A,
        begin: usize,
        end: usize,
        old_base: &[u32],
        levels: &[u32],
        last_axis: u32,
    ) -> u32 {
        if !<EncoderPolicy<LEVEL> as DynamicEncoderPolicy>::SELECT_AXIS {
            return increment_mod(last_axis, self.dimension);
        }

        draco_dcheck_eq!(true, end != begin);
        let mut best_axis = 0u32;
        if end - begin < 64 {
            for axis in 1..self.dimension as usize {
                if levels[best_axis as usize] > levels[axis] {
                    best_axis = axis as u32;
                }
            }
        } else {
            let size = (end - begin) as u32;
            for i in 0..self.dimension as usize {
                self.deviations[i] = 0;
                self.num_remaining_bits[i] = self.bit_length - levels[i];
                if self.num_remaining_bits[i] > 0 {
                    let split = old_base[i] + (1u32 << (self.num_remaining_bits[i] - 1));
                    for point_index in begin..end {
                        if points.get_component(point_index, i) < split {
                            self.deviations[i] += 1;
                        }
                    }
                    let dev = self.deviations[i];
                    self.deviations[i] = std::cmp::max(size - dev, dev);
                }
            }

            let mut max_value = 0u32;
            best_axis = 0;
            for i in 0..self.dimension as usize {
                if self.num_remaining_bits[i] > 0 && max_value < self.deviations[i] {
                    max_value = self.deviations[i];
                    best_axis = i as u32;
                }
            }
            // Axis is encoded only when large enough to benefit from selection.
            self.axis_encoder.encode_lsb32(4, best_axis);
        }

        best_axis
    }
}

fn partition_range<A: PointCloudAccess>(
    points: &mut A,
    mut begin: usize,
    mut end: usize,
    axis: usize,
    split_value: u32,
) -> usize {
    while begin < end {
        if points.get_component(begin, axis) < split_value {
            begin += 1;
        } else {
            end -= 1;
            points.swap_points(begin, end);
        }
    }
    begin
}
