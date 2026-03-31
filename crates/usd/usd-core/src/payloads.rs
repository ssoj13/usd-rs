//! UsdPayloads - API for editing payload paths.
//!
//! Port of pxr/usd/usd/payloads.h/cpp
//!
//! A proxy class for applying listOp edits to the payload paths list for a prim.

use crate::common::ListPosition;
use crate::prim::Prim;
use usd_sdf::list_op::ListOp;
use usd_sdf::{LayerOffset, Path, Payload, PayloadVector};

/// A proxy class for applying listOp edits to the payload paths list for a prim.
///
/// Matches C++ `UsdPayloads`.
///
/// Payloads behave the same as references except that payloads can be
/// optionally loaded. Payloads provide a boundary that lazy composition
/// will not traverse across.
pub struct Payloads {
    prim: Prim,
}

impl Payloads {
    /// Creates a new Payloads object for the given prim.
    ///
    /// Matches C++ `UsdPayloads(const UsdPrim& prim)` (private constructor).
    pub(crate) fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Adds a payload to the payload listOp at the current EditTarget,
    /// in the position specified by position.
    ///
    /// Matches C++ `AddPayload(const SdfPayload& payload, UsdListPosition position)`.
    pub fn add_payload(&self, payload: &Payload, position: ListPosition) -> bool {
        if !self.prim.is_valid() {
            return false;
        }

        // Get or create the payload metadata as a PayloadListOp.
        let payloads_token = usd_tf::Token::new("payload");
        let mut list_op: ListOp<Payload> =
            self.prim.get_metadata(&payloads_token).unwrap_or_default();

        // Add the payload based on position
        match position {
            ListPosition::FrontOfPrependList => {
                let mut prepended = list_op.get_prepended_items().to_vec();
                prepended.insert(0, payload.clone());
                if list_op.set_prepended_items(prepended).is_err() {
                    return false;
                }
            }
            ListPosition::BackOfPrependList => {
                let mut prepended = list_op.get_prepended_items().to_vec();
                prepended.push(payload.clone());
                if list_op.set_prepended_items(prepended).is_err() {
                    return false;
                }
            }
            ListPosition::FrontOfAppendList => {
                let mut appended = list_op.get_appended_items().to_vec();
                appended.insert(0, payload.clone());
                if list_op.set_appended_items(appended).is_err() {
                    return false;
                }
            }
            ListPosition::BackOfAppendList => {
                let mut appended = list_op.get_appended_items().to_vec();
                appended.push(payload.clone());
                if list_op.set_appended_items(appended).is_err() {
                    return false;
                }
            }
        }

        let result = self.prim.set_metadata(&payloads_token, usd_vt::Value::from(list_op));
        if result {
            if let Some(stage) = self.prim.stage() {
                if let Some(pcp_cache) = stage.pcp_cache() {
                    pcp_cache.invalidate_prim_index(self.prim.path());
                }
                stage.handle_local_change(self.prim.path());
            }
        }
        result
    }

    /// Adds a payload with the given identifier, prim path, and layer offset.
    ///
    /// Matches C++ `AddPayload(const std::string &identifier, const SdfPath &primPath, const SdfLayerOffset &layerOffset, UsdListPosition position)`.
    pub fn add_payload_with_path(
        &self,
        identifier: &str,
        prim_path: &Path,
        layer_offset: LayerOffset,
        position: ListPosition,
    ) -> bool {
        let payload = Payload::with_layer_offset(identifier, prim_path.get_string(), layer_offset);
        self.add_payload(&payload, position)
    }

    /// Adds a payload to the default prim in the given layer.
    ///
    /// Matches C++ `AddPayload(const std::string &identifier, const SdfLayerOffset &layerOffset, UsdListPosition position)`.
    pub fn add_payload_to_default_prim(
        &self,
        identifier: &str,
        layer_offset: LayerOffset,
        position: ListPosition,
    ) -> bool {
        let payload = Payload::with_layer_offset(identifier, "", layer_offset);
        self.add_payload(&payload, position)
    }

    /// Add an internal payload to the specified prim.
    ///
    /// Matches C++ `AddInternalPayload(const SdfPath &primPath, const SdfLayerOffset &layerOffset, UsdListPosition position)`.
    pub fn add_internal_payload(
        &self,
        prim_path: &Path,
        layer_offset: LayerOffset,
        position: ListPosition,
    ) -> bool {
        let payload = Payload::with_layer_offset("", prim_path.get_string(), layer_offset);
        self.add_payload(&payload, position)
    }

    /// Removes the specified payload from the payloads listOp at the current EditTarget.
    ///
    /// Matches C++ `RemovePayload(const SdfPayload& payload)`.
    pub fn remove_payload(&self, payload: &Payload) -> bool {
        if !self.prim.is_valid() {
            return false;
        }

        let payloads_token = usd_tf::Token::new("payload");
        let mut list_op: ListOp<Payload> =
            self.prim.get_metadata(&payloads_token).unwrap_or_default();

        if list_op.is_explicit() {
            let mut explicit = list_op.get_explicit_items().to_vec();
            explicit.retain(|item| item != payload);
            if list_op.set_explicit_items(explicit).is_err() {
                return false;
            }
        } else {
            let mut added = list_op.get_added_items().to_vec();
            added.retain(|item| item != payload);
            list_op.set_added_items(added);

            let mut prepended = list_op.get_prepended_items().to_vec();
            prepended.retain(|item| item != payload);
            if list_op.set_prepended_items(prepended).is_err() {
                return false;
            }

            let mut appended = list_op.get_appended_items().to_vec();
            appended.retain(|item| item != payload);
            if list_op.set_appended_items(appended).is_err() {
                return false;
            }

            let mut deleted = list_op.get_deleted_items().to_vec();
            if !deleted.iter().any(|item| item == payload) {
                deleted.push(payload.clone());
            }
            if list_op.set_deleted_items(deleted).is_err() {
                return false;
            }
        }

        let result = self.prim.set_metadata(&payloads_token, usd_vt::Value::from(list_op));
        if result {
            if let Some(stage) = self.prim.stage() {
                if let Some(pcp_cache) = stage.pcp_cache() {
                    pcp_cache.invalidate_prim_index(self.prim.path());
                }
                stage.handle_local_change(self.prim.path());
            }
        }
        result
    }

    /// Removes the authored payload listOp edits at the current edit target.
    ///
    /// Matches C++ `ClearPayloads()`.
    pub fn clear_payloads(&self) -> bool {
        if !self.prim.is_valid() {
            return false;
        }

        let payloads_token = usd_tf::Token::new("payload");
        let result = self.prim.clear_metadata(&payloads_token);
        if result {
            if let Some(stage) = self.prim.stage() {
                if let Some(pcp_cache) = stage.pcp_cache() {
                    pcp_cache.invalidate_prim_index(self.prim.path());
                }
                stage.handle_local_change(self.prim.path());
            }
        }
        result
    }

    /// Explicitly set the payloads, potentially blocking weaker opinions.
    ///
    /// Matches C++ `SetPayloads(const SdfPayloadVector& items)`.
    pub fn set_payloads(&self, items: PayloadVector) -> bool {
        if !self.prim.is_valid() {
            return false;
        }

        let payloads_token = usd_tf::Token::new("payload");
        let mut list_op = ListOp::new();
        if list_op.set_explicit_items(items).is_err() {
            return false;
        }

        let result = self.prim.set_metadata(&payloads_token, usd_vt::Value::from(list_op));
        if result {
            if let Some(stage) = self.prim.stage() {
                if let Some(pcp_cache) = stage.pcp_cache() {
                    pcp_cache.invalidate_prim_index(self.prim.path());
                }
                stage.handle_local_change(self.prim.path());
            }
        }
        result
    }

    /// Return the prim this object is bound to.
    ///
    /// Matches C++ `GetPrim()`.
    pub fn prim(&self) -> &Prim {
        &self.prim
    }

}

impl std::ops::Deref for Payloads {
    type Target = Prim;

    fn deref(&self) -> &Self::Target {
        &self.prim
    }
}

impl From<Prim> for Payloads {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}
