//! Keyboard shortcut handler.
//!
//! Maps key combinations to application actions, matching usdview reference shortcuts.

use crate::menus::RenderMode;

/// Application actions triggered by keyboard shortcuts or menu items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppAction {
    /// Open file dialog (Ctrl+O).
    OpenFile,
    /// Quit application (Ctrl+Q).
    Quit,
    /// Dismiss topmost dialog or quit if none open (Escape).
    Escape,
    /// Toggle fullscreen/maximize.
    ToggleFullscreen,
    /// Toggle play/stop (Space).
    TogglePlay,
    /// Toggle reverse playback (Shift+Space).
    ReversePlay,
    /// Step one frame forward (Right arrow).
    StepForward,
    /// Step one frame backward (Left arrow).
    StepBackward,
    /// Frame selected prim in viewport (F).
    FrameSelected,
    /// Frame all geometry in viewport (A).
    FrameAll,
    /// Increment mesh complexity (+/=).
    IncrementComplexity,
    /// Decrement mesh complexity (-).
    DecrementComplexity,
    /// Toggle visibility on selected prim (Shift+H).
    MakeVisible,
    /// Open find prims dialog (Ctrl+F).
    FindPrims,
    /// Reload all layers (F5).
    ReloadAllLayers,
    /// Toggle loop mode.
    ToggleLoop,
    /// Select stage root.
    SelectStageRoot,
    /// Select enclosing model roots for selected prims.
    SelectModelRoot,
    /// Reset viewport camera.
    ResetView,
    /// Toggle viewer mode.
    ToggleViewerMode,
    /// Open preferences dialog (Ctrl+, or F12).
    OpenPreferences,
    /// Navigate to next USD file in directory (PgDn).
    NextFile,
    /// Navigate to previous USD file in directory (PgUp).
    PrevFile,
    /// Make selected prims invisible (Ctrl+H).
    MakeInvisible,
    /// Show only selected prims.
    VisOnly,
    /// Remove session visibility for selected prims.
    RemoveSessionVis,
    /// Reset all session visibility.
    ResetAllSessionVis,
    /// Load selected prims' payloads.
    LoadSelected,
    /// Unload selected prims' payloads.
    UnloadSelected,
    /// Activate selected prims.
    ActivateSelected,
    /// Deactivate selected prims.
    DeactivateSelected,
    /// Toggle auto compute clipping planes (C).
    ToggleAutoClippingPlanes,
    /// Pause the renderer (Ctrl+P).
    PauseRender,
    /// Stop the renderer (Ctrl+Backslash).
    StopRender,
    /// Set render/shading mode (Ctrl+1..9).
    SetRenderMode(RenderMode),
    /// Toggle orthographic/perspective projection (Numpad5).
    ToggleOrthographic,
    /// Save current stage root layer (Ctrl+S).
    SaveFile,
    /// Copy selected prim path to clipboard.
    CopyPrimPath,
    /// Open spline viewer panel.
    OpenSplineViewer,
    /// Select bound preview material for selected prims.
    SelectBoundPreviewMaterial,
    /// Select bound full material for selected prims.
    SelectBoundFullMaterial,
    /// Toggle between current view and the pre-frame saved camera state.
    ToggleFramedView,
    /// Undo the last camera manipulation (restore from undo stack).
    UndoCameraMove,
    /// Switch to a scene camera by path, or free camera if None.
    SetCamera(Option<String>),
    /// Switch renderer plugin by ID.
    SetRenderer(String),
    /// Switch AOV output.
    SetAOV(String),
}

/// Processes keyboard input and returns pending actions.
pub struct KeyboardHandler;

impl KeyboardHandler {
    /// Process keyboard input for the current frame.
    /// Returns a list of actions to dispatch.
    pub fn process(ctx: &egui::Context) -> Vec<AppAction> {
        let mut actions = Vec::new();

        ctx.input(|input| {
            let ctrl = input.modifiers.ctrl || input.modifiers.command;
            let shift = input.modifiers.shift;

            // Ctrl+O: open file
            if ctrl && input.key_pressed(egui::Key::O) {
                actions.push(AppAction::OpenFile);
            }

            // Ctrl+S: save stage
            if ctrl && input.key_pressed(egui::Key::S) {
                actions.push(AppAction::SaveFile);
            }

            // Ctrl+Q: quit
            if ctrl && input.key_pressed(egui::Key::Q) {
                actions.push(AppAction::Quit);
            }

            // Escape: dismiss dialog or quit
            if input.key_pressed(egui::Key::Escape) {
                actions.push(AppAction::Escape);
            }

            // Shift+Space: reverse play toggle
            if shift && input.key_pressed(egui::Key::Space) {
                actions.push(AppAction::ReversePlay);
            }
            // Space: play/stop toggle
            else if input.key_pressed(egui::Key::Space) {
                actions.push(AppAction::TogglePlay);
            }

            // Right arrow: step forward
            if input.key_pressed(egui::Key::ArrowRight) && !ctrl {
                actions.push(AppAction::StepForward);
            }

            // Left arrow: step backward
            if input.key_pressed(egui::Key::ArrowLeft) && !ctrl {
                actions.push(AppAction::StepBackward);
            }

            // F: frame selected
            if input.key_pressed(egui::Key::F) && !ctrl && !shift {
                actions.push(AppAction::FrameSelected);
            }

            // A: frame all (standard DCC shortcut)
            if input.key_pressed(egui::Key::A) && !ctrl && !shift {
                actions.push(AppAction::FrameAll);
            }

            // Ctrl+/= : increment complexity
            if ctrl && (input.key_pressed(egui::Key::Plus) || input.key_pressed(egui::Key::Equals))
            {
                actions.push(AppAction::IncrementComplexity);
            }

            // Ctrl+- : decrement complexity
            if ctrl && input.key_pressed(egui::Key::Minus) {
                actions.push(AppAction::DecrementComplexity);
            }

            // Shift+H: toggle visibility
            if shift && input.key_pressed(egui::Key::H) {
                actions.push(AppAction::MakeVisible);
            }

            // Ctrl+F: find prims
            if ctrl && input.key_pressed(egui::Key::F) {
                actions.push(AppAction::FindPrims);
            }

            // L: toggle loop
            if input.key_pressed(egui::Key::L) && !ctrl && !shift {
                actions.push(AppAction::ToggleLoop);
            }

            // J: toggle framed view
            if input.key_pressed(egui::Key::J) && !ctrl && !shift {
                actions.push(AppAction::ToggleFramedView);
            }

            // Ctrl+,: open preferences
            if ctrl && input.key_pressed(egui::Key::Comma) {
                actions.push(AppAction::OpenPreferences);
            }

            // F12: open preferences
            if input.key_pressed(egui::Key::F12) {
                actions.push(AppAction::OpenPreferences);
            }

            // F5: reload all layers
            if input.key_pressed(egui::Key::F5) && !ctrl && !shift {
                actions.push(AppAction::ReloadAllLayers);
            }

            // F11: toggle viewer-only mode (not Z — Z is not a usdview shortcut)
            if input.key_pressed(egui::Key::F11) && !ctrl && !shift {
                actions.push(AppAction::ToggleViewerMode);
            }

            // PgDn: next file in directory
            if input.key_pressed(egui::Key::PageDown) && !ctrl {
                actions.push(AppAction::NextFile);
            }

            // PgUp: previous file in directory
            if input.key_pressed(egui::Key::PageUp) && !ctrl {
                actions.push(AppAction::PrevFile);
            }

            // Ctrl+H: make invisible
            if ctrl && input.key_pressed(egui::Key::H) {
                actions.push(AppAction::MakeInvisible);
            }

            // C: toggle auto compute clipping planes
            if input.key_pressed(egui::Key::C) && !ctrl && !shift {
                actions.push(AppAction::ToggleAutoClippingPlanes);
            }

            // Ctrl+P: pause render
            if ctrl && input.key_pressed(egui::Key::P) {
                actions.push(AppAction::PauseRender);
            }

            // Ctrl+Backslash: stop render
            if ctrl && input.key_pressed(egui::Key::Backslash) {
                actions.push(AppAction::StopRender);
            }

            // Ctrl+1..9: render/shading mode presets
            // 1=Wireframe 2=WireframeOnSurface 3=SmoothShaded 4=FlatShaded
            // 5=Points 6=GeomOnly 7=GeomSmooth 8=GeomFlat 9=HiddenSurfaceWireframe
            if ctrl {
                let mode = if input.key_pressed(egui::Key::Num1) {
                    Some(RenderMode::Wireframe)
                } else if input.key_pressed(egui::Key::Num2) {
                    Some(RenderMode::WireframeOnSurface)
                } else if input.key_pressed(egui::Key::Num3) {
                    Some(RenderMode::SmoothShaded)
                } else if input.key_pressed(egui::Key::Num4) {
                    Some(RenderMode::FlatShaded)
                } else if input.key_pressed(egui::Key::Num5) {
                    Some(RenderMode::Points)
                } else if input.key_pressed(egui::Key::Num6) {
                    Some(RenderMode::GeomOnly)
                } else if input.key_pressed(egui::Key::Num7) {
                    Some(RenderMode::GeomSmooth)
                } else if input.key_pressed(egui::Key::Num8) {
                    Some(RenderMode::GeomFlat)
                } else if input.key_pressed(egui::Key::Num9) {
                    Some(RenderMode::HiddenSurfaceWireframe)
                } else {
                    None
                };
                if let Some(m) = mode {
                    actions.push(AppAction::SetRenderMode(m));
                }
            }
            // Numpad5 without ctrl: toggle orthographic (DCC standard, matches Blender/Maya)
            if input.key_pressed(egui::Key::Num5) && !ctrl && !shift {
                actions.push(AppAction::ToggleOrthographic);
            }

        });

        actions
    }
}
