use rb_config::{Environment, Zoom};
use rb_sync::nav_url::follow_url;

use super::*;

fn environment(id: &str, url: &str) -> Environment {
    Environment {
        id: id.to_owned(),
        url: url.to_owned(),
        color: None,
        reload_interval_sec: None,
        asset_origins: Vec::new(),
    }
}

#[test]
fn the_grid_shows_the_breakpoint_once_per_environment() {
    let layout = live_layout("desktop", 3, 8, Zoom::Fit);
    assert_eq!((layout.columns, layout.rows, layout.gap), (3, 1, 8));
    let cells: Vec<(&str, u32)> = layout
        .cells
        .iter()
        .map(|cell| (cell.breakpoint.as_str(), cell.col))
        .collect();
    assert_eq!(cells, [("desktop", 1), ("desktop", 2), ("desktop", 3)]);
    rb_engine::compute_grid(&layout, rb_engine::CellRect::default(), 20).expect("a valid grid");
}

#[test]
fn each_view_loads_the_shown_page_under_its_environment() {
    let known = vec![
        environment("local", "http://localhost:3000"),
        environment("staging", "https://staging.example.test"),
        environment("prod", "https://www.example.test"),
    ];
    let compared = &known[..2];
    assert_eq!(
        live_urls("http://localhost:3000/shop?p=2#top", compared, &known),
        [
            "http://localhost:3000/shop?p=2#top",
            "https://staging.example.test/shop?p=2#top"
        ]
    );
    assert_eq!(
        live_urls("https://www.example.test/about", compared, &known),
        [
            "http://localhost:3000/about",
            "https://staging.example.test/about"
        ]
    );
    assert_eq!(
        live_urls("about:blank", compared, &known),
        ["http://localhost:3000", "https://staging.example.test"]
    );
}

/// What keeps the views in their environments once the grid shows: navigation sync's
/// follower URL (RB-020-06) - an internal link moves the other cell to the same path under its
/// own host, an external link is followed as it is.
#[test]
fn navigation_sync_keeps_each_view_in_its_environment() {
    let bases = vec![
        "http://localhost:3000".to_owned(),
        "https://staging.example.test".to_owned(),
    ];
    assert_eq!(
        follow_url(
            "http://localhost:3000/blog/post?x=1#c",
            "https://staging.example.test/",
            &bases
        ),
        "https://staging.example.test/blog/post?x=1#c"
    );
    assert_eq!(
        follow_url(
            "https://cdn.other.test/page",
            "https://staging.example.test/",
            &bases
        ),
        "https://cdn.other.test/page",
        "a foreign host is not rewritten"
    );
}
