//! Device backends — CPU evaluators, vertex buffers, patch tables.

pub mod types;
pub mod buffer_descriptor;
pub mod mesh;
pub mod patch_basis;
pub mod cpu_evaluator;
pub mod cpu_kernel;
pub mod cpu_vertex_buffer;
pub mod cpu_patch_table;

#[cfg(feature = "parallel")]
pub mod rayon_evaluator;

pub use types::*;
pub use buffer_descriptor::BufferDescriptor;
pub use mesh::{MeshInterface, CpuMesh, MeshBitset, mesh_bits};
pub use cpu_evaluator::CpuEvaluator;
pub use cpu_vertex_buffer::CpuVertexBuffer;
pub use cpu_patch_table::CpuPatchTable;
