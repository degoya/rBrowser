//! The WCAG 2.x contrast of two colours (SC 1.4.3 AA, 1.4.6 AAA): the ratio and which levels it
//! reaches for normal and for large text (at least 24 CSS px, or 18.67 px bold).
//!
//! The levels are decided on the exact ratio, never on the rounded one: 4.48:1 misses AA for
//! normal text, as WCAG wants it (the rule of `plugins/peek/src/contrast.rs`).

use serde_json::{Value, json};

use rb_plugin_api::Failure;

use crate::color::{Rgba, contrast_ratio, parse, round2};
use crate::convert::Formats;
use crate::invalid;

/// The levels one text size reaches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Levels {
    pub aa: bool,
    pub aaa: bool,
}

impl Levels {
    fn of(ratio: f64, large_text: bool) -> Self {
        let (aa, aaa) = if large_text { (3.0, 4.5) } else { (4.5, 7.0) };
        Self {
            aa: ratio >= aa,
            aaa: ratio >= aaa,
        }
    }

    fn to_json(self) -> Value {
        json!({ "aa": self.aa, "aaa": self.aaa })
    }
}

/// The contrast of a foreground over a background.
#[derive(Clone, Debug, PartialEq)]
pub struct Rating {
    /// The foreground, blended over the background when translucent.
    pub foreground: Rgba,
    /// The background, over white when translucent.
    pub background: Rgba,
    /// Rounded to two decimals.
    pub ratio: f64,
    pub normal: Levels,
    pub large: Levels,
}

impl Rating {
    #[must_use]
    pub fn of(foreground: Rgba, background: Rgba) -> Self {
        let background = background.over(Rgba::WHITE);
        let foreground = foreground.over(background);
        let exact = contrast_ratio(foreground, background);
        Self {
            foreground,
            background,
            ratio: round2(exact),
            normal: Levels::of(exact, false),
            large: Levels::of(exact, true),
        }
    }

    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "ratio": self.ratio,
            "normal": self.normal.to_json(),
            "large": self.large.to_json(),
            "foreground": Formats::of(self.foreground).to_json(),
            "background": Formats::of(self.background).to_json(),
        })
    }
}

/// `color_contrast`: the rating of `foreground` over `background` (any text [`parse`] reads).
///
/// # Errors
///
/// `invalid` naming the colour that does not read.
pub fn tool(args: &Value) -> Result<Value, Failure> {
    let color = |key: &str| {
        args[key]
            .as_str()
            .and_then(parse)
            .ok_or_else(|| invalid(key))
    };
    Ok(Rating::of(color("foreground")?, color("background")?).to_json())
}

#[cfg(test)]
#[path = "contrast_tests.rs"]
mod tests;
