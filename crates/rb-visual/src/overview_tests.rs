use std::path::PathBuf;

use super::*;
use crate::{decode_png, encode_png};

const RED: [u8; 4] = [220, 38, 38, 255];
const BLUE: [u8; 4] = [37, 99, 235, 255];

fn tiles() -> Vec<Tile> {
    vec![
        Tile {
            label: "Mobile".to_owned(),
            size: crate::size_text(40, 30, 3.0),
            detail: "chromium".to_owned(),
            color: parse_color("#16a34a"),
            image: Rgba::new(40, 30, RED).expect("small"),
        },
        Tile {
            label: "Wide desktop label that is cut".to_owned(),
            size: crate::size_text(20, 50, 0.0),
            detail: "chromium \u{b7} staging".to_owned(),
            color: None,
            image: Rgba::new(20, 50, BLUE).expect("small"),
        },
    ]
}

#[test]
fn tiles_sit_side_by_side_in_order_under_their_labels() {
    let sizes = [(40, 30), (20, 50)];
    let placed = layout(&sizes, true);
    assert_eq!(
        placed.positions,
        vec![
            (MARGIN, MARGIN + STRIP),
            (MARGIN + 40 + GAP, MARGIN + STRIP)
        ]
    );
    assert_eq!(placed.width, MARGIN * 2 + 40 + GAP + 20);
    assert_eq!(placed.height, MARGIN * 2 + STRIP + 50);
    let bare = layout(&sizes, false);
    assert_eq!(bare.positions[0], (MARGIN, MARGIN));
    assert_eq!(bare.height, MARGIN * 2 + 50);
}

#[test]
fn every_tile_is_drawn_at_its_place_with_its_frame() {
    let image = compose(&tiles(), &OverviewOptions { labels: false }).expect("composed");
    assert_eq!(
        (image.width(), image.height()),
        (MARGIN * 2 + 40 + GAP + 20, MARGIN * 2 + 50)
    );
    assert_eq!(image.get(MARGIN, MARGIN), Some(RED));
    assert_eq!(image.get(MARGIN + 39, MARGIN + 29), Some(RED));
    assert_eq!(image.get(MARGIN + 40 + GAP, MARGIN), Some(BLUE));
    // The environment colour frames the first tile, a grey line the second.
    assert_eq!(image.get(MARGIN - 1, MARGIN), Some([0x16, 0xa3, 0x4a, 255]));
    assert_eq!(image.get(MARGIN + 40 + GAP - 1, MARGIN), Some(BORDER));
    assert_eq!(image.get(1, 1), Some(BACKGROUND));
}

#[test]
fn no_tiles_is_an_error() {
    assert!(matches!(
        compose(&[], &OverviewOptions { labels: true }),
        Err(VisualError::Empty)
    ));
}

#[test]
fn colours_parse_from_hex() {
    assert_eq!(parse_color("#16a34a"), Some([0x16, 0xa3, 0x4a]));
    assert_eq!(parse_color("16a34a"), None);
    assert_eq!(parse_color("#16a34"), None);
    assert_eq!(parse_color("#zzzzzz"), None);
}

/// The overview with labels against `tests/golden/overview.png`; `RB_UPDATE_GOLDEN=1` writes
/// the file anew after an intended change of the drawing.
#[test]
fn golden_overview_with_labels() {
    let image = compose(&tiles(), &OverviewOptions { labels: true }).expect("composed");
    let golden = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join("overview.png");
    if std::env::var_os("RB_UPDATE_GOLDEN").is_some() {
        std::fs::write(&golden, encode_png(&image).expect("encoded")).expect("golden written");
    }
    let expected = decode_png(&std::fs::read(&golden).expect("golden file")).expect("golden png");
    assert_eq!(
        image,
        expected,
        "the overview differs from {}",
        golden.display()
    );
}
