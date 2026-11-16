//! The WCAG 2.x contrast rating of a text: the ratio of its colour and the background it sits
//! on, and the level it reaches (SC 1.4.3 AA, 1.4.6 AAA), with the large-text rule - at least
//! 24 CSS px, or 14 pt (18.67 px) and bold (weight 700 or more).
//!
//! The level is decided on the exact ratio, never on the rounded one: 4.48:1 is shown as such and
//! misses AA for normal text, as WCAG wants it.

use serde_json::{Value, json};

use crate::color::{Rgba, contrast_ratio, round2};

/// 18.67 px: 14 pt at 96 dpi.
const BOLD_LARGE_PX: f64 = 14.0 * 96.0 / 72.0;
/// 18 pt at 96 dpi.
const LARGE_PX: f64 = 24.0;
/// The lightest weight that counts as bold.
const BOLD: u32 = 700;

/// Whether text of `size_px` and `weight` is large text in WCAG's sense.
#[must_use]
pub fn is_large_text(size_px: f64, weight: u32) -> bool {
    // A hundredth of a pixel of slack: Chromium writes 14pt as 18.6667px.
    size_px >= LARGE_PX || (weight >= BOLD && size_px + 0.005 >= BOLD_LARGE_PX)
}

/// The best level a contrast reaches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    /// AAA for this text (7:1, large text 4.5:1).
    Aaa,
    /// AA for this text (4.5:1, large text 3:1).
    Aa,
    /// Normal text at 3:1 or more: it would pass AA only were it large.
    AaLarge,
    Fail,
}

impl Level {
    /// The level `ratio` reaches for normal or large text.
    #[must_use]
    pub fn rate(ratio: f64, large_text: bool) -> Self {
        let (aaa, aa) = if large_text { (4.5, 3.0) } else { (7.0, 4.5) };
        if ratio >= aaa {
            Self::Aaa
        } else if ratio >= aa {
            Self::Aa
        } else if !large_text && ratio >= 3.0 {
            Self::AaLarge
        } else {
            Self::Fail
        }
    }

    /// The level as the tool answers it and the page script finds its label
    /// (`plugin.peek.level.<key>`).
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::Aaa => "aaa",
            Self::Aa => "aa",
            Self::AaLarge => "aa_large",
            Self::Fail => "fail",
        }
    }
}

/// The contrast of a text over its background.
#[derive(Clone, Debug, PartialEq)]
pub struct Contrast {
    /// The text colour, blended over the background when translucent.
    pub foreground: Rgba,
    pub background: Rgba,
    /// Rounded to two decimals.
    pub ratio: f64,
    pub large_text: bool,
    pub level: Level,
}

impl Contrast {
    /// Rates `foreground` over the opaque `background` for text of `size_px` and `weight`.
    #[must_use]
    pub fn of(foreground: Rgba, background: Rgba, size_px: f64, weight: u32) -> Self {
        let background = background.over(Rgba::WHITE);
        let foreground = foreground.over(background);
        let exact = contrast_ratio(foreground, background);
        let large_text = is_large_text(size_px, weight);
        Self {
            foreground,
            background,
            ratio: round2(exact),
            large_text,
            level: Level::rate(exact, large_text),
        }
    }

    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "ratio": self.ratio,
            "level": self.level.key(),
            "largeText": self.large_text,
            "foreground": self.foreground.hex(),
            "background": self.background.hex(),
        })
    }
}

#[cfg(test)]
#[path = "contrast_tests.rs"]
mod tests;
