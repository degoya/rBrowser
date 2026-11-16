use std::sync::Arc;

use rb_commands::lint::lint_defaults;
use rb_commands::{Platform, builtin, builtin_reserved};
use rb_config::UserSettings;
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
use crate::host::Host;
use crate::host::state::default_config;
use crate::host::ui::recording::{RecordingUi, UiCall};

fn core(overflow: ToolbarOverflow) -> (Arc<RecordingUi>, Arc<Core>) {
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(default_config(), None).expect("opens");
    let mut user = UserSettings::default();
    user.ui.toolbar.overflow = overflow;
    let setup = CoreSetup {
        user,
        ..CoreSetup::default()
    };
    let core = Core::new(host, ui.clone(), Handle::current(), setup);
    (ui, core)
}

fn ask(core: &Core, rows: u8) -> Result<CoreToShell, Failure> {
    ipc(core, &ShellToCore::ToolbarRows { rows }).expect("toolbarRows is this module's")
}

#[test]
fn a_request_gets_the_rows_the_setting_allows() {
    assert_eq!(checked_rows(1, ToolbarOverflow::Menu), Ok(1));
    assert_eq!(checked_rows(1, ToolbarOverflow::Wrap), Ok(1));
    assert_eq!(checked_rows(2, ToolbarOverflow::Wrap), Ok(2));
    assert!(checked_rows(2, ToolbarOverflow::Menu).is_err());
    assert!(checked_rows(0, ToolbarOverflow::Wrap).is_err());
    assert!(checked_rows(3, ToolbarOverflow::Wrap).is_err());
    assert!(checked_rows(u8::MAX, ToolbarOverflow::Wrap).is_err());
}

// One test for the process-wide rows: the steps follow each other.
#[tokio::test]
async fn wrap_gives_the_shell_a_second_row_and_takes_it_back() {
    let (_ui, core) = core(ToolbarOverflow::Wrap);
    assert_eq!((rows(), height()), (1, SHELL_HEIGHT));
    assert_eq!(ask(&core, 2), Ok(CoreToShell::Ack));
    assert_eq!((rows(), height()), (2, 2 * SHELL_HEIGHT));
    // Refused: the rows stay.
    let refused = ask(&core, 3).expect_err("three rows");
    assert_eq!(refused.0, IpcErrorCode::Refused);
    assert_eq!(rows(), 2);
    assert_eq!(ask(&core, 1), Ok(CoreToShell::Ack));
    assert_eq!((rows(), height()), (1, SHELL_HEIGHT));

    // With `menu` the shell view never grows.
    let (_ui, core) = self::core(ToolbarOverflow::Menu);
    let refused = ask(&core, 2).expect_err("the setting is menu");
    assert_eq!(refused.0, IpcErrorCode::Refused);
    assert_eq!(rows(), 1);
    assert_eq!(ask(&core, 1), Ok(CoreToShell::Ack));
}

#[tokio::test]
async fn other_messages_are_not_this_modules() {
    let (_ui, core) = core(ToolbarOverflow::Menu);
    assert!(ipc(&core, &ShellToCore::GetState).is_none());
}

#[tokio::test]
async fn the_command_focuses_the_shell_and_opens_the_menu_there() {
    let (ui, core) = core(ToolbarOverflow::Menu);
    let before = ui.calls().len();
    core.run_command(OPEN_MENU, Value::Null).expect("runs");
    let calls = &ui.calls()[before..];
    assert!(calls.contains(&UiCall::FocusShell), "{calls:?}");
    assert!(
        calls.iter().any(
            |call| matches!(call, UiCall::Shell(script) if script.contains("uiCommand") && script.contains(OPEN_MENU))
        ),
        "{calls:?}"
    );
}

#[test]
fn the_default_passes_the_lint_and_has_no_tool() {
    let mut defs = builtin::definitions();
    defs.extend(crate::status::defs::definitions().expect("status"));
    defs.extend(crate::bookmarks::definitions().expect("bookmarks"));
    defs.extend(rb_plugin_host::core_command_defs().expect("plugins.*"));
    defs.extend(definitions().expect("definitions"));
    let issues: Vec<String> = lint_defaults(&defs, builtin_reserved(), Platform::Windows)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(issues.is_empty(), "shortcut lint:\n{}", issues.join("\n"));
    let defs = definitions().expect("definitions");
    let [def] = defs.as_slice() else {
        panic!("one command");
    };
    assert_eq!(def.id.as_str(), OPEN_MENU);
    assert_eq!(def.group, Group::Panels);
    assert_eq!(
        def.default_bindings
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["Ctrl+Shift+Space P T"]
    );
    assert_eq!(def.mcp, McpCoverage::Omitted(OPEN_OMITTED.to_owned()));
}
