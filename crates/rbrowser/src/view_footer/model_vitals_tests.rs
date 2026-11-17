// RB-090-71: the vitals bar is a line of the footer strip, below the breakpoint bar's lines; the
// view gives the footer the height of every line it shows.

use super::*;
use crate::cell_info::{Piece, PieceAction};

fn vitals() -> Option<Piece> {
    Some(Piece {
        text: "LCP 1.20 s".to_owned(),
        tooltip: "LCP 1.20 s: good".to_owned(),
        action: PieceAction::Activate,
    })
}

#[test]
fn the_vitals_bar_adds_its_line_below_the_breakpoint_bar() {
    let footer = CellFooter {
        text: "md".to_owned(),
        lines: vec!["md".to_owned(), "768px".to_owned()],
        vitals: vitals(),
        vitals_shown: true,
        ..CellFooter::default()
    };
    assert_eq!(footer.bar_lines(), ["md", "768px"]);
    assert_eq!(footer.height(), 3 * FOOTER_HEIGHT);
}

#[test]
fn a_vitals_bar_alone_is_one_line() {
    let footer = CellFooter {
        vitals: vitals(),
        vitals_shown: true,
        ..CellFooter::default()
    };
    assert!(footer.bar_lines().is_empty(), "no breakpoint bar line");
    assert_eq!(footer.height(), FOOTER_HEIGHT);
    assert!(footer.shown().is_some());
}

#[test]
fn a_breakpoint_bar_without_room_beside_the_vitals_bar_draws_no_line() {
    // The room went to the vitals bar: the breakpoint bar's text stays for a later fit.
    let footer = CellFooter {
        text: "md".to_owned(),
        vitals: vitals(),
        vitals_shown: true,
        ..CellFooter::default()
    };
    assert!(footer.bar_lines().is_empty());
    assert_eq!(footer.height(), FOOTER_HEIGHT);
}

#[test]
fn a_vitals_bar_without_room_takes_no_height() {
    let footer = CellFooter {
        text: "md".to_owned(),
        lines: vec!["md".to_owned()],
        vitals: vitals(),
        vitals_shown: false,
        ..CellFooter::default()
    };
    assert_eq!(footer.bar_lines(), ["md"]);
    assert_eq!(footer.height(), FOOTER_HEIGHT);
}
