//! The footer's pure rules (RB-080-44): the range of widths a view's width stands for - the
//! page's step when its media queries are known (RB-080-78), else among the config's
//! breakpoints - the page's media queries that hold at it, the strip's text, its colour and the
//! text colour that reads on it. No CEF, no host.

use std::collections::{BTreeMap, BTreeSet};

use rb_config::FooterField;
use rb_engine::CellRect;
use rb_ipc::{BoundaryKind, DetectedBreakpoint};
use serde::Serialize;

/// The height of one line of the footer in DIP at 100 % text size; a footer of two lines
/// (RB-080-53) is twice it. Windows' text size scales it (RB-090-26, [`CellFooter::height`]).
pub const FOOTER_HEIGHT: u32 = 18;
/// What separates the footer's fields.
pub(super) const SEPARATOR: &str = " | ";
/// The colours of the breakpoints without one of their own, by their place in the list.
pub const PALETTE: [u32; 8] = [
    0x00ef_4444,
    0x00f5_9e0b,
    0x0022_c55e,
    0x0006_b6d4,
    0x003b_82f6,
    0x00a7_8bfa,
    0x00ec_4899,
    0x0064_748b,
];
/// The text on a light footer: black, the one dark text that - with white on the dark grounds -
/// reaches a contrast of 4.5 : 1 on every colour (RB-080-47).
pub const DARK_TEXT: u32 = 0x0000_0000;
/// The text on a dark footer.
pub const LIGHT_TEXT: u32 = 0x00ff_ffff;

/// The widths a breakpoint stands for, in CSS px; `to: None` has no upper end.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Range {
    /// The first width.
    pub from: u32,
    /// The last width; `None` for the largest breakpoint.
    pub to: Option<u32>,
}

impl Range {
    /// `768-1023px`; the largest breakpoint's ends in the infinity sign.
    pub fn text(&self) -> String {
        match self.to {
            Some(to) => format!("{}-{to}px", self.from),
            // U+221E, the infinity sign.
            None => format!("{}-\u{221e}px", self.from),
        }
    }

    /// The ends of the range as media conditions (`min-width 768px`, `max-width 1023px`): what
    /// the footer names while the page's own queries are not known (RB-080-47). The smallest
    /// breakpoint has no lower end, the largest no upper one.
    pub fn queries(&self) -> Vec<String> {
        let min = (self.from > 0).then(|| format!("min-width {}px", self.from));
        let max = self.to.map(|to| format!("max-width {to}px"));
        min.into_iter().chain(max).collect()
    }
}

/// The range `width` lies in among the breakpoint widths of the config: from the largest one up
/// to it (0 for the smallest: nothing lies below) to the next larger one minus 1. A free size
/// gets the range of its neighbours this way.
pub fn range(width: u32, config_widths: &[u32]) -> Range {
    let widths: BTreeSet<u32> = config_widths.iter().copied().collect();
    let smallest = widths.first().copied();
    let below = widths.range(..=width).next_back().copied();
    let above = widths.range(width.saturating_add(1)..).next().copied();
    Range {
        from: below.filter(|below| Some(*below) != smallest).unwrap_or(0),
        to: above.map(|above| above - 1),
    }
}

/// The page's step that `width` lies in (RB-080-78): from the boundary at or below it to the
/// next one above, minus 1 - whatever kind each boundary is. A `min-width X` starts a step at
/// X, a `max-width Y` ends one at Y, so the next starts at Y + 1. On a mobile-first page, with
/// only `min-width` queries, the upper end is the next `min-width` minus 1. `None` without a
/// boundary: the page is not known.
pub fn page_range(width: u32, detected: &[DetectedBreakpoint]) -> Option<Range> {
    if detected.is_empty() {
        return None;
    }
    let starts: BTreeSet<u32> = detected
        .iter()
        .map(|found| match found.kind {
            BoundaryKind::Min => found.width,
            BoundaryKind::Max => found.width.saturating_add(1),
        })
        .collect();
    let from = starts.range(..=width).next_back().copied().unwrap_or(0);
    let to = starts
        .range(width.saturating_add(1)..)
        .next()
        .map(|next| next - 1);
    Some(Range { from, to })
}

fn holds(found: &DetectedBreakpoint, width: u32) -> bool {
    match found.kind {
        BoundaryKind::Min => found.width <= width,
        BoundaryKind::Max => found.width >= width,
    }
}

/// The boundaries of `detected` that hold at `width`, the tightest first: the nearest
/// `min-width` below, the nearest `max-width` above, then the others by their distance. A
/// boundary found more than once counts once, with its name when one of the finds has one.
fn holding(width: u32, detected: &[DetectedBreakpoint]) -> Vec<&DetectedBreakpoint> {
    let is_max = |found: &DetectedBreakpoint| found.kind == BoundaryKind::Max;
    let mut unique: BTreeMap<(u32, bool), &DetectedBreakpoint> = BTreeMap::new();
    for found in detected.iter().filter(|found| holds(found, width)) {
        let kept = unique.entry((found.width, is_max(found))).or_insert(found);
        if kept.label.is_none() && found.label.is_some() {
            *kept = found;
        }
    }
    let mut found: Vec<&DetectedBreakpoint> = unique.into_values().collect();
    found.sort_by_key(|found| (found.width.abs_diff(width), is_max(found)));
    // The nearest of each kind leads, `min-width` first.
    let mut ordered = Vec::with_capacity(found.len());
    for max in [false, true] {
        if let Some(at) = found.iter().position(|found| is_max(found) == max) {
            ordered.push(found.remove(at));
        }
    }
    ordered.extend(found);
    ordered
}

/// The `min-width` / `max-width` conditions of the page that are true at `width`
/// (`min-width 768px`), the tightest first.
pub fn queries_at(width: u32, detected: &[DetectedBreakpoint]) -> Vec<String> {
    holding(width, detected)
        .into_iter()
        .map(|found| {
            let side = match found.kind {
                BoundaryKind::Min => "min-width",
                BoundaryKind::Max => "max-width",
            };
            format!("{side} {}px", found.width)
        })
        .collect()
}

/// The framework's name for `width` (`md`): the name of the tightest `min-width` that holds.
/// A tighter boundary without a name leaves the width without one - it lies in no named step.
pub fn framework_at(width: u32, detected: &[DetectedBreakpoint]) -> Option<String> {
    holding(width, detected)
        .into_iter()
        .find(|found| found.kind == BoundaryKind::Min)
        .and_then(|found| found.label.clone())
}

/// The document of `url`: the URL without its fragment. Two URLs of one document differ only
/// in the fragment (a jump inside the page), which keeps the page's stylesheets.
pub fn document_of(url: &str) -> &str {
    url.split('#').next().unwrap_or(url)
}

/// The height of a view's own window once its footer went from `before` to `after` DIP (shown,
/// hidden, one line or two): the page keeps its size, the window takes or gives the difference.
/// Never below 1.
pub fn window_height(height: i32, before: u32, after: u32) -> i32 {
    let side = |footer: u32| i32::try_from(footer).unwrap_or(i32::MAX);
    height
        .saturating_add(side(after))
        .saturating_sub(side(before))
        .max(1)
}

/// What a footer is written from.
#[derive(Clone, Copy, Debug)]
pub struct FooterInput<'a> {
    /// The id of the breakpoint the view shows; `None` after a free size.
    pub breakpoint: Option<&'a str>,
    /// The view's width in CSS px.
    pub width: u32,
    /// The view's height in CSS px.
    pub height: u32,
    /// The view's pixel ratio; 0 leaves it to the display.
    pub dpr: f64,
    /// The widths of the config's breakpoints.
    pub config_widths: &'a [u32],
    /// The boundaries the analyser found; none before its first run - the footer then names the
    /// ends of the range as its queries.
    pub detected: &'a [DetectedBreakpoint],
    /// `ui.viewFooter.fields`.
    pub fields: &'a BTreeSet<FooterField>,
}

/// The footer of one view as the window draws it: the strip's text, its tooltip and its colours
/// as `0xrrggbb`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CellFooter {
    /// The strip's text; empty when no field has a value.
    pub text: String,
    /// Every media query that holds; empty without one.
    pub tooltip: String,
    /// The strip's colour: the breakpoint's.
    pub background: u32,
    /// The text colour that reads on it.
    pub foreground: u32,
    /// The text as the strip shows it: one line, or two in a view too narrow for one
    /// (`ui.viewFooter.wrap`, RB-080-53).
    pub lines: Vec<String>,
    /// The text stands in the middle of the strip (`ui.viewFooter.align`), else at its left.
    pub centered: bool,
    /// The view is too low for a line (RB-080-78, a canvas zoomed far out): no strip, and the
    /// view keeps its height until it is high enough again.
    pub folded: bool,
    /// The vitals bar below the breakpoint bar's lines (RB-090-71, `ui.viewFooter.vitals`):
    /// what it says, its tooltip and what a click on it does; `None` without one.
    pub vitals: Option<crate::cell_info::Piece>,
    /// The view has room for the vitals bar's line (`crate::view_footer::wrap::share`).
    pub vitals_shown: bool,
    /// The Lighthouse bar below the vitals bar (RB-090-73, `ui.viewFooter.lighthouse`): the
    /// scores of the newest run of the view's page and breakpoint; `None` without one.
    pub lighthouse: Option<super::lighthouse::LighthouseBar>,
    /// The view has room for the Lighthouse bar's line.
    pub lighthouse_shown: bool,
    /// How much of the category names the Lighthouse bar's width holds.
    pub lighthouse_form: super::lighthouse::Form,
}

impl CellFooter {
    /// The breakpoint bar's lines as the strip draws them: the fitted lines; before a fit its
    /// text on one line - unless the footer has a vitals or a Lighthouse bar, whose fit left the
    /// breakpoint bar no line (or which has no breakpoint bar at all).
    pub fn bar_lines(&self) -> Vec<&str> {
        if !self.lines.is_empty() {
            return self.lines.iter().map(String::as_str).collect();
        }
        if self.vitals.is_some() || self.lighthouse.is_some() {
            return Vec::new();
        }
        vec![self.text.as_str()]
    }

    /// The DIP the footer takes: [`FOOTER_HEIGHT`] at the text size in effect for every line it
    /// draws - the breakpoint bar's, the vitals bar's and the Lighthouse bar's; none folded.
    pub fn height(&self) -> u32 {
        if self.folded {
            return 0;
        }
        let count = self.bar_lines().len()
            + usize::from(self.vitals_shown)
            + usize::from(self.lighthouse_shown);
        let lines = u32::try_from(count).unwrap_or(u32::MAX);
        crate::text_scale::dip(FOOTER_HEIGHT).saturating_mul(lines)
    }

    /// The footer as the window shows it: `None` while folded.
    pub fn shown(&self) -> Option<&Self> {
        (!self.folded).then_some(self)
    }
}

/// The strip's text and its tooltip (every query of the page that holds; empty without one).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FooterText {
    /// `768-1023px | md` by default; with every field
    /// `md | 768-1023px | 768 x 1024 @2x | min-width 768px | max-width 1023px`.
    pub text: String,
    /// Every query that holds.
    pub tooltip: String,
}

/// The footer of `input`: its fields in their fixed order, a field without a value left out.
/// Range and queries are the page's step once the page is known ([`page_range`]), the
/// config's range until then; never two ends of one side (RB-080-78).
pub fn text(input: &FooterInput<'_>) -> FooterText {
    let queries = queries_at(input.width, input.detected);
    let range = page_range(input.width, input.detected)
        .unwrap_or_else(|| range(input.width, input.config_widths));
    let mut parts = Vec::new();
    for field in input.fields {
        let part = match field {
            FooterField::Breakpoint => input.breakpoint.map(str::to_owned),
            FooterField::Range => Some(range.text()),
            FooterField::Size => Some(size(input)),
            // The step's lower and upper end as the page's conditions.
            FooterField::Queries => Some(range.queries().join(SEPARATOR)),
            // Not beside a breakpoint of the same name.
            FooterField::Framework => framework_at(input.width, input.detected).filter(|name| {
                !(input.fields.contains(&FooterField::Breakpoint)
                    && input.breakpoint == Some(name.as_str()))
            }),
        };
        parts.extend(part.filter(|part| !part.is_empty()));
    }
    // Every query of the page that holds, behind the step it makes.
    let tooltip = if input.fields.contains(&FooterField::Queries)
        || input.fields.contains(&FooterField::Range)
    {
        queries.join(", ")
    } else {
        String::new()
    };
    FooterText {
        text: parts.join(SEPARATOR),
        tooltip,
    }
}

/// `768 x 1024 @2x` with a multiplication sign, the ratio only when it is set and not 1 - as
/// the label strip writes it.
fn size(input: &FooterInput<'_>) -> String {
    let ratio = if input.dpr > 0.0 && (input.dpr - 1.0).abs() > f64::EPSILON {
        format!(" @{}x", input.dpr)
    } else {
        String::new()
    };
    // U+00D7, the multiplication sign.
    format!("{} \u{d7} {}{ratio}", input.width, input.height)
}

/// The footer's colour as `0xrrggbb`: the breakpoint's `own` (`#rrggbb`), else the palette's
/// by the breakpoint's place `index` in the list.
pub fn background(own: Option<&str>, index: usize) -> u32 {
    own.and_then(rb_config::parse_hex_color)
        .unwrap_or(PALETTE[index % PALETTE.len()])
}

/// The text colour that reads better on `background`: [`DARK_TEXT`] or [`LIGHT_TEXT`].
pub fn foreground(background: u32) -> u32 {
    if contrast(background, DARK_TEXT) >= contrast(background, LIGHT_TEXT) {
        DARK_TEXT
    } else {
        LIGHT_TEXT
    }
}

/// The WCAG contrast ratio of two `0xrrggbb` colours, 1 to 21.
pub fn contrast(a: u32, b: u32) -> f64 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// The WCAG relative luminance of an `0xrrggbb` colour.
fn luminance(color: u32) -> f64 {
    let channel = |shift: u32| {
        let value = f64::from((color >> shift) & 0xff) / 255.0;
        if value <= 0.039_28 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
}

/// `rect` without the `footer` DIP at its bottom: the view above its footer.
pub fn above_footer(rect: CellRect, footer: u32) -> CellRect {
    CellRect {
        height: rect.height.saturating_sub(footer),
        ..rect
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "model_names_tests.rs"]
mod names_tests;

#[cfg(test)]
#[path = "model_vitals_tests.rs"]
mod vitals_tests;

#[cfg(test)]
#[path = "model_lighthouse_tests.rs"]
mod lighthouse_tests;
