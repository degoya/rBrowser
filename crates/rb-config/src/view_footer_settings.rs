//! `ui.viewFooter` (RB-080-44): the strip under every view that names the widths its
//! breakpoint holds for and the page's class for that width - by default; the breakpoint, the
//! view's size and the page's media queries can be chosen too. In a narrow view on two lines,
//! its text in the middle (RB-080-53). Since RB-080-78 the default is range and class only (the
//! owner: the name and the size stand in the label strip above); a file that holds exactly the
//! default before is read as the new one.

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};

/// One field of the breakpoint bar. The order of the variants is the order in the strip.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum FooterField {
    /// The breakpoint's id (`md`).
    Breakpoint,
    /// The widths the breakpoint stands for (`768-1023px`).
    Range,
    /// The view's width, height and pixel ratio (`768 x 1024 @2x`).
    Size,
    /// The page's `min-width` / `max-width` conditions that hold at the view's width, from the
    /// analysis of the page; the ends of the breakpoint's range until one ran.
    Queries,
    /// The framework's name for the width (`md`), when the analyser recognised one.
    Framework,
}

/// `ui.viewFooter`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct ViewFooterSettings {
    /// Show the breakpoint bar under every view (formerly the footer), in the breakpoint's
    /// colour, in every layout and in a view's own window; the view gives it 18 px of its height.
    /// Off gives the height back.
    pub visible: bool,
    /// What the breakpoint bar names, in this order: `breakpoint`, `range`, `size`, `queries`,
    /// `framework`; `range` and `framework` by default. A breakpoint bar with no field left is
    /// not shown.
    #[serde(deserialize_with = "lifted_fields")]
    #[schemars(with = "BTreeSet<FooterField>")]
    pub fields: BTreeSet<FooterField>,
    /// Read the media queries of the page a view shows from the loaded page itself, after
    /// every load - nothing is requested again - so the breakpoint bar names the page's own
    /// `min-width` / `max-width` queries. Off, the breakpoint bar names the ends of the
    /// breakpoint's range, or what Analyze Breakpoints found.
    pub analyze: bool,
    /// What the breakpoint bar does in a view too narrow for its text: `auto` breaks it into a
    /// second line between two fields, and the view gives the breakpoint bar 36 px while it has
    /// two lines; `never` keeps one line that ends in an ellipsis. What is cut is in the tooltip.
    pub wrap: FooterWrap,
    /// Where the breakpoint bar's text stands in the strip: `center` in the middle of every line,
    /// `start` at the left edge.
    pub align: FooterAlign,
}

/// `ui.viewFooter.wrap`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum FooterWrap {
    /// A second line when one does not hold the text, broken between two fields.
    #[default]
    Auto,
    /// Always one line; what does not fit is cut at the end.
    Never,
}

/// `ui.viewFooter.align`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum FooterAlign {
    /// In the middle of the strip, every line.
    #[default]
    Center,
    /// At the strip's left edge.
    Start,
}

impl Default for ViewFooterSettings {
    fn default() -> Self {
        Self {
            visible: true,
            fields: BTreeSet::from([FooterField::Range, FooterField::Framework]),
            analyze: true,
            wrap: FooterWrap::default(),
            align: FooterAlign::default(),
        }
    }
}

/// The default of `ui.viewFooter.fields` before RB-080-78.
const OLD_DEFAULT_FIELDS: [FooterField; 4] = [
    FooterField::Breakpoint,
    FooterField::Range,
    FooterField::Size,
    FooterField::Queries,
];

/// `ui.viewFooter.fields` as the file says, except that exactly the old default is read as the
/// new one (RB-080-78): a person who never chose keeps following the default, a choice of
/// one's own stays.
fn lifted_fields<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeSet<FooterField>, D::Error> {
    let fields = BTreeSet::<FooterField>::deserialize(deserializer)?;
    if fields == BTreeSet::from(OLD_DEFAULT_FIELDS) {
        return Ok(ViewFooterSettings::default().fields);
    }
    Ok(fields)
}

impl ViewFooterSettings {
    /// Whether a breakpoint bar is drawn at all: switched on, with a field to show.
    #[must_use]
    pub fn shown(&self) -> bool {
        self.visible && !self.fields.is_empty()
    }
}

#[cfg(test)]
#[path = "view_footer_settings_tests.rs"]
mod tests;
