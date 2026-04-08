//! RefinerSurfaceFactory — SurfaceFactory backed by a Far::TopologyRefiner.
//!
//! Ported from OpenSubdiv bfr/refinerSurfaceFactory.h/.cpp.
//!
//! This module provides `RefinerSurfaceFactoryBase` — a concrete struct that
//! implements `SurfaceFactoryMeshAdapter` using a `TopologyRefiner` as the
//! connected mesh representation.
//!
//! The C++ template `RefinerSurfaceFactory<CACHE_TYPE>` is represented as the
//! struct `RefinerSurfaceFactory` which owns an internal `SurfaceFactoryCache`.

use super::surface_factory::{SurfaceFactory, SurfaceFactoryOptions};
use super::surface_factory_cache::SurfaceFactoryCache;
use super::surface_factory_mesh_adapter::{FVarId, Index, SurfaceFactoryMeshAdapter};
use super::vertex_descriptor::VertexDescriptor;

/// Trait representing the minimal interface we require from a `TopologyRefiner`
/// in this context.
///
/// This allows both the real Far::TopologyRefiner and test doubles to be used.
pub trait TopologyRefinerMesh {
    fn scheme_type(&self) -> crate::sdc::types::SchemeType;
    fn scheme_options(&self) -> crate::sdc::options::Options;
    fn num_faces(&self) -> i32;
    fn num_fvar_channels(&self) -> i32;
    fn has_holes(&self) -> bool;

    // Face-level queries:
    fn is_face_hole(&self, face: Index) -> bool;
    fn face_size(&self, face: Index) -> i32;
    fn face_vertex_indices(&self, face: Index, out: &mut [Index]) -> i32;
    fn face_fvar_value_indices(&self, face: Index, channel: i32, out: &mut [Index]) -> i32;

    // Corner-level queries:
    /// Populate `vd` with the vertex topology for `corner` of `face`.
    /// Returns the ring position (face-in-ring) of `face`, or -1 on error.
    fn populate_corner_descriptor(
        &self,
        face: Index,
        corner: i32,
        vd: &mut VertexDescriptor,
    ) -> i32;

    fn corner_incident_vtx_indices(&self, face: Index, corner: i32, out: &mut [Index]) -> i32;
    fn corner_incident_fvar_indices(
        &self,
        face: Index,
        corner: i32,
        channel: i32,
        out: &mut [Index],
    ) -> i32;

    // Optional regular fast-path:
    fn neighborhood_vtx_indices_if_regular(&self, face: Index, out: Option<&mut [Index]>) -> bool {
        let _ = (face, out);
        false
    }
    fn neighborhood_fvar_indices_if_regular(
        &self,
        face: Index,
        channel: i32,
        out: Option<&mut [Index]>,
    ) -> bool {
        let _ = (face, channel, out);
        false
    }
}

// ---------------------------------------------------------------------------
//  RefinerSurfaceFactoryBase
// ---------------------------------------------------------------------------

/// Concrete `SurfaceFactoryMeshAdapter` implementation backed by a
/// `TopologyRefinerMesh`.
///
/// Mirrors `Bfr::RefinerSurfaceFactoryBase`.
pub struct RefinerSurfaceFactoryBase<'mesh, M: TopologyRefinerMesh> {
    /// Embedded base-factory state.
    pub factory: SurfaceFactory,
    mesh: &'mesh M,
    num_faces: i32,
    num_fvar_channels: i32,
}

impl<'mesh, M: TopologyRefinerMesh> RefinerSurfaceFactoryBase<'mesh, M> {
    pub fn new(mesh: &'mesh M, options: SurfaceFactoryOptions) -> Self {
        let factory = SurfaceFactory::new(mesh.scheme_type(), mesh.scheme_options(), options);
        let num_faces = mesh.num_faces();
        let num_fvar_channels = mesh.num_fvar_channels();
        RefinerSurfaceFactoryBase {
            factory,
            mesh,
            num_faces,
            num_fvar_channels,
        }
    }

    pub fn get_mesh(&self) -> &M {
        self.mesh
    }
    pub fn get_num_faces(&self) -> i32 {
        self.num_faces
    }
    pub fn get_num_fvar_channels(&self) -> i32 {
        self.num_fvar_channels
    }

    fn fvar_id_to_channel(&self, fvar_id: FVarId) -> i32 {
        if fvar_id >= 0 && (fvar_id as i32) < self.num_fvar_channels {
            fvar_id as i32
        } else {
            -1
        }
    }
}

impl<'mesh, M: TopologyRefinerMesh> SurfaceFactoryMeshAdapter
    for RefinerSurfaceFactoryBase<'mesh, M>
{
    fn is_face_hole(&self, face_index: Index) -> bool {
        self.mesh.has_holes() && self.mesh.is_face_hole(face_index)
    }

    fn get_face_size(&self, face_index: Index) -> i32 {
        self.mesh.face_size(face_index)
    }

    fn get_face_vertex_indices(&self, face_index: Index, out: &mut [Index]) -> i32 {
        self.mesh.face_vertex_indices(face_index, out)
    }

    fn get_face_fvar_value_indices(
        &self,
        face_index: Index,
        fvar_id: FVarId,
        out: &mut [Index],
    ) -> i32 {
        let ch = self.fvar_id_to_channel(fvar_id);
        if ch < 0 {
            return 0;
        }
        self.mesh.face_fvar_value_indices(face_index, ch, out)
    }

    fn populate_face_vertex_descriptor(
        &self,
        face_index: Index,
        face_vertex: i32,
        descriptor: &mut VertexDescriptor,
    ) -> i32 {
        self.mesh
            .populate_corner_descriptor(face_index, face_vertex, descriptor)
    }

    fn get_face_vertex_incident_face_vertex_indices(
        &self,
        face_index: Index,
        face_vertex: i32,
        out: &mut [Index],
    ) -> i32 {
        self.mesh
            .corner_incident_vtx_indices(face_index, face_vertex, out)
    }

    fn get_face_vertex_incident_face_fvar_value_indices(
        &self,
        face_index: Index,
        face_vertex: i32,
        fvar_id: FVarId,
        out: &mut [Index],
    ) -> i32 {
        let ch = self.fvar_id_to_channel(fvar_id);
        if ch < 0 {
            return 0;
        }
        self.mesh
            .corner_incident_fvar_indices(face_index, face_vertex, ch, out)
    }

    fn get_face_neighborhood_vertex_indices_if_regular(
        &self,
        face_index: Index,
        vertex_indices: Option<&mut [Index]>,
    ) -> bool {
        self.mesh
            .neighborhood_vtx_indices_if_regular(face_index, vertex_indices)
    }

    fn get_face_neighborhood_fvar_value_indices_if_regular(
        &self,
        face_index: Index,
        fvar_id: FVarId,
        fvar_indices: Option<&mut [Index]>,
    ) -> bool {
        let ch = self.fvar_id_to_channel(fvar_id);
        if ch < 0 {
            return false;
        }
        self.mesh
            .neighborhood_fvar_indices_if_regular(face_index, ch, fvar_indices)
    }
}

// ---------------------------------------------------------------------------
//  RefinerSurfaceFactory — owns an internal cache
// ---------------------------------------------------------------------------

/// Concrete factory owning an internal `SurfaceFactoryCache`.
///
/// Mirrors `Bfr::RefinerSurfaceFactory<SurfaceFactoryCache>`.
pub struct RefinerSurfaceFactory<'mesh, M: TopologyRefinerMesh> {
    pub base: RefinerSurfaceFactoryBase<'mesh, M>,
    _cache: std::sync::Arc<std::sync::RwLock<SurfaceFactoryCache>>,
}

impl<'mesh, M: TopologyRefinerMesh> RefinerSurfaceFactory<'mesh, M> {
    pub fn new(mesh: &'mesh M, options: SurfaceFactoryOptions) -> Self {
        let cache = std::sync::Arc::new(std::sync::RwLock::new(SurfaceFactoryCache::new()));
        let mut base = RefinerSurfaceFactoryBase::new(mesh, options);
        base.factory.set_internal_cache(cache.clone()
            as std::sync::Arc<dyn super::surface_factory_cache::SurfaceFactoryCacheTrait>);
        RefinerSurfaceFactory {
            base,
            _cache: cache,
        }
    }

    /// Convenience: initialize a vertex surface.
    pub fn init_vertex_surface<R: super::surface::SurfaceReal>(
        &self,
        face_index: Index,
        surface: &mut super::surface::Surface<R>,
    ) -> bool {
        self.base
            .factory
            .init_vertex_surface(&self.base, face_index, surface)
    }

    /// Convenience: initialize a varying surface.
    pub fn init_varying_surface<R: super::surface::SurfaceReal>(
        &self,
        face_index: Index,
        surface: &mut super::surface::Surface<R>,
    ) -> bool {
        self.base
            .factory
            .init_varying_surface(&self.base, face_index, surface)
    }

    /// Convenience: initialize a face-varying surface.
    pub fn init_fvar_surface<R: super::surface::SurfaceReal>(
        &self,
        face_index: Index,
        fvar_id: FVarId,
        surface: &mut super::surface::Surface<R>,
    ) -> bool {
        self.base
            .factory
            .init_fvar_surface(&self.base, face_index, fvar_id, surface)
    }

    /// Returns true if the face has a limit surface.
    pub fn face_has_limit_surface(&self, face_index: Index) -> bool {
        self.base
            .factory
            .face_has_limit_surface(&self.base, face_index)
    }

    /// Returns the parameterization for the given face.
    pub fn get_face_parameterization(
        &self,
        face_index: Index,
    ) -> super::parameterization::Parameterization {
        self.base
            .factory
            .get_face_parameterization(&self.base, face_index)
    }

    /// Return the mesh this factory was constructed for.
    pub fn get_mesh(&self) -> &M {
        self.base.get_mesh()
    }

    /// Number of faces at the base level.
    pub fn get_num_faces(&self) -> i32 {
        self.base.get_num_faces()
    }

    /// Number of face-varying channels.
    pub fn get_num_fvar_channels(&self) -> i32 {
        self.base.get_num_fvar_channels()
    }

    /// Scheme type of the factory.
    pub fn get_scheme_type(&self) -> crate::sdc::types::SchemeType {
        self.base.factory.get_scheme_type()
    }

    /// Scheme options of the factory.
    pub fn get_scheme_options(&self) -> crate::sdc::options::Options {
        self.base.factory.get_scheme_options()
    }
}

// ---------------------------------------------------------------------------
//  Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdc::options::Options as SdcOptions;
    use crate::sdc::types::SchemeType;

    /// Minimal mesh double: a single quad face, interior, regular.
    struct QuadMesh;

    impl TopologyRefinerMesh for QuadMesh {
        fn scheme_type(&self) -> SchemeType {
            SchemeType::Catmark
        }
        fn scheme_options(&self) -> SdcOptions {
            SdcOptions::default()
        }
        fn num_faces(&self) -> i32 {
            1
        }
        fn num_fvar_channels(&self) -> i32 {
            0
        }
        fn has_holes(&self) -> bool {
            false
        }
        fn is_face_hole(&self, _: Index) -> bool {
            false
        }
        fn face_size(&self, _: Index) -> i32 {
            4
        }
        fn face_vertex_indices(&self, _: Index, out: &mut [Index]) -> i32 {
            out[..4].copy_from_slice(&[0, 1, 2, 3]);
            4
        }
        fn face_fvar_value_indices(&self, _: Index, _: i32, _: &mut [Index]) -> i32 {
            0
        }
        fn populate_corner_descriptor(&self, _: Index, _: i32, vd: &mut VertexDescriptor) -> i32 {
            // Regular interior valence-4 vertex.
            vd.initialize(4);
            vd.set_manifold(true);
            vd.set_boundary(false);
            0
        }
        fn corner_incident_vtx_indices(&self, _: Index, _: i32, out: &mut [Index]) -> i32 {
            let n = out.len();
            for (i, v) in out.iter_mut().enumerate() {
                *v = i as i32;
            }
            n as i32
        }
        fn corner_incident_fvar_indices(&self, _: Index, _: i32, _: i32, _: &mut [Index]) -> i32 {
            0
        }
    }

    #[test]
    fn refiner_factory_creates() {
        let mesh = QuadMesh;
        let factory = RefinerSurfaceFactory::new(&mesh, SurfaceFactoryOptions::default());
        assert_eq!(factory.get_num_faces(), 1);
    }

    #[test]
    fn refiner_factory_scheme_catmark() {
        let mesh = QuadMesh;
        let factory = RefinerSurfaceFactory::new(&mesh, SurfaceFactoryOptions::default());
        assert_eq!(factory.base.factory.get_scheme_type(), SchemeType::Catmark);
    }
}
