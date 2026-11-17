//! A constant frame rate from the screencast's frames. Chromium sends a frame only when the
//! page painted one, with its time (`metadata.timestamp`, seconds); a file needs one every
//! `1/fps`. The pacer holds the latest frame and says how often to write it: once for every slot
//! (at `start + k/fps`) that passed before the next frame came, and at the stop for the slots
//! up to the stop time, so a page that does not move still gives a file of the recorded length.
//! The slots end at the maximum duration. Pure logic: the recorder writes the copies.

/// The highest frame rate taken.
pub const MAX_FPS: u32 = 60;
/// Timestamps closer than this to a slot count as on it (floating point).
const EPSILON: f64 = 1e-9;

/// The slots of one recording.
#[derive(Clone, Debug, PartialEq)]
pub struct Pacer {
    fps: u32,
    max_slots: u64,
    start: Option<f64>,
    written: u64,
    held_written: bool,
}

impl Pacer {
    /// A pacer at `fps` (1 to [`MAX_FPS`]) for at most `max_secs` seconds.
    pub fn new(fps: u32, max_secs: f64) -> Self {
        let fps = fps.clamp(1, MAX_FPS);
        let max_slots = if max_secs.is_finite() && max_secs > 0.0 {
            // At most a day at 60 fps: positive and finite, the cast cannot truncate.
            (max_secs.min(86_400.0) * f64::from(fps)).ceil() as u64
        } else {
            0
        };
        Self {
            fps,
            max_slots,
            start: None,
            written: 0,
            held_written: false,
        }
    }

    /// The frame rate in effect.
    pub fn fps(&self) -> u32 {
        self.fps
    }

    /// Slots written so far.
    pub fn written(&self) -> u64 {
        self.written
    }

    /// The length of what was written, in seconds.
    pub fn duration_secs(&self) -> f64 {
        self.written as f64 / f64::from(self.fps)
    }

    /// The slots before `time`, bounded by the maximum.
    fn slots_before(&self, time: f64) -> u64 {
        let Some(start) = self.start else {
            return 0;
        };
        let slots = ((time - start) * f64::from(self.fps) - EPSILON).ceil();
        if slots <= 0.0 {
            return 0;
        }
        let slots = slots.min(1e15) as u64;
        slots.min(self.max_slots)
    }

    /// A new frame at `time`: how often to write the frame held so far before this one is held.
    pub fn advance(&mut self, time: f64) -> u64 {
        if !time.is_finite() {
            return 0;
        }
        if self.start.is_none() {
            self.start = Some(time);
            return 0;
        }
        let repeat = self.slots_before(time).saturating_sub(self.written);
        self.written += repeat;
        // The frame from now on is held, not yet written.
        self.held_written = false;
        repeat
    }

    /// The stop at `time`: how often to write the frame held last - at least once, while the
    /// maximum allows.
    pub fn finish(&mut self, time: f64) -> u64 {
        if self.start.is_none() {
            return 0;
        }
        let due = if time.is_finite() {
            self.slots_before(time)
        } else {
            self.written
        };
        let mut repeat = due.saturating_sub(self.written);
        if repeat == 0 && !self.held_written && self.written < self.max_slots {
            repeat = 1;
        }
        self.written += repeat;
        self.held_written = true;
        repeat
    }
}

#[cfg(test)]
#[path = "pacing_tests.rs"]
mod tests;
