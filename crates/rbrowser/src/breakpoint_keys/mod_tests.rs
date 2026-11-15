use std::sync::Arc;
use std::time::{Duration, Instant};

use rb_commands::Focus;
use rb_commands::keymap::{EVENTFLAG_CONTROL_DOWN, EVENTFLAG_SHIFT_DOWN};
use rb_config::{BreakpointShortcutSettings, NewViewModifier, ProjectConfig, UserSettings};
use rb_engine::CellRect;
use rb_ipc::{BoundaryKind, DetectedBreakpoint};
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::{CoreSetup, KeyDown};
use crate::host::Host;
use crate::host::nav::marker_url;
use crate::host::ui::recording::{RecordingUi, UiCall};

const PAGE: &str = "http://localhost:3000/products/42";

/// Three breakpoints in config order desktop, tablet, mobile; mobile has a key of its own.
fn config() -> ProjectConfig {
    serde_json::from_value(json!({
        "version": 1,
        "startUrl": "http://localhost:3000/",
        "breakpoints": [
            { "id": "desktop", "width": 1440, "height": 900 },
            { "id": "tablet", "width": 768, "height": 1024, "dpr": 2 },
            { "id": "mobile", "width": 393, "height": 852, "shortcut": "Ctrl+Shift+F9" }
        ]
    }))
    .expect("the test config parses")
}

struct Setup {
    fake: Arc<FakeBrowser>,
    ui: Arc<RecordingUi>,
    core: Arc<Core>,
}

fn setup_with(config: ProjectConfig, user: UserSettings) -> Setup {
    let (fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(config, None).expect("opens");
    for id in 1..=20 {
        fake.add_target(&format!("T{id}"), &marker_url(ViewId(id)));
    }
    let setup = CoreSetup {
        user,
        ..CoreSetup::default()
    };
    let core = Core::new(host.clone(), ui.clone(), Handle::current(), setup);
    for view in host.views() {
        let rect = CellRect {
            x: 0,
            y: 0,
            width: 400,
            height: 300,
        };
        host.on_view_bounds(view.id, rect);
        host.on_load_end(view.id, &marker_url(view.id));
        host.on_address_change(view.id, PAGE);
    }
    core.shell_ready();
    on_start(&core);
    Setup { fake, ui, core }
}

fn setup() -> Setup {
    setup_with(config(), UserSettings::default())
}

async fn until(mut done: impl FnMut() -> bool) {
    for _ in 0..200 {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("condition not reached");
}

/// The first view (desktop) made active.
fn active(setup: &Setup) -> ViewId {
    let host = setup.core.host();
    let view = host.views()[0].id;
    host.set_active_view(view).expect("activates");
    view
}

fn size(setup: &Setup, view: ViewId) -> (u32, u32) {
    let emulation = setup.core.host().emulation(view).expect("a view");
    (emulation.width, emulation.height)
}

fn breakpoint_of(setup: &Setup, view: ViewId) -> Option<String> {
    setup
        .core
        .host()
        .view(view)
        .and_then(|info| info.breakpoint)
}

/// The reloads so far, by the window or over CDP.
fn reloads(setup: &Setup) -> usize {
    let by_window = setup
        .ui
        .calls()
        .iter()
        .filter(|call| matches!(call, UiCall::Reload(..)))
        .count();
    by_window + setup.fake.sent("Page.reload").len()
}

fn shell_scripts(setup: &Setup) -> String {
    setup
        .ui
        .calls()
        .iter()
        .filter_map(|call| match call {
            UiCall::Shell(script) => Some(script.clone()),
            _ => None,
        })
        .collect()
}

fn key(flags: u32, code: u32, character: char) -> KeyDown {
    KeyDown {
        flags,
        windows_key_code: code,
        character: Some(character),
        unmodified_character: Some(character),
    }
}

/// The 200 ms of the acceptance are measured live on Windows; here the breakpoint is applied
/// without a reload, the wait bounded by `until`.
#[tokio::test]
async fn breakpoint_shortcuts_ctrl_shift_2_sets_breakpoint_2_live() {
    let setup = setup();
    let view = active(&setup);
    let before = reloads(&setup);
    let taken = setup.core.on_key_down(
        Focus::View,
        key(EVENTFLAG_CONTROL_DOWN | EVENTFLAG_SHIFT_DOWN, 0x32, '2'),
        Instant::now(),
    );
    assert!(taken);
    until(|| breakpoint_of(&setup, view).as_deref() == Some("tablet")).await;
    assert_eq!(size(&setup, view), (768, 1024));
    assert_eq!(setup.core.host().emulation(view).expect("view").dpr, 2.0);
    // Live: no reload.
    assert_eq!(reloads(&setup), before);
    // The overlay in the label strip, then the label; it is written after the breakpoint.
    let written = || {
        setup.ui.calls().into_iter().find_map(|call| match call {
            UiCall::CellLabel(at, label, overlay) if at == view => Some((label, overlay)),
            _ => None,
        })
    };
    until(|| written().is_some()).await;
    let (label, overlay) = written().expect("the label strip was written");
    assert_eq!(label, "tablet");
    let overlay = overlay.expect("an overlay");
    assert_eq!(overlay.ms, 1200);
    assert!(!overlay.text.is_empty());
}

#[tokio::test]
async fn breakpoint_shortcuts_answer_with_the_core_time() {
    let setup = setup();
    active(&setup);
    let answer = setup
        .core
        .run_command("view.set_breakpoint.3", Value::Null)
        .expect("sets");
    assert_eq!(answer["width"], 393);
    assert!(answer["elapsedMs"].as_f64().expect("a time") < 200.0);
}

#[tokio::test]
async fn breakpoint_shortcuts_next_and_prev_go_by_width() {
    let setup = setup();
    let view = active(&setup);
    let mut seen = Vec::new();
    for _ in 0..3 {
        setup.core.run_command(NEXT, Value::Null).expect("next");
        seen.push(breakpoint_of(&setup, view).expect("a breakpoint"));
    }
    // From desktop (the widest) around to mobile, tablet, desktop.
    assert_eq!(seen, ["mobile", "tablet", "desktop"]);
    setup.core.run_command(PREV, Value::Null).expect("prev");
    assert_eq!(breakpoint_of(&setup, view).as_deref(), Some("tablet"));
}

#[tokio::test]
async fn breakpoint_shortcuts_nudge_one_px_and_reset_to_the_cell() {
    let setup = setup();
    let view = active(&setup);
    setup
        .core
        .run_command("view.set_breakpoint.2", Value::Null)
        .expect("tablet");
    setup.core.run_command(NARROWER, Value::Null).expect("-1");
    assert_eq!(size(&setup, view), (767, 1024));
    assert_eq!(breakpoint_of(&setup, view), None);
    setup.core.run_command(WIDER, Value::Null).expect("+1");
    setup.core.run_command(WIDER, Value::Null).expect("+1");
    assert_eq!(size(&setup, view), (769, 1024));
    setup.core.run_command(RESET, Value::Null).expect("reset");
    assert_eq!(size(&setup, view), (1440, 900));
    assert_eq!(breakpoint_of(&setup, view).as_deref(), Some("desktop"));
}

#[tokio::test]
async fn breakpoint_shortcuts_leader_b_1_takes_the_first_detected_breakpoint() {
    let setup = setup();
    let view = active(&setup);
    setup.core.host().update_shell_state(|shell| {
        shell.detected = vec![
            DetectedBreakpoint {
                width: 1024,
                kind: BoundaryKind::Min,
                label: None,
                count: 2,
                source: None,
                heuristic: false,
            },
            DetectedBreakpoint {
                width: 640,
                kind: BoundaryKind::Min,
                label: Some("sm".to_owned()),
                count: 1,
                source: None,
                heuristic: false,
            },
        ];
    });
    setup
        .core
        .run_command("breakpoints.detected.1", Value::Null)
        .expect("sets");
    assert_eq!(size(&setup, view), (640, 900));
    let missing = setup
        .core
        .run_command("breakpoints.detected.3", Value::Null);
    assert!(matches!(missing, Err(CommandError::InvalidArgs(_))));
}

#[tokio::test]
async fn breakpoint_shortcuts_project_key_from_the_config_sets_its_breakpoint() {
    let setup = setup();
    let view = active(&setup);
    let (keys, conflicts) = project_shortcuts(&setup.core);
    assert!(conflicts.is_empty(), "{conflicts:?}");
    assert_eq!(keys.len(), 1);
    let taken = setup.core.on_key_down(
        Focus::View,
        key(EVENTFLAG_CONTROL_DOWN | EVENTFLAG_SHIFT_DOWN, 0x78, '\u{0}'),
        Instant::now(),
    );
    assert!(taken);
    until(|| breakpoint_of(&setup, view).as_deref() == Some("mobile")).await;
    // Typing is never a project key.
    assert!(
        !setup
            .core
            .on_key_down(Focus::View, key(0, 0x41, 'a'), Instant::now())
    );
}

#[tokio::test]
async fn breakpoint_shortcuts_report_conflicting_project_keys() {
    let mut config = config();
    config.breakpoints[0].shortcut = Some("Ctrl+Shift+F11".to_owned());
    config.breakpoints[1].shortcut = Some("Ctrl+Shift+F11".to_owned());
    config.breakpoints[2].shortcut = Some("Alt+M".to_owned());
    let setup = setup_with(config, UserSettings::default());
    let (keys, conflicts) = project_shortcuts(&setup.core);
    assert!(keys.is_empty());
    assert_eq!(conflicts.len(), 3);
    let scripts = shell_scripts(&setup);
    assert_eq!(scripts.matches(CONFLICT_KEY).count(), 3, "{scripts}");
    assert!(scripts.contains("shortcuts.bindings"));
}

#[tokio::test]
async fn breakpoint_shortcuts_quick_pick_message_sets_a_free_size_and_a_breakpoint() {
    let setup = setup();
    let view = active(&setup);
    let free = ShellToCore::SetBreakpoint {
        view,
        breakpoint: None,
        width: Some(500),
        height: Some(800),
        dpr: None,
    };
    assert!(matches!(
        ipc(&setup.core, &free),
        Some(Ok(CoreToShell::Ack))
    ));
    assert_eq!(size(&setup, view), (500, 800));
    let named = ShellToCore::SetBreakpoint {
        view,
        breakpoint: Some("mobile".to_owned()),
        width: None,
        height: None,
        dpr: None,
    };
    assert!(matches!(
        ipc(&setup.core, &named),
        Some(Ok(CoreToShell::Ack))
    ));
    assert_eq!(size(&setup, view), (393, 852));
    let bad = ShellToCore::SetBreakpoint {
        view,
        breakpoint: Some("nope".to_owned()),
        width: None,
        height: None,
        dpr: None,
    };
    assert!(matches!(
        ipc(&setup.core, &bad),
        Some(Err((IpcErrorCode::BadRequest, _)))
    ));
    assert!(ipc(&setup.core, &ShellToCore::GetState).is_none());
}

#[tokio::test]
async fn breakpoint_shortcuts_list_the_project_shortcuts_for_the_editor() {
    let setup = setup();
    let Some(Ok(CoreToShell::ProjectShortcuts(list))) =
        ipc(&setup.core, &ShellToCore::ListProjectShortcuts)
    else {
        panic!("answered with projectShortcuts");
    };
    assert_eq!(list.items.len(), 1);
    assert_eq!(list.items[0].breakpoint, "mobile");
    assert_eq!(list.items[0].binding, "Ctrl+Shift+F9");
    assert_eq!(list.items[0].reason_key, None);
}

#[tokio::test]
async fn breakpoint_shortcuts_mcp_args_take_a_view_and_a_breakpoint_or_a_size() {
    let setup = setup();
    let views: Vec<ViewId> = setup.core.host().views().iter().map(|v| v.id).collect();
    let second = views[1];
    setup
        .core
        .run_command(SET, json!({ "viewId": second, "breakpoint": "mobile" }))
        .expect("by id");
    assert_eq!(breakpoint_of(&setup, second).as_deref(), Some("mobile"));
    setup
        .core
        .run_command(SET, json!({ "view": second, "width": 500, "height": 800 }))
        .expect("by size");
    assert_eq!(size(&setup, second), (500, 800));
    let both = setup.core.run_command(
        SET,
        json!({ "view": second, "breakpoint": "mobile", "width": 500 }),
    );
    assert!(matches!(both, Err(CommandError::InvalidArgs(_))));
    let unknown = setup
        .core
        .run_command(SET, json!({ "view": second, "colour": 1 }));
    assert!(matches!(unknown, Err(CommandError::InvalidArgs(_))));
}

#[tokio::test]
async fn breakpoint_shortcuts_without_arguments_open_the_free_input() {
    let setup = setup();
    active(&setup);
    setup.core.run_command(SET, Value::Null).expect("opens");
    assert!(shell_scripts(&setup).contains("\"uiCommand\""));
}

#[tokio::test]
async fn breakpoint_shortcuts_new_view_keeps_the_active_view() {
    let setup = setup();
    let view = active(&setup);
    let count = setup.core.host().views().len();
    let answer = setup
        .core
        .run_command("view.set_breakpoint.3", json!({ "newView": true }))
        .expect("opens");
    assert_eq!(setup.core.host().views().len(), count + 1);
    assert_eq!(breakpoint_of(&setup, view).as_deref(), Some("desktop"));
    assert_eq!(size(&setup, view), (1440, 900));
    assert_ne!(answer["view"], json!(view));
}

fn with_modifier(modifier: NewViewModifier, project_key: &str) -> Setup {
    let user = UserSettings {
        breakpoint_shortcuts: BreakpointShortcutSettings {
            new_view_modifier: Some(modifier),
            overlay_ms: 0,
        },
        ..UserSettings::default()
    };
    let mut config = config();
    config.breakpoints[2].shortcut = Some(project_key.to_owned());
    setup_with(config, user)
}

#[tokio::test]
async fn breakpoint_shortcuts_modifier_on_the_key_opens_a_new_view() {
    let setup = with_modifier(NewViewModifier::Shift, "Ctrl+F9");
    let view = active(&setup);
    let count = setup.core.host().views().len();
    let taken = setup.core.on_key_down(
        Focus::View,
        key(EVENTFLAG_CONTROL_DOWN | EVENTFLAG_SHIFT_DOWN, 0x78, '\u{0}'),
        Instant::now(),
    );
    assert!(taken);
    until(|| setup.core.host().views().len() == count + 1).await;
    assert_eq!(breakpoint_of(&setup, view).as_deref(), Some("desktop"));
    assert!(wants_new_view(
        &setup.core,
        Modifiers {
            shift: true,
            ..Modifiers::NONE
        }
    ));
    assert!(!wants_new_view(
        &setup.core,
        Modifiers {
            alt: true,
            ..Modifiers::NONE
        }
    ));
}

#[tokio::test]
async fn breakpoint_shortcuts_never_take_a_reserved_combination_as_pressed() {
    // Shift held with a key bound to Ctrl+Tab is Ctrl+Shift+Tab, reserved (the browser's previous
    // tab): the pressed combination is checked, not the one without the modifier.
    let setup = with_modifier(NewViewModifier::Shift, "Ctrl+F9");
    let view = active(&setup);
    let count = setup.core.host().views().len();
    let pressed: Shortcut = "Ctrl+Shift+Tab".parse().expect("a shortcut");
    let own = CommandId::new("view.set_breakpoint.2").expect("an id");
    let taken = on_unbound_key(&setup.core, pressed, |_| Some(own.clone()));
    assert!(!taken, "Ctrl+Shift+Tab reaches the page");
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(setup.core.host().views().len(), count);
    assert_eq!(breakpoint_of(&setup, view).as_deref(), Some("desktop"));
}

#[tokio::test]
async fn breakpoint_shortcuts_quick_pick_lists_the_project_and_the_devices() {
    let setup = setup();
    let view = active(&setup);
    let items = quick_pick(&setup.core, view);
    let sections: Vec<Section> = items.iter().map(|item| item.section).collect();
    assert_eq!(&sections[..3], [Section::Breakpoints; 3]);
    assert!(items[2].checked, "desktop, the widest, is shown");
    assert_eq!(sections.last(), Some(&Section::Custom));
}
