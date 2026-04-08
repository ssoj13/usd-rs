//! File operation methods: save overrides, save flattened, save image, clipboard, editor launch.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::ViewerApp;

/// Resolve a starting directory for file dialogs.
/// Uses parent of `hint` if it exists on disk, otherwise falls back to the executable's directory.
pub(crate) fn dialog_start_dir(hint: Option<&Path>) -> PathBuf {
    if let Some(h) = hint {
        let dir = if h.is_dir() {
            h
        } else {
            h.parent().unwrap_or(h)
        };
        if dir.exists() {
            return dir.to_path_buf();
        }
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

impl ViewerApp {
    /// Save overrides via file dialog.
    ///
    /// Reference: `appController.py:_saveOverridesAs` (lines 2864-2918).
    /// If root layer is anonymous (no file on disk), export it directly.
    /// Otherwise export the session layer and add root as sublayer.
    pub(crate) fn save_overrides_dialog(&mut self) {
        let Some(ref stage) = self.data_model.root.stage else {
            self.last_error = Some("No stage loaded".to_string());
            return;
        };
        let start = dialog_start_dir(self.data_model.root.file_path.as_deref());
        let Some(path) = rfd::FileDialog::new()
            .set_directory(&start)
            .add_filter("USD", &["usd", "usda", "usdc"])
            .save_file()
        else {
            return;
        };

        let root_layer = stage.get_root_layer();

        if root_layer.is_anonymous() {
            // In-memory root (no file) — export root layer directly
            match root_layer.export(&path) {
                Ok(_) => tracing::info!("Saved overrides (root) to {}", path.display()),
                Err(e) => self.last_error = Some(format!("Failed to save overrides: {e}")),
            }
        } else {
            // File-backed root — export session layer, add root as sublayer
            let Some(session_layer) = stage.get_session_layer() else {
                self.last_error = Some("No session layer".to_string());
                return;
            };
            if let Err(e) = session_layer.export(&path) {
                self.last_error = Some(format!("Failed to export session layer: {e}"));
                return;
            }

            // Re-open the exported file to add metadata + sublayer ref
            let path_str = path.to_string_lossy().replace('\\', "/");
            match usd_sdf::Layer::find_or_open(&path_str) {
                Ok(target_layer) => {
                    // Copy metadata from root (skip sublayer fields)
                    usd_utils::copy_layer_metadata(&root_layer, &target_layer, true, false);

                    // Append root layer's real path as sublayer
                    if let Some(root_real) = root_layer.get_resolved_path() {
                        let mut sub_paths = target_layer.sublayer_paths();
                        sub_paths.push(root_real);
                        target_layer.set_sublayer_paths(&sub_paths);
                    }

                    target_layer.remove_inert_scene_description();
                    match target_layer.save() {
                        Ok(_) => tracing::info!("Saved overrides to {}", path.display()),
                        Err(e) => {
                            self.last_error = Some(format!("Failed to save target layer: {e}"))
                        }
                    }
                }
                Err(e) => {
                    self.last_error = Some(format!("Failed to open exported layer: {e}"));
                }
            }
        }
    }

    /// Flatten stage to a single layer and save via file dialog.
    pub(crate) fn save_flattened_dialog(&mut self) {
        let Some(ref stage) = self.data_model.root.stage else {
            self.last_error = Some("No stage loaded".to_string());
            return;
        };
        let start = dialog_start_dir(self.data_model.root.file_path.as_deref());
        let Some(path) = rfd::FileDialog::new()
            .set_directory(&start)
            .add_filter("USD", &["usd", "usda", "usdc"])
            .save_file()
        else {
            return;
        };
        let path_str = path.to_string_lossy().replace('\\', "/");
        match stage.export(&path_str, false) {
            Ok(_) => tracing::info!("Saved flattened stage to {}", path_str),
            Err(e) => self.last_error = Some(format!("Failed to save flattened: {e}")),
        }
    }

    /// Save viewport image to PNG/JPEG or EXR via file dialog.
    pub(crate) fn save_image_dialog(&mut self) {
        let start = dialog_start_dir(self.data_model.root.file_path.as_deref());
        let Some(path) = rfd::FileDialog::new()
            .set_directory(&start)
            .add_filter("PNG", &["png"])
            .add_filter("JPEG", &["jpg", "jpeg"])
            .add_filter("OpenEXR", &["exr"])
            .save_file()
        else {
            return;
        };
        let result = match crate::screenshot::detect_format(&path) {
            Ok(crate::screenshot::ScreenshotFormat::Exr) => {
                let Some((pixels, w, h)) =
                    crate::panels::viewport::capture_current_frame_linear(&mut self.engine)
                else {
                    self.last_error = Some("No rendered frame available yet".to_string());
                    return;
                };
                crate::screenshot::save_exr(&pixels, w, h, &path)
            }
            Ok(
                crate::screenshot::ScreenshotFormat::Png
                | crate::screenshot::ScreenshotFormat::Jpeg,
            ) => {
                let Some((pixels, w, h)) = crate::panels::viewport::capture_current_frame(
                    &mut self.engine,
                    &mut self.viewport_state,
                    &self.data_model,
                ) else {
                    self.last_error = Some("No rendered frame available yet".to_string());
                    return;
                };
                crate::screenshot::save_ldr(&pixels, w, h, &path)
            }
            Err(e) => Err(e),
        };
        if let Err(e) = result {
            self.last_error = Some(format!("Failed to save image: {e}"));
        } else {
            tracing::info!("Saved viewport image to {}", path.display());
        }
    }

    /// Copy viewport image info to clipboard.
    ///
    /// egui's clipboard is text-only; binary image clipboard requires a
    /// platform crate (arboard) not yet in deps. For now we save to a temp
    /// file and copy the path — useful for quick paste into other apps.
    pub(crate) fn copy_image_to_clipboard(&mut self, ctx: &egui::Context) {
        let Some((pixels, w, h)) = crate::panels::viewport::capture_current_frame(
            &mut self.engine,
            &mut self.viewport_state,
            &self.data_model,
        ) else {
            self.last_error = Some("No rendered frame available yet".to_string());
            return;
        };
        // Write to a temp PNG and put the path on the clipboard
        let tmp = std::env::temp_dir().join("usdview_clipboard.png");
        match crate::screenshot::save_ldr(&pixels, w, h, &tmp) {
            Ok(()) => {
                ctx.copy_text(tmp.to_string_lossy().into_owned());
                tracing::info!("Viewport saved to temp: {}", tmp.display());
            }
            Err(e) => self.last_error = Some(format!("Failed to copy image: {e}")),
        }
    }

    /// Open a file dialog to pick an HDRI (.hdr / .exr) file for the fallback dome light.
    ///
    /// The selected path is forwarded to the engine via `set_dome_light_texture_path()`.
    /// Dome light is automatically enabled when a valid path is chosen.
    pub(crate) fn load_hdri_dialog(&mut self) {
        let hint = self.data_model.view.hdr_path.as_ref().map(PathBuf::from);
        let start = dialog_start_dir(hint.as_deref());
        let Some(path) = rfd::FileDialog::new()
            .set_title("Load HDRI Environment")
            .set_directory(&start)
            .add_filter("HDR / EXR", &["hdr", "exr"])
            .add_filter("All files", &["*"])
            .pick_file()
        else {
            return;
        };

        let path_str = path.to_string_lossy().replace('\\', "/");
        tracing::info!("Loading HDRI: {}", path_str);

        #[cfg(feature = "wgpu")]
        {
            self.engine
                .set_dome_light_texture_path(Some(path_str.clone()));
            self.data_model.view.hdr_path = Some(path_str);
            // Enable dome light so the HDRI is actually used.
            self.engine.set_dome_light_enabled(true);
            self.data_model.view.dome_light_enabled = true;
        }
        #[cfg(not(feature = "wgpu"))]
        {
            tracing::warn!("HDRI loading requires the 'wgpu' feature.");
        }
    }

    /// Opens layer file in editor (reference: OpenLayerMenuItem).
    /// Tries usdedit first, then $EDITOR / $USD_EDITOR.
    pub(crate) fn open_layer_in_editor(layer_path: &str) -> Result<(), String> {
        let path = PathBuf::from(layer_path);
        if !path.exists() {
            return Err(format!("Layer file does not exist: {}", layer_path));
        }
        let layer_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| format!("{}.tmp", s))
            .unwrap_or_else(|| "layer.tmp".to_string());

        // Try usdedit (reference uses FindUsdBinary('usdedit'))
        if Self::command_exists("usdedit") {
            tracing::info!("Opening layer in usdedit: {}", layer_path);
            Command::new("usdedit")
                .args(["-n", layer_path, "-p", &layer_name])
                .spawn()
                .map_err(|e| format!("Failed to spawn usdedit: {}", e))?;
            return Ok(());
        }

        // Fallback: EDITOR or USD_EDITOR
        let editor = std::env::var("USD_EDITOR")
            .ok()
            .or_else(|| std::env::var("EDITOR").ok())
            .or_else(|| std::env::var("VISUAL").ok());
        if let Some(editor) = editor {
            let editor = editor.split_whitespace().next().unwrap_or(&editor);
            if Self::command_exists(editor) {
                tracing::info!("Opening layer in {}: {}", editor, layer_path);
                Command::new(editor)
                    .arg(layer_path)
                    .spawn()
                    .map_err(|e| format!("Failed to spawn {}: {}", editor, e))?;
                return Ok(());
            }
        }

        // Platform default: start (Windows), xdg-open (Linux), open (macOS)
        #[cfg(windows)]
        {
            Command::new("cmd")
                .args(["/C", "start", "", layer_path])
                .spawn()
                .map_err(|e| format!("Failed to open: {}", e))?;
            return Ok(());
        }
        #[cfg(target_os = "macos")]
        {
            Command::new("open")
                .arg(layer_path)
                .spawn()
                .map_err(|e| format!("Failed to open: {}", e))?;
            return Ok(());
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            Command::new("xdg-open")
                .arg(layer_path)
                .spawn()
                .map_err(|e| format!("Failed to open: {}", e))?;
            return Ok(());
        }
        #[allow(unreachable_code)]
        Err("No editor found. Set USD_EDITOR or EDITOR, or install usdedit.".to_string())
    }

    /// Opens layer in new usdview instance (reference: UsdviewLayerMenuItem).
    pub(crate) fn open_layer_in_usdview(layer_path: &str) -> Result<(), String> {
        let exe =
            std::env::current_exe().map_err(|e| format!("Cannot get executable path: {}", e))?;
        tracing::info!("Spawning usdview: {}", layer_path);
        Command::new(&exe)
            .arg(layer_path)
            .spawn()
            .map_err(|e| format!("Failed to spawn usdview: {}", e))?;
        Ok(())
    }

    pub(crate) fn command_exists(cmd: &str) -> bool {
        #[cfg(windows)]
        return Command::new("where")
            .arg(cmd)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        #[cfg(not(windows))]
        return Command::new("which")
            .arg(cmd)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
    }
}
