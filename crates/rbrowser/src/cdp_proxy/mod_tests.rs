use std::io::{Read as _, Write as _};
use std::sync::Arc;
use std::time::Duration;

use rb_commands::{CommandRegistry, McpCoverage};
use rb_config::UserSettings;
use serde_json::Value;
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
use crate::host::state::default_config;
use crate::host::ui::recording::RecordingUi;

#[test]
fn the_commands_have_their_chords_and_their_mcp_entries() {
    let defs = definitions().expect("valid ids");
    let ids: Vec<&str> = defs.iter().map(|def| def.id.as_str()).collect();
    assert_eq!(ids, vec![COPY_ENDPOINT, ROTATE_TOKEN, COPY_SNIPPET]);
    let chords: Vec<String> = defs
        .iter()
        .flat_map(|def| def.default_bindings.iter().map(ToString::to_string))
        .collect();
    assert_eq!(
        chords,
        vec![
            "Ctrl+Shift+Space , E",
            "Ctrl+Shift+Space , O",
            "Ctrl+Shift+Space F C"
        ]
    );
    assert_eq!(defs[0].mcp, McpCoverage::Covered(MCP_TOOL.to_owned()));
    assert_eq!(defs[1].mcp, McpCoverage::Omitted(ROTATE_OMITTED.to_owned()));
    assert_eq!(defs[2].mcp, McpCoverage::Covered(MCP_TOOL.to_owned()));
    assert_eq!(defs[2].group, rb_commands::Group::Flows);
}

#[test]
fn the_commands_pass_the_registry_lint() {
    let mut registry = CommandRegistry::new();
    for def in definitions().expect("valid ids") {
        let handler: rb_commands::Handler = Arc::new(|_| Ok(Value::Null));
        registry.register(def, handler).expect("registered");
    }
}

fn get_status(port: u16, extra: &str) -> String {
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).expect("connects");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("timeout");
    let request = format!("GET /json/version HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n{extra}\r\n");
    stream.write_all(request.as_bytes()).expect("writes");
    let mut answer = String::new();
    stream.read_to_string(&mut answer).expect("reads");
    answer.lines().next().unwrap_or_default().to_owned()
}

/// A plain test on purpose: CEF calls `on_start` and the settings hooks on its UI thread,
/// outside the Tokio runtime; `#[tokio::test]` would hide a `Handle::current()` there.
#[test]
fn the_proxy_starts_rotates_and_stops_from_a_thread_outside_the_runtime() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");
    let core = runtime.block_on(async {
        let (_fake, router) = FakeBrowser::connect();
        let ui = Arc::new(RecordingUi::default());
        let host = Host::new(ui.clone(), router, Handle::current());
        host.open_config(default_config(), None).expect("opens");
        let mut user = UserSettings::default();
        user.cdp.enabled = true;
        let setup = CoreSetup {
            user,
            ..CoreSetup::default()
        };
        Core::new(host, ui, Handle::current(), setup)
    });
    on_start(&core);
    let server = current().expect("started");
    let port = server.port();
    assert!(get_status(port, "").contains(" 401 "));
    let bearer = format!("Authorization: {}\r\n", server.token().bearer());
    assert!(get_status(port, &bearer).contains(" 200 "));

    let copied = handler(COPY_ENDPOINT, Arc::downgrade(&core))(Value::Null).expect("copied");
    assert_eq!(copied["port"], serde_json::json!(port));
    assert!(copied.get("token").is_none());
    handler(ROTATE_TOKEN, Arc::downgrade(&core))(Value::Null).expect("rotated");
    assert!(get_status(port, &bearer).contains(" 401 "));

    let mut user = core.user_settings();
    user.cdp.enabled = false;
    // The other hooks `replace_user_settings` runs are not this test's; only ours runs outside.
    runtime.block_on(async { core.replace_user_settings(user) });
    on_user_settings_changed(&core);
    assert!(current().is_none());
    let mut closed = false;
    for _ in 0..200 {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_err() {
            closed = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(closed, "the port still accepts connections");
    let off = handler(COPY_ENDPOINT, Arc::downgrade(&core))(Value::Null);
    assert!(off.is_err());
    shutdown();
}

#[test]
fn a_view_is_found_by_its_target() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime");
    runtime.block_on(async {
        let (_fake, router) = FakeBrowser::connect();
        let ui = Arc::new(RecordingUi::default());
        let host = Host::new(ui, router, Handle::current());
        host.open_config(default_config(), None).expect("opens");
        assert_eq!(view_of_target(&host, "nothing"), None);
        let hooks = hooks(&host);
        assert_eq!((hooks.scale)("nothing"), None);
        (hooks.on_end)(vec!["nothing".to_owned()]);
    });
}
