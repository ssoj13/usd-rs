//! HdOverlayContainerDataSource - layered container data source.
//!
//! Corresponds to C++ `HdOverlayContainerDataSource` (overlayContainerDataSource.h/cpp).
//! Lazily composes two or more container source hierarchies.
//! Earlier entries have stronger opinion strength for overlapping child names.
//! Overlapping children that are all containers are returned as a nested overlay.

use super::base::HdDataSourceBaseHandle;
use super::container::{HdContainerDataSource, HdContainerDataSourceHandle};
use usd_tf::Token;
use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;

/// Lazily composes two or more container source hierarchies.
///
/// Earlier entries in the list have stronger opinion strength.
/// When the same child name exists in multiple sources:
/// - If all matching children are containers, they are wrapped in another overlay.
/// - If a non-container value is encountered (and no containers precede it),
///   that value is returned directly.
/// - A [`HdBlockDataSource`](super::HdBlockDataSource) masks values from weaker sources.
#[derive(Debug)]
pub struct HdOverlayContainerDataSource {
    /// Sources in priority order (index 0 = strongest opinion).
    containers: Vec<HdContainerDataSourceHandle>,
}

impl HdOverlayContainerDataSource {
    /// Create from a list of container sources (strongest opinion first).
    pub fn new(containers: Vec<HdContainerDataSourceHandle>) -> Arc<Self> {
        Arc::new(Self { containers })
    }

    /// Create from two sources.
    pub fn new2(
        src1: HdContainerDataSourceHandle,
        src2: HdContainerDataSourceHandle,
    ) -> Arc<Self> {
        Arc::new(Self { containers: vec![src1, src2] })
    }

    /// Create from three sources.
    pub fn new3(
        src1: HdContainerDataSourceHandle,
        src2: HdContainerDataSourceHandle,
        src3: HdContainerDataSourceHandle,
    ) -> Arc<Self> {
        Arc::new(Self { containers: vec![src1, src2, src3] })
    }

    /// Overlay two sources, returning `None` if both are `None`,
    /// and the surviving source if only one is non-null.
    ///
    /// Matches C++ `HdOverlayContainerDataSource::OverlayedContainerDataSources`.
    pub fn overlay(
        src1: Option<HdContainerDataSourceHandle>,
        src2: Option<HdContainerDataSourceHandle>,
    ) -> Option<HdContainerDataSourceHandle> {
        match (src1, src2) {
            (None, s) => s,
            (s, None) => s,
            (Some(s1), Some(s2)) => Some(Self::new2(s1, s2)),
        }
    }
}

impl super::base::HdDataSourceBase for HdOverlayContainerDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self { containers: self.containers.clone() })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<super::container::HdContainerDataSourceHandle> {
        Some(Arc::new(Self { containers: self.containers.clone() }))
    }
}

impl HdContainerDataSource for HdOverlayContainerDataSource {
    /// Union of all child names across all sources.
    fn get_names(&self) -> Vec<Token> {
        let mut seen: HashSet<Token> = HashSet::new();
        let mut names = Vec::new();
        for container in &self.containers {
            for name in container.get_names() {
                if seen.insert(name.clone()) {
                    names.push(name);
                }
            }
        }
        names
    }

    /// Returns the layered child matching C++ Get() logic:
    /// - Walk sources in priority order.
    /// - Collect all container children; return on first non-container.
    /// - A BlockDataSource masks weaker sources (returns None).
    /// - If multiple container children found, wrap them in a new overlay.
    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        let mut child_containers: Vec<HdContainerDataSourceHandle> = Vec::new();

        for container in &self.containers {
            if let Some(child) = container.get(name) {
                // Try to downcast to container.
                if let Some(any) = child.as_any().downcast_ref::<HdOverlayContainerDataSource>() {
                    // Already an overlay — keep it as a container.
                    drop(any);
                    // We need the Arc<dyn HdContainerDataSource> form.
                    // Since we can't downcast Arc<dyn Base> to Arc<dyn Container> directly,
                    // use the helper cast function.
                    if let Some(c) = cast_to_container(child.clone()) {
                        child_containers.push(c);
                    } else {
                        // Non-container, non-block value: stop if no containers accumulated.
                        if child_containers.is_empty() {
                            return Some(child);
                        }
                        break;
                    }
                } else if let Some(c) = cast_to_container(child.clone()) {
                    child_containers.push(c);
                } else {
                    // Non-container value.
                    if child_containers.is_empty() {
                        // Check for BlockDataSource (masks weaker sources).
                        if is_block_data_source(&child) {
                            return None;
                        }
                        return Some(child);
                    }
                    // Containers already accumulated — stop here, ignore this non-container.
                    break;
                }
            }
        }

        match child_containers.len() {
            0 => None,
            1 => Some(child_containers.into_iter().next().unwrap()),
            _ => Some(HdOverlayContainerDataSource::new(child_containers)),
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Try to cast a base handle to a container handle.
///
/// Delegates to the container module's `cast_to_container` which handles all
/// known concrete container types via `Any` downcasting.
fn cast_to_container(handle: HdDataSourceBaseHandle) -> Option<HdContainerDataSourceHandle> {
    super::container::cast_to_container(&handle)
}

/// Returns true if the data source is a block data source (used to mask values).
///
/// C++ uses `dynamic_cast<HdBlockDataSource*>` to detect block data sources.
/// Block data sources in overlays act as masks — they suppress the corresponding
/// key from lower-priority containers, preventing fallthrough.
fn is_block_data_source(handle: &HdDataSourceBaseHandle) -> bool {
    handle.as_any().downcast_ref::<super::base::HdBlockDataSource>().is_some()
}

/// Type alias for an overlay container data source handle.
pub type HdOverlayContainerDataSourceHandle = Arc<HdOverlayContainerDataSource>;
