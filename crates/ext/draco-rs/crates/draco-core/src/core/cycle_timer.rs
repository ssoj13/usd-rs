//! Simple timer utilities.
//! Reference: `_ref/draco/src/draco/core/cycle_timer.h` + `.cc`.

#[cfg(windows)]
use std::time::Instant;
#[cfg(not(windows))]
use std::time::SystemTime;

/// Portable timer approximating Draco's C++ implementation.
pub struct DracoTimer {
    #[cfg(windows)]
    start: Option<Instant>,
    #[cfg(windows)]
    end: Option<Instant>,

    #[cfg(not(windows))]
    start: Option<SystemTime>,
    #[cfg(not(windows))]
    end: Option<SystemTime>,
}

impl DracoTimer {
    pub fn new() -> Self {
        Self {
            start: None,
            end: None,
        }
    }

    pub fn start(&mut self) {
        #[cfg(windows)]
        {
            self.start = Some(Instant::now());
        }
        #[cfg(not(windows))]
        {
            self.start = Some(SystemTime::now());
        }
    }

    pub fn stop(&mut self) {
        #[cfg(windows)]
        {
            self.end = Some(Instant::now());
        }
        #[cfg(not(windows))]
        {
            self.end = Some(SystemTime::now());
        }
    }

    pub fn get_in_ms(&self) -> i64 {
        match (self.start, self.end) {
            (Some(start), Some(end)) => {
                #[cfg(windows)]
                {
                    end.duration_since(start).as_millis() as i64
                }
                #[cfg(not(windows))]
                {
                    match end.duration_since(start) {
                        Ok(d) => d.as_millis() as i64,
                        Err(e) => -(e.duration().as_millis() as i64),
                    }
                }
            }
            _ => 0,
        }
    }
}

impl Default for DracoTimer {
    fn default() -> Self {
        Self::new()
    }
}

pub type CycleTimer = DracoTimer;
