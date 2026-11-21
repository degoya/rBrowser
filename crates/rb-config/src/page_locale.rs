//! The page's language and text direction (RB-092-02): `dir` and `lang` a breakpoint (or the
//! project's `emulation` defaults) forces on the page's `<html>`, and `emulation.pseudoLocale` of
//! the user settings (Settings > Emulation > Text direction and pseudo-localisation): what the
//! pseudo-localisation of the page's texts does when a command or MCP turns it on without values
//! of its own.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The default length factor of a pseudo-localised text.
pub const DEFAULT_PSEUDO_FACTOR: f64 = 1.4;
/// The smallest and the largest length factor.
pub const PSEUDO_FACTOR_RANGE: (f64, f64) = (1.0, 3.0);

/// The text direction forced on the page's `<html dir>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum TextDirection {
    /// Left to right.
    Ltr,
    /// Right to left.
    Rtl,
    /// `dir="auto"`: the browser decides by the first strong character.
    Auto,
}

impl TextDirection {
    /// The other direction of a toggle: right to left, unless it is right to left already.
    #[must_use]
    pub fn toggled(current: Option<Self>) -> Self {
        match current {
            Some(Self::Rtl) => Self::Ltr,
            _ => Self::Rtl,
        }
    }
}

/// `emulation.pseudoLocale` (user scope): the pseudo-localisation's values when the command or
/// MCP gives none.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct PseudoLocaleSettings {
    /// How much longer a text gets: 1.4 makes it 40 % longer, as German or Finnish often are.
    #[schemars(range(min = 1.0, max = 3.0))]
    pub factor: f64,
    /// Letters are swapped for accented ones (an `a` gets an acute accent), so a font without
    /// them shows.
    pub accents: bool,
    /// The texts also run right to left (Unicode bidi override), as an RTL language would.
    pub rtl: bool,
}

impl Default for PseudoLocaleSettings {
    fn default() -> Self {
        Self {
            factor: DEFAULT_PSEUDO_FACTOR,
            accents: true,
            rtl: false,
        }
    }
}

/// `factor` within [`PSEUDO_FACTOR_RANGE`]; one that is no number is the default.
#[must_use]
pub fn clamp_pseudo_factor(factor: f64) -> f64 {
    if factor.is_finite() {
        factor.clamp(PSEUDO_FACTOR_RANGE.0, PSEUDO_FACTOR_RANGE.1)
    } else {
        DEFAULT_PSEUDO_FACTOR
    }
}

#[cfg(test)]
#[path = "page_locale_tests.rs"]
mod tests;
