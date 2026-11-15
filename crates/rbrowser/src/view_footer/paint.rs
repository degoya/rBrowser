//! What the native strip is painted with, and when (RB-080-47). CEF resets a view's background
//! to its theme's whenever the view's theme changes - and adding the view, or a parent of it,
//! to a window is such a change (`cef_view_t::set_background_color`,
//! `cef_view_delegate_t::on_theme_changed`). A colour set while the strip is built is gone once
//! the strip is in its cell. So the colours are kept beside the strip ([`StripColors`]) and the
//! strip is painted twice over: when a footer is written ([`written`]) and after every theme
//! change ([`theme_changed`], from the strip's delegate). No CEF in this file: the strip is a
//! [`Paintable`], which `cef_app/cell_footers.rs` implements for the native button.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use super::CellFooter;

/// The alpha of an opaque `0xrrggbb` colour.
const OPAQUE: u32 = 0xff00_0000;

/// A strip that can be painted: the native button, or a test's recorder.
pub trait Paintable {
    /// Sets the strip's ground, ARGB.
    fn set_ground(&self, argb: u32);
    /// Sets the colour of the strip's text in every state, ARGB.
    fn set_text_color(&self, argb: u32);
}

/// The colours of one strip, shared by the strip's delegate and whoever writes the strip.
#[derive(Clone, Debug, Default)]
pub struct StripColors {
    background: Arc<AtomicU32>,
    text: Arc<AtomicU32>,
}

impl StripColors {
    /// The colours of `footer`.
    pub fn of(footer: &CellFooter) -> Self {
        let colors = Self::default();
        colors.set(footer);
        colors
    }

    /// Takes the colours of `footer`; every clone sees them.
    pub fn set(&self, footer: &CellFooter) {
        self.background.store(footer.background, Ordering::Relaxed);
        self.text.store(footer.foreground, Ordering::Relaxed);
    }

    /// The strip's ground, opaque ARGB.
    pub fn background(&self) -> u32 {
        OPAQUE | self.background.load(Ordering::Relaxed)
    }

    /// The strip's text colour, opaque ARGB.
    pub fn text(&self) -> u32 {
        OPAQUE | self.text.load(Ordering::Relaxed)
    }
}

/// `footer` is written into `strip`: its colours are kept in `colors` and painted.
pub fn written(strip: &impl Paintable, colors: &StripColors, footer: &CellFooter) {
    colors.set(footer);
    paint(strip, colors);
}

/// The theme of `strip` changed - it came into a window, or the window's theme changed - and
/// the theme's ground is over the footer's: the footer's colours again.
pub fn theme_changed(strip: &impl Paintable, colors: &StripColors) {
    paint(strip, colors);
}

fn paint(strip: &impl Paintable, colors: &StripColors) {
    strip.set_ground(colors.background());
    strip.set_text_color(colors.text());
}

#[cfg(test)]
#[path = "paint_tests.rs"]
mod tests;
