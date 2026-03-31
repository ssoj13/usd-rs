//! View menu (largest submenu).

use crate::data_model::{CameraMaskMode, ClearColor, HighlightColor, SelectionHighlightMode};
use crate::keyboard::AppAction;

use super::{
    COMPLEXITY_PRESETS, ColorCorrection, MenuActions, MenuState, RenderMode, compact_menu,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn view_menu(
    ui: &mut egui::Ui,
    menu_state: &mut MenuState,
    result: &mut MenuActions,
    complexity: &mut f64,
    clear_color: &mut ClearColor,
    highlight_color: &mut HighlightColor,
    selection_highlight: &mut SelectionHighlightMode,
    camera_mask: &mut CameraMaskMode,
) {
    ui.menu_button("View", |ui| {
        compact_menu(ui);
        if ui.button("Reset View").clicked() {
            result.actions.push(AppAction::ResetView);
            ui.close();
        }

        if ui.button("Reset Layout").clicked() {
            result.reset_layout = true;
            ui.close();
        }

        if ui.button("Toggle Viewer Mode").clicked() {
            result.actions.push(AppAction::ToggleViewerMode);
            ui.close();
        }

        if ui
            .add(egui::Button::new("Frame Selected").shortcut_text("F"))
            .clicked()
        {
            result.actions.push(AppAction::FrameSelected);
            ui.close();
        }

        if ui
            .add(egui::Button::new("Frame All").shortcut_text("A"))
            .clicked()
        {
            result.actions.push(AppAction::FrameAll);
            ui.close();
        }

        if ui.button("Toggle Framed View").clicked() {
            result.actions.push(AppAction::ToggleFramedView);
            ui.close();
        }

        ui.separator();

        if ui
            .selectable_label(menu_state.orthographic, "Orthographic")
            .on_hover_text("Num5")
            .clicked()
        {
            result.actions.push(AppAction::ToggleOrthographic);
        }

        if ui.button("Free Camera Settings...").clicked() {
            result.adjust_free_camera = true;
            ui.close();
        }

        if ui.button("Default Material Settings...").clicked() {
            result.adjust_default_material = true;
            ui.close();
        }

        ui.separator();

        // Shading Mode submenu (radio group)
        ui.menu_button("Shading Mode", |ui| {
            for &mode in RenderMode::all() {
                if ui
                    .selectable_label(menu_state.render_mode == mode, mode.name())
                    .clicked()
                {
                    menu_state.render_mode = mode;
                    ui.close();
                }
            }
        });

        // Color Correction submenu
        // Reference: usdviewq appController.py _configureColorManagement
        ui.menu_button("Color Correction", |ui| {
            for mode in [
                ColorCorrection::Disabled,
                ColorCorrection::SRGB,
                ColorCorrection::OpenColorIO,
            ] {
                if ui
                    .selectable_label(menu_state.color_correction == mode, mode.name())
                    .clicked()
                {
                    menu_state.color_correction = mode;
                    ui.close();
                }
            }

            // OCIO sub-settings (only when OCIO mode is selected)
            if menu_state.color_correction == ColorCorrection::OpenColorIO
                && !menu_state.ocio_displays.is_empty()
            {
                ui.separator();

                // Display combo
                let display_label = if menu_state.ocio_display.is_empty() {
                    "(default)"
                } else {
                    &menu_state.ocio_display
                };
                egui::ComboBox::from_label("Display")
                    .selected_text(display_label)
                    .show_ui(ui, |ui| {
                        // Empty = use config default
                        if ui
                            .selectable_label(menu_state.ocio_display.is_empty(), "(default)")
                            .clicked()
                        {
                            menu_state.ocio_display.clear();
                            menu_state.ocio_view.clear(); // Reset view on display change
                        }
                        for (name, _) in &menu_state.ocio_displays {
                            if ui
                                .selectable_label(&menu_state.ocio_display == name, name)
                                .clicked()
                            {
                                menu_state.ocio_display = name.clone();
                                menu_state.ocio_view.clear(); // Reset view on display change
                            }
                        }
                    });

                // View combo (filtered by selected display)
                let views = menu_state
                    .ocio_displays
                    .iter()
                    .find(|(d, _)| d == &menu_state.ocio_display)
                    .map(|(_, v)| v.as_slice());
                let view_label = if menu_state.ocio_view.is_empty() {
                    "(default)"
                } else {
                    &menu_state.ocio_view
                };
                egui::ComboBox::from_label("View")
                    .selected_text(view_label)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(menu_state.ocio_view.is_empty(), "(default)")
                            .clicked()
                        {
                            menu_state.ocio_view.clear();
                        }
                        if let Some(view_list) = views {
                            for name in view_list {
                                if ui
                                    .selectable_label(&menu_state.ocio_view == name, name)
                                    .clicked()
                                {
                                    menu_state.ocio_view = name.clone();
                                }
                            }
                        }
                    });

                // Colorspace combo
                if !menu_state.ocio_colorspaces.is_empty() {
                    let cs_label = if menu_state.ocio_colorspace.is_empty() {
                        "(default)"
                    } else {
                        &menu_state.ocio_colorspace
                    };
                    egui::ComboBox::from_label("Colorspace")
                        .selected_text(cs_label)
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(
                                    menu_state.ocio_colorspace.is_empty(),
                                    "(default)",
                                )
                                .clicked()
                            {
                                menu_state.ocio_colorspace.clear();
                            }
                            for name in &menu_state.ocio_colorspaces {
                                if ui
                                    .selectable_label(&menu_state.ocio_colorspace == name, name)
                                    .clicked()
                                {
                                    menu_state.ocio_colorspace = name.clone();
                                }
                            }
                        });
                }

                // Looks (text input)
                ui.horizontal(|ui| {
                    ui.label("Looks:");
                    ui.text_edit_singleline(&mut menu_state.ocio_looks);
                });
            }
        });

        ui.separator();

        // Complexity submenu
        ui.menu_button("Complexity", |ui| {
            for level in COMPLEXITY_PRESETS {
                let label = format!("{} ({:.1})", level.name(), level.value());
                let is_current = (*complexity - level.value()).abs() < 0.05;
                if ui.selectable_label(is_current, label).clicked() {
                    *complexity = level.value();
                    ui.close();
                }
            }
        });

        ui.separator();

        // Clear Color submenu
        ui.menu_button("Clear Color", |ui| {
            for color in [
                ClearColor::Black,
                ClearColor::DarkGrey,
                ClearColor::LightGrey,
                ClearColor::White,
            ] {
                if ui
                    .selectable_label(*clear_color == color, color.name())
                    .clicked()
                {
                    *clear_color = color;
                    ui.close();
                }
            }
        });

        // Highlight Color submenu
        ui.menu_button("Highlight Color", |ui| {
            for color in [
                HighlightColor::White,
                HighlightColor::Yellow,
                HighlightColor::Cyan,
            ] {
                if ui
                    .selectable_label(*highlight_color == color, color.name())
                    .clicked()
                {
                    *highlight_color = color;
                    ui.close();
                }
            }
        });

        // Selection Highlight submenu
        ui.menu_button("Selection Highlight", |ui| {
            for mode in [
                SelectionHighlightMode::Never,
                SelectionHighlightMode::OnlyWhenPaused,
                SelectionHighlightMode::Always,
            ] {
                if ui
                    .selectable_label(*selection_highlight == mode, mode.name())
                    .clicked()
                {
                    *selection_highlight = mode;
                    ui.close();
                }
            }
        });

        ui.separator();

        // Display Purpose checkboxes
        ui.label("Display Purpose:");
        ui.checkbox(&mut menu_state.show_guide_prims, "Guide");
        ui.checkbox(&mut menu_state.show_proxy_prims, "Proxy");
        ui.checkbox(&mut menu_state.show_render_prims, "Render");

        ui.separator();

        // BBoxes
        ui.label("Bounding Boxes:");
        ui.checkbox(&mut menu_state.show_all_bboxes, "Show All");
        ui.checkbox(&mut menu_state.show_aa_bboxes, "Show Axis-Aligned");
        ui.checkbox(&mut menu_state.show_ob_bboxes, "Show Oriented");
        ui.checkbox(
            &mut menu_state.show_bboxes_during_playback,
            "Show During Playback",
        );

        ui.separator();

        // Camera Mask submenu
        ui.menu_button("Camera Mask", |ui| {
            for mode in [
                CameraMaskMode::None,
                CameraMaskMode::Partial,
                CameraMaskMode::Full,
            ] {
                if ui
                    .selectable_label(*camera_mask == mode, mode.name())
                    .clicked()
                {
                    *camera_mask = mode;
                    ui.close();
                }
            }
            ui.separator();
            ui.checkbox(&mut menu_state.camera_mask_outline, "Outline");
            ui.horizontal(|ui| {
                ui.label("Color:");
                let c = menu_state.camera_mask_color;
                let mut srgba = egui::Color32::from_rgba_unmultiplied(
                    (c[0] * 255.0) as u8,
                    (c[1] * 255.0) as u8,
                    (c[2] * 255.0) as u8,
                    (c[3] * 255.0) as u8,
                );
                ui.color_edit_button_srgba(&mut srgba);
                let [r, g, b, a] = srgba.to_array();
                menu_state.camera_mask_color = [
                    r as f32 / 255.0,
                    g as f32 / 255.0,
                    b as f32 / 255.0,
                    a as f32 / 255.0,
                ];
            });
        });

        // Camera Reticles submenu
        ui.menu_button("Camera Reticles", |ui| {
            ui.checkbox(&mut menu_state.camera_reticles_inside, "Inside");
            ui.checkbox(&mut menu_state.camera_reticles_outside, "Outside");
            ui.horizontal(|ui| {
                ui.label("Color:");
                let c = menu_state.camera_reticles_color;
                let mut srgba = egui::Color32::from_rgba_unmultiplied(
                    (c[0] * 255.0) as u8,
                    (c[1] * 255.0) as u8,
                    (c[2] * 255.0) as u8,
                    (c[3] * 255.0) as u8,
                );
                ui.color_edit_button_srgba(&mut srgba);
                let [r, g, b, a] = srgba.to_array();
                menu_state.camera_reticles_color = [
                    r as f32 / 255.0,
                    g as f32 / 255.0,
                    b as f32 / 255.0,
                    a as f32 / 255.0,
                ];
            });
        });

        ui.separator();

        // Rendering toggles
        ui.checkbox(
            &mut menu_state.enable_scene_materials,
            "Enable Scene Materials",
        );
        ui.checkbox(&mut menu_state.enable_scene_lights, "Enable Scene Lights");
        ui.checkbox(&mut menu_state.cull_backfaces, "Cull Backfaces");
        // Shortcut hint "C" next to the auto-clipping checkbox
        ui.horizontal(|ui| {
            ui.checkbox(
                &mut menu_state.auto_clipping_planes,
                "Auto Compute Clipping Planes",
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.weak("C");
            });
        });
        ui.checkbox(&mut menu_state.use_extents_hint, "Use Extents Hint");

        ui.separator();

        // Lighting
        ui.checkbox(&mut menu_state.ambient_light_only, "Ambient Light Only");
        ui.checkbox(&mut menu_state.dome_light_enabled, "Dome Light");
        ui.checkbox(
            &mut menu_state.dome_light_textures_visible,
            "Dome Light Textures Visible",
        );
        if ui.button("Load HDRI...").clicked() {
            result.load_hdri = true;
            ui.close();
        }

        ui.separator();

        ui.checkbox(
            &mut menu_state.display_camera_oracles,
            "Display Camera Oracles",
        );
        ui.checkbox(&mut menu_state.redraw_on_scrub, "Redraw On Scrub");

        ui.separator();

        // Stage Interpolation submenu
        ui.menu_button("Stage Interpolation", |ui| {
            if ui
                .selectable_label(menu_state.interpolation_held, "Held")
                .clicked()
            {
                menu_state.interpolation_held = true;
                ui.close();
            }
            if ui
                .selectable_label(!menu_state.interpolation_held, "Linear")
                .clicked()
            {
                menu_state.interpolation_held = false;
                ui.close();
            }
        });
    });
}
