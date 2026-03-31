//! UsdGeomPrimvarsAPI - API schema for geometric primitive variables.
//!
//! Port of pxr/usd/usdGeom/primvarsAPI.h/cpp
//!
//! UsdGeomPrimvarsAPI encodes geometric "primitive variables", as UsdGeomPrimvar,
//! which interpolate across a primitive's topology, can override shader inputs,
//! and inherit down namespace.

use super::primvar::Primvar;
use super::tokens::usd_geom_tokens;
use usd_core::{Attribute, Prim, SchemaBase};
use usd_sdf::ValueTypeName;
use usd_tf::Token;

// ============================================================================
// PrimvarsAPI
// ============================================================================

/// API schema for geometric primitive variables.
///
/// Matches C++ `UsdGeomPrimvarsAPI`.
#[derive(Debug, Clone)]
pub struct PrimvarsAPI {
    /// Base schema.
    inner: SchemaBase,
}

impl PrimvarsAPI {
    /// Creates a PrimvarsAPI schema from a prim.
    ///
    /// Matches C++ `UsdGeomPrimvarsAPI(const UsdPrim& prim)`.
    pub fn new(prim: Prim) -> Self {
        Self {
            inner: SchemaBase::new(prim),
        }
    }

    /// Creates a PrimvarsAPI schema from a SchemaBase.
    ///
    /// Matches C++ `UsdGeomPrimvarsAPI(const UsdSchemaBase& schemaObj)`.
    pub fn from_schema_base(schema: SchemaBase) -> Self {
        Self { inner: schema }
    }

    /// Creates an invalid PrimvarsAPI schema.
    pub fn invalid() -> Self {
        Self {
            inner: SchemaBase::invalid(),
        }
    }

    /// Returns true if this schema is valid.
    pub fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    /// Returns the wrapped prim.
    pub fn prim(&self) -> &Prim {
        self.inner.prim()
    }

    // ========================================================================
    // Create / Get single primvar
    // ========================================================================

    /// Author scene description to create an attribute on this prim that
    /// will be recognized as a Primvar.
    ///
    /// Matches C++ `CreatePrimvar()`.
    pub fn create_primvar(
        &self,
        name: &Token,
        type_name: &ValueTypeName,
        interpolation: Option<&Token>,
        element_size: i32,
    ) -> Primvar {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Primvar::new(usd_core::Attribute::invalid());
        }

        // Build full primvar attribute name (with "primvars:" namespace)
        let attr_name = Primvar::make_namespaced(name);
        if attr_name.as_str().is_empty() {
            return Primvar::new(usd_core::Attribute::invalid());
        }

        // Get or create the attribute
        let attr = if prim.has_authored_attribute(attr_name.as_str()) {
            prim.get_attribute(attr_name.as_str())
                .unwrap_or_else(|| Attribute::invalid())
        } else {
            // Create new attribute with proper type and variability
            match prim.create_attribute(
                attr_name.as_str(),
                type_name,
                false, // not custom (primvars are schema-defined)
                Some(usd_core::attribute::Variability::Varying),
            ) {
                Some(a) => a,
                None => return Primvar::new(usd_core::Attribute::invalid()),
            }
        };

        let pv = Primvar::new(attr);

        // Set interpolation metadata if provided
        if let Some(interp) = interpolation {
            if !interp.as_str().is_empty() {
                pv.set_interpolation(interp);
            }
        }

        // Set elementSize metadata if provided
        if element_size > 0 {
            pv.set_element_size(element_size);
        }

        pv
    }

    /// Get a primvar by name.
    ///
    /// Matches C++ `GetPrimvar()`.
    pub fn get_primvar(&self, name: &Token) -> Primvar {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Primvar::new(usd_core::Attribute::invalid());
        }

        let attr_name = Primvar::make_namespaced(name);
        if attr_name.as_str().is_empty() {
            return Primvar::new(usd_core::Attribute::invalid());
        }

        let attr = prim
            .get_attribute(attr_name.as_str())
            .unwrap_or_else(usd_core::Attribute::invalid);
        Primvar::new(attr)
    }

    // ========================================================================
    // Remove / Block
    // ========================================================================

    /// Remove a primvar and its indices attribute from the current edit target.
    ///
    /// Matches C++ `RemovePrimvar()`.
    pub fn remove_primvar(&self, name: &Token) -> bool {
        let attr_name = Primvar::make_namespaced(name);
        if attr_name.as_str().is_empty() {
            return false;
        }

        let prim = self.inner.prim();
        if !prim.is_valid() {
            log::error!("RemovePrimvar called on invalid prim");
            return false;
        }

        // Get the primvar to check for indices
        let attr = match prim.get_attribute(attr_name.as_str()) {
            Some(a) => a,
            None => return false,
        };
        let primvar = Primvar::new(attr);
        if !primvar.is_valid() {
            return false;
        }

        // If indexed, also remove the indices attribute
        let mut success = true;
        if let Some(idx_attr) = primvar.get_indices_attr() {
            success = prim.remove_property(idx_attr.name().as_str());
        }

        prim.remove_property(attr_name.as_str()) && success
    }

    /// Block a primvar's value and its indices, hiding weaker opinions.
    ///
    /// Matches C++ `BlockPrimvar()`.
    pub fn block_primvar(&self, name: &Token) {
        let attr_name = Primvar::make_namespaced(name);
        if attr_name.as_str().is_empty() {
            return;
        }

        let prim = self.inner.prim();
        if !prim.is_valid() {
            log::error!("BlockPrimvar called on invalid prim");
            return;
        }

        let attr = match prim.get_attribute(attr_name.as_str()) {
            Some(a) => a,
            None => return,
        };
        let primvar = Primvar::new(attr);
        if !primvar.is_valid() {
            return;
        }

        // Always block indices to prevent leaking from weaker layers
        primvar.block_indices();
        primvar.get_attr().block();
    }

    // ========================================================================
    // Query: has / get collections
    // ========================================================================

    /// Returns true if a primvar named `name` is defined on this prim.
    ///
    /// Matches C++ `HasPrimvar()`.
    pub fn has_primvar(&self, name: &Token) -> bool {
        let attr_name = Primvar::make_namespaced_quiet(name, true);
        if attr_name.as_str().is_empty() {
            return false;
        }
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return false;
        }
        match prim.get_attribute(attr_name.as_str()) {
            Some(attr) => Primvar::is_primvar(&attr),
            None => false,
        }
    }

    /// Returns true if a primvar named `name` exists on this prim or an ancestor
    /// (with constant interpolation).
    ///
    /// Matches C++ `HasPossiblyInheritedPrimvar()`.
    pub fn has_possibly_inherited_primvar(&self, name: &Token) -> bool {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return false;
        }

        // Check local first
        let pv = self.get_primvar(name);
        if pv.has_authored_value() {
            return true;
        }

        let attr_name = Primvar::make_namespaced(name);
        if attr_name.as_str().is_empty() {
            return false;
        }

        // Walk ancestors
        let mut cur = prim.parent();
        while cur.is_valid() && !cur.is_pseudo_root() {
            if let Some(attr) = cur.get_attribute(attr_name.as_str()) {
                if attr.has_authored_value() && Primvar::is_primvar(&attr) {
                    let inherited_pv = Primvar::new(attr);
                    // Only constant interpolation can be inherited
                    return inherited_pv.get_interpolation() == usd_geom_tokens().constant;
                }
            }
            cur = cur.parent();
        }
        false
    }

    /// Return all primvars defined on this prim (including those without values).
    ///
    /// Matches C++ `GetPrimvars()`.
    pub fn get_primvars(&self) -> Vec<Primvar> {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Vec::new();
        }
        self.make_primvars_from_props(prim, |_| true)
    }

    /// Return only primvars with some authored scene description.
    ///
    /// Matches C++ `GetAuthoredPrimvars()`.
    pub fn get_authored_primvars(&self) -> Vec<Primvar> {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Vec::new();
        }
        // get_authored_properties_in_namespace returns only authored ones
        self.make_primvars_from_authored_props(prim, |_| true)
    }

    /// Return primvars that have a value (authored or fallback).
    ///
    /// Matches C++ `GetPrimvarsWithValues()`.
    pub fn get_primvars_with_values(&self) -> Vec<Primvar> {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Vec::new();
        }
        self.make_primvars_from_authored_props(prim, |pv| pv.has_value())
    }

    /// Return primvars that have an authored value.
    ///
    /// Matches C++ `GetPrimvarsWithAuthoredValues()`.
    pub fn get_primvars_with_authored_values(&self) -> Vec<Primvar> {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Vec::new();
        }
        self.make_primvars_from_authored_props(prim, |pv| pv.has_authored_value())
    }

    // ========================================================================
    // Inheritance
    // ========================================================================

    /// Compute all inheritable primvars by recursing up to the root.
    /// Only constant-interpolation primvars with authored values are inheritable.
    ///
    /// Matches C++ `FindInheritablePrimvars()`.
    pub fn find_inheritable_primvars(&self) -> Vec<Primvar> {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Vec::new();
        }

        let mut primvars = Vec::new();
        let prefix = Primvar::namespace_prefix();
        Self::recurse_for_inheritable_primvars(&prim, &prefix, &mut primvars, false);
        primvars
    }

    /// Compute inheritable primvars incrementally from a pre-computed ancestor set.
    /// Returns empty vec if this prim contributes no changes (caller should reuse
    /// `inherited_from_ancestors` for children).
    ///
    /// Matches C++ `FindIncrementallyInheritablePrimvars()`.
    pub fn find_incrementally_inheritable_primvars(
        &self,
        inherited_from_ancestors: &[Primvar],
    ) -> Vec<Primvar> {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Vec::new();
        }

        let prefix = Primvar::namespace_prefix();
        let mut output = Vec::new();
        Self::add_prim_to_inherited_primvars(
            &prim,
            &prefix,
            inherited_from_ancestors,
            &mut output,
            false,
        );
        output
    }

    /// Find a single primvar by name, searching ancestors if not locally authored.
    ///
    /// Matches C++ `FindPrimvarWithInheritance(name)`.
    pub fn find_primvar_with_inheritance(&self, name: &Token) -> Primvar {
        let attr_name = Primvar::make_namespaced(name);
        let prim = self.inner.prim();
        if !prim.is_valid() || attr_name.as_str().is_empty() {
            return Primvar::new(usd_core::Attribute::invalid());
        }

        // Check local first
        let local_pv = self.get_primvar(name);
        if local_pv.has_authored_value() {
            return local_pv;
        }

        // Walk ancestors
        let tokens = usd_geom_tokens();
        let mut cur = prim.parent();
        while cur.is_valid() && !cur.is_pseudo_root() {
            if let Some(attr) = cur.get_attribute(attr_name.as_str()) {
                if attr.has_authored_value() {
                    let pv = Primvar::new(attr);
                    if pv.is_valid() {
                        // Only constant interpolation can be inherited
                        if pv.get_interpolation() == tokens.constant {
                            return pv;
                        } else {
                            // Non-constant blocks inheritance
                            return Primvar::new(usd_core::Attribute::invalid());
                        }
                    }
                }
            }
            cur = cur.parent();
        }

        // Return local (possibly invalid/valueless) if no ancestor had it
        local_pv
    }

    /// Find a single primvar by name using a pre-computed inherited set.
    ///
    /// Matches C++ `FindPrimvarWithInheritance(name, inheritedFromAncestors)`.
    pub fn find_primvar_with_inheritance_from(
        &self,
        name: &Token,
        inherited_from_ancestors: &[Primvar],
    ) -> Primvar {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Primvar::new(usd_core::Attribute::invalid());
        }

        let attr_name = Primvar::make_namespaced(name);
        // Check local first (use the already-namespaced name for GetPrimvar)
        let pv = self.get_primvar(&attr_name);
        if pv.has_authored_value() {
            return pv;
        }

        // Search in inherited set
        for inherited in inherited_from_ancestors {
            if inherited.get_name() == attr_name {
                return inherited.clone();
            }
        }

        pv
    }

    /// Find all value-producing primvars on this prim and ancestors.
    ///
    /// Matches C++ `FindPrimvarsWithInheritance()`.
    pub fn find_primvars_with_inheritance(&self) -> Vec<Primvar> {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Vec::new();
        }

        let mut primvars = Vec::new();
        let prefix = Primvar::namespace_prefix();
        Self::recurse_for_inheritable_primvars(&prim, &prefix, &mut primvars, true);
        primvars
    }

    /// Find all value-producing primvars using a pre-computed inherited set.
    ///
    /// Matches C++ `FindPrimvarsWithInheritance(inheritedFromAncestors)`.
    pub fn find_primvars_with_inheritance_from(
        &self,
        inherited_from_ancestors: &[Primvar],
    ) -> Vec<Primvar> {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Vec::new();
        }

        let prefix = Primvar::namespace_prefix();
        let mut output = Vec::new();
        Self::add_prim_to_inherited_primvars(
            &prim,
            &prefix,
            inherited_from_ancestors,
            &mut output,
            true,
        );

        // If this prim contributed nothing, return the ancestor set
        if output.is_empty() {
            inherited_from_ancestors.to_vec()
        } else {
            output
        }
    }

    // ========================================================================
    // Static utilities
    // ========================================================================

    /// Test whether a given name contains the "primvars:" prefix.
    ///
    /// Matches C++ `CanContainPropertyName()`.
    pub fn can_contain_property_name(name: &Token) -> bool {
        name.as_str().starts_with("primvars:")
    }

    // ========================================================================
    // Private helpers
    // ========================================================================

    /// Filter properties in primvar namespace into Primvar objects.
    fn make_primvars_from_props(
        &self,
        prim: &Prim,
        filter: impl Fn(&Primvar) -> bool,
    ) -> Vec<Primvar> {
        let prefix = Primvar::namespace_prefix();
        let props = prim.get_properties_in_namespace(&prefix);
        let mut result = Vec::with_capacity(props.len());
        for prop in props {
            if let Some(attr) = prop.as_attribute() {
                let pv = Primvar::new(attr);
                if pv.is_valid() && filter(&pv) {
                    result.push(pv);
                }
            }
        }
        result
    }

    /// Filter authored properties in primvar namespace into Primvar objects.
    fn make_primvars_from_authored_props(
        &self,
        prim: &Prim,
        filter: impl Fn(&Primvar) -> bool,
    ) -> Vec<Primvar> {
        let prefix = Primvar::namespace_prefix();
        let props = prim.get_authored_properties_in_namespace(&prefix);
        let mut result = Vec::with_capacity(props.len());
        for prop in props {
            if let Some(attr) = prop.as_attribute() {
                let pv = Primvar::new(attr);
                if pv.is_valid() && filter(&pv) {
                    result.push(pv);
                }
            }
        }
        result
    }

    /// Recursively walk ancestors, collecting inheritable primvars.
    /// If `accept_all` is true, all interpolations are accepted (for FindPrimvarsWithInheritance).
    fn recurse_for_inheritable_primvars(
        prim: &Prim,
        prefix: &Token,
        primvars: &mut Vec<Primvar>,
        accept_all: bool,
    ) {
        if prim.is_pseudo_root() {
            return;
        }

        // Recurse to parent first (ancestors only with accept_all=false)
        let parent = prim.parent();
        if parent.is_valid() {
            Self::recurse_for_inheritable_primvars(&parent, prefix, primvars, false);
        }

        // Then add this prim's contributions
        let snapshot: Vec<Primvar> = primvars.clone();
        let mut output = Vec::new();
        Self::add_prim_to_inherited_primvars(prim, prefix, &snapshot, &mut output, accept_all);
        if !output.is_empty() {
            *primvars = output;
        }
    }

    /// Add a prim's primvars to the inherited set, replacing/removing as needed.
    ///
    /// Port of C++ `_AddPrimToInheritedPrimvars()`.
    fn add_prim_to_inherited_primvars(
        prim: &Prim,
        prefix: &Token,
        input: &[Primvar],
        output: &mut Vec<Primvar>,
        accept_all: bool,
    ) {
        let tokens = usd_geom_tokens();
        let props = prim.get_authored_properties_in_namespace(prefix);

        // Track whether we've copied input -> output yet (copy-on-write)
        let mut copied = false;

        for prop in &props {
            let Some(attr) = prop.as_attribute() else {
                continue;
            };
            let pv = Primvar::new(attr);
            if !pv.is_valid() || !pv.has_authored_value() {
                continue;
            }

            let pv_name = pv.get_name();
            let pv_is_constant = pv.get_interpolation() == tokens.constant;

            // Search for existing entry with same name
            let mut found = false;
            for i in 0..input.len() {
                if pv_name == input[i].get_name() {
                    // Ensure output is a copy of input
                    if !copied {
                        *output = input.to_vec();
                        copied = true;
                    }
                    found = true;
                    if pv_is_constant || accept_all {
                        // Replace with this prim's version
                        output[i] = pv.clone();
                    } else {
                        // Non-constant: remove from set (swap-remove for efficiency)
                        output.swap_remove(i);
                    }
                    break;
                }
            }

            if !found && (pv_is_constant || accept_all) {
                if !copied {
                    *output = input.to_vec();
                    copied = true;
                }
                output.push(pv);
            }
        }
    }
}
