//! The page's direction, language and pseudo-localisation as the sync agent applies them
//! (RB-092-02, [`crate::AgentCommand::Locale`]): `dir` and `lang` on the page's `<html>`, and the
//! page's texts made longer, accented and marked - their originals kept in the agent's world, so
//! the selectors still find the elements in every view. Mirrors `PageLocale` of
//! `agent/src/protocol.ts`.

use serde::{Deserialize, Serialize};

/// The text direction forced on `<html dir>` (`rb_config::TextDirection`, which this crate does
/// not depend on).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PageDir {
    /// Left to right.
    Ltr,
    /// Right to left.
    Rtl,
    /// `dir="auto"`.
    Auto,
}

/// The pseudo-localisation of the page's texts.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PseudoLocale {
    /// How much longer a text gets, 1 to 3.
    pub factor: f64,
    /// Letters are swapped for accented ones.
    pub accents: bool,
    /// The texts run right to left (Unicode bidi override).
    pub rtl: bool,
}

/// What the core forces on a view's page; an absent field is the page's own.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageLocale {
    /// `<html dir>`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dir: Option<PageDir>,
    /// `<html lang>`, a BCP 47 tag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    /// The pseudo-localisation, when on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pseudo: Option<PseudoLocale>,
}

impl PageLocale {
    /// Whether the page is left as it is.
    #[must_use]
    pub fn is_plain(&self) -> bool {
        self == &Self::default()
    }
}

#[cfg(test)]
#[path = "locale_tests.rs"]
mod tests;
