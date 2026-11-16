use std::io::{Read as _, Write as _};
use std::sync::Arc;
use std::time::Duration;

use rb_commands::{CommandRegistry, McpCoverage};
use rb_config::UserSettings;
use serde_json::{Value, json};
use tokio::runtime::{Handle, Runtime};

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
use crate::host::Host;
use crate::host::state::default_config;
use crate::host::ui::recording::{RecordingUi, UiCall};

#[test]
fn the_commands_have_their_chords_and_their_mcp_entries() {
    let defs = definitions().expect("valid ids");
    let ids: Vec<&str> = defs.iter().map(|def| def.id.as_str()).collect();
    assert_eq!(ids, vec![TOGGLE, SHOW_QR, DEVICES, DISCONNECT]);
    let chords: Vec<String> = defs
        .iter()
        .flat_map(|def| def.default_bindings.iter().map(ToString::to_string))
        .collect();
    assert_eq!(
        chords,
        vec![
            "Ctrl+Shift+Space S P",
            "Ctrl+Shift+Space S Q",
            "Ctrl+Shift+Space P D",
            "Ctrl+Shift+Space S D",
        ]
    );
    assert_eq!(defs[0].mcp, McpCoverage::Omitted(TOGGLE_OMITTED.to_owned()));
    assert_eq!(
        defs[1].mcp,
        McpCoverage::Omitted(SHOW_QR_OMITTED.to_owned())
    );
    assert_eq!(
        defs[2].mcp,
        McpCoverage::Covered("portal_devices".to_owned())
    );
    assert_eq!(
        defs[3].mcp,
        McpCoverage::Covered("portal_disconnect".to_owned())
    );
    let mut registry = CommandRegistry::new();
    for def in defs {
        let handler: rb_commands::Handler = Arc::new(|_| Ok(Value::Null));
        registry.register(def, handler).expect("registered");
    }
}

/// A core outside the runtime, as CEF's UI thread runs the commands.
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

fn get(address: std::net::SocketAddr, path: &str, extra: &str) -> String {
    let mut stream = std::net::TcpStream::connect(address).expect("connects");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("timeout");
    let request = format!("GET {path} HTTP/1.1\r\nHost: {address}\r\n{extra}\r\n");
    stream.write_all(request.as_bytes()).expect("writes");
    let mut answer = String::new();
    let _ = stream.read_to_string(&mut answer);
    answer
}

#[test]
fn fresh_settings_never_start_the_portal_and_mcp_cannot_either() {
    let (_runtime, core, _ui) = core(UserSettings::default());
    assert!(!core.user_settings().portal.enabled);
    app::on_user_settings_changed(&core);
    assert!(
        app::current().is_none(),
        "nothing starts the portal by itself"
    );
    let toggled = run(&core, TOGGLE, &Value::Null);
    assert!(toggled.is_err());
    let failure = mcp::host_tool(&core, "portal_start", &json!({})).expect_err("refused");
    assert_eq!(failure.code, "unknown_tool");
    assert!(app::current().is_none(), "no port was opened");
    let state = app::panel_state(&core, true);
    assert!(!state.enabled && !state.running && state.address.is_none());
}

#[test]
fn the_portal_pairs_once_lists_disconnects_and_stops() {
    let mut user = UserSettings::default();
    user.portal.enabled = true;
    user.portal.interface = "127.0.0.1".to_owned();
    let (runtime, core, ui) = core(user);
    let started = run(&core, TOGGLE, &Value::Null).expect("started");
    assert_eq!(started["running"], true);
    let server = app::current().expect("running");
    let address = server.address();
    assert!(
        ui.calls()
            .iter()
            .any(|call| matches!(call, UiCall::SettingsWindow(url) if url.ends_with("#/portal"))),
        "the panel opens with the QR code"
    );
    let state = app::panel_state(&core, true);
    let pairing = state.pairing.expect("an open pairing");
    assert_eq!(pairing.rows.len(), pairing.size as usize);
    let path = pairing.url.trim_start_matches(server.origin()).to_owned();

    // MCP's answers carry no token, now or later.
    let answer = mcp::host_tool(&core, mcp::DEVICES_TOOL, &json!({})).expect("answers");
    let token = path.rsplit('=').next().expect("a token");
    assert!(!answer.to_string().contains(token));
    assert!(answer.get("pairing").is_none());

    let first = get(address, &path, "User-Agent: Phone\r\n");
    assert!(first.starts_with("HTTP/1.1 200 "), "{first}");
    let again = get(address, &path, "");
    assert!(again.starts_with("HTTP/1.1 403 "), "{again}");
    assert!(
        app::panel_state(&core, true).pairing.is_none(),
        "the code is spent"
    );
    let cookie = first
        .lines()
        .find_map(|line| line.strip_prefix("Set-Cookie: "))
        .and_then(|value| value.split(';').next())
        .expect("a cookie")
        .to_owned();
    let devices = mcp::host_tool(&core, mcp::DEVICES_TOOL, &json!({})).expect("answers");
    assert_eq!(devices["devices"][0]["name"], "Phone");
    assert!(
        !devices
            .to_string()
            .contains(cookie.split('=').nth(1).expect("value"))
    );

    let gone = run(
        &core,
        DISCONNECT,
        &json!({ "device": devices["devices"][0]["id"] }),
    )
    .expect("disconnected");
    assert_eq!(gone["disconnected"], 1);
    let after = get(address, "/", &format!("Cookie: {cookie}\r\n"));
    assert!(after.starts_with("HTTP/1.1 403 "), "{after}");

    let stopped = run(&core, TOGGLE, &json!({ "enabled": false })).expect("stopped");
    assert_eq!(stopped["running"], false);
    assert!(app::current().is_none());
    let mut closed = false;
    // A give-up deadline of 10 s, no upper bound: the full check runs eight tests at once.
    for _ in 0..2000 {
        if std::net::TcpStream::connect(address).is_err() {
            closed = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(closed, "the port still accepts connections");
    drop(runtime);
}

#[test]
fn switching_the_setting_off_stops_a_running_portal() {
    let mut user = UserSettings::default();
    user.portal.enabled = true;
    user.portal.interface = "127.0.0.1".to_owned();
    let (runtime, core, _ui) = core(user);
    app::start(&core).expect("started");
    assert!(app::current().is_some());
    let mut user = core.user_settings();
    user.portal.enabled = false;
    runtime.block_on(async { core.replace_user_settings(user) });
    app::on_user_settings_changed(&core);
    assert!(app::current().is_none());
    app::shutdown();
}

#[test]
fn every_interface_is_refused() {
    let mut user = UserSettings::default();
    user.portal.enabled = true;
    user.portal.interface = "0.0.0.0".to_owned();
    let (_runtime, core, _ui) = core(user);
    assert!(matches!(
        app::start(&core),
        Err(app::StartError::Interface(
            interface::InterfaceError::Unspecified
        ))
    ));
    assert!(app::current().is_none());
}

#[test]
fn mcp_cannot_start_an_allowed_portal() {
    let mut user = UserSettings::default();
    user.portal.enabled = true;
    user.portal.interface = "127.0.0.1".to_owned();
    let (_runtime, core, _ui) = core(user);
    assert!(!mcp::TOOLS.contains(&"portal_start"));
    let failure = mcp::host_tool(&core, "portal_start", &json!({})).expect_err("refused");
    assert_eq!(failure.code, "unknown_tool");
    assert!(
        app::current().is_none(),
        "a LAN listener is a person's decision"
    );
    // What MCP still may: read the state and stop.
    let state = mcp::host_tool(&core, mcp::DEVICES_TOOL, &json!({})).expect("answers");
    assert_eq!(state["running"], false);
    mcp::host_tool(&core, mcp::STOP_TOOL, &json!({})).expect("answers");
}

#[test]
fn a_bad_device_argument_is_refused() {
    let (_runtime, core, _ui) = core(UserSettings::default());
    assert!(matches!(
        run(&core, DISCONNECT, &json!({ "device": "phone" })),
        Err(CommandError::InvalidArgs(_))
    ));
    assert!(matches!(
        run(&core, TOGGLE, &json!({ "enabled": "yes" })),
        Err(CommandError::InvalidArgs(_))
    ));
    assert_eq!(
        run(&core, DISCONNECT, &Value::Null).expect("nothing to do")["disconnected"],
        0
    );
}

#[test]
fn any_portal_setting_change_stops_a_running_portal() {
    let mut user = UserSettings::default();
    user.portal.enabled = true;
    user.portal.interface = "127.0.0.1".to_owned();
    let (runtime, core, _ui) = core(user);
    let changes: [fn(&mut UserSettings); 2] = [
        |user| user.portal.max_devices = 1,
        |user| user.portal.pairing_ttl_sec = 60,
    ];
    for change in changes {
        app::start(&core).expect("started");
        let mut user = core.user_settings();
        change(&mut user);
        runtime.block_on(async { core.replace_user_settings(user) });
        assert!(
            app::current().is_none(),
            "a lower limit must not wait for the next start"
        );
    }
    // Any other setting leaves it running.
    app::start(&core).expect("started");
    let mut user = core.user_settings();
    user.pop_out.fit_to_screen = !user.pop_out.fit_to_screen;
    runtime.block_on(async { core.replace_user_settings(user) });
    assert!(app::current().is_some());
    app::shutdown();
}

#[test]
fn a_new_project_config_reaches_the_allowlist() {
    let mut user = UserSettings::default();
    user.portal.enabled = true;
    user.portal.interface = "127.0.0.1".to_owned();
    let (_runtime, core, _ui) = core(user);
    let server = app::start(&core).expect("started").server;
    let mut config = default_config();
    config.start_url = Some("http://localhost:4321/".to_owned());
    core.host().open_config(config, None).expect("opens");
    let mut allowed = false;
    // A give-up deadline of 10 s, no upper bound: the full check runs eight tests at once.
    for _ in 0..2000 {
        if crate::cdp::lock(&server.shared)
            .allowlist
            .contains(&"http://localhost:4321".to_owned())
        {
            allowed = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(allowed, "the allowlist kept the old project's origins");
    app::shutdown();
}
