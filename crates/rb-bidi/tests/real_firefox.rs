//! The engine against the installed Firefox (RB-040-02 acceptance, Windows): a 393 x 852 view at
//! DPR 3 reports `innerWidth == 393` and `devicePixelRatio == 3`, and a replayed click reaches
//! the page as a trusted `click`. Ignored by default - it starts the real Firefox.
//!
//! Run it on Windows (from WSL, with Firefox installed there):
//!
//! ```text
//! cargo xwin test --no-run --target x86_64-pc-windows-msvc -p rb-bidi --test real_firefox
//! # copy target/x86_64-pc-windows-msvc/debug/deps/real_firefox-<hash>.exe to a local folder, then
//! real_firefox-<hash>.exe --ignored --nocapture
//! ```
//!
//! `RB_FIREFOX` names another Firefox executable.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use rb_bidi::firefox::InstalledFirefox;
use rb_bidi::launcher::find_firefox;
use rb_bidi::script::{evaluate, string_result};
use rb_bidi::{FirefoxConfig, FirefoxEngine};
use rb_engine::test_support::conformance::sample_spec;
use rb_engine::{Button, Engine, EngineKind, InputEvent, Phase, ViewId};
use rb_sync::{AgentConfig, CaptureSwitches};
use tokio::runtime::Handle;

const PAGE: &str = "data:text/html,<title>rb</title><button id=b style='position:absolute;left:40px;top:60px;width:120px;height:40px' onclick=\"document.title='click:'+event.isTrusted\">Go</button>";

async fn evaluate_text(engine: &FirefoxEngine, view: ViewId, expression: &str) -> String {
    let state = engine.view_state(view).expect("the view runs");
    let answer = state
        .client
        .send(evaluate(&state.context, false, expression))
        .await
        .expect("evaluated");
    string_result(&answer).expect("a string")
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "starts the installed Firefox; run on Windows (see the file's header)"]
async fn the_installed_firefox_emulates_and_takes_trusted_clicks() {
    let executable = std::env::var_os("RB_FIREFOX")
        .map(PathBuf::from)
        .map_or_else(|| find_firefox(None), Ok)
        .expect("an installed Firefox");
    let profile = tempfile::tempdir().expect("a profile folder");
    let engine = FirefoxEngine::new(
        FirefoxConfig {
            launch: Arc::new(InstalledFirefox {
                executable,
                profile: profile.path().join("ff-profile"),
                extra_prefs: BTreeMap::new(),
                hide_chrome: false,
                render_scale: None,
            }),
            agent_source: Arc::from(""),
            agent_config: AgentConfig {
                active: false,
                capture: CaptureSwitches {
                    click: true,
                    hover: true,
                    scroll: true,
                    keyboard: true,
                    input: true,
                    forms: true,
                    focus: false,
                },
                hover_throttle_ms: 100,
                scroll_debounce_ms: rb_sync::message::DEFAULT_SCROLL_DEBOUNCE_MS,
                scroll_mode: rb_sync::AgentScrollMode::Anchor,
                record: None,
            },
        },
        Handle::current(),
    );
    let mut spec = sample_spec(EngineKind::Firefox, 1);
    spec.emulation.width = 393;
    spec.emulation.height = 852;
    spec.emulation.dpr = 3.0;
    spec.url = PAGE.to_owned();
    spec.window = None;
    let view = engine.create_view(&spec).await.expect("a Firefox window");
    let size = evaluate_text(
        &engine,
        ViewId(1),
        "JSON.stringify([innerWidth, devicePixelRatio])",
    )
    .await;
    assert_eq!(size, "[393,3]");
    for phase in [Phase::Down, Phase::Up] {
        let click = InputEvent::Mouse {
            phase,
            x: 100.0,
            y: 80.0,
            button: Button::Left,
            clicks: 1,
            modifiers: 0,
        };
        view.dispatch_input(&click).await.expect("replayed");
    }
    let title = evaluate_text(&engine, ViewId(1), "document.title").await;
    assert_eq!(title, "click:true");
    engine.shutdown().await;
}
