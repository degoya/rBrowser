//! The page zoom's arithmetic and its model (RB-090-20): Chromium's zoom level is the power of
//! 1.2 the page is scaled by (`0` is 100 %, `1` is 120 %), its steps are Chromium's preset
//! factors, and [`Levels`] holds each view's level as rBrowser last set or read it. What the
//! model decides is a list of `(view, level)` to set; the window sets them
//! (`cef_app/page_zoom.rs`). No CEF here: it runs and is tested everywhere.

use std::collections::HashMap;

use rb_config::PageZoomSettings;
use rb_engine::ViewId;

/// Chromium's zoom factors, the steps of Ctrl+Plus and Ctrl+Minus (`kPresetZoomFactors`,
/// `components/zoom/page_zoom_constants.cc`): 25 % to 500 %.
pub const PRESETS: [f64; 17] = [
    0.25,
    1.0 / 3.0,
    0.5,
    2.0 / 3.0,
    0.75,
    0.8,
    0.9,
    1.0,
    1.1,
    1.25,
    1.5,
    1.75,
    2.0,
    2.5,
    3.0,
    4.0,
    5.0,
];
/// The smallest factor a page zooms to (25 %).
pub const MIN_FACTOR: f64 = 0.25;
/// The largest factor a page zooms to (500 %).
pub const MAX_FACTOR: f64 = 5.0;
/// Two levels closer than this are the same (Chromium's `blink::ZoomValuesEqual`).
const EPSILON: f64 = 0.001;
/// The base of Chromium's zoom levels.
const BASE: f64 = 1.2;

/// The factor of `level` (`1.0` for 100 %).
#[must_use]
pub fn factor_of(level: f64) -> f64 {
    BASE.powf(level)
}

/// The level of `factor`.
#[must_use]
pub fn level_of(factor: f64) -> f64 {
    factor.ln() / BASE.ln()
}

/// `level` in whole percent, as Chromium names it (`110`).
#[must_use]
pub fn percent_of(level: f64) -> u32 {
    let percent = (factor_of(level) * 100.0).round();
    // `as` saturates; a level that cannot be read is 100 %.
    if percent.is_finite() {
        percent as u32
    } else {
        100
    }
}

/// Whether a page at `level` is zoomed; a level that cannot be read (`NaN`) is not.
#[must_use]
pub fn is_zoomed(level: f64) -> bool {
    level.is_finite() && level.abs() >= EPSILON
}

/// The level one preset from `level`: the next larger factor (`up`) or the next smaller one.
/// Past the last preset in that direction the level stays.
#[must_use]
pub fn step(level: f64, up: bool) -> f64 {
    let presets = PRESETS.iter().map(|factor| level_of(*factor));
    let next = if up {
        presets
            .filter(|preset| *preset > level + EPSILON)
            .reduce(f64::min)
    } else {
        presets
            .filter(|preset| *preset < level - EPSILON)
            .reduce(f64::max)
    };
    next.unwrap_or(level)
}

/// What a zoom does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    /// Back to 100 % (Ctrl+0).
    Reset,
    /// To this level.
    Set(f64),
    /// One preset larger (Ctrl+Plus).
    In,
    /// One preset smaller (Ctrl+Minus).
    Out,
}

/// CEF's `EVENTFLAG_SHIFT_DOWN`.
const FLAG_SHIFT: u32 = 1 << 1;

/// The zoom of a key-down in a page with CEF's event `flags` and the Windows key code `key`:
/// Ctrl+0 (the main key or the number pad's) resets, Ctrl+Plus and Ctrl+Minus step - the keys
/// Chromium zooms a page with ([`crate::shell::is_zoom_key`]). Every other key is `None`.
#[must_use]
pub fn key_action(flags: u32, key: i32) -> Option<Action> {
    /// `VK_OEM_PLUS`, `VK_ADD`.
    const IN_KEYS: [i32; 2] = [0xBB, 0x6B];
    /// `VK_OEM_MINUS`, `VK_SUBTRACT`.
    const OUT_KEYS: [i32; 2] = [0xBD, 0x6D];
    if !crate::shell::is_zoom_key(flags, key) {
        return None;
    }
    if IN_KEYS.contains(&key) {
        Some(Action::In)
    } else if OUT_KEYS.contains(&key) {
        Some(Action::Out)
    } else if flags & FLAG_SHIFT == 0 {
        Some(Action::Reset)
    } else {
        None
    }
}

/// Each view's level as rBrowser last set or read it; a view it never saw is at 0.
#[derive(Debug, Default)]
pub struct Levels {
    by_view: HashMap<ViewId, f64>,
}

impl Levels {
    /// `view`'s level.
    #[must_use]
    pub fn get(&self, view: ViewId) -> f64 {
        self.by_view.get(&view).copied().unwrap_or(0.0)
    }

    /// Takes `sets` as the views' levels.
    pub fn record(&mut self, sets: &[(ViewId, f64)]) {
        for (view, level) in sets {
            self.by_view.insert(*view, *level);
        }
    }

    /// Forgets every view not in `views`.
    pub fn retain(&mut self, views: &[ViewId]) {
        self.by_view.retain(|view, _| views.contains(view));
    }

    /// The levels `action` sets on `targets`. Without `sync` each target moves from its own
    /// level; with `sync` (every view) all of them take the level the first target moves to.
    #[must_use]
    pub fn plan(
        &self,
        action: Action,
        targets: &[ViewId],
        sync: Option<&[ViewId]>,
    ) -> Vec<(ViewId, f64)> {
        let next = |view: ViewId| match action {
            Action::Reset => 0.0,
            Action::Set(level) => level,
            Action::In => step(self.get(view), true),
            Action::Out => step(self.get(view), false),
        };
        match (sync, targets.first()) {
            (Some(all), Some(anchor)) => {
                let level = next(*anchor);
                all.iter().map(|view| (*view, level)).collect()
            }
            _ => targets.iter().map(|view| (*view, next(*view))).collect(),
        }
    }

    /// The window read `level` on `view`, a change rBrowser did not make (the wheel, a page of
    /// another zoomed site): what to set in answer. The first sight of a view is taken as it is
    /// (Chromium brings a site's zoom back); with `settings.wheel` off the view goes back to
    /// the level the model holds; with `settings.sync` every other view of `all` follows.
    pub fn observe(
        &mut self,
        view: ViewId,
        level: f64,
        settings: &PageZoomSettings,
        all: &[ViewId],
    ) -> Vec<(ViewId, f64)> {
        let Some(known) = self.by_view.get(&view).copied() else {
            self.record(&[(view, level)]);
            return Vec::new();
        };
        if (known - level).abs() < EPSILON {
            return Vec::new();
        }
        if !settings.wheel {
            return vec![(view, known)];
        }
        self.record(&[(view, level)]);
        if !settings.sync {
            return Vec::new();
        }
        let follow: Vec<(ViewId, f64)> = all
            .iter()
            .filter(|other| **other != view && (self.get(**other) - level).abs() >= EPSILON)
            .map(|other| (*other, level))
            .collect();
        self.record(&follow);
        follow
    }

    /// `view`'s page finished loading at `level`: with `settings.keepOnReload` off a zoomed
    /// page goes back to 100 %; kept, the level the page loaded at is taken as it is (Chromium
    /// brings the zoom of the page's site along).
    pub fn loaded(
        &mut self,
        view: ViewId,
        level: f64,
        settings: &PageZoomSettings,
    ) -> Vec<(ViewId, f64)> {
        if !settings.keep_on_reload && is_zoomed(level) {
            self.record(&[(view, 0.0)]);
            return vec![(view, 0.0)];
        }
        if settings.keep_on_reload {
            self.record(&[(view, level)]);
        }
        Vec::new()
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
