//! Device backends — CPU evaluators, vertex buffers, patch tables.

pub mod buffer_descriptor;
pub mod cpu_evaluator;
pub mod cpu_kernel;
pub mod cpu_patch_table;
pub mod cpu_vertex_buffer;
pub mod mesh;
pub mod patch_basis;
pub mod types;

#[cfg(feature = "parallel")]
pub mod rayon_evaluator;

pub use buffer_descriptor::BufferDescriptor;
pub use cpu_evaluator::CpuEvaluator;
pub use cpu_patch_table::CpuPatchTable;
pub use cpu_vertex_buffer::CpuVertexBuffer;
pub use mesh::{CpuMesh, MeshBitset, MeshInterface, mesh_bits};
pub use types::*;
