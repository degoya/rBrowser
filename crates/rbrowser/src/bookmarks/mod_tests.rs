use std::sync::Arc;

use rb_config::{ConfigFile, Environment};
use rb_engine::ViewId;
use serde_json::json;
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
use crate::host::state::default_config;
use crate::host::ui::recording::{RecordingUi, UiCall};
use crate::sync_hub;
use crate::sync_hub::tests::{Setup, attach_all, setup};

fn bookmarks() -> Vec<rb_config::Bookmark> {
    serde_json::from_value(json!([
        { "id": "checkout", "label": "Checkout", "path": "/checkout", "group": "Shop",
          "shortcut": "Ctrl+Shift+F9" },
        { "id": "docs", "label": "Docs", "url": "https://docs.example.com/guide" },
        { "id": "tab", "label": "New tab", "path": "/", "shortcut": "Ctrl+T" }
    ]))
    .expect("bookmarks")
}

fn environment(id: &str, url: &str) -> Environment {
    Environment {
        id: id.to_owned(),
        url: url.to_owned(),
        color: None,
        reload_interval_sec: None,
        asset_origins: Vec::new(),
    }
}

/// Views 1 desktop, 2 tablet, 3 mobile with `local` and `staging`, the bookmarks above, the
/// breakpoints `decoupled` with `syncNavigation: false`, sync and navigation sync started.
fn started(decoupled: &[&str]) -> Setup {
    let setup = setup();
    let host = setup.core.host();
    let mut config = host.config();
    config.environments = vec![
        environment("local", "http://localhost:5173"),
        environment("staging", "https://staging.example.com/"),
    ];
    config.bookmarks = bookmarks();
    for breakpoint in &mut config.breakpoints {
        breakpoint.sync_navigation = !decoupled.contains(&breakpoint.id.as_str());
    }
    host.open_config(config, None).expect("reopens");
    host.update_shell_state(|shell| shell.environment = Some("local".to_owned()));
    sync_hub::on_start(&setup.core);
    crate::nav_sync::on_start(&setup.core);
    setup
}

fn navigations(setup: &Setup) -> Vec<(ViewId, String)> {
    setup
        .ui
        .calls()
        .into_iter()
        .filter_map(|call| match call {
            UiCall::Navigate(view, url) => Some((view, url)),
            _ => None,
        })
        .collect()
}

fn shell_scripts(ui: &RecordingUi) -> Vec<String> {
    ui.calls()
        .into_iter()
        .filter_map(|call| match call {
            UiCall::Shell(script) => Some(script),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn without_arguments_the_shell_asks() {
    let setup = setup();
    setup.core.shell_ready();
    for id in [OPEN, ADD] {
        assert_eq!(
            run(&setup.core, id, &Value::Null).expect("runs"),
            Value::Null
        );
        assert!(
            shell_scripts(&setup.ui)
                .iter()
                .any(|script| script.contains("uiCommand") && script.contains(id)),
            "{id} reached the shell"
        );
    }
    let focused = setup
        .ui
        .calls()
        .iter()
        .filter(|call| **call == UiCall::FocusShell)
        .count();
    assert_eq!(focused, 2, "the shell takes the keyboard");
    assert!(matches!(
        run(&setup.core, "bookmarks.nope", &Value::Null),
        Err(CommandError::UnknownCommand(_))
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_path_opens_in_the_environment_in_effect_and_a_url_everywhere_the_same() {
    let setup = started(&[]);
    attach_all(&setup).await;
    let core = &setup.core;
    assert_eq!(
        run(core, OPEN, &json!({ "id": "checkout" })).expect("opens"),
        json!({ "id": "checkout", "url": "http://localhost:5173/checkout", "relative": true })
    );
    core.host()
        .update_shell_state(|shell| shell.environment = Some("staging".to_owned()));
    run(core, OPEN, &json!({ "id": "checkout" })).expect("opens");
    run(core, OPEN, &json!({ "id": "docs" })).expect("opens");
    let calls = navigations(&setup);
    let to = |url: &str| -> Vec<ViewId> {
        calls
            .iter()
            .filter(|(_, called)| called == url)
            .map(|(view, _)| *view)
            .collect()
    };
    let all = vec![ViewId(1), ViewId(2), ViewId(3)];
    assert_eq!(to("http://localhost:5173/checkout"), all);
    assert_eq!(to("https://staging.example.com/checkout"), all);
    assert_eq!(to("https://docs.example.com/guide"), all);
    let summaries = summaries(core.host());
    assert_eq!(
        summaries[0].url.as_deref(),
        Some("https://staging.example.com/checkout")
    );
    assert_eq!(summaries[0].group.as_deref(), Some("Shop"));
    assert_eq!(
        summaries[1].url.as_deref(),
        Some("https://docs.example.com/guide")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_decoupled_active_view_opens_the_bookmark_alone() {
    let setup = started(&["tablet"]);
    attach_all(&setup).await;
    let core = &setup.core;
    core.host().set_active_view(ViewId(2)).expect("a view");
    assert_eq!(open_target(core.host()), Target::View(ViewId(2)));
    run(core, OPEN, &json!({ "id": "docs" })).expect("opens");
    core.host().set_active_view(ViewId(1)).expect("a view");
    assert_eq!(open_target(core.host()), Target::All);
    run(core, OPEN, &json!({ "id": "checkout" })).expect("opens");
    let calls = navigations(&setup);
    let views = |url: &str| -> Vec<ViewId> {
        calls
            .iter()
            .filter(|(_, called)| called == url)
            .map(|(view, _)| *view)
            .collect()
    };
    assert_eq!(views("https://docs.example.com/guide"), [ViewId(2)]);
    assert_eq!(
        views("http://localhost:5173/checkout"),
        [ViewId(1), ViewId(3)]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unknown_bookmark_or_bad_arguments_are_refused() {
    let setup = started(&[]);
    let core = &setup.core;
    assert!(matches!(
        run(core, OPEN, &json!({ "id": "nope" })),
        Err(CommandError::InvalidArgs(_))
    ));
    assert!(matches!(
        run(core, OPEN, &json!({ "id": "docs", "extra": 1 })),
        Err(CommandError::InvalidArgs(_))
    ));
    assert!(navigations(&setup).is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_bookmark_shortcut_opens_it_and_a_reserved_one_is_reported_with_its_reason() {
    let setup = started(&[]);
    let core = &setup.core;
    core.shell_ready();
    let (keys, conflicts) = shortcuts(core);
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].bookmark, "checkout");
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].bookmark, "tab");
    assert!(conflicts[0].reason_key.starts_with("shortcuts.reserved."));
    assert!(on_unbound_key(
        core,
        "Ctrl+Shift+F9".parse().expect("a shortcut")
    ));
    assert!(!on_unbound_key(core, "Ctrl+T".parse().expect("a shortcut")));
    assert!(!on_unbound_key(core, "A".parse().expect("a shortcut")));
    on_config_changed(core);
    assert!(
        shell_scripts(&setup.ui)
            .iter()
            .any(|script| script.contains(CONFLICT_KEY) && script.contains("Ctrl+T")),
        "the conflict reached the shell"
    );
}

/// CEF calls the start hooks on its UI thread, outside the Tokio runtime.
#[test]
fn the_start_hook_runs_outside_the_runtime() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("a runtime");
    let core = {
        let _inside = runtime.enter();
        let (_fake, router) = FakeBrowser::connect();
        let ui = Arc::new(RecordingUi::default());
        let host = Host::new(ui.clone(), router, Handle::current());
        let mut config = default_config();
        config.bookmarks = bookmarks();
        host.open_config(config, None).expect("opens");
        Core::new(host, ui, Handle::current(), CoreSetup::default())
    };
    assert!(Handle::try_current().is_err());
    on_start(&core);
}

/// A project on disk with `config.json` and the window opened on it.
fn on_disk(setup: &Setup) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut config = setup.core.host().config();
    config.environments = vec![environment("local", "http://localhost:5173")];
    config.bookmarks.clear();
    let document = json!({
        "version": 1,
        "environments": [{ "id": "local", "url": "http://localhost:5173" }],
        "bookmarks": [{ "id": "home", "label": "Home", "path": "/" }]
    });
    rb_config::write_config_file(dir.path(), ConfigFile::Project, &document).expect("written");
    config.bookmarks = serde_json::from_value(document["bookmarks"].clone()).expect("bookmarks");
    setup
        .core
        .host()
        .open_config(config, Some(dir.path().to_path_buf()))
        .expect("opens");
    setup
        .core
        .host()
        .update_shell_state(|shell| shell.environment = Some("local".to_owned()));
    dir
}

#[tokio::test(flavor = "multi_thread")]
async fn bookmark_this_page_writes_the_chosen_file() {
    let setup = setup();
    let dir = on_disk(&setup);
    let core = &setup.core;
    let answer = run(
        core,
        ADD,
        &json!({ "label": "Checkout", "path": "/checkout", "group": "Shop" }),
    )
    .expect("added");
    assert_eq!(
        answer,
        json!({ "added": "checkout", "scope": "project", "relative": true,
                "url": "http://localhost:5173/checkout" })
    );
    let project = rb_config::read_json(&ConfigFile::Project.path(dir.path())).expect("reads");
    assert_eq!(
        project["bookmarks"],
        json!([
            { "id": "home", "label": "Home", "path": "/" },
            { "id": "checkout", "label": "Checkout", "path": "/checkout", "group": "Shop" }
        ])
    );
    assert_eq!(
        project["environments"][0]["id"], "local",
        "the rest is kept"
    );
    // The file stays in its canonical form: written again unchanged, it is byte-identical.
    let path = ConfigFile::Project.path(dir.path());
    let before = std::fs::read(&path).expect("read");
    rb_config::write_config_file(dir.path(), ConfigFile::Project, &project).expect("writes");
    assert_eq!(std::fs::read(&path).expect("read"), before);

    run(
        core,
        ADD,
        &json!({ "label": "Mine", "url": "http://localhost:6006", "scope": "local" }),
    )
    .expect("added");
    let local = rb_config::read_json(&ConfigFile::Local.path(dir.path())).expect("reads");
    assert_eq!(
        local,
        json!({ "bookmarks": [{ "id": "mine", "label": "Mine", "url": "http://localhost:6006" }] })
    );
}

/// A project on disk whose `config.json` is the config in effect, the window opened on it.
fn on_disk_as_in_effect(setup: &Setup) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    let host = setup.core.host();
    let mut config = host.config();
    // The window's stand-in `about:blank` is no start URL the config check takes.
    config.start_url = Some("http://localhost:5173/".to_owned());
    config.environments = vec![environment("local", "http://localhost:5173")];
    config.bookmarks =
        serde_json::from_value(json!([{ "id": "home", "label": "Home", "path": "/" }]))
            .expect("bookmarks");
    let mut document = serde_json::to_value(&config).expect("a document");
    // A field without a value is left out of the file, as the loader leaves it out.
    without_nulls(&mut document);
    rb_config::write_config_file(dir.path(), ConfigFile::Project, &document).expect("written");
    host.open_config(config, Some(dir.path().to_path_buf()))
        .expect("opens");
    host.update_shell_state(|shell| shell.environment = Some("local".to_owned()));
    dir
}

fn without_nulls(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.retain(|_, item| !item.is_null());
            map.values_mut().for_each(without_nulls);
        }
        Value::Array(items) => items.iter_mut().for_each(without_nulls),
        _ => {}
    }
}

fn grid_count(ui: &RecordingUi) -> usize {
    ui.calls()
        .iter()
        .filter(|call| matches!(call, UiCall::Grid(_)))
        .count()
}

/// The bookmark set a moment ago is in the toolbar's list at once: the config is read again
/// right after the write - the watcher hears nothing over a WSL share - and the shell gets the
/// state. With nothing but the bookmarks changed the grid is left alone (RB-080-16).
#[tokio::test(flavor = "multi_thread")]
async fn a_set_bookmark_is_listed_at_once_without_rebuilding_the_grid() {
    let setup = setup();
    let _dir = on_disk_as_in_effect(&setup);
    let host = setup.core.host();
    let mut events = host.subscribe();
    let grids = grid_count(&setup.ui);
    run(
        &setup.core,
        ADD,
        &json!({ "label": "Checkout", "path": "/checkout" }),
    )
    .expect("added");
    let listed: Vec<String> = summaries(host).iter().map(|b| b.id.clone()).collect();
    assert_eq!(listed, ["home", "checkout"]);
    assert_eq!(grid_count(&setup.ui), grids, "the grid is left alone");
    let mut seen = Vec::new();
    while let Ok(event) = events.try_recv() {
        seen.push(event);
    }
    assert!(
        seen.contains(&rb_engine::ViewEvent::StateChanged),
        "the shell gets the state: {seen:?}"
    );
    // A bookmark of `config.local.json` is listed the same.
    run(
        &setup.core,
        ADD,
        &json!({ "label": "Mine", "url": "http://localhost:6006", "scope": "local" }),
    )
    .expect("added");
    let listed: Vec<String> = summaries(host).iter().map(|b| b.id.clone()).collect();
    assert_eq!(listed, ["home", "checkout", "mine"]);
    assert_eq!(grid_count(&setup.ui), grids, "the grid is left alone");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_bookmark_the_check_refuses_is_not_written() {
    let setup = setup();
    let dir = on_disk(&setup);
    let path = ConfigFile::Project.path(dir.path());
    let before = std::fs::read(&path).expect("read");
    for bad in [
        json!({ "label": "x", "path": "/a", "url": "https://x.example" }),
        json!({ "label": "x", "url": "javascript:alert(1)" }),
        json!({ "label": "x", "path": "relative" }),
        json!({ "id": "home", "label": "x", "path": "/x" }),
        json!({ "label": "x" }),
    ] {
        assert!(
            matches!(
                run(&setup.core, ADD, &bad),
                Err(CommandError::InvalidArgs(_))
            ),
            "{bad}"
        );
    }
    assert_eq!(std::fs::read(&path).expect("read"), before);
    assert!(!ConfigFile::Local.path(dir.path()).exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn without_a_project_nothing_is_added() {
    let setup = setup();
    assert!(matches!(
        run(&setup.core, ADD, &json!({ "label": "x", "path": "/" })),
        Err(CommandError::Failed(_))
    ));
}
