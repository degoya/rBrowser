use rb_sync::FrameworkInfo;
use serde_json::Value;

use super::*;
use crate::cdp::fake::FakeBrowser;

const BUNDLES: Bundles = Bundles {
    probe: "probe();",
    react: "reactHook();",
    vue: "vueHook();",
};

fn setup() -> (Arc<FakeBrowser>, Arc<FrameworkHooks>) {
    let (fake, router) = FakeBrowser::connect();
    (fake, FrameworkHooks::with_bundles(router, BUNDLES))
}

fn modes(vue: FrameworkDevtoolsMode, react: FrameworkDevtoolsMode) -> FrameworkDevtoolsSettings {
    FrameworkDevtoolsSettings { vue, react }
}

fn report(vue: bool, react: bool) -> FrameworkMessage {
    let info = |on: bool| {
        on.then_some(FrameworkInfo {
            version: None,
            hook: false,
            devtools: true,
        })
    };
    FrameworkMessage {
        v: 1,
        top: true,
        frame: None,
        vue: info(vue),
        react: info(react),
    }
}

/// The sources of the main-world scripts registered in `session`, in order.
fn sources(fake: &FakeBrowser, session: &str) -> Vec<String> {
    fake.sent("Page.addScriptToEvaluateOnNewDocument")
        .into_iter()
        .filter(|call| call["sessionId"] == session && call["params"].get("worldName").is_none())
        .map(|call| {
            call["params"]["source"]
                .as_str()
                .unwrap_or_default()
                .to_owned()
        })
        .collect()
}

fn removed(fake: &FakeBrowser) -> Vec<Value> {
    fake.sent("Page.removeScriptToEvaluateOnNewDocument")
}

#[test]
fn wanted_follows_the_modes_and_what_was_seen() {
    use FrameworkDevtoolsMode::{Auto, Off, On};
    let none = BTreeSet::new();
    let vue: BTreeSet<Framework> = [Framework::Vue].into();
    assert_eq!(wanted(&modes(Auto, Auto), &none), [Script::Probe].into());
    assert_eq!(
        wanted(&modes(Auto, Auto), &vue),
        [Script::Probe, Script::Hook(Framework::Vue)].into()
    );
    assert_eq!(
        wanted(&modes(Off, On), &vue),
        [Script::Probe, Script::Hook(Framework::React)].into()
    );
    assert_eq!(wanted(&modes(Off, Off), &vue), BTreeSet::new());
}

#[tokio::test(flavor = "multi_thread")]
async fn without_a_framework_only_the_probe_is_injected() {
    let (fake, hooks) = setup();
    hooks.attach(ViewId(1), "S1").await;
    assert_eq!(sources(&fake, "S1"), ["probe();"]);
    let probe = &fake.sent("Page.addScriptToEvaluateOnNewDocument")[0];
    assert_eq!(probe["params"]["runImmediately"], true);
    hooks.on_report(ViewId(1), &report(false, false)).await;
    assert_eq!(sources(&fake, "S1"), ["probe();"]);
    assert!(hooks.frameworks().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_found_framework_gets_its_hook_for_the_next_document_in_auto() {
    let (fake, hooks) = setup();
    hooks.attach(ViewId(1), "S1").await;
    hooks.on_report(ViewId(1), &report(true, false)).await;
    assert_eq!(sources(&fake, "S1"), ["probe();", "vueHook();"]);
    assert!(hooks.is_registered(ViewId(1), Script::Hook(Framework::Vue)));
    assert!(!hooks.is_registered(ViewId(1), Script::Hook(Framework::React)));
    let offered = hooks.frameworks();
    assert_eq!(offered.len(), 1);
    assert_eq!(offered[0].view, ViewId(1));
    assert!(offered[0].vue.is_some());
    assert!(offered[0].react.is_none());
    // A new document: nothing is offered until the probe speaks, the hook stays.
    hooks.on_navigated(ViewId(1));
    assert!(hooks.frameworks().is_empty());
    assert!(hooks.is_registered(ViewId(1), Script::Hook(Framework::Vue)));
    assert!(removed(&fake).is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn on_registers_the_hook_at_attach_and_off_registers_nothing() {
    let (fake, hooks) = setup();
    hooks
        .set_modes(modes(FrameworkDevtoolsMode::Off, FrameworkDevtoolsMode::On))
        .await;
    hooks.attach(ViewId(1), "S1").await;
    assert_eq!(sources(&fake, "S1"), ["probe();", "reactHook();"]);
    // Vue found, but off: no hook, no offer.
    hooks.on_report(ViewId(1), &report(true, true)).await;
    assert_eq!(sources(&fake, "S1"), ["probe();", "reactHook();"]);
    let offered = hooks.frameworks();
    assert!(offered[0].vue.is_none());
    assert!(offered[0].react.is_some());
    hooks
        .set_modes(modes(
            FrameworkDevtoolsMode::Off,
            FrameworkDevtoolsMode::Off,
        ))
        .await;
    assert_eq!(removed(&fake).len(), 2);
    assert!(!hooks.is_registered(ViewId(1), Script::Probe));
    assert!(hooks.frameworks().is_empty());
    hooks.attach(ViewId(2), "S2").await;
    assert!(sources(&fake, "S2").is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_new_session_registers_again_and_a_closed_view_is_forgotten() {
    let (fake, hooks) = setup();
    hooks.attach(ViewId(1), "S1").await;
    hooks.on_report(ViewId(1), &report(false, true)).await;
    hooks.attach(ViewId(1), "S1b").await;
    assert_eq!(sources(&fake, "S1b"), ["probe();", "reactHook();"]);
    assert!(removed(&fake).is_empty());
    hooks.forget(ViewId(1));
    assert!(hooks.frameworks().is_empty());
    assert!(!hooks.is_registered(ViewId(1), Script::Probe));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unbuilt_bundle_registers_nothing() {
    let (fake, router) = FakeBrowser::connect();
    let hooks = FrameworkHooks::with_bundles(
        router,
        Bundles {
            probe: "",
            react: " \n",
            vue: "",
        },
    );
    hooks.attach(ViewId(1), "S1").await;
    hooks.on_report(ViewId(1), &report(true, true)).await;
    assert!(sources(&fake, "S1").is_empty());
}
