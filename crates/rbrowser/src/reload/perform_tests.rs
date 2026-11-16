use rb_config::EngineKind;
use rb_engine::{Target, ViewId};

use super::*;
use crate::reload::fake_host::{FakeHost, Page};

fn chromium(swaps: u64) -> Page {
    Page {
        swaps,
        ..Page::default()
    }
}

fn css(names: &[&str]) -> Work {
    Work::Swap {
        css: names.iter().map(|n| (*n).to_owned()).collect(),
        images: Vec::new(),
        all_css: false,
    }
}

const WATCH: HowTo = HowTo {
    respect_hmr: true,
    fallback_full: true,
    ignore_cache: false,
};

#[tokio::test]
async fn a_css_change_swaps_in_every_view_without_a_reload() {
    let host = FakeHost::new(&[chromium(1), chromium(2)]);
    let done = perform(&host, Target::All, &css(&["app.css"]), WATCH).await;
    assert_eq!(
        done.iter().map(|d| d.action.clone()).collect::<Vec<_>>(),
        vec![
            ViewAction::Swapped { count: 1 },
            ViewAction::Swapped { count: 2 }
        ]
    );
    assert!(host.reloaded().is_empty());
    assert_eq!(host.swaps_sent(), 2);
}

#[tokio::test]
async fn a_full_reload_reloads_each_view() {
    let host = FakeHost::new(&[chromium(0), chromium(0)]);
    let how = HowTo {
        ignore_cache: true,
        ..HowTo::default()
    };
    let done = perform(&host, Target::All, &Work::Full, how).await;
    assert!(done.iter().all(|d| d.action == ViewAction::Reloaded));
    assert_eq!(
        *host.reloads.lock().expect("unpoisoned"),
        vec![(ViewId(1), true), (ViewId(2), true)]
    );
}

#[tokio::test]
async fn a_page_with_its_own_hmr_is_left_alone_by_the_watch() {
    let host = FakeHost::new(&[
        Page {
            hmr: true,
            ..chromium(1)
        },
        chromium(1),
    ]);
    let done = perform(&host, Target::All, &Work::Full, WATCH).await;
    assert_eq!(done[0].action, ViewAction::LeftToHmr);
    assert_eq!(done[1].action, ViewAction::Reloaded);
    assert_eq!(host.reloaded(), vec![ViewId(2)]);
}

#[tokio::test]
async fn by_hand_a_page_with_hmr_is_reloaded_too() {
    let host = FakeHost::new(&[Page {
        hmr: true,
        ..chromium(1)
    }]);
    let done = perform(&host, Target::All, &Work::Full, HowTo::default()).await;
    assert_eq!(done[0].action, ViewAction::Reloaded);
}

#[tokio::test]
async fn nothing_to_swap_reloads_for_the_watch_but_not_by_hand() {
    let host = FakeHost::new(&[chromium(0)]);
    let done = perform(&host, Target::All, &css(&["app.css"]), WATCH).await;
    assert_eq!(done[0].action, ViewAction::Reloaded);

    let host = FakeHost::new(&[chromium(0)]);
    let done = perform(&host, Target::All, &Work::all_css(), HowTo::default()).await;
    assert_eq!(done[0].action, ViewAction::Swapped { count: 0 });
    assert!(host.reloaded().is_empty(), "CSS only keeps the page");
}

#[tokio::test]
async fn views_without_cdp_reload_instead_of_swapping() {
    let host = FakeHost::new(&[Page {
        engine: EngineKind::Firefox,
        ..chromium(1)
    }]);
    let done = perform(&host, Target::All, &css(&["app.css"]), WATCH).await;
    assert_eq!(done[0].action, ViewAction::Reloaded);
    assert!(host.scripts.lock().expect("unpoisoned").is_empty());
}

#[tokio::test]
async fn one_view_or_an_unknown_one() {
    let host = FakeHost::new(&[chromium(1), chromium(1)]);
    let done = perform(
        &host,
        Target::View(ViewId(2)),
        &Work::Full,
        HowTo::default(),
    )
    .await;
    assert_eq!(done.len(), 1);
    assert_eq!(host.reloaded(), vec![ViewId(2)]);
    let done = perform(
        &host,
        Target::View(ViewId(9)),
        &Work::Full,
        HowTo::default(),
    )
    .await;
    assert!(matches!(done[0].action, ViewAction::Failed { .. }));
}

#[tokio::test]
async fn a_script_that_throws_is_a_failure_of_that_view() {
    let host = FakeHost::new(&[Page {
        broken: true,
        ..chromium(1)
    }]);
    let done = perform(&host, Target::All, &Work::all_css(), HowTo::default()).await;
    assert!(matches!(done[0].action, ViewAction::Failed { .. }));
}

#[test]
fn a_plan_becomes_work() {
    assert_eq!(Work::from_plan(Plan::Nothing), None);
    assert_eq!(Work::from_plan(Plan::Full), Some(Work::Full));
    assert_eq!(
        Work::from_plan(Plan::Swap {
            css: vec!["a.css".into()],
            images: vec![],
        }),
        Some(css(&["a.css"]))
    );
}

/// Page code can neither patch nor read the swap: it runs in an isolated world of the page's
/// main frame. The probes only read the page and stay in it.
#[tokio::test]
async fn the_swap_runs_in_an_isolated_world() {
    let host = FakeHost::new(&[chromium(1)]);
    let done = perform(&host, Target::All, &css(&["app.css"]), WATCH).await;
    assert_eq!(done[0].action, ViewAction::Swapped { count: 1 });
    let calls = host.calls();
    let world = calls
        .iter()
        .find(|(_, method, _)| method == "Page.createIsolatedWorld")
        .expect("an isolated world");
    assert_eq!(world.2["frameId"], "F1");
    assert_eq!(world.2["worldName"], super::super::script::RELOAD_WORLD);
    assert!(world.2.get("grantUniveralAccess").is_none());
    let evaluated = |marker: &str| {
        calls
            .iter()
            .find(|(_, method, params)| {
                method == "Runtime.evaluate"
                    && params["expression"]
                        .as_str()
                        .is_some_and(|text| text.contains(marker))
            })
            .map(|(_, _, params)| params.clone())
            .expect("evaluated")
    };
    assert_eq!(evaluated("Swaps changed stylesheets")["contextId"], 101);
    assert!(
        evaluated("vite").get("contextId").is_none(),
        "the probe reads the page"
    );
}

#[tokio::test]
async fn without_an_isolated_world_nothing_runs_in_the_page() {
    let host = FakeHost::new(&[Page {
        no_world: true,
        ..chromium(1)
    }]);
    let done = perform(&host, Target::All, &css(&["app.css"]), WATCH).await;
    assert!(matches!(done[0].action, ViewAction::Failed { .. }));
    assert_eq!(host.swaps_sent(), 0);
}
