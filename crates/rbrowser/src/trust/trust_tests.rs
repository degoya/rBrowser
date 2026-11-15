use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use rb_commands::McpCoverage;
use rb_ipc::{IpcErrorCode, TrustDecision};
use rb_plugin_host::WorkspaceTrust;
use serde_json::{Value, json};
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
use crate::host::Host;
use crate::host::ui::recording::{RecordingUi, UiCall};

pub(super) fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/untrusted-project")
}

pub(super) fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("mkdir");
    for entry in std::fs::read_dir(from).expect("read dir") {
        let entry = entry.expect("entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("copy");
        }
    }
}

pub(super) struct Setup {
    _fake: Arc<FakeBrowser>,
    settings: tempfile::TempDir,
    pub(super) project: tempfile::TempDir,
    ui: Arc<RecordingUi>,
    pub(super) core: Arc<Core>,
}

impl Setup {
    pub(super) fn root(&self) -> PathBuf {
        self.project.path().to_path_buf()
    }

    pub(super) fn store(&self) -> PathBuf {
        self.settings.path().join(rb_config::trust::TRUST_FILE)
    }

    pub(super) fn trusted(&self) -> bool {
        self.core.host().shell_state().trusted
    }

    /// The messages sent to the shell, as JSON.
    pub(super) fn sent(&self) -> Vec<Value> {
        self.ui
            .calls()
            .into_iter()
            .filter_map(|call| match call {
                UiCall::Shell(script) => {
                    let start = script.find("detail:")? + "detail:".len();
                    let end = script.rfind("}));")?;
                    serde_json::from_str(&script[start..end]).ok()
                }
                _ => None,
            })
            .collect()
    }

    pub(super) fn prompts(&self) -> Vec<Value> {
        self.sent()
            .into_iter()
            .filter(|message| message["type"] == "trustPrompt")
            .collect()
    }

    pub(super) fn last_prompt_id(&self) -> u32 {
        let prompts = self.prompts();
        let id = prompts.last().expect("a prompt")["promptId"]
            .as_u64()
            .expect("an id");
        u32::try_from(id).expect("fits")
    }

    pub(super) fn decide(&self, decision: TrustDecision) -> Result<CoreToShell, Failure> {
        let message = ShellToCore::TrustDecision {
            prompt_id: self.last_prompt_id(),
            decision,
        };
        ipc(&self.core, &message).expect("claimed")
    }
}

/// A core over a copy of the K9 fixture, its user settings (and so `trust.json`) in a temp
/// directory, the shell ready.
pub(super) fn setup() -> Setup {
    setup_with(|project| copy_dir(&fixture(), project))
}

/// [`setup`] over the project `fill` writes.
pub(super) fn setup_with(fill: impl FnOnce(&Path)) -> Setup {
    let settings = tempfile::tempdir().expect("temp dir");
    let project = tempfile::tempdir().expect("temp dir");
    fill(project.path());
    let (fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    let config = rb_config::load_project(project.path())
        .expect("loads")
        .config;
    host.open_config(config, Some(project.path().to_path_buf()))
        .expect("opens");
    let setup = CoreSetup {
        user_path: Some(settings.path().join("settings.json")),
        ..CoreSetup::default()
    };
    let core = Core::new(host, ui.clone(), Handle::current(), setup);
    core.shell_ready();
    Setup {
        _fake: fake,
        settings,
        project,
        ui,
        core,
    }
}

#[tokio::test]
async fn an_untrusted_project_is_prompted_with_every_blocked_part() {
    let s = setup();
    on_start(&s.core);
    assert!(!s.trusted());
    let prompts = s.prompts();
    assert_eq!(prompts.len(), 1);
    let prompt = &prompts[0];
    assert_eq!(prompt["projectRoot"], json!(s.root().to_string_lossy()));
    let kinds: Vec<&str> = prompt["blocked"]
        .as_array()
        .expect("a list")
        .iter()
        .filter_map(|item| item["kind"].as_str())
        .collect();
    for kind in [
        "flows",
        "snippets",
        "plugins",
        "mocks",
        "headers",
        "hostResolverRules",
        "pageScripts",
        "extensions",
        "ignoreCertificateErrors",
        "proxy",
    ] {
        assert!(kinds.contains(&kind), "{kind} in {kinds:?}");
    }
    assert!(!s.store().exists(), "nothing decided, nothing written");
}

#[tokio::test]
async fn the_dialogs_decision_trusts_without_a_restart() {
    let s = setup();
    on_start(&s.core);
    let answer = s.decide(TrustDecision::Trust).expect("trusts");
    assert_eq!(answer, CoreToShell::Ack);
    assert!(s.trusted(), "the shell state follows at once");
    let store = rb_config::trust::load_trust_store(&s.store()).expect("written");
    assert_eq!(store.records.len(), 1);
    assert_eq!(store.records[0].root, s.root().to_string_lossy());
    assert!(
        s.sent()
            .iter()
            .any(|message| message["messageKey"] == RESTART_NOTICE_KEY),
        "the network keys change at the next start"
    );
    let CoreToShell::Trust(info) = ipc(&s.core, &ShellToCore::GetTrust)
        .expect("claimed")
        .expect("answers")
    else {
        panic!("a trust answer");
    };
    let current = info.current.expect("a project");
    assert!(current.trusted);
    assert_eq!(info.projects.len(), 1);
    assert_eq!(s.prompts().len(), 1, "a trusted project is not asked again");
}

#[tokio::test]
async fn a_project_trusted_at_start_is_not_applied_again() {
    let s = setup();
    let (_, state) =
        rb_config::trust::load_project_trusted(&s.root(), &rb_config::trust::TrustStore::default())
            .expect("loads");
    rb_config::trust::update_trust_store(&s.store(), |store| {
        store.grant(&s.root(), &state.hash, "2026-11-10T12:00:00Z");
    })
    .expect("writes");
    start_trust(s.core.host(), true);
    on_start(&s.core);
    assert!(s.trusted());
    assert!(
        !s.sent()
            .iter()
            .any(|message| message["messageKey"] == RESTART_NOTICE_KEY),
        "the network keys applied at this start already"
    );
    assert!(s.prompts().is_empty());
}

#[tokio::test]
async fn restricted_keeps_the_project_blocked_and_the_prompt_is_used_up() {
    let s = setup();
    on_start(&s.core);
    let id = s.last_prompt_id();
    assert_eq!(
        s.decide(TrustDecision::Restricted).expect("answers"),
        CoreToShell::Ack
    );
    assert!(!s.trusted());
    assert!(!s.store().exists());
    let again = ShellToCore::TrustDecision {
        prompt_id: id,
        decision: TrustDecision::Trust,
    };
    let error = ipc(&s.core, &again).expect("claimed").expect_err("used up");
    assert_eq!(error.0, IpcErrorCode::NotFound);
    assert!(!s.trusted());
}

#[tokio::test]
async fn the_trust_page_gets_the_open_prompt_again() {
    let s = setup();
    on_start(&s.core);
    let first = s.last_prompt_id();
    let CoreToShell::Trust(info) = ipc(&s.core, &ShellToCore::GetTrust)
        .expect("claimed")
        .expect("answers")
    else {
        panic!("a trust answer");
    };
    assert!(!info.current.expect("a project").trusted);
    assert_eq!(s.prompts().len(), 2);
    assert_eq!(s.last_prompt_id(), first, "the same question, the same id");
}

#[tokio::test]
async fn a_changed_flow_asks_again_after_the_reload_but_a_breakpoint_does_not() {
    let s = setup();
    on_start(&s.core);
    s.decide(TrustDecision::Trust).expect("trusts");
    let first = s.last_prompt_id();

    let config = s.root().join(".rbrowser/config.json");
    let text = std::fs::read_to_string(&config).expect("reads");
    std::fs::write(&config, text.replace("1440", "1600")).expect("writes");
    on_config_changed(&s.core);
    assert!(s.trusted(), "a breakpoint is not protected");

    std::fs::write(
        s.root().join(".rbrowser/flows/login.ts"),
        "await page.goto('https://example.invalid/changed');\n",
    )
    .expect("writes");
    on_config_changed(&s.core);
    assert!(!s.trusted(), "a changed flow is blocked again");
    assert_ne!(s.last_prompt_id(), first, "and asked about anew");
}

#[tokio::test]
async fn revoking_blocks_at_once() {
    let s = setup();
    on_start(&s.core);
    s.decide(TrustDecision::Trust).expect("trusts");
    let message = ShellToCore::RevokeTrust {
        root: s.root().to_string_lossy().into_owned(),
    };
    let CoreToShell::Trust(info) = ipc(&s.core, &message).expect("claimed").expect("answers")
    else {
        panic!("a trust answer");
    };
    assert!(!s.trusted());
    assert!(info.projects.is_empty());
    assert!(!info.current.expect("a project").trusted);

    s.decide(TrustDecision::Trust).expect("trusts again");
    assert!(s.trusted());
    run_command(&s.core, "trust.revoke", &Value::Null)
        .expect("claimed")
        .expect("revokes");
    assert!(!s.trusted());
}

#[tokio::test]
async fn no_command_and_no_setting_grants_trust() {
    let s = setup();
    on_start(&s.core);
    for id in ["trust.show", "trust.grant", "trust.revoke"] {
        let result = run_command(&s.core, id, &json!({ "decision": "trust", "promptId": 1 }));
        assert!(result.expect("claimed").is_ok(), "{id}");
        assert!(!s.trusted(), "{id} must not trust");
    }
    assert!(
        s.ui.calls()
            .iter()
            .any(|call| matches!(call, UiCall::SettingsWindow(url) if url.ends_with("#/trust"))),
        "trust.grant opens the dialog"
    );
    let store = rb_config::trust::load_trust_store(&s.store()).expect("loads");
    assert!(store.records.is_empty(), "{store:?}");

    // No MCP tool reaches a trust command, and no settings key holds a decision.
    let CoreToShell::Commands { commands, .. } = s.core.commands() else {
        panic!("a command list");
    };
    for command in commands.iter().filter(|c| c.id.starts_with("trust.")) {
        assert!(
            matches!(command.mcp, McpCoverage::Omitted(_)),
            "{}",
            command.id
        );
    }
    let keys: Vec<&str> = s
        .core
        .settings_registry()
        .iter()
        .map(|def| def.key.as_str())
        .filter(|key| key.to_ascii_lowercase().contains("trust"))
        .collect();
    assert!(
        keys.iter()
            .all(|key| key.starts_with("plugins.trustedKeys")),
        "{keys:?}"
    );
}

#[tokio::test]
async fn the_plugin_host_trusts_only_a_decided_unchanged_project() {
    let s = setup();
    on_start(&s.core);
    let trust = ProjectTrust::new(Some(s.store()));
    assert!(!trust.is_trusted(&s.root()));
    assert!(!ProjectTrust::new(None).is_trusted(&s.root()));
    s.decide(TrustDecision::Trust).expect("trusts");
    assert!(trust.is_trusted(&s.root()));
    let config_path = s.root().join(".rbrowser/config.json");
    let mut config = rb_config::read_json(&config_path).expect("reads");
    config["breakpoints"][0]["width"] = json!(-5);
    rb_config::write_json(&config_path, &config).expect("writes");
    assert!(
        rb_config::load_project(&s.root()).is_err(),
        "the config does not validate"
    );
    assert!(
        trust.is_trusted(&s.root()),
        "an error in an unprotected part does not untrust the plugins"
    );
    std::fs::write(
        s.root().join(".rbrowser/plugins/greeter/plugin.wasm"),
        [0u8],
    )
    .expect("w");
    assert!(
        !trust.is_trusted(&s.root()),
        "a changed plugin is not trusted"
    );
}

#[tokio::test]
async fn a_prompt_for_a_closed_project_cannot_trust_the_open_one() {
    let s = setup();
    on_start(&s.core);
    let other = tempfile::tempdir().expect("temp dir");
    copy_dir(&fixture(), other.path());
    let config = rb_config::load_project(other.path()).expect("loads").config;
    s.core
        .host()
        .open_config(config, Some(other.path().to_path_buf()))
        .expect("opens");
    let error = s.decide(TrustDecision::Trust).expect_err("refused");
    assert_eq!(error.0, IpcErrorCode::BadRequest);
    assert!(!s.trusted());
}

#[cfg(unix)]
#[tokio::test]
async fn a_link_out_of_the_project_cannot_be_trusted() {
    let s = setup();
    let outside = tempfile::tempdir().expect("temp dir");
    std::fs::write(outside.path().join("big.ts"), "one").expect("write");
    std::os::unix::fs::symlink(
        outside.path().join("big.ts"),
        s.root().join(".rbrowser/flows/linked.ts"),
    )
    .expect("link");
    on_start(&s.core);
    let error = s.decide(TrustDecision::Trust).expect_err("refused");
    assert_eq!(error.0, IpcErrorCode::BadRequest);
    assert!(error.1.contains(".rbrowser/flows/linked.ts"), "{}", error.1);
    assert!(!s.trusted());
    assert!(!s.store().exists(), "nothing decided, nothing written");
}

#[tokio::test]
async fn without_a_project_nothing_is_asked() {
    let s = setup();
    s.core
        .host()
        .open_config(crate::host::state::default_config(), None)
        .expect("opens");
    on_start(&s.core);
    on_config_changed(&s.core);
    assert!(s.prompts().is_empty());
    let CoreToShell::Trust(info) = ipc(&s.core, &ShellToCore::GetTrust)
        .expect("claimed")
        .expect("answers")
    else {
        panic!("a trust answer");
    };
    assert!(info.current.is_none());
    assert!(
        run_command(&s.core, "trust.grant", &Value::Null)
            .expect("claimed")
            .is_err()
    );
    assert!(run_command(&s.core, "view.back", &Value::Null).is_none());
    assert!(ipc(&s.core, &ShellToCore::GetDevices).is_none());
}

#[test]
fn decisions_are_dated_in_rfc_3339_utc() {
    let at = SystemTime::UNIX_EPOCH + Duration::from_secs(1_794_312_000 + 61);
    assert_eq!(rfc3339(at), "2026-11-10T12:01:01Z");
}
