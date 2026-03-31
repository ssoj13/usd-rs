//! Per-prim cached display data (PrimViewItem) and related enums.

use egui::{Color32, FontId, RichText};
use usd_core::Prim;
use usd_geom::imageable::Imageable;
use usd_geom::visibility_api::VisibilityAPI;
use usd_sdf::{Path, TimeCode};
use usd_tf::Token;

use super::{CLR_HAS_ARCS, CLR_INSTANCE, CLR_NORMAL, CLR_PROTOTYPE};

// ---------------------------------------------------------------------------
// Visibility enum
// ---------------------------------------------------------------------------

/// Prim visibility state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimVisibility {
    Visible,
    /// Prim is explicitly invisible.
    Invisible,
    /// Prim's own opinion is "inherited" (visible), but a parent is invisible.
    InheritedInvisible,
    /// Prim has no visibility opinion (fully inherits, and parent is visible).
    Inherited,
}

impl PrimVisibility {
    /// Short label for column display.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Visible | Self::Inherited => "V",
            Self::Invisible | Self::InheritedInvisible => "I",
        }
    }

    /// True when the invisible state is inherited from a parent (italic display).
    pub fn is_inherited_invisible(&self) -> bool {
        *self == Self::InheritedInvisible
    }
}

// ---------------------------------------------------------------------------
// Draw mode enum (per-prim, not viewport)
// ---------------------------------------------------------------------------

/// Per-prim draw mode (UsdGeomModelAPI::drawMode).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PrimDrawMode {
    #[default]
    Default,
    Origin,
    Bounds,
    Cards,
    Inherited,
}

impl PrimDrawMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Origin => "origin",
            Self::Bounds => "bounds",
            Self::Cards => "cards",
            Self::Inherited => "inherited",
        }
    }

    pub fn from_token(t: &str) -> Self {
        match t {
            "origin" => Self::Origin,
            "bounds" => Self::Bounds,
            "cards" => Self::Cards,
            "inherited" => Self::Inherited,
            _ => Self::Default,
        }
    }

    /// All valid modes for the combo box.
    pub const ALL: &'static [PrimDrawMode] = &[
        PrimDrawMode::Default,
        PrimDrawMode::Origin,
        PrimDrawMode::Bounds,
        PrimDrawMode::Cards,
        PrimDrawMode::Inherited,
    ];
}

// ---------------------------------------------------------------------------
// PrimViewItem -- cached display info per prim
// ---------------------------------------------------------------------------

/// Cached per-prim display data (matches reference primViewItem.py).
/// Queried from USD once, reused across frames until invalidated.
#[derive(Debug, Clone)]
pub struct PrimViewItem {
    pub path: Path,
    pub name: String,
    pub type_name: String,
    pub depth: usize,
    pub is_active: bool,
    pub is_defined: bool,
    pub is_abstract: bool,
    pub is_instance: bool,
    pub is_prototype: bool,
    pub has_arcs: bool,
    pub is_loaded: bool,
    pub is_model: bool,
    pub is_camera: bool,
    pub is_render_settings: bool,
    pub is_render_pass: bool,
    pub has_payload: bool,
    /// Computed (inherited) visibility state.
    pub visibility: PrimVisibility,
    /// Whether this prim supports guide purpose (UsdGeomVisibilityAPI).
    pub supports_guides: bool,
    /// Guide visibility state (only valid when supports_guides is true).
    pub guide_visibility: PrimVisibility,
    pub draw_mode: PrimDrawMode,
    pub draw_mode_inherited: bool,
    pub has_children: bool,
    pub display_name: Option<String>,
}

impl PrimViewItem {
    /// Build from a USD prim at the given time code.
    pub fn from_prim(prim: &Prim, tc: TimeCode, depth: usize) -> Self {
        let path = prim.path().clone();
        let name = prim.name().to_string();
        let type_name = prim.type_name().to_string();
        let is_active = prim.is_active();
        let is_defined = prim.is_defined();
        let is_abstract = prim.is_abstract();
        let is_instance = prim.is_instance();
        let is_prototype = prim.is_prototype();
        let is_loaded = prim.is_loaded();
        let has_children = prim.has_children();
        let is_model = prim.is_model();
        let is_camera = type_name == "Camera";
        let is_render_settings = type_name == "RenderSettings";
        let is_render_pass = type_name == "RenderPass";
        let has_payload = prim.has_payload();

        let has_arcs = prim.has_authored_references()
            || has_payload
            || !prim.get_variant_sets().get_names().is_empty()
            || !prim.get_inherit_arcs().is_empty()
            || !prim.get_specialize_arcs().is_empty();

        let tokens = usd_geom::tokens::usd_geom_tokens();
        let imageable = Imageable::new(prim.clone());

        // Compute both local authored visibility and the inherited (computed) visibility.
        // is_vis_inherited: prim's own opinion is "inherited" but parent makes it invisible.
        // Matches Python: _isVisInherited() = imageable && active && vis != invisible && computedVis == invisible
        let visibility = if imageable.is_valid() && is_active {
            let computed_vis = imageable.compute_visibility(tc);
            let local_vis = imageable
                .get_visibility_attr()
                .get(tc)
                .and_then(|v| v.downcast::<Token>().cloned())
                .unwrap_or_else(|| tokens.inherited.clone());

            if local_vis == tokens.invisible {
                PrimVisibility::Invisible
            } else if computed_vis == tokens.invisible {
                // Local says inherited/visible, but computed says invisible => parent blocked us.
                PrimVisibility::InheritedInvisible
            } else {
                PrimVisibility::Inherited
            }
        } else {
            PrimVisibility::Inherited
        };

        // Draw mode via UsdGeomModelAPI (only for models)
        let (draw_mode, draw_mode_inherited) = if is_model {
            let model_api = usd_geom::model_api::ModelAPI::new(prim.clone());
            let mode_token = model_api.compute_model_draw_mode(None);
            let mode = PrimDrawMode::from_token(mode_token.get_text());
            let inherited = model_api.get_model_draw_mode_attr().is_none();
            (mode, inherited)
        } else {
            (PrimDrawMode::Default, true)
        };

        // Guide visibility: check UsdGeomVisibilityAPI.guideVisibility attribute.
        // A prim supports guides if VisibilityAPI is applicable (any valid prim).
        let vis_api = VisibilityAPI::new(prim.clone());
        let (supports_guides, guide_visibility) = if vis_api.is_valid() {
            let guide_attr = vis_api.get_guide_visibility_attr();
            if guide_attr.is_valid() && guide_attr.has_authored_value() {
                let gv = guide_attr
                    .get(tc)
                    .and_then(|v| v.downcast::<Token>().cloned())
                    .unwrap_or_else(|| tokens.invisible.clone());
                let gvis = if gv == tokens.visible {
                    PrimVisibility::Visible
                } else {
                    PrimVisibility::Invisible
                };
                (true, gvis)
            } else {
                // No authored guide visibility — treat as visible (inherited default)
                (true, PrimVisibility::Inherited)
            }
        } else {
            (false, PrimVisibility::Inherited)
        };

        // Display name from metadata
        let display_name: Option<String> = prim
            .get_metadata::<String>(&Token::new("displayName"))
            .filter(|s| !s.is_empty());

        Self {
            path,
            name,
            type_name,
            depth,
            is_active,
            is_defined,
            is_abstract,
            is_instance,
            is_prototype,
            has_arcs,
            is_loaded,
            is_model,
            is_camera,
            is_render_settings,
            is_render_pass,
            has_payload,
            visibility,
            supports_guides,
            guide_visibility,
            draw_mode,
            draw_mode_inherited,
            has_children,
            display_name,
        }
    }

    /// Text color based on prim state.
    ///
    /// Inactive and unloaded prims use a darkened version of the type color.
    /// Python HALF_DARKER=150 maps to ~0.35 factor.
    pub fn text_color(&self) -> Color32 {
        if !self.is_active || (!self.is_loaded && !self.is_prototype) {
            // Compute the type color first, then darken (reference: color.darker(HALF_DARKER)).
            // HALF_DARKER=150 in Qt maps to factor ~0.33 (150/255 * 0.55 approx).
            // We use 0.35 to match the visual intent: clearly darker but still distinguishable.
            let base = if self.is_instance {
                CLR_INSTANCE
            } else if self.is_prototype {
                CLR_PROTOTYPE
            } else if self.has_arcs {
                CLR_HAS_ARCS
            } else {
                CLR_NORMAL
            };
            // Qt darker(150) = multiply by 1/1.5 ≈ 0.667
            Color32::from_rgb(
                (base.r() as f32 * 0.667) as u8,
                (base.g() as f32 * 0.667) as u8,
                (base.b() as f32 * 0.667) as u8,
            )
        } else if self.is_instance {
            CLR_INSTANCE
        } else if self.is_prototype {
            CLR_PROTOTYPE
        } else if self.has_arcs {
            CLR_HAS_ARCS
        } else {
            CLR_NORMAL
        }
    }

    /// Font style: bold=defined, italic=over (not defined), normal=abstract.
    pub fn rich_label(&self, use_display_name: bool) -> RichText {
        let label = if use_display_name {
            self.display_name.as_deref().unwrap_or(&self.name)
        } else {
            &self.name
        };
        let mut rt = RichText::new(label).color(self.text_color());
        if self.is_abstract {
            // Abstract (class): normal weight
            rt = rt.font(FontId::proportional(11.0));
        } else if self.is_defined {
            // Defined (def): bold
            rt = rt.font(FontId::proportional(11.0)).strong();
        } else {
            // Over (not defined, not abstract): italic
            rt = rt.font(FontId::proportional(11.0)).italics();
        }
        rt
    }

    /// Toggle visibility action: returns the new visibility token to write.
    /// Matches Python toggleVis(): invisible -> inherited, visible/inherited -> invisible.
    pub fn toggle_vis_token(&self) -> &'static str {
        if self.visibility == PrimVisibility::Invisible {
            "inherited"
        } else {
            "invisible"
        }
    }

    /// Multi-line tooltip string.
    pub fn tooltip(&self) -> String {
        let mut lines = vec![
            format!("Path: {}", self.path),
            format!(
                "Type: {}",
                if self.type_name.is_empty() {
                    "<none>"
                } else {
                    &self.type_name
                }
            ),
        ];
        if !self.is_active {
            lines.push("INACTIVE".to_string());
        }
        if self.is_instance {
            lines.push("Instance".to_string());
        }
        if self.is_prototype {
            lines.push("Prototype".to_string());
        }
        if self.has_arcs {
            lines.push("Has composition arcs".to_string());
        }
        if !self.is_loaded && !self.is_prototype {
            lines.push("UNLOADED".to_string());
        }
        if matches!(
            self.visibility,
            PrimVisibility::Invisible | PrimVisibility::InheritedInvisible
        ) {
            lines.push("Invisible".to_string());
        }
        if let Some(ref dn) = self.display_name {
            lines.push(format!("Display name: {}", dn));
        }
        lines.join("\n")
    }
}
