//! # usd-view
//!
//! Rust/egui USD scene viewer — alternative to usdview (usdviewq).
//!
//! Provides:
//! - 3D viewport with `usd_imaging::gl::Engine`
//! - Prim tree
//! - Attribute inspection
//! - Layer stack
//! - Dockable panels (egui_dock)
//!
//! On wgpu builds the normal viewport path presents the engine color target as
//! a native egui texture and keeps color correction on GPU when possible. CPU
//! readback remains as a fallback path and for explicit capture/export flows.

pub mod app;
pub mod bounds;
pub mod camera;
pub mod data_model;
pub mod dock;
pub mod event_bus;
pub mod events;
pub mod file_watcher;
pub mod formatting;
pub mod keyboard;
pub mod launcher;
pub mod menus;
pub mod panels;
pub mod persistence;
pub mod playback;
pub mod recent_files;
pub mod screenshot;
pub mod status_bar;

pub use app::ViewerApp;
pub use app::ViewerConfig;
pub use data_model::DataModel;
pub use keyboard::AppAction;
pub use launcher::run;
pub use playback::PlaybackState;
