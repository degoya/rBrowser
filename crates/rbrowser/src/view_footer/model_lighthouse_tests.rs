// RB-090-73: the Lighthouse bar is a line of the footer strip, below the vitals bar; the view
// gives the footer the height of every line it shows.

use super::*;
use crate::cell_info::{Piece, PieceAction};
use crate::view_footer::lighthouse::{Category, Gauge, LighthouseBar};

fn lighthouse() -> Option<LighthouseBar> {
    Some(LighthouseBar {
        gauges: vec![Gauge {
            category: Category::Seo,
            score: Some(100),
        }],
        tooltip: "SEO: 100 (good)".to_owned(),
        run: "20261117-143200".to_owned(),
        breakpoint: "md".to_owned(),
    })
}

#[test]
fn the_lighthouse_bar_adds_its_line_below_the_vitals_bar() {
    let footer = CellFooter {
        text: "md".to_owned(),
        lines: vec!["md".to_owned()],
        vitals: Some(Piece {
            text: "LCP 1.20 s".to_owned(),
            tooltip: String::new(),
            action: PieceAction::Activate,
        }),
        vitals_shown: true,
        lighthouse: lighthouse(),
        lighthouse_shown: true,
        ..CellFooter::default()
    };
    assert_eq!(footer.height(), 3 * FOOTER_HEIGHT);
}

#[test]
fn a_lighthouse_bar_alone_is_one_line_and_leaves_the_breakpoint_bar_none() {
    let footer = CellFooter {
        text: "md".to_owned(),
        lighthouse: lighthouse(),
        lighthouse_shown: true,
        ..CellFooter::default()
    };
    assert!(
        footer.bar_lines().is_empty(),
        "the room went to the Lighthouse bar"
    );
    assert_eq!(footer.height(), FOOTER_HEIGHT);
}

#[test]
fn a_lighthouse_bar_without_room_takes_no_height() {
    let footer = CellFooter {
        text: "md".to_owned(),
        lines: vec!["md".to_owned()],
        lighthouse: lighthouse(),
        lighthouse_shown: false,
        ..CellFooter::default()
    };
    assert_eq!(footer.height(), FOOTER_HEIGHT);
}
