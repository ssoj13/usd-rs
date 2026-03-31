//! Vectorized Topology Representation — internal SOA mesh topology storage.

pub mod types;
pub mod array;
pub mod stack_buffer;
pub mod level;
pub mod refinement;
pub mod quad_refinement;
pub mod tri_refinement;
pub mod fvar_level;
pub mod fvar_refinement;
pub mod sparse_selector;
pub mod component_interfaces;

pub use types::*;
pub use array::*;
pub use level::{Level, VTag, ETag, FTag, VSpan, TopologyError, ValidationCallback};
pub use refinement::{Refinement, RefinementOptions, SparseTag, ChildTag};
pub use quad_refinement::QuadRefinement;
pub use tri_refinement::TriRefinement;
pub use sparse_selector::SparseSelector;
