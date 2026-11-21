//! RB-090-89: a played recording whose views would all be skipped as hidden brings one into
//! sight; a recording with a view in sight, and a mode that hides none by its place, keep `skip`.

use std::collections::BTreeSet;

use rb_engine::{EngineKind, ViewId, ViewInfo};
use rb_mcp::testing::view;
use serde_json::{Value, json};

use super::*;
use crate::flows::run::InlineFlow;
use crate::layout::defs::{CANVAS_PLACE, CENTER};
use crate::layout::test_support::{config, setup};

fn grid() -> Vec<ViewInfo> {
    let mut firefox = view(4, "w2000", "https://a.test/");
    firefox.engine = EngineKind::Firefox;
    vec![
        view(1, "mobile", "https://a.test/"),
        view(2, "tablet", "https://a.test/"),
        view(3, "w2000", "https://a.test/"),
        firefox,
    ]
}

/// The config's breakpoints: `w2000` labelled "2000 px", a Firefox one first.
fn known() -> Vec<Breakpoint> {
    serde_json::from_value(json!([
        { "id": "ff", "label": "Firefox", "engine": "firefox", "width": 800, "height": 600 },
        { "id": "mobile", "width": 375, "height": 812 },
        { "id": "tablet", "width": 768, "height": 1024 },
        { "id": "w2000", "label": "2000 px", "width": 2000, "height": 1250 }
    ]))
    .expect("breakpoints")
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|name| (*name).to_owned()).collect()
}

fn hidden(ids: &[u32]) -> BTreeSet<ViewId> {
    ids.iter().map(|id| ViewId(*id)).collect()
}

/// diag29: the recording's only view lies behind a docked panel on the canvas.
#[test]
fn every_view_of_the_recording_hidden_brings_the_first_into_sight() {
    let views = grid();
    let w2000 = names(&["w2000"]);
    assert_eq!(
        needed(LayoutMode::Canvas, &views, &hidden(&[3]), &w2000, &known()),
        Some(Reveal::Center(ViewId(3))),
        "the Firefox view of the breakpoint cannot play: only the Chromium one counts"
    );
    assert_eq!(
        needed(
            LayoutMode::Horizontal,
            &views,
            &hidden(&[2, 3]),
            &w2000,
            &known()
        ),
        Some(Reveal::Scroll(2)),
        "the strip scrolls to the view's place"
    );
    // A recording without breakpoints plays in every view: the first one comes.
    assert_eq!(
        needed(
            LayoutMode::Canvas,
            &views,
            &hidden(&[1, 2, 3]),
            &[],
            &known()
        ),
        Some(Reveal::Center(ViewId(1)))
    );
}

#[test]
fn a_view_in_sight_keeps_skip() {
    let views = grid();
    let both = names(&["tablet", "w2000"]);
    assert_eq!(
        needed(LayoutMode::Canvas, &views, &hidden(&[3]), &both, &known()),
        None,
        "tablet plays, w2000 is skipped"
    );
    assert_eq!(
        needed(
            LayoutMode::Canvas,
            &views,
            &hidden(&[]),
            &names(&["w2000"]),
            &known()
        ),
        None
    );
}

#[test]
fn grid_and_split_hide_no_view_by_its_place() {
    let views = grid();
    let w2000 = names(&["w2000"]);
    assert_eq!(
        needed(LayoutMode::Grid, &views, &hidden(&[3]), &w2000, &known()),
        None
    );
    assert_eq!(
        needed(LayoutMode::Split, &views, &hidden(&[3]), &w2000, &known()),
        None
    );
}

#[test]
fn focus_mode_on_another_breakpoint_focuses_the_recordings() {
    let focused = vec![view(1, "mobile", "https://a.test/")];
    assert_eq!(
        needed(
            LayoutMode::Focus,
            &focused,
            &hidden(&[]),
            &names(&["w2000", "tablet"]),
            &known()
        ),
        Some(Reveal::Focus("w2000".to_owned()))
    );
    // A label names its breakpoint's id; a Firefox breakpoint cannot be focused; a name the
    // config does not know is no breakpoint.
    let focus = |list: &[&str]| {
        needed(
            LayoutMode::Focus,
            &focused,
            &hidden(&[]),
            &names(list),
            &known(),
        )
    };
    assert_eq!(focus(&["2000 PX"]), Some(Reveal::Focus("w2000".to_owned())));
    assert_eq!(
        focus(&["Firefox", "tablet"]),
        Some(Reveal::Focus("tablet".to_owned()))
    );
    assert_eq!(focus(&["1999x800"]), None);
    // Outside focus mode a breakpoint no view shows stays the run's error.
    assert_eq!(
        needed(
            LayoutMode::Canvas,
            &focused,
            &hidden(&[]),
            &names(&["w2000"]),
            &known()
        ),
        None
    );
    // The focused view is the recording's: nothing to do.
    assert_eq!(
        needed(
            LayoutMode::Focus,
            &focused,
            &hidden(&[]),
            &names(&["mobile"]),
            &known()
        ),
        None
    );
}

fn played(breakpoint: &str) -> RunRequest {
    RunRequest {
        name: "recording-1".to_owned(),
        views: None,
        run_id: "20261119-150000-a1b2c3".to_owned(),
        inline: Some(InlineFlow {
            source: "await page.goto(env);\n".to_owned(),
            file: ".rbrowser/out/recordings/recording-1.json".to_owned(),
            views: vec![breakpoint.to_owned()],
            pause_ms: 0,
        }),
    }
}

fn run_ok(core: &Core, id: &str, args: Value) -> Value {
    crate::layout::actions::run(core, id, &args).unwrap_or_else(|error| panic!("{id}: {error}"))
}

/// Play on the canvas: the recording's view, placed far outside the area, is centred.
#[tokio::test]
async fn play_centres_the_canvas_on_the_recordings_hidden_view() {
    let setup = setup(config());
    let core = &setup.core;
    crate::presets::run_command(core, "layout.select_preset", &json!({ "id": "board" }))
        .expect("a presets command")
        .expect("selects");
    run_ok(
        core,
        CANVAS_PLACE,
        json!({ "view": "mobile", "x": 8000, "y": 0 }),
    );
    run_ok(core, CENTER, json!({ "x": 0, "y": 0 }));
    let host = core.host();
    let mobile = host
        .views()
        .into_iter()
        .find(|view| view.cell_breakpoint.as_deref() == Some("mobile"))
        .expect("mobile")
        .id;
    assert!(host.is_off_screen(mobile), "far outside the area");

    assert!(show(core, &played("mobile")).await, "the views changed");
    assert!(!host.is_off_screen(mobile), "in sight now");

    // Switched off, the run skips the view as any other.
    run_ok(core, CENTER, json!({ "x": 0, "y": 0 }));
    let mut user = core.user_settings();
    user.automation.flows.reveal_recording = false;
    core.replace_user_settings(user.clone());
    assert!(!show(core, &played("mobile")).await);
    assert!(host.is_off_screen(mobile));
    user.automation.flows.reveal_recording = true;
    core.replace_user_settings(user);

    // Views asked for by the person keep `skip`.
    run_ok(core, CENTER, json!({ "x": 0, "y": 0 }));
    let mut asked = played("mobile");
    asked.views = Some(vec![mobile]);
    assert!(!show(core, &asked).await);
    assert!(host.is_off_screen(mobile));
}

/// Play in focus mode on another breakpoint: the recording's is focused.
#[tokio::test(start_paused = true)]
async fn play_in_focus_mode_focuses_the_recordings_breakpoint() {
    let setup = setup(config());
    let core = &setup.core;
    crate::presets::run_command(
        core,
        "layout.focus_mode",
        &json!({ "enabled": true, "focus": "desktop" }),
    )
    .expect("a presets command")
    .expect("focus mode");
    assert!(show(core, &played("mobile")).await);
    let host = core.host();
    let selection = current_selection(host);
    assert_eq!(selection.mode, LayoutMode::Focus);
    assert_eq!(selection.focus.as_deref(), Some("mobile"));
    let views = host.views();
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].cell_breakpoint.as_deref(), Some("mobile"));
    assert!(!host.is_off_screen(views[0].id));
}
