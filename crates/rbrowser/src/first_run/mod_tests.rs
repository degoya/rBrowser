use std::path::Path;
use std::sync::Arc;

use rb_config::UserSettings;
use rb_ipc::SetupTemplate;
use serde_json::json;
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
use crate::host::Host;
use crate::host::state::default_config;
use crate::host::ui::recording::{RecordingUi, UiCall};

/// A window on the project at `root` (none without), the user settings at `settings`.
fn window(
    root: Option<&Path>,
    settings: Option<&Path>,
    user: UserSettings,
) -> (Arc<Core>, Arc<RecordingUi>) {
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(default_config(), root.map(Path::to_path_buf))
        .expect("opens");
    let setup = CoreSetup {
        user,
        user_path: settings.map(Path::to_path_buf),
        ..CoreSetup::default()
    };
    let core = Core::new(host, ui.clone(), Handle::current(), setup);
    core.shell_ready();
    (core, ui)
}

fn settings_windows(ui: &RecordingUi) -> Vec<String> {
    ui.calls()
        .into_iter()
        .filter_map(|call| match call {
            UiCall::SettingsWindow(url) => Some(url),
            _ => None,
        })
        .collect()
}

fn off() -> UserSettings {
    let mut user = UserSettings::default();
    user.ui.first_run_wizard = false;
    user
}

#[test]
fn the_command_is_leader_comma_w_and_never_mcps() {
    let defs = definitions().expect("well-formed");
    assert_eq!(defs.len(), 1);
    assert_eq!(defs[0].id.as_str(), COMMAND);
    assert_eq!(defs[0].group, Group::Settings);
    assert_eq!(
        defs[0].default_bindings,
        vec![Binding::Leader(Chord {
            keys: vec![Key::Char(','), Key::Char('W')]
        })]
    );
    assert_eq!(defs[0].mcp, McpCoverage::Omitted(MCP_OMITTED.to_owned()));
    assert!(refuse_mcp(COMMAND).is_err());
    assert!(refuse_mcp("app.about").is_ok());
}

#[tokio::test(flavor = "multi_thread")]
async fn the_command_opens_the_wizard_or_its_project_step() {
    let (core, ui) = window(None, None, UserSettings::default());
    open(&core, &Value::Null).expect("opens");
    open(&core, &json!({ "step": "project" })).expect("opens");
    assert_eq!(
        settings_windows(&ui),
        [
            crate::shell::page_url("setup"),
            crate::shell::page_url("setup/project")
        ]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn switched_off_the_command_refuses() {
    let (core, ui) = window(None, None, off());
    let error = open(&core, &Value::Null).expect_err("refused");
    assert_eq!(error, CommandError::Failed(OFF_KEY.to_owned()));
    assert!(settings_windows(&ui).is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn the_very_first_start_opens_the_wizard_and_the_second_does_not() {
    let dir = tempfile::tempdir().expect("temp dir");
    let settings = dir.path().join("settings.json");
    let (core, ui) = window(None, Some(&settings), UserSettings::default());
    on_start(&core);
    assert_eq!(settings_windows(&ui), [crate::shell::page_url("setup")]);
    assert!(state::path_for(&settings).is_file(), "remembered");
    let (core, ui) = window(None, Some(&settings), UserSettings::default());
    on_start(&core);
    assert!(settings_windows(&ui).is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn without_a_settings_file_path_nothing_shows() {
    let (core, ui) = window(None, None, UserSettings::default());
    on_start(&core);
    assert!(settings_windows(&ui).is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_project_without_config_is_hinted_once_with_its_button() {
    let dir = tempfile::tempdir().expect("temp dir");
    let settings = dir.path().join("settings.json");
    std::fs::write(&settings, "{ \"version\": 1 }").expect("writes");
    let project = tempfile::tempdir().expect("temp dir");
    let (core, ui) = window(
        Some(project.path()),
        Some(&settings),
        UserSettings::default(),
    );
    on_start(&core);
    assert!(settings_windows(&ui).is_empty(), "no wizard: used before");
    let hints: Vec<_> = core
        .status_log()
        .items()
        .into_iter()
        .filter(|item| item.message_key == HINT_KEY)
        .collect();
    assert_eq!(hints.len(), 1);
    let action = hints[0].action.clone().expect("a button");
    assert_eq!(action.command, COMMAND);
    assert_eq!(action.args, json!({ "step": "project" }));
    assert_eq!(hints[0].settings_key.as_deref(), Some(SETTING_KEY));
    let (core, _) = window(
        Some(project.path()),
        Some(&settings),
        UserSettings::default(),
    );
    on_start(&core);
    assert!(
        core.status_log()
            .items()
            .iter()
            .all(|item| item.message_key != HINT_KEY),
        "never again"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn switched_off_the_start_shows_nothing() {
    let dir = tempfile::tempdir().expect("temp dir");
    let settings = dir.path().join("settings.json");
    let project = tempfile::tempdir().expect("temp dir");
    let (core, ui) = window(Some(project.path()), Some(&settings), off());
    on_start(&core);
    assert!(settings_windows(&ui).is_empty());
    assert!(core.status_log().items().is_empty());
    assert!(!state::path_for(&settings).exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn info_names_the_switch_and_the_open_project() {
    let project = tempfile::tempdir().expect("temp dir");
    let (core, _) = window(Some(project.path()), None, UserSettings::default());
    let answer = ipc(
        &core,
        &ShellToCore::Setup {
            request: SetupRequest::Info,
        },
    )
    .expect("ours")
    .expect("answers");
    let CoreToShell::Setup {
        answer:
            SetupAnswer::Info {
                enabled,
                project: found,
                ..
            },
    } = answer
    else {
        panic!("not info: {answer:?}");
    };
    assert!(enabled);
    let found = found.expect("a project");
    assert!(!found.has_config);
    assert_eq!(Path::new(&found.root), project.path());
}

#[tokio::test(flavor = "multi_thread")]
async fn close_closes_the_settings_window_and_other_messages_are_not_ours() {
    let (core, ui) = window(None, None, UserSettings::default());
    let answer = ipc(
        &core,
        &ShellToCore::Setup {
            request: SetupRequest::Close,
        },
    )
    .expect("ours")
    .expect("answers");
    assert_eq!(answer, CoreToShell::Ack);
    assert!(ui.calls().contains(&UiCall::CloseSettingsWindow));
    assert!(ipc(&core, &ShellToCore::GetAbout).is_none());
    assert!(is_setup_request(
        r#"{"type":"setup","request":{"op":"info"}}"#
    ));
    assert!(!is_setup_request(r#"{"type":"getAbout"}"#));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_project_step_previews_writes_and_then_takes_the_basics() {
    let (core, _) = window(None, None, UserSettings::default());
    let folder = tempfile::tempdir().expect("temp dir");
    let preview = SetupRequest::Preview {
        request_id: 1,
        root: folder.path().to_string_lossy().into_owned(),
        url: Some("http://localhost:3000".to_owned()),
        template: SetupTemplate::Default,
        devices: Vec::new(),
    };
    let SetupAnswer::Proposal {
        proposal_id,
        request_id: 1,
        ..
    } = answer(&core, &preview)
    else {
        panic!("no proposal for request 1");
    };
    assert!(
        !ConfigFile::Project.path(folder.path()).exists(),
        "a preview writes nothing"
    );
    let written = answer(
        &core,
        &SetupRequest::Write {
            request_id: 2,
            proposal_id,
        },
    );
    let SetupAnswer::Written {
        open,
        request_id: 2,
        ..
    } = written
    else {
        panic!("not written: {written:?}");
    };
    assert!(!open, "another folder than the open project");
    assert!(
        core.user_settings()
            .recent_projects
            .iter()
            .any(|recent| Path::new(recent) == folder.path()),
        "a recent project now"
    );
    let again = answer(
        &core,
        &SetupRequest::Write {
            request_id: 3,
            proposal_id,
        },
    );
    assert!(
        matches!(again, SetupAnswer::Failed { request_id: 3, .. }),
        "written once"
    );
    let applied = answer(
        &core,
        &SetupRequest::Basics {
            request_id: 4,
            layout: Some(SetupLayout::Canvas),
            sync: Some(false),
        },
    );
    assert!(
        matches!(applied, SetupAnswer::Applied { request_id: 4, .. }),
        "{applied:?}"
    );
    let config = rb_config::read_json(&ConfigFile::Project.path(folder.path())).expect("reads");
    assert_eq!(config["layouts"]["default"], "canvas");
    assert_eq!(config["sync"]["scroll"], false);
    assert_eq!(config["startUrl"], "http://localhost:3000");
}

#[tokio::test(flavor = "multi_thread")]
async fn basics_without_any_project_say_so() {
    let (core, _) = window(None, None, UserSettings::default());
    let answer = answer(
        &core,
        &SetupRequest::Basics {
            request_id: 5,
            layout: Some(SetupLayout::Grid),
            sync: None,
        },
    );
    let SetupAnswer::Failed {
        message_key,
        request_id: 5,
        ..
    } = answer
    else {
        panic!("not failed");
    };
    assert_eq!(message_key, NO_PROJECT_KEY);
}

/// A step's answer comes as a `setup` event with its request's number, from the blocking lane.
#[tokio::test(flavor = "multi_thread")]
async fn a_step_is_acked_and_its_answer_follows_as_an_event() {
    let (core, ui) = window(None, None, UserSettings::default());
    let request = ShellToCore::Setup {
        request: SetupRequest::Basics {
            request_id: 42,
            layout: None,
            sync: Some(true),
        },
    };
    let ack = ipc(&core, &request).expect("ours").expect("answers");
    assert_eq!(ack, CoreToShell::Ack);
    let event = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let found = ui.calls().into_iter().find_map(|call| match call {
                UiCall::Shell(script) if script.contains("\"requestId\":42") => Some(script),
                _ => None,
            });
            if let Some(script) = found {
                return script;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("the answer arrives");
    assert!(event.contains("\"op\":\"failed\""), "{event}");
}

/// RB-091-08: a step that panics answers with a failure and its request's number - the page's
/// spinner never waits for an answer that does not come.
#[test]
fn a_step_that_panics_answers_with_a_failure() {
    let write = SetupRequest::Write {
        request_id: 7,
        proposal_id: 1,
    };
    let answer = guarded(&write, || panic!("a step broke"));
    assert_eq!(
        answer,
        SetupAnswer::Failed {
            request_id: 7,
            step: SetupStep::Write,
            message_key: STEP_FAILED_KEY.to_owned(),
            args: BTreeMap::new(),
        }
    );
    let preview = SetupRequest::Preview {
        request_id: 8,
        root: "C:\\site".to_owned(),
        url: None,
        template: SetupTemplate::Default,
        devices: Vec::new(),
    };
    assert!(matches!(
        guarded(&preview, || panic!("again")),
        SetupAnswer::Failed {
            request_id: 8,
            step: SetupStep::Preview,
            ..
        }
    ));
    // A step that does not panic answers as it does.
    let applied = SetupAnswer::Applied {
        request_id: 9,
        root: "C:\\site".to_owned(),
    };
    let basics = SetupRequest::Basics {
        request_id: 9,
        layout: None,
        sync: None,
    };
    assert_eq!(guarded(&basics, || applied.clone()), applied);
}
