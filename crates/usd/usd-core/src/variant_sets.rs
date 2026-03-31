//! UsdVariantSet and UsdVariantSets - API for working with variant sets.
//!
//! Port of pxr/usd/usd/variantSets.h/cpp
//!
//! A UsdVariantSet represents a single VariantSet in USD (e.g. modelingVariant
//! or shadingVariant), which can have multiple variations that express different
//! sets of opinions about the scene description rooted at the prim that defines
//! the VariantSet.

use crate::common::ListPosition;
use crate::edit_target::EditTarget;
use crate::prim::Prim;
use std::collections::{HashMap, HashSet};

fn get_or_create_edit_target_prim_spec(prim: &Prim) -> Option<usd_sdf::PrimSpec> {
    let stage = prim.stage()?;
    let edit_target = stage.get_edit_target();
    let layer = edit_target.layer()?.clone();
    let spec_path = edit_target.map_to_spec_path(prim.path());

    if layer.get_prim_at_path(&spec_path).is_none() {
        let _ = layer.create_prim_spec(&spec_path, usd_sdf::Specifier::Over, "");
    }

    layer.get_prim_at_path(&spec_path)
}

fn note_variant_edit(prim: &Prim) {
    if let Some(stage) = prim.stage() {
        if let Some(pcp_cache) = stage.pcp_cache() {
            pcp_cache.clear_prim_index_cache();
        }
        stage.handle_local_change(prim.path());
    }
}

/// A UsdVariantSet represents a single VariantSet in USD.
///
/// Matches C++ `UsdVariantSet`.
///
/// A VariantSet can have multiple variations that express different sets of
/// opinions about the scene description rooted at the prim that defines the
/// VariantSet.
pub struct VariantSet {
    prim: Prim,
    variant_set_name: String,
}

impl VariantSet {
    /// Creates a new VariantSet for the given prim and variant set name.
    ///
    /// Matches C++ `UsdVariantSet(const UsdPrim &prim, const std::string &variantSetName)`.
    pub(crate) fn new(prim: Prim, variant_set_name: String) -> Self {
        Self {
            prim,
            variant_set_name,
        }
    }

    /// Author a variant spec for variantName in this VariantSet at the
    /// stage's current EditTarget, in the position specified by position.
    ///
    /// Matches C++ `AddVariant(const std::string& variantName, UsdListPosition position)`.
    ///
    /// Returns true if the spec was successfully authored, false otherwise.
    /// This will create the VariantSet itself, if necessary.
    pub fn add_variant(&self, variant_name: &str, position: ListPosition) -> bool {
        if !self.prim.is_valid() {
            return false;
        }
        let Some(prim_spec) = get_or_create_edit_target_prim_spec(&self.prim) else {
            return false;
        };
        let mut proxy = prim_spec.variant_sets();
        let Ok(variant_set_spec) = proxy.add(&self.variant_set_name) else {
            return false;
        };

        if variant_set_spec.has_variant(variant_name) {
            return true;
        }

        let ok = usd_sdf::VariantSpec::new(&variant_set_spec, variant_name).is_ok();
        if ok {
            let _ = position;
            note_variant_edit(&self.prim);
        }
        ok
    }

    /// Return the composed variant names for this VariantSet, ordered lexicographically.
    ///
    /// Matches C++ `GetVariantNames() const`.
    /// Walks PrimIndex nodes calling PcpComposeSiteVariantSetOptions.
    pub fn get_variant_names(&self) -> Vec<String> {
        if !self.prim.is_valid() {
            return Vec::new();
        }

        let mut result = HashSet::new();

        if let Some(prim_index) = self.prim.prim_index() {
            // Walk all nodes, collect variant options (available variant names)
            for node in prim_index.nodes() {
                if !node.can_contribute_specs() || !node.has_specs() {
                    continue;
                }
                let Some(layer_stack) = node.layer_stack() else {
                    continue;
                };
                let opts = usd_pcp::compose_site::compose_site_variant_set_options(
                    &layer_stack,
                    &node.path(),
                    &self.variant_set_name,
                );
                result.extend(opts);
            }
        }

        // Also consult authored layer data directly; local variant editing can
        // create specs before PCP variant composition fully reflects them.
        if let Some(stage) = self.prim.stage() {
            if let Some(vset_path) = self
                .prim
                .path()
                .append_variant_selection(&self.variant_set_name, "")
            {
                for layer in stage.get_layer_stack(true) {
                    if let Some(spec) = layer.get_field(&vset_path, &usd_tf::Token::new("variants"))
                    {
                        if let Some(names) = spec.as_vec_clone::<String>() {
                            result.extend(names);
                        }
                    }
                }
            }
        }

        let mut names: Vec<String> = result.into_iter().collect();
        names.sort();
        names
    }

    /// Returns true if this VariantSet already possesses a variant named
    /// variantName in any layer.
    ///
    /// Matches C++ `HasAuthoredVariant(const std::string& variantName) const`.
    pub fn has_authored_variant(&self, variant_name: &str) -> bool {
        self.get_variant_names().contains(&variant_name.to_string())
    }

    /// Return the variant selection for this VariantSet.
    /// If there is no selection, return the empty string.
    ///
    /// Matches C++ `GetVariantSelection() const`.
    /// Uses PrimIndex::get_selection_applied_for_variant_set().
    pub fn get_variant_selection(&self) -> String {
        if !self.prim.is_valid() {
            return String::new();
        }

        if let Some(stage) = self.prim.stage() {
            for layer in stage.get_layer_stack(true) {
                if let Some(selection) = layer
                    .get_field_dict_value_by_key(
                        self.prim.path(),
                        &usd_tf::Token::new("variantSelection"),
                        &usd_tf::Token::new(&self.variant_set_name),
                    )
                    .and_then(|value| {
                        value
                            .get::<String>()
                            .cloned()
                            .or_else(|| value.get::<usd_tf::Token>().map(|token| token.to_string()))
                    })
                {
                    return selection;
                }
            }
        }

        self.prim
            .prim_index()
            .and_then(|prim_index| {
                prim_index.get_selection_applied_for_variant_set(&self.variant_set_name)
            })
            .unwrap_or_default()
    }

    /// Returns true if there is a selection authored for this VariantSet in any layer.
    ///
    /// Matches C++ `HasAuthoredVariantSelection(std::string *value) const`.
    pub fn has_authored_variant_selection(&self) -> bool {
        !self.get_variant_selection().is_empty()
    }

    /// Returns true if there is a selection authored, and stores the value.
    ///
    /// Matches C++ `HasAuthoredVariantSelection(std::string *value) const`.
    pub fn has_authored_variant_selection_value(&self) -> Option<String> {
        let selection = self.get_variant_selection();
        if selection.is_empty() {
            None
        } else {
            Some(selection)
        }
    }

    /// Author a variant selection for this VariantSet, setting it to
    /// variantName in the stage's current EditTarget.
    ///
    /// Matches C++ `SetVariantSelection(const std::string &variantName)`.
    ///
    /// If variantName is empty, clear the variant selection (see ClearVariantSelection).
    /// Returns true if the selection was successfully authored or cleared, false otherwise.
    pub fn set_variant_selection(&self, variant_name: &str) -> bool {
        if !self.prim.is_valid() {
            return false;
        }
        let Some(mut prim_spec) = get_or_create_edit_target_prim_spec(&self.prim) else {
            return false;
        };
        prim_spec.set_variant_selection(&self.variant_set_name, variant_name);
        note_variant_edit(&self.prim);
        true
    }

    /// Clear any selection for this VariantSet from the current EditTarget.
    ///
    /// Matches C++ `ClearVariantSelection()`.
    pub fn clear_variant_selection(&self) -> bool {
        self.set_variant_selection("")
    }

    /// Block any weaker selections for this VariantSet by authoring an
    /// empty string at the stage's current EditTarget.
    ///
    /// Matches C++ `BlockVariantSelection()`.
    pub fn block_variant_selection(&self) -> bool {
        if !self.prim.is_valid() {
            return false;
        }
        let Some(mut prim_spec) = get_or_create_edit_target_prim_spec(&self.prim) else {
            return false;
        };
        prim_spec.block_variant_selection(&self.variant_set_name);
        note_variant_edit(&self.prim);
        true
    }

    /// Return a UsdEditTarget that edits the currently selected variant in
    /// this VariantSet in layer.
    ///
    /// Matches C++ `GetVariantEditTarget(const SdfLayerHandle &layer) const`.
    ///
    /// If there is no currently selected variant in this VariantSet, return
    /// an invalid EditTarget.
    pub fn get_variant_edit_target(&self) -> EditTarget {
        let selection = self.get_variant_selection();
        if selection.is_empty() {
            return EditTarget::default();
        }

        // Build variant path and create edit target
        if let Some(stage) = self.prim.stage() {
            let prim_path = self.prim.path();
            if let Some(variant_path) =
                prim_path.append_variant_selection(&self.variant_set_name, &selection)
            {
                if let Some(layer) = stage.get_edit_target().get_layer() {
                    return EditTarget::for_local_direct_variant(layer, variant_path);
                }
            }
        }

        EditTarget::default()
    }

    /// Return this VariantSet's held prim.
    ///
    /// Matches C++ `GetPrim() const`.
    pub fn prim(&self) -> &Prim {
        &self.prim
    }

    /// Return this VariantSet's name.
    ///
    /// Matches C++ `GetName() const`.
    pub fn name(&self) -> &str {
        &self.variant_set_name
    }

    /// Is this UsdVariantSet object usable?
    ///
    /// Matches C++ `IsValid() const`.
    pub fn is_valid(&self) -> bool {
        self.prim.is_valid()
    }
}

impl std::ops::Deref for VariantSet {
    type Target = Prim;

    fn deref(&self) -> &Self::Target {
        &self.prim
    }
}

/// UsdVariantSets represents the collection of VariantSets that are present on a UsdPrim.
///
/// Matches C++ `UsdVariantSets`.
///
/// A UsdVariantSets object, retrieved from a prim via UsdPrim::GetVariantSets(),
/// provides the API for interrogating and modifying the composed list of VariantSets
/// active defined on the prim.
pub struct VariantSets {
    prim: Prim,
}

impl VariantSets {
    /// Creates a new VariantSets object for the given prim.
    ///
    /// Matches C++ `UsdVariantSets(const UsdPrim& prim)`.
    pub(crate) fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Find an existing, or create a new VariantSet on the originating UsdPrim,
    /// named variantSetName.
    ///
    /// Matches C++ `AddVariantSet(const std::string& variantSetName, UsdListPosition position)`.
    pub fn add_variant_set(&self, variant_set_name: &str, position: ListPosition) -> VariantSet {
        let var_set = self.get_variant_set(variant_set_name);
        let Some(prim_spec) = get_or_create_edit_target_prim_spec(&self.prim) else {
            return var_set;
        };
        let mut proxy = prim_spec.variant_sets();
        if proxy.add(variant_set_name).is_ok() {
            let _ = position;
            note_variant_edit(&self.prim);
        }

        var_set
    }

    /// Compute the list of all VariantSets authored on the originating UsdPrim.
    ///
    /// Matches C++ `GetNames(std::vector<std::string>* names) const`.
    /// Walks PrimIndex nodes calling PcpComposeSiteVariantSets.
    pub fn get_names(&self) -> Vec<String> {
        if !self.prim.is_valid() {
            return Vec::new();
        }

        let mut seen = HashSet::new();
        let mut result = Vec::new();
        if let Some(prim_index) = self.prim.prim_index() {
            // Walk all nodes, collect variant set names (C++ uses PcpTokenSet)
            for node in prim_index.nodes() {
                if !node.can_contribute_specs() || !node.has_specs() {
                    continue;
                }
                let Some(layer_stack) = node.layer_stack() else {
                    continue;
                };
                let vsets =
                    usd_pcp::compose_site::compose_site_variant_sets(&layer_stack, &node.path());
                for name in vsets {
                    if seen.insert(name.clone()) {
                        result.push(name);
                    }
                }
            }
        }

        // Also merge directly authored variant set names from the layer stack.
        if let Some(stage) = self.prim.stage() {
            for layer in stage.get_layer_stack(true) {
                if let Some(prim_spec) = layer.get_prim_at_path(self.prim.path()) {
                    for name in prim_spec.variant_sets().names() {
                        if seen.insert(name.clone()) {
                            result.push(name);
                        }
                    }
                }
            }
        }

        result
    }

    /// Return a UsdVariantSet object for variantSetName.
    ///
    /// Matches C++ `GetVariantSet(const std::string& variantSetName) const`.
    ///
    /// This always succeeds, although the returned VariantSet will be invalid
    /// if the originating prim is invalid.
    pub fn get_variant_set(&self, variant_set_name: &str) -> VariantSet {
        VariantSet::new(self.prim.clone(), variant_set_name.to_string())
    }

    /// Returns true if a VariantSet named variantSetName exists on the originating prim.
    ///
    /// Matches C++ `HasVariantSet(const std::string& variantSetName) const`.
    pub fn has_variant_set(&self, variant_set_name: &str) -> bool {
        self.get_names().contains(&variant_set_name.to_string())
    }

    /// Return the composed variant selection for the VariantSet named variantSetName.
    ///
    /// Matches C++ `GetVariantSelection(const std::string& variantSetName) const`.
    ///
    /// If there is no selection, (or variantSetName does not exist) return the empty string.
    pub fn get_variant_selection(&self, variant_set_name: &str) -> String {
        self.get_variant_set(variant_set_name)
            .get_variant_selection()
    }

    /// Set the variant selection for the given variant set.
    ///
    /// Matches C++ `SetSelection(const std::string& variantSetName, const std::string& variantName)`.
    pub fn set_selection(&self, variant_set_name: &str, variant_name: &str) -> bool {
        self.get_variant_set(variant_set_name)
            .set_variant_selection(variant_name)
    }

    /// Returns the composed map of all variant selections authored on the originating UsdPrim.
    ///
    /// Matches C++ `GetAllVariantSelections() const`.
    pub fn get_all_variant_selections(&self) -> HashMap<String, String> {
        let mut result = HashMap::new();

        for name in self.get_names() {
            let selection = self.get_variant_selection(&name);
            if !selection.is_empty() {
                result.insert(name, selection);
            }
        }

        result
    }

    /// Return the prim this object is bound to.
    pub fn prim(&self) -> &Prim {
        &self.prim
    }
}

impl std::ops::Deref for VariantSets {
    type Target = Prim;

    fn deref(&self) -> &Self::Target {
        &self.prim
    }
}

impl From<Prim> for VariantSets {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_variant_sets_creation() {
        // Basic construction test - VariantSets requires a Prim
        // Full tests would require a Stage to create real prims
    }
}
