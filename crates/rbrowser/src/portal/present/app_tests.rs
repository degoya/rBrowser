//! The presentation in the app with a core outside the runtime (as CEF's UI thread runs the
//! commands) and, on Unix, a fake cloudflared: off by default, the consent first, MCP neither
//! starts it nor sees its link, a run that goes live, a new link, the stop that leaves no
//! process, a crash that ends it.

use std::sync::Arc;
#[cfg(unix)]
use std::time::Duration;

use rb_config::UserSettings;
use rb_ipc::PresentationPhase;
use serde_json::{Value, json};
use tokio::runtime::{Handle, Runtime};

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
use crate::host::Host;
use crate::host::state::default_config;
use crate::host::ui::recording::{RecordingUi, UiCall};
use crate::portal::present::{NEW_LINK, START, STOP, panel, run};

fn core(user: UserSettings) -> (Runtime, Arc<Core>, Arc<RecordingUi>) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");
    let (core, ui) = runtime.block_on(async {
        let (_fake, router) = FakeBrowser::connect();
        let ui = Arc::new(RecordingUi::default());
        let host = Host::new(ui.clone(), router, Handle::current());
        host.open_config(default_config(), None).expect("opens");
        let setup = CoreSetup {
            user,
            ..CoreSetup::default()
        };
        (Core::new(host, ui.clone(), Handle::current(), setup), ui)
    });
    (runtime, core, ui)
}

fn panel_opened(ui: &RecordingUi) -> bool {
    ui.calls()
        .iter()
        .any(|call| matches!(call, UiCall::SettingsWindow(url) if url.ends_with("#/portal")))
}

#[test]
fn nothing_starts_by_itself_and_mcp_cannot_start_it() {
    let (_runtime, core, _ui) = core(UserSettings::default());
    assert!(!core.user_settings().portal.presentation.enabled);
    assert!(run(&core, START, &json!({ "consent": true })).is_err());
    assert!(!is_running());
    for name in ["portal_present_start", "portal_present_link"] {
        let failure = crate::portal::mcp::host_tool(&core, name, &json!({})).expect_err("refused");
        assert_eq!(failure.code, "unknown_tool", "{name}");
    }
    let status =
        crate::portal::mcp::host_tool(&core, "portal_present_status", &json!({})).expect("answers");
    assert_eq!(status["running"], false);
    assert_eq!(
        panel::panel(&core, true).phase,
        PresentationPhase::Off,
        "{status}"
    );
}

#[test]
fn the_first_start_waits_for_the_consent() {
    let mut user = UserSettings::default();
    user.portal.presentation.enabled = true;
    let (_runtime, core, ui) = core(user);
    let asked = run(&core, START, &Value::Null).expect("asks");
    assert_eq!(asked["needs"], "consent");
    assert!(!is_running());
    assert!(panel_opened(&ui), "the panel asks");
    assert!(!core.user_settings().portal.presentation.consented);
    assert!(matches!(
        run(&core, START, &json!({ "consent": "yes" })),
        Err(CommandError::InvalidArgs(_))
    ));
}

#[test]
fn new_link_and_stop_without_a_presentation_do_nothing() {
    let (_runtime, core, _ui) = core(UserSettings::default());
    assert!(run(&core, NEW_LINK, &Value::Null).is_err());
    assert_eq!(
        run(&core, STOP, &Value::Null).expect("answers")["stopped"],
        false
    );
}

#[cfg(unix)]
mod fake_cloudflared {
    use std::io::{Read as _, Write as _};
    use std::os::unix::fs::PermissionsExt as _;
    use std::path::{Path, PathBuf};

    use super::*;

    const ADDRESS: &str = "https://quick-brown-fox.trycloudflare.com";

    fn script(dir: &Path, body: &str) -> PathBuf {
        let path = dir.join("cloudflared");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("script");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).expect("mode");
        path
    }

    fn live_user(program: &Path) -> UserSettings {
        let mut user = UserSettings::default();
        user.portal.presentation.enabled = true;
        user.portal.presentation.consented = true;
        user.tools.cloudflared_path = Some(program.display().to_string());
        user
    }

    /// Waits up to 10 s (a give-up deadline, no upper bound) for `done`.
    fn wait_for(mut done: impl FnMut() -> bool) -> bool {
        for _ in 0..2000 {
            if done() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        false
    }

    fn post(address: std::net::SocketAddr, token: &str) -> String {
        let mut stream = std::net::TcpStream::connect(address).expect("connects");
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("timeout");
        let request = format!(
            "POST /__rbrowser/pair HTTP/1.1\r\nHost: quick-brown-fox.trycloudflare.com\r\n\
             Content-Length: {}\r\n\r\n{token}",
            token.len()
        );
        stream.write_all(request.as_bytes()).expect("writes");
        let mut answer = String::new();
        let _ = stream.read_to_string(&mut answer);
        answer
    }

    #[test]
    fn a_presentation_goes_live_hands_out_links_once_and_stops_without_a_process_left() {
        let dir = tempfile::tempdir().expect("tempdir");
        let program = script(
            dir.path(),
            &format!(
                "echo 'INF |  {ADDRESS}  |' >&2\necho 'INF Registered tunnel connection' >&2\n\
                 exec sleep 600"
            ),
        );
        let (runtime, core, ui) = core(live_user(&program));
        let started = run(&core, START, &Value::Null).expect("started");
        assert_eq!(started["running"], true);
        assert!(panel_opened(&ui));
        assert!(
            wait_for(|| panel::panel(&core, true).phase == PresentationPhase::Live),
            "the fake tunnel connected"
        );
        let shown = panel::panel(&core, true);
        assert_eq!(shown.address.as_deref(), Some(ADDRESS));
        let link = shown.link.expect("a first link").url;
        let token = link.split_once("#t=").expect("a fragment").1.to_owned();

        // MCP sees the state, never the link, its token or the address.
        for name in ["portal_present_status", "portal_devices"] {
            let answer = crate::portal::mcp::host_tool(&core, name, &json!({})).expect("answers");
            let text = answer.to_string();
            assert!(!text.contains(&token), "{name}: {text}");
            assert!(!text.contains("trycloudflare"), "{name}: {text}");
        }
        let status = panel::status(&core);
        assert_eq!(status["phase"], "live");
        assert_eq!(status["linkOpen"], true);

        let (server, pid) = {
            let state = STATE.lock().expect("state");
            let running = state.as_ref().expect("running");
            (
                running.server.clone(),
                running
                    .tunnel
                    .as_ref()
                    .and_then(Tunnel::pid)
                    .expect("a pid"),
            )
        };
        let joined = post(server.address(), &token);
        assert!(joined.starts_with("HTTP/1.1 200 "), "{joined}");
        assert!(
            post(server.address(), &token).starts_with("HTTP/1.1 403 "),
            "once"
        );
        assert_eq!(panel::status(&core)["viewers"], 1);

        run(&core, NEW_LINK, &Value::Null).expect("a new link");
        let fresh = panel::panel(&core, true).link.expect("a link").url;
        assert_ne!(fresh, link);

        let stopped = crate::portal::mcp::host_tool(&core, "portal_present_stop", &json!({}))
            .expect("answers");
        assert_eq!(stopped["running"], false);
        assert!(!is_running());
        assert!(
            wait_for(|| !Path::new(&format!("/proc/{pid}")).exists()),
            "cloudflared still runs"
        );
        assert!(server.is_stopped(), "every viewer was disconnected");
        drop(runtime);
    }

    #[test]
    fn a_crash_of_cloudflared_ends_the_presentation() {
        let dir = tempfile::tempdir().expect("tempdir");
        let program = script(
            dir.path(),
            &format!("echo 'INF |  {ADDRESS}  |' >&2\nexit 1"),
        );
        let (_runtime, core, _ui) = core(live_user(&program));
        run(&core, START, &Value::Null).expect("started");
        assert!(wait_for(|| !is_running()), "the crash ended it");
    }

    #[test]
    fn a_change_of_its_settings_ends_it() {
        let dir = tempfile::tempdir().expect("tempdir");
        let program = script(dir.path(), "exec sleep 600");
        let (runtime, core, _ui) = core(live_user(&program));
        run(&core, START, &Value::Null).expect("started");
        assert!(is_running());
        let mut user = core.user_settings();
        user.portal.presentation.max_viewers = 1;
        runtime.block_on(async { core.replace_user_settings(user) });
        assert!(
            !is_running(),
            "a lower limit must not wait for the next start"
        );
        shutdown();
    }

    #[test]
    fn a_stop_during_the_download_keeps_the_presentation_private() {
        let dir = tempfile::tempdir().expect("tempdir");
        let program = script(dir.path(), "exec sleep 600");
        let (runtime, core, _ui) = core(live_user(&program));
        let downloading = || {
            // As `fetch_then_launch` leaves it while the download runs.
            ABORT.store(false, Ordering::Release);
            DOWNLOADING.store(true, Ordering::Release);
        };
        let set = |change: &dyn Fn(&mut UserSettings)| {
            let mut user = core.user_settings();
            change(&mut user);
            runtime.block_on(async { core.replace_user_settings(user) });
        };

        // The person's stop.
        downloading();
        assert!(stop(&core), "the stop is taken");
        after_download(&core, Ok(program.clone()));
        assert!(!is_running());
        assert!(!DOWNLOADING.load(Ordering::Acquire));

        // MCP's stop.
        downloading();
        crate::portal::mcp::host_tool(&core, "portal_present_stop", &json!({})).expect("answers");
        after_download(&core, Ok(program.clone()));
        assert!(!is_running());

        // The setting switched off.
        downloading();
        set(&|user: &mut UserSettings| user.portal.presentation.enabled = false);
        after_download(&core, Ok(program.clone()));
        assert!(!is_running());

        // The consent withdrawn: read again before the start.
        set(&|user: &mut UserSettings| {
            user.portal.presentation.enabled = true;
            user.portal.presentation.consented = false;
        });
        downloading();
        after_download(&core, Ok(program.clone()));
        assert!(!is_running());

        // A stop after the check in `after_download`, before the start becomes a presentation.
        set(&|user: &mut UserSettings| user.portal.presentation.consented = true);
        ABORT.store(true, Ordering::Release);
        let answer = launch(
            &core,
            &core.user_settings().portal.presentation,
            program.clone(),
        )
        .expect("answers");
        assert_eq!(answer["running"], false);
        assert!(!is_running());

        // Without a stop the download goes on to the start.
        downloading();
        after_download(&core, Ok(program.clone()));
        assert!(is_running());
        assert!(!DOWNLOADING.load(Ordering::Acquire));
        shutdown();
    }
}
