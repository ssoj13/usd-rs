//! Shared constants for constrained multi-parallelogram prediction.
//! Reference: `_ref/draco/src/draco/compression/attributes/prediction_schemes/mesh_prediction_scheme_constrained_multi_parallelogram_shared.h`.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    OptimalMultiParallelogram = 0,
}

pub const MAX_NUM_PARALLELOGRAMS: usize = 4;
