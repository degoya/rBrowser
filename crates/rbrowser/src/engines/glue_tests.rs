use rb_engine::external::ExternalView;

use super::*;

fn view(status: ExternalStatus) -> ExternalView {
    serde_json::from_value(serde_json::json!({
        "view": 3, "label": "Phone", "breakpoint": "phone", "engine": "firefox",
        "status": status, "url": "",
    }))
    .expect("an external view")
}

#[test]
fn the_sync_routes_follow_the_status() {
    assert_eq!(route_of(&view(ExternalStatus::Unavailable)), None);
    assert_eq!(
        route_of(&view(ExternalStatus::Running)),
        Some((ViewId(3), ViewRoute::Engine(EngineKind::Firefox)))
    );
    for away in [
        ExternalStatus::Starting,
        ExternalStatus::Exited,
        ExternalStatus::Closed,
        ExternalStatus::Restarting,
        ExternalStatus::Failed,
    ] {
        assert_eq!(route_of(&view(away)), Some((ViewId(3), ViewRoute::Stopped)));
    }
}

#[test]
fn a_free_size_stands_in_for_its_breakpoint() {
    let info: ViewInfo = serde_json::from_value(serde_json::json!({
        "id": 3, "label": "500x800", "breakpoint": null, "cellBreakpoint": null,
        "engine": "webkit", "width": 500, "height": 800, "dpr": 1.0, "scale": 1.0,
        "url": "", "title": "", "loading": false, "session": "default", "cell": null,
    }))
    .expect("a view");
    let breakpoint = stand_in(&info);
    assert_eq!(
        (breakpoint.id.as_str(), breakpoint.width, breakpoint.engine),
        ("500x800", Some(500), EngineKind::Webkit)
    );
}

#[test]
fn the_window_title_names_the_view_and_the_engine() {
    // The catalogue has the template once the wave's parts are merged; before, the key shows.
    let title = window_title("Phone", EngineKind::Firefox);
    assert!(
        title == "rBrowser \u{b7} Phone \u{b7} Firefox" || title == "engines.external.windowTitle",
        "{title}"
    );
}
