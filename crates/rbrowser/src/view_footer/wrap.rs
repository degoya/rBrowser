//! Whether a footer's text fits into its strip, and where it breaks when it does not
//! (RB-080-53). Pure: text and width in, lines out - no CEF, so the rule is tested on Linux.
//!
//! The strip is a native button that draws one line and cuts what does not fit with an
//! ellipsis; its width is the cell's on screen, which in a zoomed-out view is far below the
//! page's width. The width a text takes is estimated here, because the host decides the lines
//! (the footer's height is a layout size) and has no font to measure with. The estimate holds
//! for the strip's font, Consolas at 11 px (`cef_app/cell_footers.rs`): monospace, an advance
//! of 1126 / 2048 em = 6.05 DIP for every character - measured as 9.07 px at 150 % in the
//! owner's screenshot of 2026-11-14 - plus the button's insets of 6 DIP at each side. One
//! character of slack covers a glyph from a fallback font. Once per run the window logs what
//! CEF measures for a line beside this estimate and the strip's width (`a view footer line was
//! laid out`), and the host logs the first footer that took two lines (`a view footer took two
//! lines`).

use rb_config::FooterWrap;

use super::model::SEPARATOR;

/// The advance of one character of the strip's font in thousandths of a DIP.
pub const CHAR_MILLI_DIP: u32 = 6_050;
/// The button's insets, left and right together, in DIP.
pub const INSETS: u32 = 12;
/// The room kept free beyond the estimate, in DIP: one character.
pub const SLACK: u32 = 6;
/// The most lines a footer takes.
pub const MAX_LINES: usize = 2;

/// The DIP a strip needs to show `text` on one line at the text size in effect, estimated.
pub fn text_width(text: &str) -> u32 {
    text_width_at(text, crate::text_scale::percent())
}

/// [`text_width`] at `percent` text size (RB-090-26): the characters and the slack grow with
/// the strip's font, the button's insets do not.
pub fn text_width_at(text: &str, percent: u32) -> u32 {
    let chars = u32::try_from(text.chars().count()).unwrap_or(u32::MAX);
    let advance = crate::text_scale::scale(CHAR_MILLI_DIP, percent);
    let slack = crate::text_scale::scale(SLACK, percent);
    chars
        .saturating_mul(advance)
        .div_ceil(1_000)
        .saturating_add(INSETS)
        .saturating_add(slack)
}

/// A footer's text as its strip shows it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Fitted {
    /// One line, or two.
    pub lines: Vec<String>,
    /// A line is wider than the strip: the button cuts its end.
    pub cut: bool,
}

/// `text` for a strip `available` DIP wide (`None` or 0: not laid out yet, taken as wide
/// enough). One line when it fits or `wrap` is `never`; else two, broken between two fields
/// (` | `): of the breaks whose lines both fit, the one that leaves the lines most alike; when
/// none does, the first line as full as it fits (the first field alone at least), and the end
/// of the second is cut.
pub fn fit(text: &str, available: Option<u32>, wrap: FooterWrap) -> Fitted {
    let one = |cut: bool| Fitted {
        lines: vec![text.to_owned()],
        cut,
    };
    let Some(available) = available.filter(|available| *available > 0) else {
        return one(false);
    };
    if text_width(text) <= available {
        return one(false);
    }
    let fields: Vec<&str> = text.split(SEPARATOR).collect();
    if wrap == FooterWrap::Never || fields.len() < 2 {
        return one(true);
    }
    let lines = |at: usize| (fields[..at].join(SEPARATOR), fields[at..].join(SEPARATOR));
    let fits = |line: &str| text_width(line) <= available;
    let both = (1..fields.len())
        .map(lines)
        .filter(|(first, second)| fits(first) && fits(second))
        .min_by_key(|(first, second)| text_width(first).max(text_width(second)));
    let (first, second, cut) = match both {
        Some((first, second)) => (first, second, false),
        None => {
            let at = (1..fields.len())
                .rev()
                .find(|at| fits(&lines(*at).0))
                .unwrap_or(1);
            let (first, second) = lines(at);
            (first, second, true)
        }
    };
    Fitted {
        lines: vec![first, second],
        cut,
    }
}

/// How many lines a footer may take under a view `height` DIP high (its rectangle with the
/// footer; `None`: not laid out, or a window that grows by the footer): at most half of it, so
/// the footer is never higher than what is left of the view (RB-080-78) - on a canvas zoomed far
/// out the second line goes first, then the footer. One bar takes [`MAX_LINES`] at most.
pub fn lines_for_height(height: Option<u32>) -> usize {
    lines_for_height_at(height, crate::text_scale::percent())
}

/// [`lines_for_height`] at `percent` text size (RB-090-26): a line is as high as the strip
/// draws it there.
pub fn lines_for_height_at(height: Option<u32>, percent: u32) -> usize {
    room_lines_at(height, percent).min(MAX_LINES)
}

/// Every line half of a view `height` DIP high holds - the breakpoint bar's, the vitals bar's
/// and the Lighthouse bar's together (RB-090-71, RB-090-73); without a height any number.
pub fn room_lines(height: Option<u32>) -> usize {
    room_lines_at(height, crate::text_scale::percent())
}

/// [`room_lines`] at `percent` text size.
pub fn room_lines_at(height: Option<u32>, percent: u32) -> usize {
    let Some(height) = height else {
        return usize::MAX;
    };
    let line = crate::text_scale::scale(super::FOOTER_HEIGHT, percent).max(1);
    usize::try_from(height / line.saturating_mul(2)).unwrap_or(usize::MAX)
}

/// What each bar of a footer gets of its view's room ([`share`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Share {
    /// The breakpoint bar's lines, at most [`MAX_LINES`].
    pub bar: usize,
    /// The vitals bar shows.
    pub vitals: bool,
    /// The Lighthouse bar shows.
    pub lighthouse: bool,
}

/// How `room` lines go between the breakpoint bar (`text`: it has one), the vitals bar
/// (`vitals`, RB-090-71) and the Lighthouse bar (`lighthouse`, RB-090-73): the breakpoint bar's
/// first line, then the vitals bar, then the Lighthouse bar, then the breakpoint bar's second
/// line - a bar the footer does not have takes no place in that order.
pub fn share(room: usize, text: bool, vitals: bool, lighthouse: bool) -> Share {
    /// One line of a footer, in the order the room goes to them.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Line {
        Bar,
        Vitals,
        Lighthouse,
    }
    let order = [
        (Line::Bar, text),
        (Line::Vitals, vitals),
        (Line::Lighthouse, lighthouse),
        (Line::Bar, text),
    ];
    let given: Vec<Line> = order
        .into_iter()
        .filter(|(_, has)| *has)
        .map(|(line, _)| line)
        .take(room)
        .collect();
    Share {
        bar: given.iter().filter(|line| **line == Line::Bar).count(),
        vitals: given.contains(&Line::Vitals),
        lighthouse: given.contains(&Line::Lighthouse),
    }
}

/// [`fit`] into `lines` lines: none (cut), one cut at its end, or as `wrap` says.
pub fn fit_lines(text: &str, available: Option<u32>, lines: usize, wrap: FooterWrap) -> Fitted {
    match lines {
        0 => Fitted {
            lines: Vec::new(),
            cut: true,
        },
        1 => fit(text, available, FooterWrap::Never),
        _ => fit(text, available, wrap),
    }
}

/// [`fit`] for a strip `available` DIP wide under a view `height` DIP high
/// ([`lines_for_height`]): one line, cut at its end, where two do not fit the height; no line
/// where one does not.
pub fn fit_room(
    text: &str,
    available: Option<u32>,
    height: Option<u32>,
    wrap: FooterWrap,
) -> Fitted {
    fit_lines(text, available, lines_for_height(height), wrap)
}

#[cfg(test)]
#[path = "wrap_tests.rs"]
mod tests;
