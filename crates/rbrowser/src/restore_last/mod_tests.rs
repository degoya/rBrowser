use rb_config::Environment;
use rb_engine::ViewId;
use serde_json::json;

use super::model::{FORMAT_VERSION, LastPage};
use super::*;
use crate::host::ui::recording::UiCall;
use crate::sync_hub;
use crate::sync_hub::tests::{Setup, attach_all, setup};

fn environment(id: &str, url: &str) -> Environment {
    Environment {
        id: id.to_owned(),
        url: url.to_owned(),
        color: None,
        reload_interval_sec: None,
        asset_origins: Vec::new(),
    }
}

/// Views 1 desktop, 2 tablet, 3 mobile with `local` and `staging`, `environment` in effect,
/// sync and navigation sync started.
fn started(environment_in_effect: &str) -> Setup {
    let setup = setup();
    let host = setup.core.host();
    let mut config = host.config();
    config.environments = vec![
        environment("local", "http://localhost:5173"),
        environment("staging", "https://staging.example.com/"),
    ];
    host.open_config(config, None).expect("reopens");
    host.update_shell_state(|shell| shell.environment = Some(environment_in_effect.to_owned()));
    sync_hub::on_start(&setup.core);
    crate::nav_sync::on_start(&setup.core);
    setup
}

fn entry(breakpoint: &str, path: Option<&str>, url: Option<&str>, decoupled: bool) -> LastPage {
    LastPage {
        breakpoint: breakpoint.to_owned(),
        index: 0,
        path: path.map(str::to_owned),
        url: url.map(str::to_owned),
        decoupled,
    }
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

#[tokio::test(flavor = "multi_thread")]
async fn the_snapshot_keeps_each_views_path_url_and_coupling() {
    let setup = started("local");
    let host = setup.core.host();
    host.on_address_change(ViewId(1), "http://localhost:5173/a?token=SECRET123");
    host.on_address_change(ViewId(3), "https://foreign.test/x");
    crate::nav_sync::couple(host, ViewId(3), false).expect("decoupled");
    let pages = snapshot(host, None);
    assert_eq!(
        pages.views,
        vec![
            entry("desktop", Some("/a?token=SECRET123"), None, false),
            entry("mobile", None, Some("https://foreign.test/x"), true),
        ],
        "the tablet showed no web page"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_last_pages_open_in_the_environment_in_effect_with_the_decoupled_view_decoupled() {
    let setup = started("staging");
    attach_all(&setup).await;
    let host = setup.core.host();
    let pages = LastPages {
        version: FORMAT_VERSION,
        views: vec![
            entry("desktop", Some("/one"), None, false),
            entry("tablet", None, Some("https://foreign.test/two"), false),
            entry("mobile", Some("/three"), None, true),
            entry("removed", Some("/four"), None, false),
        ],
    };
    let applied = apply(host, &pages, Target::All).expect("applied");
    assert_eq!(applied.views, [ViewId(1), ViewId(2), ViewId(3)]);
    assert_eq!(applied.decoupled, [ViewId(3)]);
    let calls = navigations(&setup);
    for expected in [
        (ViewId(1), "https://staging.example.com/one"),
        (ViewId(2), "https://foreign.test/two"),
        (ViewId(3), "https://staging.example.com/three"),
    ] {
        assert!(
            calls
                .iter()
                .any(|(view, url)| *view == expected.0 && url == expected.1),
            "{expected:?} in {calls:?}"
        );
    }
    let nav = crate::nav_sync::for_host(host).expect("navigation sync runs");
    assert!(!nav.is_coupled(ViewId(3)));
    assert!(nav.is_coupled(ViewId(1)));
}

#[tokio::test(flavor = "multi_thread")]
async fn one_view_alone_opens_its_last_page() {
    let setup = started("local");
    attach_all(&setup).await;
    let pages = LastPages {
        version: FORMAT_VERSION,
        views: vec![
            entry("desktop", Some("/one"), None, false),
            entry("tablet", Some("/two"), None, false),
        ],
    };
    // The views' first loads at attach are not the restore's.
    let before = navigations(&setup).len();
    let applied = apply(setup.core.host(), &pages, Target::View(ViewId(2))).expect("applied");
    assert_eq!(applied.views, [ViewId(2)]);
    let calls = navigations(&setup).split_off(before);
    assert!(
        calls
            .iter()
            .any(|(view, url)| *view == ViewId(2) && url == "http://localhost:5173/two"),
        "{calls:?}"
    );
    assert!(
        !calls.iter().any(|(view, _)| *view == ViewId(1)),
        "{calls:?}"
    );
}

#[tokio::test]
async fn unknown_arguments_are_refused() {
    let setup = setup();
    assert!(matches!(
        run(&setup.core, &json!({ "bogus": 1 })),
        Err(CommandError::InvalidArgs(_))
    ));
}
