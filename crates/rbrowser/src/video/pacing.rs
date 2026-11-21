//! A constant frame rate from the screencast's frames. Chromium sends a frame only when the
//! page painted one, with its time (`metadata.timestamp`, seconds); a file needs one every
//! `1/fps`. The pacer holds the latest frame and says how often to write it: once for every slot
//! (at `start + k/fps`) that passed before the next frame came, and at the stop for the slots
//! up to the stop time, so a page that does not move still gives a file of the recorded length.
//! The slots end at the maximum duration. Pure logic: the recorder writes the copies.
//!
//! A pause (RB-090-68, [`Pauses`]) stops the slots' clock: the time between a pause and its
//! resume counts for nothing, so the frame held at the pause is written up to it, the next one
//! from there, and the file has no gap and no frozen stretch. An open pause stops the clock at
//! its start.

/// The highest frame rate taken.
pub const MAX_FPS: u32 = 60;
/// Timestamps closer than this to a slot count as on it (floating point).
const EPSILON: f64 = 1e-9;

/// The pauses of one recording on the frames' clock (seconds): spans from a pause to its
/// resume, the last one open while the recording is paused.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pauses {
    spans: Vec<(f64, Option<f64>)>,
}

impl Pauses {
    /// Pauses at `at`; `false` while paused or for a time that is no number. A pause never
    /// starts before the last one ended.
    pub fn pause(&mut self, at: f64) -> bool {
        if !at.is_finite() || self.is_paused() {
            return false;
        }
        let after = self
            .spans
            .last()
            .and_then(|(_, end)| *end)
            .map_or(at, |end| at.max(end));
        self.spans.push((after, None));
        true
    }

    /// Resumes at `at` (at the pause at the earliest); `false` without a pause or a time.
    pub fn resume(&mut self, at: f64) -> bool {
        match self.spans.last_mut() {
            Some((from, end @ None)) if at.is_finite() => {
                *end = Some(at.max(*from));
                true
            }
            _ => false,
        }
    }

    /// Whether the last pause is still open.
    pub fn is_paused(&self) -> bool {
        matches!(self.spans.last(), Some((_, None)))
    }

    /// Where the open pause started; `None` while not paused.
    pub fn paused_since(&self) -> Option<f64> {
        match self.spans.last() {
            Some((from, None)) => Some(*from),
            _ => None,
        }
    }

    /// The seconds paused before `time`; an open pause runs to `time`.
    pub fn before(&self, time: f64) -> f64 {
        self.spans
            .iter()
            .map(|(from, end)| (end.unwrap_or(f64::INFINITY).min(time) - from).max(0.0))
            .sum()
    }

    /// The seconds of the pauses that ended.
    pub fn ended_secs(&self) -> f64 {
        self.spans
            .iter()
            .filter_map(|(from, end)| end.map(|end| (end - from).max(0.0)))
            .sum()
    }
}

/// The slots of one recording.
#[derive(Clone, Debug, PartialEq)]
pub struct Pacer {
    fps: u32,
    max_slots: u64,
    start: Option<f64>,
    written: u64,
    held_written: bool,
    pauses: Pauses,
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
            pauses: Pauses::default(),
        }
    }

    /// The recording's pauses so far (RB-090-68): their time has no slots.
    pub fn set_pauses(&mut self, pauses: Pauses) {
        self.pauses = pauses;
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

    /// The slots before `time`, bounded by the maximum; paused time has none.
    fn slots_before(&self, time: f64) -> u64 {
        let Some(start) = self.start else {
            return 0;
        };
        let paused = self.pauses.before(time) - self.pauses.before(start);
        let slots = ((time - start - paused) * f64::from(self.fps) - EPSILON).ceil();
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
