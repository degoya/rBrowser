use std::collections::BTreeMap;
use std::sync::Arc;

use rb_commands::{Binding, Group, McpCoverage};
use rb_plugin_host::Cancel;
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::checks::progress::PanelSink;
use crate::control::CoreSetup;
use crate::host::Host;
use crate::host::state::default_config;
use crate::host::ui::recording::RecordingUi;

#[test]
fn the_cancel_is_a_checks_command_with_its_tool() {
    let defs = definitions().expect("valid");
    assert_eq!(defs.len(), 1);
    assert_eq!(defs[0].id.as_str(), "checks.cancel");
    assert_eq!(defs[0].group, Group::Checks);
    assert_eq!(
        defs[0].default_bindings,
        vec!["Ctrl+Shift+Space K Q".parse::<Binding>().expect("binding")]
    );
    assert_eq!(
        defs[0].mcp,
        McpCoverage::Covered("checks_cancel".to_owned())
    );
}

#[tokio::test]
async fn the_cancel_reaches_the_run_in_progress_once_and_nothing_after_it() {
    assert_eq!(
        run(),
        json!({ "cancelled": false }),
        "no run, nothing to cancel"
    );
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(default_config(), None).expect("opens");
    let core = Core::new(host, ui, Handle::current(), CoreSetup::default());
    let cancel = Arc::new(Cancel::default());
    let sink = PanelSink::start(
        core,
        "run-0000000000001".to_owned(),
        BTreeMap::new(),
        None,
        Default::default(),
        Arc::clone(&cancel),
    );
    assert_eq!(run(), json!({ "cancelled": true }));
    assert!(cancel.is_cancelled());
    assert_eq!(run(), json!({ "cancelled": false }), "cancelled already");
    sink.close();
    assert_eq!(run(), json!({ "cancelled": false }), "the run is over");
}
