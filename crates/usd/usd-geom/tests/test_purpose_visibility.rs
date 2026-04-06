use std::sync::Once;

static INIT: Once = Once::new();
fn setup() {
    INIT.call_once(|| usd_sdf::init());
}

//! Tests for UsdGeomImageable purpose and visibility.
//!
//! Ported from: testenv/testUsdGeomPurposeVisibility.py

use std::sync::Arc;

use usd_core::{InitialLoadSet, Stage};
use usd_geom::*;
use usd_sdf::TimeCode;
use usd_tf::Token;

fn stage() -> Arc<Stage> {
    Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap()
}

fn default_tc() -> TimeCode {
    TimeCode::default_time()
}

fn path(s: &str) -> usd_sdf::Path {
    usd_sdf::Path::from_string(s).unwrap()
}

// ============================================================================
// test_ComputeVisibility
// ============================================================================

#[test]
fn test_compute_visibility() {
    setup();
    let s = stage();
    let t = usd_geom_tokens();

    // Non-imageable prims with no opinions evaluate to defaults
    let ni_root = s.define_prim("/ni_Root", "").unwrap();
    let ni_sub = s.define_prim("/ni_Root/ni_Sub", "").unwrap();
    let ni_leaf = s.define_prim("/ni_Root/ni_Sub/ni_leaf", "").unwrap();

    let img = Imageable::new(ni_root.clone());
    assert_eq!(
        img.compute_visibility(default_tc()),
        t.inherited,
        "ni_Root should be inherited"
    );

    let img = Imageable::new(ni_sub.clone());
    assert_eq!(
        img.compute_visibility(default_tc()),
        t.inherited,
        "ni_Sub should be inherited"
    );

    let img = Imageable::new(ni_leaf.clone());
    assert_eq!(
        img.compute_visibility(default_tc()),
        t.inherited,
        "ni_leaf should be inherited"
    );

    // Non-imageable prims WITH opinions STILL evaluate to defaults
    let registry = usd_sdf::ValueTypeRegistry::instance();
    let token_type = registry.find_type_by_token(&Token::new("token"));
    let vis_attr = ni_sub
        .create_attribute(
            t.visibility.as_str(),
            &token_type,
            false,
            Some(usd_core::attribute::Variability::Varying),
        )
        .expect("create visibility attr");
    vis_attr.set(t.invisible.clone(), default_tc());

    let img = Imageable::new(ni_sub.clone());
    assert_eq!(
        img.compute_visibility(default_tc()),
        t.inherited,
        "ni_Sub with opinion still inherited (non-imageable)"
    );
    let img = Imageable::new(ni_leaf.clone());
    assert_eq!(
        img.compute_visibility(default_tc()),
        t.inherited,
        "ni_leaf with parent opinion still inherited (non-imageable)"
    );

    // Imageable leaf prim can have opinions
    let i_root = Scope::define(&s, &path("/i_Root"));
    let i_sub = Scope::define(&s, &path("/i_Root/i_Sub"));
    let i_leaf = Scope::define(&s, &path("/i_Root/i_Sub/i_leaf"));

    i_leaf
        .imageable()
        .get_visibility_attr()
        .set(t.invisible.clone(), default_tc());

    assert_eq!(
        i_leaf.imageable().compute_visibility(default_tc()),
        t.invisible,
        "i_leaf should be invisible"
    );

    // Imageable leaf prim is shadowed by Imageable parent opinions
    i_leaf
        .imageable()
        .get_visibility_attr()
        .set(t.inherited.clone(), default_tc());
    i_sub
        .imageable()
        .get_visibility_attr()
        .set(t.invisible.clone(), default_tc());

    assert_eq!(
        i_leaf.imageable().compute_visibility(default_tc()),
        t.invisible,
        "i_leaf shadowed by i_sub invisible"
    );

    // Imageable leaf prim is NOT shadowed by non-Imageable parent opinions
    i_sub.prim().set_type_name("");

    assert_eq!(
        i_leaf.imageable().compute_visibility(default_tc()),
        t.inherited,
        "i_leaf not shadowed by non-imageable parent"
    );

    // Most ancestral imageable opinion wins when there are many
    i_sub.prim().set_type_name("Scope");
    // Authoring fallback value on root shouldn't change results
    i_root
        .imageable()
        .get_visibility_attr()
        .set(t.inherited.clone(), default_tc());

    assert_eq!(
        i_leaf.imageable().compute_visibility(default_tc()),
        t.invisible,
        "i_leaf should be invisible from i_sub"
    );

    i_root
        .imageable()
        .get_visibility_attr()
        .set(t.invisible.clone(), default_tc());

    assert_eq!(
        i_leaf.imageable().compute_visibility(default_tc()),
        t.invisible,
        "i_leaf should be invisible from i_root"
    );

    // Verify that the Compute*() API works correctly
    assert_eq!(
        i_leaf.imageable().compute_visibility(default_tc()),
        t.invisible,
        "final visibility check"
    );
}

// ============================================================================
// test_ComputePurposeVisibility
// ============================================================================

#[test]
fn test_compute_purpose_visibility() {
    setup();
    let s = stage();
    let t = usd_geom_tokens();

    let root_prim = s.define_prim("/Root", "Scope").unwrap();
    let imageable_prim = s.define_prim("/Root/Imageable", "Scope").unwrap();
    let _non_imageable_prim = s.define_prim("/Root/NonImageable", "").unwrap();

    let root = Imageable::new(root_prim.clone());
    assert!(root.is_valid());
    let imageable = Imageable::new(imageable_prim.clone());
    assert!(imageable.is_valid());

    // VisibilityAPI is not applied by default
    assert!(!root_prim.has_api(&Token::new("VisibilityAPI")));
    assert!(!imageable_prim.has_api(&Token::new("VisibilityAPI")));

    // Overall visibility is initially visible, purpose-specific has expected fallbacks
    assert_eq!(
        imageable.compute_effective_visibility(&t.default_, default_tc()),
        t.visible
    );
    assert_eq!(
        imageable.compute_effective_visibility(&t.guide, default_tc()),
        t.invisible
    );
    assert_eq!(
        imageable.compute_effective_visibility(&t.proxy, default_tc()),
        t.inherited
    );
    assert_eq!(
        imageable.compute_effective_visibility(&t.render, default_tc()),
        t.inherited
    );

    // VisibilityAPI can only apply to Imageable prims
    assert!(VisibilityAPI::can_apply(&imageable_prim));
    assert!(VisibilityAPI::can_apply(&_non_imageable_prim));

    // Default purpose visibility attr exists (it's the overall visibility)
    assert!(
        imageable
            .get_purpose_visibility_attr(&t.default_)
            .is_valid()
    );

    // Purpose-specific attrs don't exist yet
    assert!(!imageable.get_purpose_visibility_attr(&t.guide).is_valid());
    assert!(!imageable.get_purpose_visibility_attr(&t.proxy).is_valid());
    assert!(!imageable.get_purpose_visibility_attr(&t.render).is_valid());

    // Apply VisibilityAPI to imageable prim
    let imageable_vis_api = VisibilityAPI::apply(&imageable_prim);
    assert!(imageable_vis_api.is_valid());

    // Guide visibility: after applying VisibilityAPI, check guide attr
    let guide_visibility = imageable.get_purpose_visibility_attr(&t.guide);
    assert!(guide_visibility.is_valid());
    assert_eq!(
        imageable.compute_effective_visibility(&t.guide, default_tc()),
        t.invisible,
        "guide default fallback is invisible"
    );

    // Proxy visibility
    let proxy_visibility = imageable.get_purpose_visibility_attr(&t.proxy);
    assert!(proxy_visibility.is_valid());
    assert_eq!(
        imageable.compute_effective_visibility(&t.proxy, default_tc()),
        t.inherited,
        "proxy default fallback is inherited"
    );

    // Render visibility
    let render_visibility = imageable.get_purpose_visibility_attr(&t.render);
    assert!(render_visibility.is_valid());
    assert_eq!(
        imageable.compute_effective_visibility(&t.render, default_tc()),
        t.inherited,
        "render default fallback is inherited"
    );

    // Set purpose visibility on the root and ensure inheritance
    let root_vis_api = VisibilityAPI::apply(&root_prim);
    assert!(root_vis_api.is_valid());

    let root_guide_vis = root.get_purpose_visibility_attr(&t.guide);
    assert!(root_guide_vis.is_valid());
    root_guide_vis.set(t.visible.clone(), default_tc());
    assert_eq!(
        root.compute_effective_visibility(&t.guide, default_tc()),
        t.visible
    );
    assert_eq!(
        imageable.compute_effective_visibility(&t.guide, default_tc()),
        t.visible,
        "guide visibility inherits from root"
    );

    // Set overall visibility to invisible, causing all purpose visibility to become invisible
    let overall_visibility = root.get_purpose_visibility_attr(&t.default_);
    overall_visibility.set(t.invisible.clone(), default_tc());
    assert_eq!(
        root.compute_effective_visibility(&t.default_, default_tc()),
        t.invisible
    );
    assert_eq!(
        imageable.compute_effective_visibility(&t.default_, default_tc()),
        t.invisible
    );
    assert_eq!(
        root.compute_effective_visibility(&t.guide, default_tc()),
        t.invisible
    );
    assert_eq!(
        imageable.compute_effective_visibility(&t.guide, default_tc()),
        t.invisible
    );
    assert_eq!(
        root.compute_effective_visibility(&t.render, default_tc()),
        t.invisible
    );
    assert_eq!(
        imageable.compute_effective_visibility(&t.render, default_tc()),
        t.invisible
    );

    // Unapply VisibilityAPI from root
    root_prim.remove_api(&Token::new("VisibilityAPI"));
    assert!(!root_prim.has_api(&Token::new("VisibilityAPI")));

    // Purpose visibility attrs are no longer available via Imageable
    assert!(
        root.get_purpose_visibility_attr(&t.default_).is_valid(),
        "default purpose always returns overall visibility"
    );
    assert!(
        !root.get_purpose_visibility_attr(&t.guide).is_valid(),
        "guide not available without VisibilityAPI"
    );
    assert!(
        !root.get_purpose_visibility_attr(&t.proxy).is_valid(),
        "proxy not available without VisibilityAPI"
    );
    assert!(
        !root.get_purpose_visibility_attr(&t.render).is_valid(),
        "render not available without VisibilityAPI"
    );

    // Even without VisibilityAPI, root still has overall visibility set to invisible
    assert_eq!(
        root.compute_effective_visibility(&t.default_, default_tc()),
        t.invisible
    );
    assert_eq!(
        imageable.compute_effective_visibility(&t.default_, default_tc()),
        t.invisible
    );
    assert_eq!(
        root.compute_effective_visibility(&t.guide, default_tc()),
        t.invisible
    );
    assert_eq!(
        imageable.compute_effective_visibility(&t.guide, default_tc()),
        t.invisible
    );
    assert_eq!(
        root.compute_effective_visibility(&t.render, default_tc()),
        t.invisible
    );
    assert_eq!(
        imageable.compute_effective_visibility(&t.render, default_tc()),
        t.invisible
    );

    // Set root's overall visibility back to visible
    overall_visibility.set(t.visible.clone(), default_tc());

    // Default and render visibility are back to inherited defaults
    assert_eq!(
        root.compute_effective_visibility(&t.default_, default_tc()),
        t.visible
    );
    assert_eq!(
        imageable.compute_effective_visibility(&t.default_, default_tc()),
        t.visible
    );
    assert_eq!(
        root.compute_effective_visibility(&t.render, default_tc()),
        t.inherited
    );
    assert_eq!(
        imageable.compute_effective_visibility(&t.render, default_tc()),
        t.inherited
    );

    // Guide visibility is back to default "invisible" even though the attr exists
    // and has "visible" authored, because VisibilityAPI is not applied
    let root_vis_api_unapplied = VisibilityAPI::new(root_prim.clone());
    let guide_vis_attr = root_vis_api_unapplied.get_purpose_visibility_attr(&t.guide);
    if guide_vis_attr.is_valid() {
        if let Some(val) = guide_vis_attr.get(default_tc()) {
            if let Some(token) = val.downcast::<Token>() {
                assert_eq!(*token, t.visible, "authored guide vis is still visible");
            }
        }
    }
    assert_eq!(
        root.compute_effective_visibility(&t.guide, default_tc()),
        t.invisible,
        "guide falls back to invisible without VisibilityAPI"
    );
    assert_eq!(
        imageable.compute_effective_visibility(&t.guide, default_tc()),
        t.invisible,
        "child guide also falls back to invisible"
    );
}

// ============================================================================
// test_ComputePurposeVisibilityWithInstancing
// ============================================================================

#[test]
fn test_compute_purpose_visibility_with_instancing() {
    setup();
    let s = stage();
    let t = usd_geom_tokens();

    let root_layer = s.get_root_layer();
    root_layer.import_from_string(
        r#"#usda 1.0
def Scope "_prototype" (
    prepend apiSchemas = ["VisibilityAPI"]
)
{
    token guideVisibility = "visible"

    def Scope "child" (
        prepend apiSchemas = ["VisibilityAPI"]
    )
    {
    }
}

def Scope "instance" (
    instanceable = true
    references = </_prototype>
)
{
}
"#,
    );

    // The instance child prim (which proxies _prototype/child) is initially
    // visible, due to the guideVisibility opinion on _prototype.
    let child_prim = s
        .get_prim_at_path(&path("/instance/child"))
        .expect("child prim should exist");
    assert!(child_prim.is_valid());
    assert!(child_prim.is_instance_proxy());

    let child = Imageable::new(child_prim.clone());
    assert_eq!(
        child.compute_effective_visibility(&t.guide, default_tc()),
        t.visible,
        "instance child guide visibility should be visible"
    );

    // If we invis guides on the instance root, this is inherited by the child
    let instance_prim = s
        .get_prim_at_path(&path("/instance"))
        .expect("instance prim should exist");
    assert!(instance_prim.is_valid());
    assert!(!instance_prim.is_instance_proxy());

    let instance = Imageable::new(instance_prim.clone());
    let guide_visibility = instance.get_purpose_visibility_attr(&t.guide);
    guide_visibility.set(t.invisible.clone(), default_tc());
    assert_eq!(
        child.compute_effective_visibility(&t.guide, default_tc()),
        t.invisible,
        "instance child inherits guide invisibility from instance root"
    );

    // The prototype child doesn't inherit guide visibility from instance
    let prototype_child_prim = instance_prim
        .get_prototype()
        .get_child(&Token::new("child"));
    assert!(prototype_child_prim.is_valid());
    assert!(!prototype_child_prim.is_instance_proxy());

    let prototype_child = Imageable::new(prototype_child_prim);
    assert_eq!(
        prototype_child.compute_effective_visibility(&t.guide, default_tc()),
        t.invisible,
        "prototype child guide visibility"
    );
}

// ============================================================================
// test_ComputePurpose
// ============================================================================

#[test]
fn test_compute_purpose() {
    setup();
    let s = stage();
    let t = usd_geom_tokens();

    // Non-imageable with purpose attribute opinion
    let root = s.define_prim("/Root", "").unwrap();
    let registry = usd_sdf::ValueTypeRegistry::instance();
    let token_type = registry.find_type_by_token(&Token::new("token"));
    root.create_attribute(
        t.purpose.as_str(),
        &token_type,
        false,
        Some(usd_core::attribute::Variability::Uniform),
    )
    .unwrap()
    .set(t.proxy.clone(), default_tc());
    let root_img = Imageable::new(root.clone());

    // Imageable with purpose attribute opinion
    let render_scope = s.define_prim("/Root/RenderScope", "Scope").unwrap();
    let render_scope_img = Imageable::new(render_scope.clone());
    render_scope_img
        .get_purpose_attr()
        .set(t.render.clone(), default_tc());

    // Non-imageable with purpose attribute opinion
    let default_prim = s.define_prim("/Root/RenderScope/DefPrim", "").unwrap();
    default_prim
        .create_attribute(
            t.purpose.as_str(),
            &token_type,
            false,
            Some(usd_core::attribute::Variability::Uniform),
        )
        .unwrap()
        .set(t.default_.clone(), default_tc());
    let default_prim_img = Imageable::new(default_prim.clone());

    // Imageable with no purpose opinion
    let scope = s
        .define_prim("/Root/RenderScope/DefPrim/Scope", "Scope")
        .unwrap();
    let scope_img = Imageable::new(scope.clone());

    // Imageable with no purpose opinion
    let inherit_xform = s
        .define_prim("/Root/RenderScope/DefPrim/Scope/InheritXform", "Xform")
        .unwrap();
    let inherit_xform_img = Imageable::new(inherit_xform.clone());

    // Imageable with purpose opinion
    let guide_xform = s
        .define_prim("/Root/RenderScope/DefPrim/Scope/GuideXform", "Xform")
        .unwrap();
    let guide_xform_img = Imageable::new(guide_xform.clone());
    guide_xform_img
        .get_purpose_attr()
        .set(t.guide.clone(), default_tc());

    // Imageable with no purpose opinion
    let xform = s.define_prim("/Root/Xform", "Xform").unwrap();
    let xform_img = Imageable::new(xform.clone());

    // Non-imageable root evaluates to default
    assert!(!root_img.is_valid());
    let root_info = root_img.compute_purpose_info();
    assert_eq!(root_info.purpose, t.default_);
    assert!(!root_info.is_inheritable);

    // Imageable with authored opinion always evaluates to authored purpose
    assert!(render_scope_img.is_valid());
    let render_scope_info = render_scope_img.compute_purpose_info();
    assert_eq!(render_scope_info.purpose, t.render);
    assert!(render_scope_info.is_inheritable);

    // Non-imageable under imageable prim inherits purpose from authored imageable parent
    assert!(!default_prim_img.is_valid());
    let default_prim_info = default_prim_img.compute_purpose_info();
    assert_eq!(default_prim_info.purpose, t.render);
    assert!(default_prim_info.is_inheritable);

    // Imageable with no opinion inherits from nearest imageable ancestor with authored purpose
    assert!(scope_img.is_valid());
    let scope_info = scope_img.compute_purpose_info();
    assert_eq!(scope_info.purpose, t.render);
    assert!(scope_info.is_inheritable);

    // Imageable with no opinion whose parent is also imageable with no opinion
    // still inherits from nearest possible imageable ancestor
    assert!(inherit_xform_img.is_valid());
    let inherit_xform_info = inherit_xform_img.compute_purpose_info();
    assert_eq!(inherit_xform_info.purpose, t.render);
    assert!(inherit_xform_info.is_inheritable);

    // Imageable with a different authored opinion than ancestor's purpose
    // always uses its own authored purpose
    assert!(guide_xform_img.is_valid());
    let guide_xform_info = guide_xform_img.compute_purpose_info();
    assert_eq!(guide_xform_info.purpose, t.guide);
    assert!(guide_xform_info.is_inheritable);

    // Imageable with no opinion and no inheritable ancestor opinion uses fallback
    assert!(xform_img.is_valid());
    let xform_info = xform_img.compute_purpose_info();
    assert_eq!(xform_info.purpose, t.default_);
    assert!(!xform_info.is_inheritable);

    // Testing ComputePurposeInfo API that takes a precomputed parent purpose
    let inheritable_default = PurposeInfo::new(t.default_.clone(), true);
    let non_inheritable_default = PurposeInfo::new(t.default_.clone(), false);
    let inheritable_proxy = PurposeInfo::new(t.proxy.clone(), true);
    let non_inheritable_proxy = PurposeInfo::new(t.proxy.clone(), false);

    // Scope with authored opinion: parent purpose info is always ignored
    assert_eq!(
        render_scope_img.compute_purpose_info_with_parent(&inheritable_default),
        PurposeInfo::new(t.render.clone(), true)
    );
    assert_eq!(
        render_scope_img.compute_purpose_info_with_parent(&non_inheritable_default),
        PurposeInfo::new(t.render.clone(), true)
    );
    assert_eq!(
        render_scope_img.compute_purpose_info_with_parent(&inheritable_proxy),
        PurposeInfo::new(t.render.clone(), true)
    );
    assert_eq!(
        render_scope_img.compute_purpose_info_with_parent(&non_inheritable_proxy),
        PurposeInfo::new(t.render.clone(), true)
    );

    // Imageable with no purpose opinion: uses parent if inheritable, fallback otherwise
    assert_eq!(
        scope_img.compute_purpose_info_with_parent(&inheritable_default),
        PurposeInfo::new(t.default_.clone(), true)
    );
    assert_eq!(
        scope_img.compute_purpose_info_with_parent(&non_inheritable_default),
        PurposeInfo::new(t.default_.clone(), false)
    );
    assert_eq!(
        scope_img.compute_purpose_info_with_parent(&inheritable_proxy),
        PurposeInfo::new(t.proxy.clone(), true)
    );
    assert_eq!(
        scope_img.compute_purpose_info_with_parent(&non_inheritable_proxy),
        PurposeInfo::new(t.default_.clone(), false)
    );
}

// ============================================================================
// test_MakeVisInvis
// ============================================================================

#[test]
fn test_make_vis_invis() {
    setup();
    let s = stage();
    let t = usd_geom_tokens();

    let root = Scope::define(&s, &path("/Root"));
    let sub = Scope::define(&s, &path("/Root/Sub"));
    let leaf = Scope::define(&s, &path("/Root/Sub/leaf"));

    assert_eq!(
        root.imageable().compute_visibility(default_tc()),
        t.inherited
    );

    // Making a root invisible makes all prims invisible
    root.imageable().make_invisible(default_tc());
    assert_eq!(
        root.imageable().compute_visibility(default_tc()),
        t.invisible
    );
    assert_eq!(
        sub.imageable().compute_visibility(default_tc()),
        t.invisible
    );
    assert_eq!(
        leaf.imageable().compute_visibility(default_tc()),
        t.invisible
    );

    // Making the leaf visible causes everything to become visible
    leaf.imageable().make_visible(default_tc());
    assert_eq!(
        root.imageable().compute_visibility(default_tc()),
        t.inherited
    );
    assert_eq!(
        sub.imageable().compute_visibility(default_tc()),
        t.inherited
    );
    assert_eq!(
        leaf.imageable().compute_visibility(default_tc()),
        t.inherited
    );

    // Making the subscope invisible: only subscope and leaf are invisible
    sub.imageable().make_invisible(default_tc());
    assert_eq!(
        root.imageable().compute_visibility(default_tc()),
        t.inherited
    );
    assert_eq!(
        sub.imageable().compute_visibility(default_tc()),
        t.invisible
    );
    assert_eq!(
        leaf.imageable().compute_visibility(default_tc()),
        t.invisible
    );

    // Invising just the leaf
    leaf.imageable().make_invisible(default_tc());
    sub.imageable().make_visible(default_tc());
    assert_eq!(
        root.imageable().compute_visibility(default_tc()),
        t.inherited
    );
    assert_eq!(
        sub.imageable().compute_visibility(default_tc()),
        t.inherited
    );
    assert_eq!(
        leaf.imageable().compute_visibility(default_tc()),
        t.invisible
    );

    // Vising everything again
    root.imageable().make_visible(default_tc());
    leaf.imageable().make_visible(default_tc());
    assert_eq!(
        root.imageable().compute_visibility(default_tc()),
        t.inherited
    );
    assert_eq!(
        sub.imageable().compute_visibility(default_tc()),
        t.inherited
    );
    assert_eq!(
        leaf.imageable().compute_visibility(default_tc()),
        t.inherited
    );

    // Test with a couple of new subtrees
    let root2 = Scope::define(&s, &path("/Root2"));
    let sub2 = Scope::define(&s, &path("/Root/Sub2"));
    let leaf2 = Scope::define(&s, &path("/Root/Sub2/Leaf2"));
    let sub3 = Scope::define(&s, &path("/Root/Sub3"));
    let leaf3 = Scope::define(&s, &path("/Root/Sub3/Leaf3"));

    root.imageable().make_invisible(default_tc());
    leaf.imageable().make_visible(default_tc());
    leaf3.imageable().make_visible(default_tc());

    assert_eq!(
        root.imageable().compute_visibility(default_tc()),
        t.inherited,
        "/Root"
    );
    assert_eq!(
        root2.imageable().compute_visibility(default_tc()),
        t.inherited,
        "/Root2"
    );
    assert_eq!(
        sub.imageable().compute_visibility(default_tc()),
        t.inherited,
        "/Root/Sub"
    );
    assert_eq!(
        leaf.imageable().compute_visibility(default_tc()),
        t.inherited,
        "/Root/Sub/leaf"
    );
    assert_eq!(
        sub2.imageable().compute_visibility(default_tc()),
        t.invisible,
        "/Root/Sub2"
    );
    assert_eq!(
        leaf2.imageable().compute_visibility(default_tc()),
        t.invisible,
        "/Root/Sub2/Leaf2"
    );
    assert_eq!(
        sub3.imageable().compute_visibility(default_tc()),
        t.inherited,
        "/Root/Sub3"
    );
    assert_eq!(
        leaf3.imageable().compute_visibility(default_tc()),
        t.inherited,
        "/Root/Sub3/Leaf3"
    );

    // Test preservation of visibility state:
    //       A
    //       |
    //       B
    //      / \
    //     C   D
    //     |
    //     E
    // Make A invisible then E visible. D should remain invisible.
    let a = Scope::define(&s, &path("/A"));
    let b = Scope::define(&s, &path("/A/B"));
    let c = Scope::define(&s, &path("/A/B/C"));
    let d = Scope::define(&s, &path("/A/B/D"));
    let e = Scope::define(&s, &path("/A/B/C/E"));

    a.imageable().make_invisible(default_tc());
    e.imageable().make_visible(default_tc());

    assert_eq!(
        a.imageable().compute_visibility(default_tc()),
        t.inherited,
        "/A"
    );
    assert_eq!(
        b.imageable().compute_visibility(default_tc()),
        t.inherited,
        "/A/B"
    );
    assert_eq!(
        c.imageable().compute_visibility(default_tc()),
        t.inherited,
        "/A/B/C"
    );
    assert_eq!(
        d.imageable().compute_visibility(default_tc()),
        t.invisible,
        "/A/B/D"
    );
    assert_eq!(
        e.imageable().compute_visibility(default_tc()),
        t.inherited,
        "/A/B/C/E"
    );

    // Non-default visibility authoring at time=1.0
    d.imageable().make_visible(default_tc());
    a.imageable().make_invisible(TimeCode::new(1.0));
    e.imageable().make_visible(TimeCode::new(1.0));

    assert_eq!(
        a.imageable().compute_visibility(TimeCode::new(1.0)),
        t.inherited,
        "/A at t=1"
    );
    assert_eq!(
        b.imageable().compute_visibility(TimeCode::new(1.0)),
        t.inherited,
        "/A/B at t=1"
    );
    assert_eq!(
        c.imageable().compute_visibility(TimeCode::new(1.0)),
        t.inherited,
        "/A/B/C at t=1"
    );
    assert_eq!(
        e.imageable().compute_visibility(TimeCode::new(1.0)),
        t.inherited,
        "/A/B/C/E at t=1"
    );
    assert_eq!(
        d.imageable().compute_visibility(TimeCode::new(1.0)),
        t.invisible,
        "/A/B/D at t=1"
    );
}

// ============================================================================
// test_ProxyPrim
// ============================================================================

#[test]
fn test_proxy_prim() {
    setup();
    let s = stage();
    let t = usd_geom_tokens();

    //         A
    //        / \
    //       B   F
    //      / \
    //     C   D
    //     |
    //     E
    // C has purpose 'render' and proxyPrim targets D
    // D has purpose proxy
    // F has purpose render and proxyPrim targets B (which is default)
    let a = Scope::define(&s, &path("/A"));
    let b = Scope::define(&s, &path("/A/B"));
    let c = Scope::define(&s, &path("/A/B/C"));
    let d = Scope::define(&s, &path("/A/B/D"));
    let e = Scope::define(&s, &path("/A/B/C/E"));
    let f = Scope::define(&s, &path("/A/F"));

    c.imageable()
        .create_purpose_attr()
        .set(t.render.clone(), default_tc());
    c.imageable().set_proxy_prim(d.prim());
    d.imageable()
        .create_purpose_attr()
        .set(t.proxy.clone(), default_tc());
    f.imageable()
        .create_purpose_attr()
        .set(t.render.clone(), default_tc());
    f.imageable().set_proxy_prim(b.prim());

    // A has no proxy prim
    assert!(
        a.imageable().compute_proxy_prim().is_none(),
        "A should have no proxy prim"
    );

    // C's proxy prim is D, with render root C
    let c_proxy = c.imageable().compute_proxy_prim();
    assert!(c_proxy.is_some(), "C should have proxy prim");
    let (proxy_prim, render_prim) = c_proxy.unwrap();
    assert_eq!(proxy_prim.path(), d.prim().path());
    assert_eq!(render_prim.path(), c.prim().path());

    // E inherits proxy prim from C
    let e_proxy = e.imageable().compute_proxy_prim();
    assert!(e_proxy.is_some(), "E should inherit proxy prim from C");
    let (proxy_prim, render_prim) = e_proxy.unwrap();
    assert_eq!(proxy_prim.path(), d.prim().path());
    assert_eq!(render_prim.path(), c.prim().path());

    // F targets B but B doesn't have 'proxy' purpose, so invalid
    assert!(
        f.imageable().compute_proxy_prim().is_none(),
        "F's proxy is invalid (B is not proxy purpose)"
    );

    // Set purpose 'guide' on A, ensure D's purpose value isn't determined by ancestor
    a.imageable()
        .create_purpose_attr()
        .set(t.guide.clone(), default_tc());
    let e_proxy = e.imageable().compute_proxy_prim();
    assert!(
        e_proxy.is_some(),
        "E should still have proxy prim after A gets guide purpose"
    );
    let (proxy_prim, render_prim) = e_proxy.unwrap();
    assert_eq!(proxy_prim.path(), d.prim().path());
    assert_eq!(render_prim.path(), c.prim().path());
}
