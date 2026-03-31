//! Event types for usd-view EventBus.
//!
//! Stage loading events flow from background thread -> UI thread.
//! Sync progress events flow from engine -> viewport overlay.

use std::path::PathBuf;
use std::sync::Arc;
use usd_core::Stage;

/// Request to load a stage file (emitted by UI, consumed by loader).
#[derive(Clone, Debug)]
pub struct StageLoadRequested {
    pub path: PathBuf,
}

/// Stage successfully loaded in background thread.
#[derive(Clone)]
pub struct StageLoaded {
    pub stage: Arc<Stage>,
    pub path: PathBuf,
    pub time_samples: Vec<f64>,
    pub generation: u64,
}

/// Stage loading failed in background thread.
#[derive(Clone, Debug)]
pub struct StageLoadFailed {
    pub path: PathBuf,
    pub error: String,
    pub generation: u64,
}

/// Progress update during stage loading.
#[derive(Clone, Debug)]
pub struct LoadProgress {
    pub phase: LoadPhase,
    pub progress: f32,
    pub message: String,
    pub generation: u64,
}

/// Phases of background stage loading.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadPhase {
    Opening,
    Composing,
    TimeSamples,
    Ready,
}

impl std::fmt::Display for LoadPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Opening => write!(f, "Opening"),
            Self::Composing => write!(f, "Composing"),
            Self::TimeSamples => write!(f, "Collecting time samples"),
            Self::Ready => write!(f, "Ready"),
        }
    }
}
