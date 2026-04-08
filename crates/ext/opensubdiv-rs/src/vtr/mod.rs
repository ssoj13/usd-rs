//! Vectorized Topology Representation — internal SOA mesh topology storage.

pub mod array;
pub mod component_interfaces;
pub mod fvar_level;
pub mod fvar_refinement;
pub mod level;
pub mod quad_refinement;
pub mod refinement;
pub mod sparse_selector;
pub mod stack_buffer;
pub mod tri_refinement;
pub mod types;

pub use array::*;
pub use level::{ETag, FTag, Level, TopologyError, VSpan, VTag, ValidationCallback};
pub use quad_refinement::QuadRefinement;
pub use refinement::{ChildTag, Refinement, RefinementOptions, SparseTag};
pub use sparse_selector::SparseSelector;
pub use tri_refinement::TriRefinement;
pub use types::*;
