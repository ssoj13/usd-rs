//! Base Face Representation — direct limit surface evaluation per face.

pub mod parameterization;
pub mod tessellation;
pub mod surface;
pub mod surface_data;
pub mod surface_factory;
pub mod surface_factory_mesh_adapter;
pub mod surface_factory_cache;
pub mod refiner_surface_factory;
pub mod vertex_descriptor;
pub mod face_topology;
pub mod face_surface;
pub mod face_vertex;
pub mod face_vertex_subset;
pub mod patch_tree;
pub mod patch_tree_builder;
pub mod regular_patch_builder;
pub mod irregular_patch_builder;
pub mod irregular_patch_type;
pub(crate) mod hash;
pub mod limits;
pub mod point_operations;
pub mod vertex_tag;

pub use parameterization::{Parameterization, ParameterizationType};
pub use tessellation::Tessellation;
pub use surface::Surface;
pub use surface_factory::SurfaceFactory;
pub use surface_factory_mesh_adapter::SurfaceFactoryMeshAdapter;
pub use surface_factory_cache::SurfaceFactoryCache;
pub use refiner_surface_factory::RefinerSurfaceFactory;
pub use vertex_descriptor::VertexDescriptor;
