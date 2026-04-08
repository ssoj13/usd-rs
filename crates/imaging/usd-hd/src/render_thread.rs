//! HdRenderThread - Background render thread with state machine.
//!
//! Corresponds to pxr/imaging/hd/renderThread.h.
//! Synchronizes between Hydra (main/sync threads) and render thread.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};

/// Render thread state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Not started.
    Initial,
    /// Running, idle (no rendering).
    Idle,
    /// Running, rendering.
    Rendering,
    /// Shutting down.
    Terminated,
}

/// Shared state for render thread.
struct RenderThreadState {
    render_callback: Mutex<Option<Box<dyn FnMut() + Send>>>,
    shutdown_callback: Mutex<Option<Box<dyn FnOnce() + Send>>>,
    requested_state: Mutex<State>,
    requested_state_cv: Condvar,
    enable_render: AtomicBool,
    pause_render: AtomicBool,
    pause_dirty: AtomicBool,
    rendering: AtomicBool,
}

/// Utility for background rendering with sync support.
///
/// Corresponds to C++ `HdRenderThread`.
/// States: Initial -> Idle (StartThread) -> Rendering (StartRender) -> Idle (StopRender).
pub struct HdRenderThread {
    state: Arc<RenderThreadState>,
    render_thread: Mutex<Option<JoinHandle<()>>>,
    frame_buffer_mutex: Mutex<()>,
}

impl Default for HdRenderThread {
    fn default() -> Self {
        Self::new()
    }
}

impl HdRenderThread {
    /// Create new render thread.
    pub fn new() -> Self {
        Self {
            state: Arc::new(RenderThreadState {
                render_callback: Mutex::new(None),
                shutdown_callback: Mutex::new(None),
                requested_state: Mutex::new(State::Initial),
                requested_state_cv: Condvar::new(),
                enable_render: AtomicBool::new(false),
                pause_render: AtomicBool::new(false),
                pause_dirty: AtomicBool::new(false),
                rendering: AtomicBool::new(false),
            }),
            render_thread: Mutex::new(None),
            frame_buffer_mutex: Mutex::new(()),
        }
    }

    /// Set render callback (called from render thread).
    pub fn set_render_callback<F: FnMut() + Send + 'static>(&self, cb: F) {
        *self.state.render_callback.lock().expect("poisoned") = Some(Box::new(cb));
    }

    /// Set shutdown callback (called once before thread exits).
    pub fn set_shutdown_callback<F: FnOnce() + Send + 'static>(&self, cb: F) {
        *self.state.shutdown_callback.lock().expect("poisoned") = Some(Box::new(cb));
    }

    /// Start the background render thread.
    pub fn start_thread(&self) {
        let mut rt = self.render_thread.lock().expect("poisoned");
        if rt.is_some() {
            return;
        }
        let state = Arc::clone(&self.state);
        let handle = thread::spawn(move || Self::render_loop(state));
        *rt = Some(handle);
        *self.state.requested_state.lock().expect("poisoned") = State::Idle;
    }

    /// Stop the render thread (blocks until joined).
    pub fn stop_thread(&self) {
        {
            let mut s = self.state.requested_state.lock().expect("poisoned");
            *s = State::Terminated;
            self.state.enable_render.store(false, Ordering::SeqCst);
            self.state.requested_state_cv.notify_all();
        }
        let mut rt = self.render_thread.lock().expect("poisoned");
        if let Some(handle) = rt.take() {
            let _ = handle.join();
        }
    }

    /// Check if thread is running.
    pub fn is_thread_running(&self) -> bool {
        self.render_thread.lock().expect("poisoned").is_some()
    }

    /// Ask render thread to start rendering. May block briefly.
    pub fn start_render(&self) {
        let mut s = self.state.requested_state.lock().expect("poisoned");
        if *s == State::Rendering {
            return;
        }
        self.state.enable_render.store(true, Ordering::SeqCst);
        *s = State::Rendering;
        self.state.requested_state_cv.notify_all();
    }

    /// Ask render thread to stop and block until idle.
    pub fn stop_render(&self) {
        self.state.enable_render.store(false, Ordering::SeqCst);
        self.state.requested_state_cv.notify_all();
        while self.state.rendering.load(Ordering::SeqCst) {
            std::hint::spin_loop();
        }
        *self.state.requested_state.lock().expect("poisoned") = State::Idle;
    }

    /// Query if currently rendering.
    pub fn is_rendering(&self) -> bool {
        self.state.rendering.load(Ordering::SeqCst)
    }

    /// Request pause.
    pub fn pause_render(&self) {
        self.state.pause_render.store(true, Ordering::SeqCst);
        self.state.pause_dirty.store(true, Ordering::SeqCst);
    }

    /// Request resume.
    pub fn resume_render(&self) {
        self.state.pause_render.store(false, Ordering::SeqCst);
        self.state.pause_dirty.store(true, Ordering::SeqCst);
    }

    /// Check if stop requested (call from render callback).
    pub fn is_stop_requested(&self) -> bool {
        !self.state.enable_render.load(Ordering::SeqCst)
    }

    /// Check if pause requested (call from render callback).
    pub fn is_pause_requested(&self) -> bool {
        self.state.pause_render.load(Ordering::SeqCst)
    }

    /// Check if pause state changed.
    pub fn is_pause_dirty(&self) -> bool {
        self.state.pause_dirty.swap(false, Ordering::SeqCst)
    }

    /// Lock framebuffer for blit sync.
    pub fn lock_framebuffer(&self) -> std::sync::MutexGuard<'_, ()> {
        self.frame_buffer_mutex.lock().expect("poisoned")
    }

    /// C++ renderThread.cpp:148-164 `_RenderLoop()`:
    /// 1. Wait for state != Idle (condvar with predicate)
    /// 2. If Terminated, break
    /// 3. Reset state to Idle (so each StartRender triggers one render pass)
    /// 4. Unlock, call render callback, loop
    /// 5. After loop exits, call shutdown callback
    fn render_loop(state: Arc<RenderThreadState>) {
        loop {
            let mut s = state.requested_state.lock().expect("poisoned");
            // C++ uses wait with predicate: wait until state != Idle/Initial
            while *s == State::Idle || *s == State::Initial {
                s = state.requested_state_cv.wait(s).expect("poisoned");
            }
            if *s == State::Terminated {
                break;
            }
            // C++ renderThread.cpp:158: reset to Idle after waking.
            // Without this, the loop busy-spins calling the callback
            // repeatedly instead of once per StartRender().
            *s = State::Idle;
            drop(s);

            state.rendering.store(true, Ordering::SeqCst);
            if let Some(ref mut cb) = *state.render_callback.lock().expect("poisoned") {
                cb();
            }
            state.rendering.store(false, Ordering::SeqCst);
        }
        // C++ renderThread.cpp:164: shutdown callback called AFTER loop exits,
        // not before. This ensures all rendering is complete before cleanup.
        if let Some(shutdown) = state.shutdown_callback.lock().expect("poisoned").take() {
            shutdown();
        }
    }
}
