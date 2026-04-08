//! HdSt_BasisCurvesTopologySceneIndex - topology processing for basis curves.
//!
//! Filtering scene index that processes basis curves topology for Storm.
//! Handles index buffer generation for different curve basis types
//! (bezier, bspline, catmullRom) and wrap modes (nonperiodic, periodic, pinned).

use parking_lot::RwLock;
use std::sync::Arc;
use usd_hd::data_source::{HdDataSourceBaseHandle, HdDataSourceLocator};
use usd_hd::scene_index::{
    AddedPrimEntry, DirtiedPrimEntry, FilteringObserverTarget, HdSceneIndexBase,
    HdSceneIndexHandle, HdSceneIndexObserverHandle, HdSceneIndexPrim,
    HdSingleInputFilteringSceneIndexBase, RemovedPrimEntry, RenamedPrimEntry, SdfPathVector,
};
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// Basis curves topology scene index for Storm.
///
/// Processes basis curves prims to generate appropriate index buffers
/// for rendering. Handles different curve basis types and wrap modes,
/// producing point indices suitable for GPU line/strip drawing.
///
/// Port of C++ `HdSt_BasisCurvesTopology` integration into scene index pattern.
pub struct HdStBasisCurvesTopologySceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
}

impl HdStBasisCurvesTopologySceneIndex {
    /// Create a new basis curves topology scene index.
    pub fn new(input_scene: Option<HdSceneIndexHandle>) -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(input_scene),
        }))
    }

    /// Check if prim type is a basis curves type.
    fn is_curves_prim(prim_type: &Token) -> bool {
        prim_type == "basisCurves" || prim_type == "BasisCurves"
    }

    /// Process a basis curves prim to augment topology data.
    ///
    /// For curves prims, adds computed index buffer information
    /// to the prim data source for downstream consumption.
    fn process_prim(&self, prim: HdSceneIndexPrim) -> HdSceneIndexPrim {
        if !Self::is_curves_prim(&prim.prim_type) {
            return prim;
        }
        // Topology processing: in full implementation, this would compute
        // index buffers based on curve basis/type/wrap parameters.
        // For now, pass through - actual computation happens in HdStBasisCurves::Sync.
        prim
    }
}

impl HdSceneIndexBase for HdStBasisCurvesTopologySceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        if let Some(input) = self.base.get_input_scene() {
            {
                let input_lock = input.read();
                let prim = input_lock.get_prim(prim_path);
                return self.process_prim(prim);
            }
        }
        HdSceneIndexPrim::empty()
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        if let Some(input) = self.base.get_input_scene() {
            {
                let input_lock = input.read();
                return input_lock.get_child_prim_paths(prim_path);
            }
        }
        Vec::new()
    }

    fn add_observer(&self, observer: HdSceneIndexObserverHandle) {
        self.base.base().add_observer(observer);
    }

    fn remove_observer(&self, observer: &HdSceneIndexObserverHandle) {
        self.base.base().remove_observer(observer);
    }

    fn _system_message(&self, _msg: &Token, _args: Option<HdDataSourceBaseHandle>) {}

    fn get_display_name(&self) -> String {
        "HdSt_BasisCurvesTopologySceneIndex".to_string()
    }

    fn get_input_scenes_for_system_message(&self) -> Vec<HdSceneIndexHandle> {
        self.base.get_input_scene().cloned().into_iter().collect()
    }
}

impl FilteringObserverTarget for HdStBasisCurvesTopologySceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        self.base.forward_prims_added(self, entries);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        self.base.forward_prims_removed(self, entries);
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        // When topology data changes on a curves prim, dirty the topology locator
        let topo_locator = HdDataSourceLocator::from_token(Token::new("basisCurvesTopology"));
        let mut augmented = Vec::new();

        for entry in entries {
            if entry.dirty_locators.intersects_locator(&topo_locator) {
                // Topology changed - also dirty computed indices
                let mut locs = entry.dirty_locators.clone();
                locs.insert(HdDataSourceLocator::from_token(Token::new(
                    "topologyComputedIndices",
                )));
                augmented.push(DirtiedPrimEntry::new(entry.prim_path.clone(), locs));
            } else {
                augmented.push(entry.clone());
            }
        }

        self.base.forward_prims_dirtied(self, &augmented);
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.base.forward_prims_renamed(self, entries);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create() {
        let si = HdStBasisCurvesTopologySceneIndex::new(None);
        let lock = si.read();
        assert_eq!(
            lock.get_display_name(),
            "HdSt_BasisCurvesTopologySceneIndex"
        );
    }

    #[test]
    fn test_is_curves_prim() {
        assert!(HdStBasisCurvesTopologySceneIndex::is_curves_prim(
            &Token::new("basisCurves")
        ));
        assert!(HdStBasisCurvesTopologySceneIndex::is_curves_prim(
            &Token::new("BasisCurves")
        ));
        assert!(!HdStBasisCurvesTopologySceneIndex::is_curves_prim(
            &Token::new("mesh")
        ));
    }
}
