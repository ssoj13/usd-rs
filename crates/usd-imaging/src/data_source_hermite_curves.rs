//! DataSourceHermiteCurves - HermiteCurves data source for Hydra.
//!
//! OpenUSD does not expose a native Hydra `hermiteCurves` prim type.
//! `_ref` `UsdImagingHermiteCurvesAdapter` therefore populates Hermite curves
//! as `basisCurves`, using a synthetic topology:
//! - `type = linear`
//! - `basis = bezier`
//! - `wrap = nonperiodic`
//! - `curveVertexCounts = authored Hermite counts`
//!
//! Tangents and point weights stay authored on the USD prim, but they are not
//! consumed by Hydra Storm in this compatibility path. The important contract
//! is that every scene-index and legacy-delegate caller sees the same
//! `basisCurves` schema shape as the reference implementation.

use crate::data_source_gprim::DataSourceGprim;
use crate::data_source_stage_globals::DataSourceStageGlobalsHandle;
use crate::types::PropertyInvalidationType;
use std::sync::Arc;
use usd_core::Prim;
use usd_geom::hermite_curves::HermiteCurves;
use usd_hd::data_source::HdRetainedTypedSampledDataSource;
use usd_hd::{
    HdContainerDataSource, HdDataSourceBase, HdDataSourceBaseHandle, HdDataSourceLocator,
    HdDataSourceLocatorSet,
};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Array;

mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static BASIS_CURVES: LazyLock<Token> = LazyLock::new(|| Token::new("basisCurves"));
    pub static TOPOLOGY: LazyLock<Token> = LazyLock::new(|| Token::new("topology"));
    pub static CURVE_VERTEX_COUNTS: LazyLock<Token> =
        LazyLock::new(|| Token::new("curveVertexCounts"));
    pub static TYPE: LazyLock<Token> = LazyLock::new(|| Token::new("type"));
    pub static BASIS: LazyLock<Token> = LazyLock::new(|| Token::new("basis"));
    pub static WRAP: LazyLock<Token> = LazyLock::new(|| Token::new("wrap"));
    pub static LINEAR: LazyLock<Token> = LazyLock::new(|| Token::new("linear"));
    pub static BEZIER: LazyLock<Token> = LazyLock::new(|| Token::new("bezier"));
    pub static NONPERIODIC: LazyLock<Token> = LazyLock::new(|| Token::new("nonperiodic"));
}

/// Container data source exposing Hermite curves as a synthetic Hydra
/// basis-curves topology.
#[derive(Clone)]
pub struct DataSourceHermiteCurvesTopology {
    scene_index_path: Path,
    curves: HermiteCurves,
    stage_globals: DataSourceStageGlobalsHandle,
}

impl std::fmt::Debug for DataSourceHermiteCurvesTopology {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceHermiteCurvesTopology").finish()
    }
}

impl DataSourceHermiteCurvesTopology {
    /// Create a new topology data source for the `_ref` Hermite compatibility
    /// path.
    pub fn new(
        scene_index_path: Path,
        curves: HermiteCurves,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Arc<Self> {
        Arc::new(Self {
            scene_index_path,
            curves,
            stage_globals,
        })
    }
}

impl HdDataSourceBase for DataSourceHermiteCurvesTopology {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for DataSourceHermiteCurvesTopology {
    fn get_names(&self) -> Vec<Token> {
        vec![
            tokens::CURVE_VERTEX_COUNTS.clone(),
            tokens::BASIS.clone(),
            tokens::TYPE.clone(),
            tokens::WRAP.clone(),
        ]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if *name == *tokens::CURVE_VERTEX_COUNTS {
            let attr = self.curves.curves().get_curve_vertex_counts_attr();
            if !attr.is_valid() {
                return None;
            }
            return Some(
                crate::data_source_attribute::DataSourceAttribute::<Array<i32>>::new(
                    attr,
                    self.stage_globals.clone(),
                    self.scene_index_path.clone(),
                ) as HdDataSourceBaseHandle,
            );
        }

        if *name == *tokens::TYPE {
            return Some(
                HdRetainedTypedSampledDataSource::new(tokens::LINEAR.clone())
                    as HdDataSourceBaseHandle,
            );
        }
        if *name == *tokens::BASIS {
            return Some(
                HdRetainedTypedSampledDataSource::new(tokens::BEZIER.clone())
                    as HdDataSourceBaseHandle,
            );
        }
        if *name == *tokens::WRAP {
            return Some(
                HdRetainedTypedSampledDataSource::new(tokens::NONPERIODIC.clone())
                    as HdDataSourceBaseHandle,
            );
        }

        None
    }
}

/// Basis-curves compatibility container for Hermite curves.
#[derive(Clone)]
pub struct DataSourceHermiteCurves {
    scene_index_path: Path,
    curves: HermiteCurves,
    stage_globals: DataSourceStageGlobalsHandle,
}

impl std::fmt::Debug for DataSourceHermiteCurves {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceHermiteCurves").finish()
    }
}

impl DataSourceHermiteCurves {
    pub fn new(
        scene_index_path: Path,
        curves: HermiteCurves,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Arc<Self> {
        Arc::new(Self {
            scene_index_path,
            curves,
            stage_globals,
        })
    }
}

impl HdDataSourceBase for DataSourceHermiteCurves {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for DataSourceHermiteCurves {
    fn get_names(&self) -> Vec<Token> {
        vec![tokens::TOPOLOGY.clone()]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if *name == *tokens::TOPOLOGY {
            return Some(
                DataSourceHermiteCurvesTopology::new(
                    self.scene_index_path.clone(),
                    self.curves.clone(),
                    self.stage_globals.clone(),
                ) as HdDataSourceBaseHandle,
            );
        }
        None
    }
}

/// Prim data source for `UsdGeomHermiteCurves` that exposes Hydra
/// `basisCurves` instead of a non-existent `hermiteCurves` prim type.
pub struct DataSourceHermiteCurvesPrim {
    base: Arc<DataSourceGprim>,
    prim: Prim,
    scene_index_path: Path,
    stage_globals: DataSourceStageGlobalsHandle,
}

impl HdDataSourceBase for DataSourceHermiteCurvesPrim {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            base: Arc::clone(&self.base),
            prim: self.prim.clone(),
            scene_index_path: self.scene_index_path.clone(),
            stage_globals: self.stage_globals.clone(),
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(Arc::new(Self {
            base: Arc::clone(&self.base),
            prim: self.prim.clone(),
            scene_index_path: self.scene_index_path.clone(),
            stage_globals: self.stage_globals.clone(),
        }))
    }
}

impl HdContainerDataSource for DataSourceHermiteCurvesPrim {
    fn get_names(&self) -> Vec<Token> {
        Self::get_names(self)
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        Self::get(self, name)
    }
}

impl std::fmt::Debug for DataSourceHermiteCurvesPrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceHermiteCurvesPrim").finish()
    }
}

impl DataSourceHermiteCurvesPrim {
    pub fn new(
        scene_index_path: Path,
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Self {
        Self {
            base: DataSourceGprim::new(
                scene_index_path.clone(),
                prim.clone(),
                stage_globals.clone(),
            ),
            prim,
            scene_index_path,
            stage_globals,
        }
    }

    pub fn get_names(&self) -> Vec<Token> {
        let mut names = self.base.get_names();
        names.push(tokens::BASIS_CURVES.clone());
        names
    }

    pub fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if *name == *tokens::BASIS_CURVES {
            return Some(
                DataSourceHermiteCurves::new(
                    self.scene_index_path.clone(),
                    HermiteCurves::new(self.prim.clone()),
                    self.stage_globals.clone(),
                ) as HdDataSourceBaseHandle,
            );
        }
        self.base.get(name)
    }

    /// `_ref` only invalidates the synthetic topology when Hermite
    /// `curveVertexCounts` changes. Tangents and weights are ignored by the
    /// compatibility render path and must not create fake topology dirties.
    pub fn invalidate(
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        let mut result =
            DataSourceGprim::invalidate(prim, subprim, properties, invalidation_type);

        if subprim.is_empty() {
            for property_name in properties {
                if property_name.as_str() == "curveVertexCounts" {
                    result.insert(HdDataSourceLocator::from_tokens_3(
                        tokens::BASIS_CURVES.clone(),
                        tokens::TOPOLOGY.clone(),
                        tokens::CURVE_VERTEX_COUNTS.clone(),
                    ));
                }
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source_stage_globals::NoOpStageGlobals;
    use usd_core::common::InitialLoadSet;
    use usd_core::Stage;
    use usd_hd::HdTypedSampledDataSource;

    #[test]
    fn test_hermite_datasource_reports_basis_curves_schema() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("stage");
        let prim = stage.get_pseudo_root();
        let globals: DataSourceStageGlobalsHandle = Arc::new(NoOpStageGlobals::default());
        let ds = DataSourceHermiteCurvesPrim::new(prim.path().clone(), prim, globals);
        let names = ds.get_names();
        assert!(names.iter().any(|name| name == "basisCurves"));
    }

    #[test]
    fn test_hermite_topology_uses_reference_constants() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("stage");
        let prim = stage.get_pseudo_root();
        let globals: DataSourceStageGlobalsHandle = Arc::new(NoOpStageGlobals::default());
        let ds = DataSourceHermiteCurvesTopology::new(
            prim.path().clone(),
            HermiteCurves::new(prim),
            globals,
        );
        let basis = ds
            .get(&tokens::BASIS)
            .expect("basis ds")
            .as_any()
            .downcast_ref::<HdRetainedTypedSampledDataSource<Token>>()
            .expect("basis typed")
            .get_typed_value(0.0);
        let curve_type = ds
            .get(&tokens::TYPE)
            .expect("type ds")
            .as_any()
            .downcast_ref::<HdRetainedTypedSampledDataSource<Token>>()
            .expect("type typed")
            .get_typed_value(0.0);
        let wrap = ds
            .get(&tokens::WRAP)
            .expect("wrap ds")
            .as_any()
            .downcast_ref::<HdRetainedTypedSampledDataSource<Token>>()
            .expect("wrap typed")
            .get_typed_value(0.0);

        assert_eq!(basis.as_str(), "bezier");
        assert_eq!(curve_type.as_str(), "linear");
        assert_eq!(wrap.as_str(), "nonperiodic");
    }
}
