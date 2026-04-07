//! UsdGeomImageable - base class for all renderable prims.
//!
//! Port of pxr/usd/usdGeom/imageable.h/cpp
//!
//! Base class for all prims that may require rendering or visualization.
//! The primary attributes are visibility and purpose.

use super::bbox_cache::BBoxCache;
use super::tokens::usd_geom_tokens;
use super::visibility_api::VisibilityAPI;
use super::xform_cache::XformCache;
use usd_core::{Attribute, Prim, Relationship, Typed};
use usd_gf::bbox3d::BBox3d;
use usd_gf::matrix4::Matrix4d;
use usd_sdf::TimeCode;
use usd_tf::Token;

/// Check if the given type name is an Imageable-derived schema type.
///
/// Matches the C++ TfType::IsA<UsdGeomImageable>() check that validates
/// whether a prim type derives from Imageable in the schema hierarchy.
fn is_imageable_type(type_name: &str) -> bool {
    matches!(
        type_name,
        "Imageable"
            | "Scope"
            | "Xformable"
            | "Xform"
            | "Camera"
            | "Boundable"
            | "Gprim"
            | "Mesh"
            | "Points"
            | "PointBased"
            | "Curves"
            | "BasisCurves"
            | "NurbsCurves"
            | "NurbsPatch"
            | "HermiteCurves"
            | "TetMesh"
            | "Sphere"
            | "Cube"
            | "Cylinder"
            | "Cylinder_1"
            | "Cone"
            | "Capsule"
            | "Capsule_1"
            | "Plane"
            | "Volume"
            | "FieldBase"
            | "OpenVDBAsset"
            | "Field3DAsset"
            | "SkelRoot"
            | "Skeleton"
            | "PointInstancer"
            | "GeomSubset"
            // UsdLux light types (derive from BoundableLightBase -> Boundable -> Xformable -> Imageable)
            | "RectLight"
            | "SphereLight"
            | "DiskLight"
            | "CylinderLight"
            | "PortalLight"
            | "GeometryLight"
            | "PluginLight"
            | "BoundableLightBase"
            // UsdLux non-boundable lights (derive from NonboundableLightBase -> Xformable -> Imageable)
            | "DistantLight"
            | "DomeLight"
            | "DomeLight_1"
            | "NonboundableLightBase"
            // UsdLux light filter (derives from Xformable -> Imageable)
            | "LightFilter"
    )
}

/// Recursively compute visibility walking up the prim hierarchy.
///
/// Matches C++ `_ComputeVisibility()` -- recurses through ALL parents regardless
/// of whether they are Imageable, only checking visibility on Imageable prims.
fn compute_visibility_recursive(prim: &Prim, time: TimeCode) -> Token {
    // Only check visibility on prims with Imageable-derived types
    if is_imageable_type(prim.type_name().as_str()) {
        // Read attr directly to avoid auto-creation side effect
        if let Some(vis_attr) = prim.get_attribute(usd_geom_tokens().visibility.as_str()) {
            if let Some(vis_value) = vis_attr.get(time) {
                if let Some(token) = vis_value.downcast::<Token>() {
                    if *token == usd_geom_tokens().invisible {
                        return usd_geom_tokens().invisible.clone();
                    }
                }
            }
        }
    }

    // Recurse to parent (C++ stops when GetParent() returns invalid)
    let parent = prim.parent();
    if parent.is_valid() {
        return compute_visibility_recursive(&parent, time);
    }

    usd_geom_tokens().inherited.clone()
}

// ============================================================================
// Imageable
// ============================================================================

/// Base class for all prims that may require rendering or visualization.
///
/// The primary attributes of Imageable are visibility and purpose, which
/// provide instructions for what geometry should be included for processing
/// by rendering and other computations.
///
/// Matches C++ `UsdGeomImageable`.
#[derive(Debug, Clone)]
pub struct Imageable {
    /// Base typed schema.
    inner: Typed,
}

impl Imageable {
    /// Creates an Imageable schema from a prim.
    ///
    /// Matches C++ `UsdGeomImageable(const UsdPrim& prim)`.
    pub fn new(prim: Prim) -> Self {
        Self {
            inner: Typed::new(prim),
        }
    }

    /// Creates an Imageable schema from a Typed schema.
    ///
    /// Matches C++ `UsdGeomImageable(const UsdSchemaBase& schemaObj)`.
    pub fn from_typed(typed: Typed) -> Self {
        Self { inner: typed }
    }

    /// Creates an invalid Imageable schema.
    pub fn invalid() -> Self {
        Self {
            inner: Typed::invalid(),
        }
    }

    /// Returns true if this schema wraps a valid prim with an Imageable-derived type.
    ///
    /// In C++ this is handled by TfType::IsA<UsdGeomImageable>() -- a prim with
    /// no type or a non-Imageable type (e.g. "") is not a valid Imageable.
    pub fn is_valid(&self) -> bool {
        self.inner.is_valid() && is_imageable_type(self.inner.prim().type_name().as_str())
    }

    /// Returns the wrapped prim.
    pub fn prim(&self) -> &Prim {
        self.inner.prim()
    }

    /// Returns the schema type name.
    pub fn schema_type_name() -> Token {
        Token::new("Imageable")
    }

    // ========================================================================
    // Visibility
    // ========================================================================

    /// Returns the visibility attribute without authoring into the stage.
    ///
    /// `_ref` `GetVisibilityAttr()` is a pure read accessor. It returns the
    /// schema attribute handle if present through schema/prim definition, but it
    /// does not create authored specs as a side effect. Authoring belongs to
    /// `CreateVisibilityAttr()`, not to the getter.
    ///
    /// Matches C++ `GetVisibilityAttr()`.
    pub fn get_visibility_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().visibility.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the visibility attribute.
    ///
    /// Matches C++ `CreateVisibilityAttr()`.
    pub fn create_visibility_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        // Return existing attr if it has a spec, otherwise create it.
        if prim.has_authored_attribute(usd_geom_tokens().visibility.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().visibility.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = usd_sdf::ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        prim.create_attribute(
            usd_geom_tokens().visibility.as_str(),
            &token_type,
            false,                                           // not custom
            Some(usd_core::attribute::Variability::Varying), // visibility can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // Purpose
    // ========================================================================

    /// Returns the purpose attribute without authoring into the stage.
    ///
    /// Like visibility, `_ref` `GetPurposeAttr()` is a pure getter. Creating an
    /// authored spec here is incorrect and breaks read-only backends such as
    /// Alembic.
    ///
    /// Matches C++ `GetPurposeAttr()`.
    pub fn get_purpose_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().purpose.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the purpose attribute.
    ///
    /// Matches C++ `CreatePurposeAttr()`.
    pub fn create_purpose_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        // Get or create the attribute with proper type (Token) and variability (Uniform)
        if prim.has_authored_attribute(usd_geom_tokens().purpose.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().purpose.as_str())
                .unwrap_or_else(Attribute::invalid);
        }

        let registry = usd_sdf::ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        prim.create_attribute(
            usd_geom_tokens().purpose.as_str(),
            &token_type,
            false,                                           // not custom
            Some(usd_core::attribute::Variability::Uniform), // purpose is uniform
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // ProxyPrim
    // ========================================================================

    /// Returns the proxyPrim relationship.
    ///
    /// The proxyPrim relationship allows linking a prim whose purpose is
    /// "render" to its (single target) purpose="proxy" prim.
    ///
    /// Matches C++ `GetProxyPrimRel()`.
    pub fn get_proxy_prim_rel(&self) -> Relationship {
        let prim = self.inner.prim();
        prim.get_relationship(usd_geom_tokens().proxy_prim.as_str())
            .unwrap_or_else(Relationship::invalid)
    }

    /// Creates the proxyPrim relationship.
    ///
    /// Matches C++ `CreateProxyPrimRel()`.
    pub fn create_proxy_prim_rel(&self) -> Relationship {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Relationship::invalid();
        }

        // Get or create the relationship
        if let Some(rel) = prim.get_relationship(usd_geom_tokens().proxy_prim.as_str()) {
            return rel;
        }

        prim.create_relationship(usd_geom_tokens().proxy_prim.as_str(), false)
            .unwrap_or_else(Relationship::invalid)
    }

    // ========================================================================
    // Static Methods
    // ========================================================================

    /// Returns an ordered list of allowed values of the purpose attribute.
    ///
    /// The order is: [default, render, proxy, guide]
    ///
    /// Matches C++ `GetOrderedPurposeTokens()`.
    pub fn get_ordered_purpose_tokens() -> Vec<Token> {
        vec![
            usd_geom_tokens().default_.clone(),
            usd_geom_tokens().render.clone(),
            usd_geom_tokens().proxy.clone(),
            usd_geom_tokens().guide.clone(),
        ]
    }

    // ========================================================================
    // Visibility Authoring Helpers
    // ========================================================================

    /// Makes the imageable visible if it is invisible at the given time.
    ///
    /// Since visibility is pruning, this may need to override some
    /// ancestor's visibility and all-but-one of the ancestor's children's
    /// visibility, for all the ancestors of this prim up to the highest
    /// ancestor that is explicitly invisible, to preserve the visibility state.
    ///
    /// Matches C++ `MakeVisible()`.
    pub fn make_visible(&self, time: TimeCode) {
        // C++ imageable.cpp:336-338: handles SELF FIRST, then ancestors.
        // This order matters because the ancestor processing may check
        // this prim's visibility state.
        self.set_inherited_if_invisible(time);

        let mut has_invisible_ancestor = false;
        self.make_visible_recursive(self.inner.prim(), time, &mut has_invisible_ancestor);
    }

    /// Internal helper: Returns true if the imageable has its visibility set to 'invisible'
    /// at the given time. It also sets the visibility to inherited before returning.
    fn set_inherited_if_invisible(&self, time: TimeCode) -> bool {
        let vis_attr = self.get_visibility_attr();
        if vis_attr.is_valid() {
            if let Some(vis_value) = vis_attr.get(time) {
                if let Some(token) = vis_value.downcast::<Token>() {
                    if *token == usd_geom_tokens().invisible {
                        let create_vis_attr = self.create_visibility_attr();
                        let token_value = usd_vt::Value::new(usd_geom_tokens().inherited.clone());
                        create_vis_attr.set(token_value, time);
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Internal helper: Recursively make visible, handling ancestors.
    fn make_visible_recursive(
        &self,
        prim: &Prim,
        time: TimeCode,
        has_invisible_ancestor: &mut bool,
    ) {
        let parent = prim.parent();
        if parent.is_valid() {
            let parent_imageable = Imageable::new(parent.clone());
            if parent_imageable.is_valid() {
                self.make_visible_recursive(&parent, time, has_invisible_ancestor);

                // Change visibility of parent to inherited if it is invisible.
                if parent_imageable.set_inherited_if_invisible(time) || *has_invisible_ancestor {
                    *has_invisible_ancestor = true;

                    // Invis all siblings of prim.
                    // Use GetAllChildren to get all siblings
                    let parent = prim.parent();
                    if parent.is_valid() {
                        let siblings = parent.get_all_children();
                        let prim_path = prim.path();
                        for sibling in siblings {
                            if sibling.path() != prim_path {
                                // C++ imageable.cpp:321-324: wraps child as
                                // UsdGeomImageable, checks validity, then uses
                                // _SetVisibility with Token (not String).
                                let sibling_imageable = Imageable::new(sibling.clone());
                                if sibling_imageable.is_valid() {
                                    let vis_attr = sibling_imageable.create_visibility_attr();
                                    let token_value =
                                        usd_vt::Value::new(usd_geom_tokens().invisible.clone());
                                    let _ = vis_attr.set(token_value, time);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Makes the imageable invisible if it is visible at the given time.
    ///
    /// Matches C++ `MakeInvisible()`.
    pub fn make_invisible(&self, time: TimeCode) {
        let vis_attr = self.create_visibility_attr();
        // Only set if not already invisible
        if let Some(vis_value) = vis_attr.get(time) {
            if let Some(token) = vis_value.downcast::<Token>() {
                if *token == usd_geom_tokens().invisible {
                    return; // Already invisible
                }
            }
        }
        let token_value = usd_vt::Value::new(usd_geom_tokens().invisible.clone());
        vis_attr.set(token_value, time);
    }

    // ========================================================================
    // Computed Attribute Helpers
    // ========================================================================

    /// Calculate the effective visibility of this prim.
    ///
    /// A prim is considered visible if none of its Imageable ancestors
    /// express an authored "invisible" opinion.
    ///
    /// Matches C++ `ComputeVisibility()`.
    pub fn compute_visibility(&self, time: TimeCode) -> Token {
        compute_visibility_recursive(self.inner.prim(), time)
    }

    /// Calculate the effective purpose of this prim.
    ///
    /// C++ `_ComputeAuthoredPurpose` + `ComputePurpose()` (imageable.cpp:354-404):
    /// Only returns a purpose if the attribute has an AUTHORED value, not a
    /// schema fallback. Without this check, every prim returns the fallback
    /// "default" and never inherits purpose from ancestors.
    pub fn compute_purpose(&self) -> Token {
        // Only consider purpose attr if THIS prim is actually imageable
        if self.is_valid() {
            let purpose_attr = self.get_purpose_attr();
            // C++ checks HasAuthoredValue() -- only use explicit opinions, not fallbacks
            if purpose_attr.is_valid() && purpose_attr.has_authored_value() {
                if let Some(purpose_value) = purpose_attr.get(usd_sdf::TimeCode::default()) {
                    if let Some(token) = purpose_value.downcast::<Token>() {
                        if !token.is_empty() {
                            return token.clone();
                        }
                    }
                }
            }
        }

        // No authored purpose (or non-imageable) -- inherit from parent.
        // Walk through ALL parents, not just imageable ones.
        let prim = self.inner.prim();
        let parent = prim.parent();
        if parent.is_valid() {
            let parent_imageable = Imageable::new(parent);
            return parent_imageable.compute_purpose();
        }

        usd_geom_tokens().default_.clone()
    }

    /// Calculate the effective purpose information about this prim.
    ///
    /// Matches C++ `ComputePurposeInfo()`.
    pub fn compute_purpose_info(&self) -> PurposeInfo {
        // Only consider authored purpose if THIS prim is actually imageable
        let mut authored_purpose = Token::new("");

        if self.is_valid() {
            let purpose_attr = self.get_purpose_attr();
            // Only use explicit opinions, not schema fallbacks
            if purpose_attr.is_valid() && purpose_attr.has_authored_value() {
                if let Some(purpose_value) = purpose_attr.get(usd_sdf::TimeCode::default()) {
                    if let Some(token) = purpose_value.downcast::<Token>() {
                        if !token.is_empty() {
                            authored_purpose = token.clone();
                        }
                    }
                }
            }
        }

        if authored_purpose.is_empty() {
            // Walk through ALL parents (not just imageable ones) to find
            // inheritable purpose from the nearest imageable ancestor.
            let prim = self.inner.prim();
            let parent = prim.parent();
            if parent.is_valid() {
                let parent_imageable = Imageable::new(parent);
                let parent_purpose_info = parent_imageable.compute_purpose_info();
                if parent_purpose_info.is_inheritable {
                    return parent_purpose_info;
                }
            }
            // Return fallback purpose
            return PurposeInfo::new(usd_geom_tokens().default_.clone(), false);
        }

        PurposeInfo::new(authored_purpose, true)
    }

    /// Calculates the effective purpose information given the computed purpose
    /// information of its parent prim.
    ///
    /// Matches C++ `ComputePurposeInfo(const PurposeInfo &parentPurposeInfo)`.
    pub fn compute_purpose_info_with_parent(
        &self,
        parent_purpose_info: &PurposeInfo,
    ) -> PurposeInfo {
        // Check for an authored purpose opinion first (only explicit, not schema fallback)
        let purpose_attr = self.get_purpose_attr();
        let mut authored_purpose = Token::new("");

        if purpose_attr.is_valid() && purpose_attr.has_authored_value() {
            if let Some(purpose_value) = purpose_attr.get(usd_sdf::TimeCode::default()) {
                if let Some(token) = purpose_value.downcast::<Token>() {
                    if !token.is_empty() {
                        authored_purpose = token.clone();
                    }
                }
            }
        }

        if authored_purpose.is_empty() {
            if parent_purpose_info.is_inheritable {
                return parent_purpose_info.clone();
            } else {
                // Return fallback purpose
                return PurposeInfo::new(usd_geom_tokens().default_.clone(), false);
            }
        }

        PurposeInfo::new(authored_purpose, true)
    }

    /// Return the attribute that is used for expressing visibility opinions
    /// for the given purpose.
    ///
    /// For "default" purpose, return the overall visibility attribute.
    /// For "guide", "proxy", or "render" purpose, return guideVisibility,
    /// proxyVisibility, or renderVisibility if UsdGeomVisibilityAPI is
    /// applied to the prim. If UsdGeomVisibilityAPI is not applied, an
    /// empty attribute is returned for purposes other than default.
    ///
    /// Matches C++ `GetPurposeVisibilityAttr()`.
    pub fn get_purpose_visibility_attr(&self, purpose: &Token) -> Attribute {
        if *purpose == usd_geom_tokens().default_ {
            return self.get_visibility_attr();
        }

        // Check for UsdGeomVisibilityAPI and return purpose visibility attr
        let prim = self.inner.prim();
        // Check if VisibilityAPI is applied to the prim
        if prim.has_api(&Token::new("VisibilityAPI")) {
            let visibility_api = VisibilityAPI::new(prim.clone());
            return visibility_api.get_purpose_visibility_attr(purpose);
        }

        // Return invalid attribute if VisibilityAPI is not available
        Attribute::invalid()
    }

    /// Calculate the effective purpose visibility of this prim for the
    /// given purpose, taking into account opinions for the corresponding
    /// purpose attribute, along with overall visibility opinions.
    ///
    /// Matches C++ `ComputeEffectiveVisibility()`.
    pub fn compute_effective_visibility(&self, purpose: &Token, time: TimeCode) -> Token {
        // If overall visibility is invisible, effective purpose visibility is invisible.
        if self.compute_visibility(time) == usd_geom_tokens().invisible {
            return usd_geom_tokens().invisible.clone();
        }

        // Default visibility is entirely determined by overall visibility
        if *purpose == usd_geom_tokens().default_ {
            return usd_geom_tokens().visible.clone();
        }

        // Compute purpose visibility recursively
        self.compute_purpose_visibility(self.inner.prim(), purpose, time)
    }

    /// Internal helper to compute purpose visibility recursively.
    ///
    /// C++ imageable.cpp:231: checks `attr.HasAuthoredValue()` before `attr.Get()`.
    /// Without this, fallback values ("visible") block inheritance from ancestors,
    /// because `attr.get()` returns schema defaults even when not authored.
    fn compute_purpose_visibility(&self, prim: &Prim, purpose: &Token, time: TimeCode) -> Token {
        let imageable = Imageable::new(prim.clone());
        if imageable.is_valid() {
            let attr = imageable.get_purpose_visibility_attr(purpose);
            // C++ checks HasAuthoredValue() -- only return explicitly authored opinions
            if attr.is_valid() && attr.has_authored_value() {
                if let Some(vis_value) = attr.get(time) {
                    if let Some(token) = vis_value.downcast::<Token>() {
                        if !token.is_empty() {
                            return token.clone();
                        }
                    }
                }
            }
        }

        // Otherwise, we inherit purpose visibility from the parent.
        let parent = prim.parent();
        if parent.is_valid() {
            return self.compute_purpose_visibility(&parent, purpose, time);
        }

        // If we don't have an authored opinion and we don't have a parent,
        // return a fallback value, depending on the purpose.
        if *purpose == usd_geom_tokens().guide {
            return usd_geom_tokens().invisible.clone();
        }
        if *purpose == usd_geom_tokens().proxy || *purpose == usd_geom_tokens().render {
            return usd_geom_tokens().inherited.clone();
        }

        // Unexpected purpose - return invisible
        usd_geom_tokens().invisible.clone()
    }

    /// Find the prim whose purpose is proxy that serves as the proxy
    /// for this prim, as established by the GetProxyPrimRel().
    ///
    /// Matches C++ `ComputeProxyPrim()`.
    pub fn compute_proxy_prim(&self) -> Option<(Prim, Prim)> {
        let self_prim = self.inner.prim();
        let mut render_root: Option<Prim> = None;
        let mut prim = self_prim.clone();

        // Walk up the parent chain until we find the last prim that still has render purpose
        let mut current_purpose = self.compute_purpose();
        while current_purpose == usd_geom_tokens().render {
            render_root = Some(prim.clone());
            let parent = prim.parent();
            if !parent.is_valid() {
                break;
            }
            let parent_imageable = Imageable::new(parent.clone());
            if !parent_imageable.is_valid() {
                break;
            }
            current_purpose = parent_imageable.compute_purpose();
            prim = parent;
        }

        if let Some(render_root_prim) = render_root {
            let render_root_imageable = Imageable::new(render_root_prim.clone());
            let proxy_prim_rel = render_root_imageable.get_proxy_prim_rel();
            if proxy_prim_rel.is_valid() {
                let targets = proxy_prim_rel.get_forwarded_targets();
                if targets.len() == 1 {
                    if let Some(stage) = self_prim.stage() {
                        if let Some(proxy_prim) = stage.get_prim_at_path(&targets[0]) {
                            // Verify proxy prim has proxy purpose
                            let proxy_imageable = Imageable::new(proxy_prim.clone());
                            let proxy_purpose = proxy_imageable.compute_purpose();
                            if proxy_purpose == usd_geom_tokens().proxy {
                                return Some((proxy_prim, render_root_prim));
                            }
                        }
                    }
                } else if targets.len() > 1 {
                    // Warning: multiple targets
                }
            }
        }

        None
    }

    /// Convenience function for authoring the proxyPrim rel on this
    /// prim to target the given proxy prim.
    ///
    /// Matches C++ `SetProxyPrim(const UsdPrim &proxy)`.
    pub fn set_proxy_prim(&self, proxy: &Prim) -> bool {
        if proxy.is_valid() {
            let targets = vec![proxy.path().clone()];
            return self.create_proxy_prim_rel().set_targets(&targets);
        }
        false
    }

    /// Convenience function for authoring the proxyPrim rel on this
    /// prim to target the given proxy prim (from schema base).
    ///
    /// Matches C++ `SetProxyPrim(const UsdSchemaBase &proxy)`.
    pub fn set_proxy_prim_from_schema(&self, proxy_prim: &Prim) -> bool {
        self.set_proxy_prim(proxy_prim)
    }

    /// Convenience function for authoring the proxyPrim rel on this
    /// prim to target the given proxy prim (from schema base).
    ///
    /// Matches C++ `SetProxyPrim(const UsdSchemaBase &proxy)` overload.
    pub fn set_proxy_prim_from_typed(&self, proxy_typed: &usd_core::Typed) -> bool {
        self.set_proxy_prim(proxy_typed.prim())
    }

    /// Compute the bound of this prim in world space, at the specified
    /// time, and for the specified purposes.
    ///
    /// Matches C++ `ComputeWorldBound()`.
    pub fn compute_world_bound(
        &self,
        time: TimeCode,
        purpose1: Option<&Token>,
        purpose2: Option<&Token>,
        purpose3: Option<&Token>,
        purpose4: Option<&Token>,
    ) -> BBox3d {
        let mut purposes = Vec::new();
        if let Some(p) = purpose1 {
            if !p.is_empty() {
                purposes.push(p.clone());
            }
        }
        if let Some(p) = purpose2 {
            if !p.is_empty() {
                purposes.push(p.clone());
            }
        }
        if let Some(p) = purpose3 {
            if !p.is_empty() {
                purposes.push(p.clone());
            }
        }
        if let Some(p) = purpose4 {
            if !p.is_empty() {
                purposes.push(p.clone());
            }
        }
        if purposes.is_empty() {
            return BBox3d::new();
        }

        let prim = self.inner.prim();
        if !prim.is_valid() {
            return BBox3d::new();
        }

        let mut bbox_cache = BBoxCache::new(time, purposes, false, false);
        bbox_cache.compute_world_bound(prim)
    }

    /// Compute the bound of this prim in local space, at the specified
    /// time, and for the specified purposes.
    ///
    /// Matches C++ `ComputeLocalBound()`.
    pub fn compute_local_bound(
        &self,
        time: TimeCode,
        purpose1: Option<&Token>,
        purpose2: Option<&Token>,
        purpose3: Option<&Token>,
        purpose4: Option<&Token>,
    ) -> BBox3d {
        let mut purposes = Vec::new();
        if let Some(p) = purpose1 {
            if !p.is_empty() {
                purposes.push(p.clone());
            }
        }
        if let Some(p) = purpose2 {
            if !p.is_empty() {
                purposes.push(p.clone());
            }
        }
        if let Some(p) = purpose3 {
            if !p.is_empty() {
                purposes.push(p.clone());
            }
        }
        if let Some(p) = purpose4 {
            if !p.is_empty() {
                purposes.push(p.clone());
            }
        }
        if purposes.is_empty() {
            return BBox3d::new();
        }

        let prim = self.inner.prim();
        if !prim.is_valid() {
            return BBox3d::new();
        }

        let mut bbox_cache = BBoxCache::new(time, purposes, false, false);
        bbox_cache.compute_local_bound(prim)
    }

    /// Compute the untransformed bound of this prim, at the specified
    /// time, and for the specified purposes.
    ///
    /// Matches C++ `ComputeUntransformedBound()`.
    pub fn compute_untransformed_bound(
        &self,
        time: TimeCode,
        purpose1: Option<&Token>,
        purpose2: Option<&Token>,
        purpose3: Option<&Token>,
        purpose4: Option<&Token>,
    ) -> BBox3d {
        let mut purposes = Vec::new();
        if let Some(p) = purpose1 {
            if !p.is_empty() {
                purposes.push(p.clone());
            }
        }
        if let Some(p) = purpose2 {
            if !p.is_empty() {
                purposes.push(p.clone());
            }
        }
        if let Some(p) = purpose3 {
            if !p.is_empty() {
                purposes.push(p.clone());
            }
        }
        if let Some(p) = purpose4 {
            if !p.is_empty() {
                purposes.push(p.clone());
            }
        }
        if purposes.is_empty() {
            return BBox3d::new();
        }

        let prim = self.inner.prim();
        if !prim.is_valid() {
            return BBox3d::new();
        }

        let mut bbox_cache = BBoxCache::new(time, purposes, false, false);
        bbox_cache.compute_untransformed_bound(prim)
    }

    /// Compute the transformation matrix for this prim at the given time,
    /// including the transform authored on the Prim itself, if present.
    ///
    /// Matches C++ `ComputeLocalToWorldTransform()`.
    pub fn compute_local_to_world_transform(&self, time: TimeCode) -> Matrix4d {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Matrix4d::identity();
        }
        let mut xform_cache = XformCache::new(time);
        xform_cache.get_local_to_world_transform(prim)
    }

    /// Compute the transformation matrix for this prim at the given time,
    /// NOT including the transform authored on the prim itself.
    ///
    /// Matches C++ `ComputeParentToWorldTransform()`.
    pub fn compute_parent_to_world_transform(&self, time: TimeCode) -> Matrix4d {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Matrix4d::identity();
        }
        let mut xform_cache = XformCache::new(time);
        xform_cache.get_parent_to_world_transform(prim)
    }

    /// Return a vector of names of all pre-declared attributes for this schema
    /// class and all its ancestor classes.
    ///
    /// Matches C++ `GetSchemaAttributeNames()`.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let local_names = vec![
            usd_geom_tokens().visibility.clone(),
            usd_geom_tokens().purpose.clone(),
        ];

        if include_inherited {
            let mut all_names = usd_core::Typed::get_schema_attribute_names(true);
            all_names.extend(local_names);
            all_names
        } else {
            local_names
        }
    }

    /// Return an Imageable holding the prim adhering to this schema at path on stage.
    ///
    /// Matches C++ `Get(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn get(stage: &usd_core::Stage, path: &usd_sdf::Path) -> Self {
        if let Some(prim) = stage.get_prim_at_path(path) {
            Self::new(prim)
        } else {
            Self::invalid()
        }
    }
}

// ============================================================================
// PurposeInfo
// ============================================================================

/// Value type containing information about a prim's computed effective
/// purpose as well as storing whether the prim's purpose value can be
/// inherited by namespace children if necessary.
///
/// Matches C++ `UsdGeomImageable::PurposeInfo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurposeInfo {
    /// The computed purpose. An empty purpose indicates that this
    /// represents a purpose that hasn't been computed yet.
    pub purpose: Token,
    /// Whether this purpose should be inherited by namespace children
    /// that do not have their own authored purpose value.
    pub is_inheritable: bool,
}

impl PurposeInfo {
    /// Creates a new PurposeInfo.
    pub fn new(purpose: Token, is_inheritable: bool) -> Self {
        Self {
            purpose,
            is_inheritable,
        }
    }

    /// Returns true if this represents a purpose that has been computed.
    pub fn is_valid(&self) -> bool {
        !self.purpose.is_empty()
    }

    /// Returns the purpose if it's inheritable, returns empty if it is not.
    pub fn inheritable_purpose(&self) -> Token {
        if self.is_inheritable {
            self.purpose.clone()
        } else {
            Token::new("")
        }
    }
}

impl PartialEq for Imageable {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for Imageable {}
