use std::ffi::OsString;
use std::sync::Arc;

use rb_commands::McpCoverage;
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
use crate::host::Host;
use crate::host::state::default_config;
use crate::host::ui::recording::{RecordingUi, UiCall};

#[test]
fn the_command_is_leader_comma_s_and_no_mcp_tool() {
    let defs = definitions().expect("well formed");
    assert_eq!(defs.len(), 1);
    assert_eq!(defs[0].id.as_str(), COMMAND);
    assert!(matches!(defs[0].mcp, McpCoverage::Omitted(_)));
    assert!(refuse_mcp(COMMAND).is_err());
    assert!(refuse_mcp("app.about").is_ok());
}

/// The next start gets this start's arguments as given - the project, `--no-sandbox` - so the
/// same project opens again.
#[test]
fn the_next_start_takes_this_start_s_arguments() {
    let args = [
        OsString::from("rbrowser.exe"),
        OsString::from("--project"),
        OsString::from("C:\\site"),
    ];
    let command = relaunch_command(std::path::Path::new("C:\\Tools\\rbrowser.exe"), args);
    assert_eq!(command.get_program(), "C:\\Tools\\rbrowser.exe");
    let passed: Vec<_> = command.get_args().collect();
    assert_eq!(passed, ["--project", "C:\\site"]);
}

#[tokio::test]
async fn the_command_closes_the_window_and_the_quit_starts_again() {
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(default_config(), None).expect("opens");
    let core = Core::new(host, ui.clone(), Handle::current(), CoreSetup::default());
    assert!(!requested());
    request(&core);
    assert!(requested());
    assert!(ui.calls().iter().any(|call| matches!(call, UiCall::Close)));
}

/// The quitting instance stops listening before the next start, so the relaunched process does
/// not hand its start over to this ending one and exit without a window.
#[test]
fn the_listener_stops_before_the_next_start_and_only_on_a_restart() {
    let steps = std::cell::RefCell::new(Vec::new());
    relaunch_after(
        true,
        || steps.borrow_mut().push("stop"),
        || steps.borrow_mut().push("start"),
    );
    assert_eq!(*steps.borrow(), ["stop", "start"]);

    steps.borrow_mut().clear();
    relaunch_after(
        false,
        || steps.borrow_mut().push("stop"),
        || steps.borrow_mut().push("start"),
    );
    assert!(steps.borrow().is_empty());
}
