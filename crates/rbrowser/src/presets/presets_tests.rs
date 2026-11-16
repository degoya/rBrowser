use std::sync::Arc;
use std::time::Duration;

use rb_config::{LayoutMode, ProjectConfig, UserSettings, Zoom};
use rb_engine::{CellRect, ViewHost, ViewId};
use serde_json::{Value, json};
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
use crate::host::Host;
use crate::host::nav::marker_url;
use crate::host::ui::recording::{RecordingUi, UiCall};

const PAGE: &str = "http://localhost:3000/products/42?x=1#top";

/// Three breakpoints, two environments and two presets (`wide`: desktop and mobile; `solo`:
/// focus mode on tablet).
fn config() -> ProjectConfig {
    serde_json::from_value(json!({
        "version": 1,
        "startUrl": "http://localhost:3000/",
        "environments": [
            { "id": "local", "url": "http://localhost:3000" },
            { "id": "staging", "url": "https://staging.example.com", "color": "#f9a825" }
        ],
        "breakpoints": [
            { "id": "desktop", "width": 1440, "height": 900 },
            { "id": "tablet", "width": 768, "height": 1024 },
            { "id": "mobile", "width": 393, "height": 852 }
        ],
        "layouts": {
            "presets": {
                "wide": { "layout": { "columns": 2, "rows": 1, "cells": [
                    { "breakpoint": "desktop", "col": 1, "row": 1, "zoom": 0.5 },
                    { "breakpoint": "mobile", "col": 2, "row": 1 }
                ] } },
                "solo": { "mode": "focus", "focus": "tablet" }
            }
        }
    }))
    .expect("the test config parses")
}

struct Setup {
    fake: Arc<FakeBrowser>,
    ui: Arc<RecordingUi>,
    core: Arc<Core>,
}

fn setup(config: ProjectConfig) -> Setup {
    let (fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(config, None).expect("opens");
    for id in 1..=20 {
        fake.add_target(&format!("T{id}"), &marker_url(ViewId(id)));
    }
    let core = Core::new(host, ui.clone(), Handle::current(), CoreSetup::default());
    on_start(&core);
    Setup { fake, ui, core }
}

async fn until(mut done: impl FnMut() -> bool) {
    for _ in 0..200 {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("condition not reached");
}

/// Attaches every view and lets it show `PAGE`.
async fn attach_all(setup: &Setup) {
    let host = setup.core.host();
    for view in host.views() {
        host.on_view_bounds(
            view.id,
            CellRect {
                x: 0,
                y: 0,
                width: 400,
                height: 300,
            },
        );
        host.on_load_end(view.id, &marker_url(view.id));
    }
    let count = host.views().len();
    until(|| setup.fake.sent("Target.attachToTarget").len() >= count).await;
    for view in host.views() {
        host.on_address_change(view.id, PAGE);
    }
}

fn run(setup: &Setup, id: &str, args: Value) -> Result<Value, CommandError> {
    run_command(&setup.core, id, &args).expect("a command of this module")
}

fn ids(setup: &Setup) -> Vec<ViewId> {
    setup
        .core
        .host()
        .views()
        .iter()
        .map(|view| view.id)
        .collect()
}

#[tokio::test]
async fn it_starts_in_the_environment_of_the_start_url_without_a_frame() {
    let setup = setup(config());
    let shell = setup.core.host().shell_state();
    assert_eq!(shell.environment.as_deref(), Some("local"));
    assert_eq!(shell.preset, None);
    assert!(setup.ui.calls().contains(&UiCall::FrameColor(None)));
}

#[tokio::test]
async fn switching_to_staging_keeps_the_path_in_every_view_and_frames_them() {
    let setup = setup(config());
    attach_all(&setup).await;
    run(&setup, "environments.switch.2", Value::Null).expect("switches");
    let expected = "https://staging.example.com/products/42?x=1#top";
    for view in ids(&setup) {
        assert!(
            setup
                .ui
                .calls()
                .contains(&UiCall::Navigate(view, expected.to_owned())),
            "{view}"
        );
    }
    assert!(
        setup
            .ui
            .calls()
            .contains(&UiCall::FrameColor(Some("#f9a825".to_owned())))
    );
    assert_eq!(
        setup.core.host().shell_state().environment.as_deref(),
        Some("staging")
    );
    // Without an id the next one follows, wrapping.
    run(&setup, "environments.select", Value::Null).expect("switches");
    assert_eq!(
        setup.core.host().shell_state().environment.as_deref(),
        Some("local")
    );
}

#[tokio::test]
async fn a_preset_rebuilds_the_grid_and_keeps_the_unchanged_views() {
    let setup = setup(config());
    let before = ids(&setup);
    run(&setup, "layout.preset.1", Value::Null).expect("switches");
    let after = ids(&setup);
    assert_eq!(after.len(), 2);
    // desktop (view 1) and mobile (view 3) live on; tablet (view 2) is closed.
    assert_eq!(after, [before[0], before[2]]);
    assert_eq!(
        setup.core.host().shell_state().preset.as_deref(),
        Some("wide")
    );
}

#[tokio::test]
async fn focus_mode_shows_one_view_and_the_arrow_moves_to_the_next_breakpoint() {
    let setup = setup(config());
    attach_all(&setup).await;
    let answer = ipc(
        &setup.core,
        &ShellToCore::SelectLayoutPreset { id: "solo".into() },
    );
    assert!(matches!(answer, Some(Ok(CoreToShell::Ack))));
    assert_eq!(ids(&setup).len(), 1);
    let arrow = json!({ "source": ARROW_KEY_SOURCE });
    run(&setup, "layout.focus_next", arrow.clone()).expect("steps");
    let shell = setup.core.host().shell_state();
    assert_eq!(shell.mode, LayoutMode::Focus);
    assert_eq!(shell.focus.as_deref(), Some("mobile"));
    let views = setup.core.host().views();
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].cell_breakpoint.as_deref(), Some("mobile"));
    // The new view loads the page the grid showed, not the start URL.
    let new = views[0].id;
    setup.core.host().on_load_end(new, &marker_url(new));
    until(|| {
        setup
            .ui
            .calls()
            .contains(&UiCall::Navigate(new, PAGE.to_owned()))
    })
    .await;
    // With the setting off, the arrow keys do nothing.
    let mut user = UserSettings::default();
    user.focus_mode.arrow_keys = false;
    setup.core.replace_user_settings(user);
    run(&setup, "layout.focus_next", arrow).expect("ignored");
    assert_eq!(
        setup.core.host().shell_state().focus.as_deref(),
        Some("mobile")
    );
}

#[tokio::test]
async fn focus_mode_toggles_on_the_active_breakpoint_and_back() {
    let setup = setup(config());
    let before = ids(&setup);
    run(&setup, "layout.focus_mode", json!({ "focus": "tablet" })).expect("on");
    assert_eq!(ids(&setup), [before[1]], "the tablet view lives on");
    run(&setup, "layout.focus_mode", Value::Null).expect("off");
    let shell = setup.core.host().shell_state();
    assert_eq!((shell.mode, shell.focus), (LayoutMode::Grid, None));
    assert_eq!(ids(&setup).len(), 3);
    assert!(run(&setup, "layout.focus_next", Value::Null).is_err());
    assert!(matches!(
        run(&setup, "layout.focus_mode", json!({ "focus": "watch" })),
        Err(CommandError::InvalidArgs(_))
    ));
}

#[tokio::test]
async fn zoom_to_fit_fits_every_cell() {
    let setup = setup(config());
    let host = setup.core.host();
    host.on_grid_area(CellRect {
        x: 0,
        y: 0,
        width: 1200,
        height: 800,
    });
    run(&setup, "layout.select_preset", json!({ "id": "wide" })).expect("switches");
    let zooms = |host: &Host| {
        host.views()
            .iter()
            .filter_map(|view| view.cell.as_ref().map(|cell| cell.zoom))
            .collect::<Vec<_>>()
    };
    assert_eq!(zooms(host), [Zoom::Factor(0.5), Zoom::Fit]);
    run(&setup, "layout.zoom_to_fit", Value::Null).expect("fits");
    assert_eq!(zooms(host), [Zoom::Fit, Zoom::Fit]);
}

#[tokio::test]
async fn unknown_ids_and_missing_places_are_refused() {
    let setup = setup(config());
    let answer = ipc(
        &setup.core,
        &ShellToCore::SelectEnvironment { id: "prod".into() },
    );
    assert!(matches!(answer, Some(Err((IpcErrorCode::NotFound, _)))));
    assert!(matches!(
        run(&setup, "layout.preset.9", Value::Null),
        Err(CommandError::Failed(_))
    ));
    assert!(run_command(&setup.core, "view.back", &Value::Null).is_none());
}

#[tokio::test]
async fn get_devices_answers_the_presets_in_effect() {
    let setup = setup(config());
    let Some(Ok(CoreToShell::Devices { items })) = ipc(&setup.core, &ShellToCore::GetDevices)
    else {
        panic!("devices");
    };
    assert!(items.iter().any(|device| device.id == "iphone-15"));
    let mut user = UserSettings::default();
    user.devices.replace_defaults = true;
    user.devices.items = items.into_iter().take(2).collect();
    setup.core.replace_user_settings(user);
    let Some(Ok(CoreToShell::Devices { items })) = ipc(&setup.core, &ShellToCore::GetDevices)
    else {
        panic!("devices");
    };
    assert_eq!(items.len(), 2);
}

#[tokio::test]
async fn a_config_change_keeps_the_selection_and_drops_a_removed_one() {
    let setup = setup(config());
    run(&setup, "layout.select_preset", json!({ "id": "wide" })).expect("switches");
    let host = setup.core.host();
    host.open_config(config(), None).expect("reopens");
    on_config_changed(&setup.core);
    assert_eq!(host.shell_state().preset.as_deref(), Some("wide"));
    assert_eq!(ids(&setup).len(), 2);
    let mut changed = config();
    changed.layouts.presets.shift_remove("wide");
    host.open_config(changed, None).expect("reopens");
    on_config_changed(&setup.core);
    assert_eq!(host.shell_state().preset, None);
    assert_eq!(ids(&setup).len(), 3);
}

/// An environment whose URL is no `http`/`https` URL is never loaded, even when it reached the
/// running config without the check (charge-4 review, finding 2: `file:` or `data:` in every
/// view).
#[tokio::test]
async fn an_environment_that_is_no_web_url_is_refused_and_nothing_navigates() {
    let mut config = config();
    config.environments[1].url = "file:///C:/Users".to_owned();
    let setup = setup(config);
    attach_all(&setup).await;
    let before = setup.ui.calls().len();
    run(&setup, "environments.switch.2", Value::Null).expect_err("refused");
    assert!(
        !setup.ui.calls()[before..]
            .iter()
            .any(|call| matches!(call, UiCall::Navigate(..))),
        "{:?}",
        &setup.ui.calls()[before..]
    );
    assert_eq!(
        setup.core.host().shell_state().environment.as_deref(),
        Some("local")
    );
}
