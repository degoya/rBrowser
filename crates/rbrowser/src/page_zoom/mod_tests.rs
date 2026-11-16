use std::sync::Arc;

use rb_config::{PageZoomSettings, UserSettings, ViewsSettings};
use rb_engine::ViewId;
use serde_json::{Value, json};
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::{Core, CoreSetup};
use crate::host::Host;
use crate::host::state::default_config;
use crate::host::ui::recording::{RecordingUi, UiCall};

fn core_with(settings: PageZoomSettings) -> (Arc<Core>, Arc<RecordingUi>) {
    // Each test starts from an empty model (nextest runs a test per process; this keeps a
    // shared runner honest too).
    reset_model();
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(default_config(), None).expect("opens");
    let user = UserSettings {
        views: ViewsSettings {
            page_zoom: settings,
        },
        ..UserSettings::default()
    };
    let setup = CoreSetup {
        user,
        ..CoreSetup::default()
    };
    (Core::new(host, ui.clone(), Handle::current(), setup), ui)
}

fn views(core: &Core) -> Vec<ViewId> {
    core.host().views().iter().map(|view| view.id).collect()
}

/// The page zooms the window was asked for, in percent.
fn zooms(ui: &RecordingUi) -> Vec<(ViewId, u32)> {
    ui.calls()
        .into_iter()
        .filter_map(|call| match call {
            UiCall::PageZoom(view, level) => Some((view, percent_of(level))),
            _ => None,
        })
        .collect()
}

fn sync() -> PageZoomSettings {
    PageZoomSettings {
        sync: true,
        ..PageZoomSettings::default()
    }
}

#[tokio::test]
async fn reset_sets_the_active_view_back_to_100_percent() {
    let (core, ui) = core_with(PageZoomSettings::default());
    let first = views(&core)[0];
    run(&core, ZOOM_IN, &Value::Null).expect("zooms in");
    let answer = run(&core, RESET, &Value::Null).expect("resets");
    assert_eq!(zooms(&ui), vec![(first, 110), (first, 100)]);
    assert_eq!(
        answer,
        json!({ "views": [{ "view": first.0, "level": 1.0, "percent": 100 }] })
    );
}

#[tokio::test]
async fn reset_with_a_level_sets_that_factor_on_the_target() {
    let (core, ui) = core_with(PageZoomSettings::default());
    let second = views(&core)[1];
    let answer = run(&core, RESET, &json!({ "target": second.0, "level": 1.5 })).expect("sets");
    assert_eq!(zooms(&ui), vec![(second, 150)]);
    assert_eq!(answer["views"][0]["percent"], json!(150));
    assert_eq!(answer["views"][0]["level"], json!(1.5));
}

#[tokio::test]
async fn a_level_outside_chromiums_range_is_refused() {
    let (core, ui) = core_with(PageZoomSettings::default());
    for level in [json!(0.1), json!(6), json!(-1), json!("big")] {
        let result = run(&core, RESET, &json!({ "level": level }));
        assert!(
            matches!(result, Err(CommandError::InvalidArgs(_))),
            "{level}: {result:?}"
        );
    }
    let stepped = run(&core, ZOOM_IN, &json!({ "level": 1.5 }));
    assert!(matches!(stepped, Err(CommandError::InvalidArgs(_))));
    assert!(zooms(&ui).is_empty());
}

#[tokio::test]
async fn in_and_out_on_all_move_each_view_from_its_own_level() {
    let (core, ui) = core_with(PageZoomSettings::default());
    let ids = views(&core);
    run(&core, ZOOM_IN, &json!({ "target": ids[0].0 })).expect("in");
    run(&core, ZOOM_OUT, &json!({ "target": "all" })).expect("out");
    let mut expected = vec![(ids[0], 110), (ids[0], 100)];
    expected.extend(ids[1..].iter().map(|view| (*view, 90)));
    assert_eq!(zooms(&ui), expected);
}

#[tokio::test]
async fn with_sync_one_views_zoom_reaches_every_view() {
    let (core, ui) = core_with(sync());
    let ids = views(&core);
    run(&core, ZOOM_IN, &json!({ "target": ids[1].0 })).expect("in");
    let all: Vec<(ViewId, u32)> = ids.iter().map(|view| (*view, 110)).collect();
    assert_eq!(zooms(&ui), all);
}

#[tokio::test]
async fn an_unknown_target_is_refused() {
    let (core, _ui) = core_with(PageZoomSettings::default());
    let result = run(&core, RESET, &json!({ "target": 999 }));
    assert!(
        matches!(result, Err(CommandError::InvalidArgs(_))),
        "{result:?}"
    );
}

#[tokio::test]
async fn a_wheel_zoom_follows_the_settings() {
    let (core, ui) = core_with(PageZoomSettings {
        wheel: false,
        ..PageZoomSettings::default()
    });
    let first = views(&core)[0];
    // The window's first read, then the wheel.
    observed(&core, first, 0.0);
    observed(&core, first, level_of(1.25));
    assert_eq!(
        zooms(&ui),
        vec![(first, 100)],
        "the wheel's zoom is put back"
    );

    let (core, ui) = core_with(sync());
    let ids = views(&core);
    for view in &ids {
        observed(&core, *view, 0.0);
    }
    observed(&core, ids[0], level_of(1.25));
    let others: Vec<(ViewId, u32)> = ids[1..].iter().map(|view| (*view, 125)).collect();
    assert_eq!(zooms(&ui), others, "every other view follows");
}

#[tokio::test]
async fn a_load_starts_at_100_percent_when_the_zoom_is_not_kept() {
    let (core, ui) = core_with(PageZoomSettings {
        keep_on_reload: false,
        ..PageZoomSettings::default()
    });
    let first = views(&core)[0];
    loaded(&core, first, level_of(1.1));
    assert_eq!(zooms(&ui), vec![(first, 100)]);
    let (core, ui) = core_with(PageZoomSettings::default());
    let first = views(&core)[0];
    loaded(&core, first, level_of(1.1));
    assert!(zooms(&ui).is_empty(), "kept");
}

#[tokio::test]
async fn a_key_runs_only_while_the_setting_lets_it() {
    let (core, _ui) = core_with(PageZoomSettings::default());
    assert!(keys_on(&core));
    let (core, _ui) = core_with(PageZoomSettings {
        keys: false,
        ..PageZoomSettings::default()
    });
    assert!(!keys_on(&core));
}

#[tokio::test]
async fn a_level_that_followed_a_set_is_no_wheel_zoom() {
    // Chromium keeps the zoom per site: setting one view moves its neighbours on the same host.
    // The window reports them as followers, and the wheel's guard leaves them alone.
    let (core, ui) = core_with(PageZoomSettings {
        wheel: false,
        ..PageZoomSettings::default()
    });
    let ids = views(&core);
    for view in &ids {
        observed(&core, *view, 0.0);
    }
    run(&core, ZOOM_IN, &json!({ "target": ids[0].0 })).expect("in");
    followed(&[(ids[1], level_of(1.1))]);
    observed(&core, ids[1], level_of(1.1));
    assert_eq!(
        zooms(&ui),
        vec![(ids[0], 110)],
        "nothing put back, no ping-pong"
    );
}
