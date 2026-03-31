//! Playback system for USD timeline animation.
//!
//! FPS-synchronized playback with loop, step, and frame clamping.

use std::time::Instant;

/// Timeline playback state with FPS-synchronized advance.
#[derive(Debug)]
pub struct PlaybackState {
    /// Currently playing (advancing frames).
    playing: bool,
    /// Paused (playing but halted — resume doesn't reset).
    paused: bool,
    /// Loop mode: wrap to start when reaching end.
    looping: bool,
    /// Target framerate (default 24.0).
    target_fps: f64,
    /// Frame step size (default 1.0).
    step_size: f64,
    /// Reverse playback direction.
    reverse: bool,
    /// Timestamp of last frame advance (for FPS sync).
    last_frame_time: Instant,
    /// Frame range (start, end) inclusive.
    frame_range: (f64, f64),
    /// Current frame.
    current_frame: f64,
    /// Was playing before scrub started (for scrub-pause-resume).
    scrub_was_playing: bool,
    /// Time when scrub ended (for 500ms resume delay).
    scrub_end_time: Option<Instant>,
    /// True while the user is actively dragging the frame slider.
    scrubbing: bool,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self::new()
    }
}

impl PlaybackState {
    /// Creates new playback state with default 24fps, range 1..100.
    pub fn new() -> Self {
        Self {
            playing: false,
            paused: false,
            looping: true,
            target_fps: 24.0,
            step_size: 1.0,
            reverse: false,
            last_frame_time: Instant::now(),
            frame_range: (1.0, 100.0),
            current_frame: 1.0,
            scrub_was_playing: false,
            scrub_end_time: None,
            scrubbing: false,
        }
    }

    // --- Accessors ---

    pub fn is_playing(&self) -> bool {
        self.playing && !self.paused
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn is_looping(&self) -> bool {
        self.looping
    }

    pub fn set_looping(&mut self, looping: bool) {
        self.looping = looping;
    }

    pub fn toggle_looping(&mut self) {
        self.looping = !self.looping;
    }

    pub fn current_frame(&self) -> f64 {
        self.current_frame
    }

    /// True while the user is actively scrubbing the frame slider.
    pub fn is_scrubbing(&self) -> bool {
        self.scrubbing
    }

    /// Called when user starts scrubbing the frame slider.
    /// Pauses playback if playing, remembers state for resume.
    pub fn scrub_start(&mut self) {
        self.scrubbing = true;
        self.scrub_was_playing = self.is_playing();
        if self.scrub_was_playing {
            self.paused = true;
        }
        self.scrub_end_time = None;
    }

    /// Called when user releases the frame slider.
    /// Starts 500ms timer to resume playback if it was playing.
    pub fn scrub_end(&mut self) {
        self.scrubbing = false;
        if self.scrub_was_playing {
            self.scrub_end_time = Some(Instant::now());
        }
    }

    /// Call each frame to check if scrub-resume timer expired (500ms).
    pub fn check_scrub_resume(&mut self) {
        if let Some(end) = self.scrub_end_time {
            if end.elapsed() >= std::time::Duration::from_millis(500) {
                self.paused = false;
                self.scrub_was_playing = false;
                self.scrub_end_time = None;
                self.last_frame_time = Instant::now();
            }
        }
    }

    pub fn frame_range(&self) -> (f64, f64) {
        self.frame_range
    }

    pub fn target_fps(&self) -> f64 {
        self.target_fps
    }

    pub fn step_size(&self) -> f64 {
        self.step_size
    }

    pub fn is_reverse(&self) -> bool {
        self.reverse
    }

    // --- Playback controls ---

    /// Start forward playback from current position.
    pub fn play(&mut self) {
        self.playing = true;
        self.paused = false;
        self.reverse = false;
        self.last_frame_time = Instant::now();
    }

    /// Start reverse playback from current position.
    pub fn play_reverse(&mut self) {
        self.playing = true;
        self.paused = false;
        self.reverse = true;
        self.last_frame_time = Instant::now();
    }

    /// Toggle between forward and reverse play (while keeping play state).
    pub fn toggle_reverse(&mut self) {
        if self.playing && !self.paused {
            self.reverse = !self.reverse;
        } else {
            // Start playing in reverse
            self.play_reverse();
        }
    }

    /// Pause playback (can resume without resetting).
    pub fn pause(&mut self) {
        self.paused = true;
    }

    /// Stop playback and reset to first frame.
    pub fn stop(&mut self) {
        self.playing = false;
        self.paused = false;
        self.current_frame = self.frame_range.0;
    }

    /// Toggle between play and pause (spacebar behavior).
    /// Does NOT reset frame — use stop() for that.
    pub fn toggle_play(&mut self) {
        if self.playing && !self.paused {
            // Pause in place — preserve current_frame.
            self.playing = false;
        } else {
            self.play();
        }
    }

    /// Advance playback by elapsed time. Returns Some(frame) if a new frame
    /// should be displayed, None if not enough time has elapsed.
    pub fn advance(&mut self) -> Option<f64> {
        if !self.playing || self.paused {
            return None;
        }

        let now = Instant::now();
        let elapsed = now.duration_since(self.last_frame_time).as_secs_f64();
        let frame_duration = 1.0 / self.target_fps;

        if elapsed < frame_duration {
            return None;
        }

        // Always advance exactly 1 step per tick (C++ parity).
        // Dropping frames causes visual judder; keeping constant step
        // ensures smooth playback at the target FPS.
        self.last_frame_time = now;
        let delta = self.step_size;
        let mut frame = if self.reverse {
            self.current_frame - delta
        } else {
            self.current_frame + delta
        };

        // Handle end-of-range (forward and reverse)
        let (start, end) = self.frame_range;
        if frame > end {
            if self.looping {
                let range = end - start;
                frame = if range > 0.0 {
                    start + ((frame - start) % range)
                } else {
                    start
                };
            } else {
                frame = end;
                self.playing = false;
            }
        } else if frame < start {
            if self.looping {
                let range = end - start;
                frame = if range > 0.0 {
                    end - ((start - frame) % range)
                } else {
                    end
                };
            } else {
                frame = start;
                self.playing = false;
            }
        }

        self.current_frame = frame;
        Some(self.current_frame)
    }

    /// Step forward by given amount, clamp to range.
    pub fn step_forward(&mut self, step: f64) -> f64 {
        self.playing = false;
        self.paused = false;
        let next = self.current_frame + step;
        // Per C++ _advanceFrame: wrap around when past end (not clamp).
        self.current_frame = if next > self.frame_range.1 {
            self.frame_range.0
        } else {
            next
        };
        self.current_frame
    }

    /// Step backward by given amount, wrap to end when past start.
    pub fn step_backward(&mut self, step: f64) -> f64 {
        self.playing = false;
        self.paused = false;
        let prev = self.current_frame - step;
        // Per C++ _retreatFrame: wrap around when past start.
        self.current_frame = if prev < self.frame_range.0 {
            self.frame_range.1
        } else {
            prev
        };
        self.current_frame
    }

    /// Set frame directly, clamped to range.
    pub fn set_frame(&mut self, frame: f64) -> f64 {
        self.current_frame = frame.clamp(self.frame_range.0, self.frame_range.1);
        self.current_frame
    }

    /// Update frame range (e.g. from stage time code range).
    pub fn set_frame_range(&mut self, start: f64, end: f64) {
        self.frame_range = (start, end);
        // Clamp current frame to new range
        self.current_frame = self.current_frame.clamp(start, end);
    }

    /// Set target FPS (clamped to 1..=240).
    pub fn set_fps(&mut self, fps: f64) {
        self.target_fps = fps.clamp(1.0, 240.0);
    }

    /// Set step size.
    pub fn set_step_size(&mut self, step: f64) {
        self.step_size = step.max(0.001);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_defaults() {
        let pb = PlaybackState::new();
        assert!(!pb.is_playing());
        assert!(!pb.is_paused());
        assert!(pb.is_looping());
        assert!((pb.target_fps() - 24.0).abs() < f64::EPSILON);
        assert!((pb.current_frame() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_toggle_play() {
        let mut pb = PlaybackState::new();
        pb.toggle_play();
        assert!(pb.is_playing());
        pb.toggle_play();
        assert!(!pb.is_playing());
        // After toggle-off (pause), frame is preserved — NOT reset.
        // set_frame was not called, so it stays at start only by coincidence;
        // test the invariant that frame is not forcibly reset:
        let frame_after = pb.current_frame();
        assert!((frame_after - 1.0).abs() < f64::EPSILON); // still 1.0 (was never advanced)
    }

    #[test]
    fn test_step() {
        let mut pb = PlaybackState::new();
        pb.set_frame_range(1.0, 10.0);
        pb.set_frame(5.0);
        assert!((pb.step_forward(1.0) - 6.0).abs() < f64::EPSILON);
        assert!((pb.step_backward(3.0) - 3.0).abs() < f64::EPSILON);
        // Wrap at boundaries (matches C++ usdview behavior)
        pb.set_frame(1.0);
        let wrapped = pb.step_backward(1.0);
        assert!(
            (wrapped - 10.0).abs() < f64::EPSILON,
            "backward wrap: {wrapped}"
        );
        pb.set_frame(10.0);
        let wrapped = pb.step_forward(1.0);
        assert!(
            (wrapped - 1.0).abs() < f64::EPSILON,
            "forward wrap: {wrapped}"
        );
    }

    #[test]
    fn test_set_frame_clamp() {
        let mut pb = PlaybackState::new();
        pb.set_frame_range(10.0, 50.0);
        assert!((pb.set_frame(0.0) - 10.0).abs() < f64::EPSILON);
        assert!((pb.set_frame(999.0) - 50.0).abs() < f64::EPSILON);
        assert!((pb.set_frame(25.0) - 25.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_reverse_play() {
        let mut pb = PlaybackState::new();
        assert!(!pb.is_reverse());
        pb.play_reverse();
        assert!(pb.is_playing());
        assert!(pb.is_reverse());
        // Forward play clears reverse
        pb.play();
        assert!(pb.is_playing());
        assert!(!pb.is_reverse());
    }

    #[test]
    fn test_toggle_reverse() {
        let mut pb = PlaybackState::new();
        // From stopped: toggle_reverse starts reverse play
        pb.toggle_reverse();
        assert!(pb.is_playing());
        assert!(pb.is_reverse());
        // Toggle again while playing: flips to forward
        pb.toggle_reverse();
        assert!(pb.is_playing());
        assert!(!pb.is_reverse());
    }

    #[test]
    fn test_scrub_state_tracks_drag_lifetime() {
        let mut pb = PlaybackState::new();
        assert!(!pb.is_scrubbing());
        pb.scrub_start();
        assert!(pb.is_scrubbing());
        pb.scrub_end();
        assert!(!pb.is_scrubbing());
    }

    #[test]
    fn test_scrub_pause_resume_keeps_resume_timer_behavior() {
        let mut pb = PlaybackState::new();
        pb.play();
        assert!(pb.is_playing());
        pb.scrub_start();
        assert!(pb.is_scrubbing());
        assert!(pb.is_paused());
        pb.scrub_end();
        assert!(!pb.is_scrubbing());
        assert!(pb.is_paused());
    }
}
