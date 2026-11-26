use std::sync::{Arc, Mutex};

use rb_config::{ProjectConfig, UserSettings};
use rb_ipc::{AuthPromptReply, AuthPromptSignIn, CoreToShell, IpcErrorCode};
use serde_json::json;
use tokio::runtime::Handle;

use super::*;
use crate::basic_auth::store::MemoryStore;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
use crate::host::Host;
use crate::host::state::default_config;
use crate::host::ui::recording::{RecordingUi, UiCall};

/// The default project with two environments, `staging` behind Basic auth.
pub(crate) fn config() -> ProjectConfig {
    let mut config = default_config();
    config.environments = serde_json::from_value(json!([
        { "id": "development", "url": "https://dev.example.test" },
        { "id": "staging", "url": "https://staging.example.test/" }
    ]))
    .expect("environments");
    config.basic_auth =
        serde_json::from_value(json!({ "environments": ["staging"] })).expect("basic auth");
    config
}

pub(crate) fn core() -> (Arc<RecordingUi>, Arc<Core>) {
    core_at(None)
}

/// The project root of the tests that store a credential: a path only, nothing is written there.
pub(crate) fn root() -> std::path::PathBuf {
    std::path::PathBuf::from("/work/ask-test-project")
}

pub(crate) fn core_at(root: Option<std::path::PathBuf>) -> (Arc<RecordingUi>, Arc<Core>) {
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(config(), root).expect("opens");
    let setup = CoreSetup {
        user: UserSettings::default(),
        ..CoreSetup::default()
    };
    let core = Core::new(host, ui.clone(), Handle::current(), setup);
    (ui, core)
}

/// What a waiting request got.
#[derive(Clone, Default)]
pub(crate) struct Got(Arc<Mutex<Vec<String>>>);

impl Got {
    pub(crate) fn waiter(&self, name: &str) -> Box<dyn Waiter> {
        Box::new(Named(name.to_owned(), self.clone()))
    }

    pub(crate) fn list(&self) -> Vec<String> {
        rb_util::lock(&self.0).clone()
    }
}

struct Named(String, Got);

impl Waiter for Named {
    fn sign_in(&self, user: &str, password: &str) {
        rb_util::lock(&(self.1).0).push(format!("{} {user} {password}", self.0));
    }

    fn cancel(&self) {
        rb_util::lock(&(self.1).0).push(format!("{} cancelled", self.0));
    }
}

pub(crate) fn site(host: &'static str) -> Challenge<'static> {
    Challenge {
        origin_url: "https://dev.example.test/account?step=2",
        is_proxy: false,
        host,
        port: 443,
        realm: "Members",
        scheme: "basic",
    }
}

/// The scripts that carried a sign-in question to the shell.
pub(crate) fn shell_scripts(ui: &RecordingUi) -> Vec<String> {
    ui.calls()
        .into_iter()
        .filter_map(|call| match call {
            UiCall::Shell(script) if script.contains("\"authPrompt") => Some(script),
            _ => None,
        })
        .collect()
}

pub(crate) fn sign_in(id: u64, save: bool) -> AuthPromptReply {
    AuthPromptReply {
        id,
        sign_in: Some(AuthPromptSignIn {
            user: "ask-test-user".to_owned(),
            password: "ask-test-password".to_owned(),
            save,
        }),
    }
}

#[tokio::test]
async fn views_wait_on_one_question_in_the_shell_and_all_get_the_answer() {
    let (ui, core) = core();
    let asking = Asking::default();
    let got = Got::default();
    let focused = || {
        ui.calls()
            .iter()
            .filter(|call| matches!(call, UiCall::FocusShell))
            .count()
    };
    let before = focused();
    let first = asking.ask(
        &core,
        ViewId(1),
        11,
        &site("dev.example.test"),
        Refusal::NoCredential,
        got.waiter("a"),
    );
    let id = match first {
        Ask::Waiting(Some(id)) => id,
        _ => panic!("a new question"),
    };
    let second = asking.ask(
        &core,
        ViewId(2),
        12,
        &site("dev.example.test"),
        Refusal::NoCredential,
        got.waiter("b"),
    );
    assert!(
        matches!(second, Ask::Waiting(None)),
        "joins the open question"
    );
    let scripts = shell_scripts(&ui);
    assert_eq!(scripts.len(), 2, "the question, then its second view");
    assert!(
        scripts[0].contains("\"authPrompt\"")
            && scripts[0].contains("https://dev.example.test:443")
    );
    assert!(
        scripts[0].contains("\"environment\":\"development\""),
        "{}",
        scripts[0]
    );
    assert!(scripts[1].contains("\"views\":[1,2]"), "{}", scripts[1]);
    assert!(
        !scripts.iter().any(|script| script.contains("account")),
        "no path reaches the shell"
    );
    assert_eq!(
        focused() - before,
        1,
        "the shell takes the focus for a new question only"
    );
    let auth = BasicAuth::new(Box::new(MemoryStore::default()));
    assert_eq!(
        asking.reply(&core, &auth, &sign_in(id, false)),
        Ok(CoreToShell::Ack)
    );
    assert_eq!(
        got.list(),
        vec![
            "a ask-test-user ask-test-password",
            "b ask-test-user ask-test-password"
        ]
    );
    // Answered once: a late second answer finds nothing.
    assert_eq!(
        asking
            .reply(&core, &auth, &sign_in(id, false))
            .map_err(|(code, _)| code),
        Err(IpcErrorCode::NotFound)
    );
}

#[tokio::test]
async fn cancel_hands_every_request_back_to_cef() {
    let (_ui, core) = core();
    let asking = Asking::default();
    let got = Got::default();
    let Ask::Waiting(Some(id)) = asking.ask(
        &core,
        ViewId(1),
        11,
        &site("dev.example.test"),
        Refusal::NotAnEnvironment,
        got.waiter("a"),
    ) else {
        panic!("a new question");
    };
    let auth = BasicAuth::new(Box::new(MemoryStore::default()));
    let cancel = AuthPromptReply { id, sign_in: None };
    assert_eq!(asking.reply(&core, &auth, &cancel), Ok(CoreToShell::Ack));
    assert_eq!(got.list(), vec!["a cancelled"]);
}

#[tokio::test]
async fn an_answer_is_saved_only_for_the_environment_the_core_named() {
    let (_ui, core) = core();
    let asking = Asking::default();
    let got = Got::default();
    let other = Challenge {
        origin_url: "https://other.example.test/",
        ..site("other.example.test")
    };
    let Ask::Waiting(Some(id)) = asking.ask(
        &core,
        ViewId(1),
        11,
        &other,
        Refusal::NotAnEnvironment,
        got.waiter("a"),
    ) else {
        panic!("a new question");
    };
    let auth = BasicAuth::new(Box::new(MemoryStore::default()));
    let refused = asking.reply(&core, &auth, &sign_in(id, true));
    assert_eq!(
        refused.map_err(|(code, _)| code),
        Err(IpcErrorCode::BadRequest)
    );
    assert!(got.list().is_empty(), "the question stays open");
    // Without a project nothing is stored, and nothing is sent either.
    let Ask::Waiting(Some(dev)) = asking.ask(
        &core,
        ViewId(1),
        11,
        &site("dev.example.test"),
        Refusal::NoProject,
        got.waiter("b"),
    ) else {
        panic!("a new question");
    };
    let failed = asking.reply(&core, &auth, &sign_in(dev, true));
    let (_, message) = failed.expect_err("no project");
    assert!(!message.contains("ask-test"), "{message}");
    assert!(got.list().is_empty());
    assert_eq!(
        asking.reply(&core, &auth, &sign_in(dev, false)),
        Ok(CoreToShell::Ack)
    );
    assert_eq!(got.list(), vec!["b ask-test-user ask-test-password"]);
}

#[tokio::test]
async fn a_closed_view_and_the_timeout_withdraw_the_question() {
    let (ui, core) = core();
    let asking = Asking::default();
    let got = Got::default();
    let Ask::Waiting(Some(id)) = asking.ask(
        &core,
        ViewId(1),
        11,
        &site("dev.example.test"),
        Refusal::NoCredential,
        got.waiter("a"),
    ) else {
        panic!("a new question");
    };
    asking.view_closed(&core, 11);
    assert_eq!(got.list(), vec!["a cancelled"]);
    assert!(
        shell_scripts(&ui)
            .last()
            .is_some_and(|script| script.contains("\"authPromptClosed\""))
    );
    let Ask::Waiting(Some(next)) = asking.ask(
        &core,
        ViewId(2),
        12,
        &site("dev.example.test"),
        Refusal::NoCredential,
        got.waiter("b"),
    ) else {
        panic!("a new question");
    };
    assert_ne!(next, id);
    asking.expire(&core, next);
    assert_eq!(got.list(), vec!["a cancelled", "b cancelled"]);
    asking.expire(&core, next);
    assert_eq!(got.list().len(), 2, "expired once");
}

#[tokio::test]
async fn a_view_refused_too_often_is_not_asked_again() {
    let (_ui, core) = core();
    let asking = Asking::default();
    let got = Got::default();
    let auth = BasicAuth::new(Box::new(MemoryStore::default()));
    for round in 0..crate::basic_auth::prompt::MAX_TRIES {
        let Ask::Waiting(Some(id)) = asking.ask(
            &core,
            ViewId(1),
            11,
            &site("dev.example.test"),
            Refusal::NoCredential,
            got.waiter(&round.to_string()),
        ) else {
            panic!("round {round}: a new question");
        };
        assert_eq!(
            asking.reply(&core, &auth, &sign_in(id, false)),
            Ok(CoreToShell::Ack)
        );
    }
    let last = asking.ask(
        &core,
        ViewId(1),
        11,
        &site("dev.example.test"),
        Refusal::NoCredential,
        got.waiter("x"),
    );
    assert!(matches!(last, Ask::NotAsked));
    assert_eq!(got.list().last().map(String::as_str), Some("x cancelled"));
}

/// RB-099-04: the INFO lines of a question and its answer name the origin and realm - never the
/// user, the password or the request's path.
#[tokio::test]
async fn the_log_never_shows_what_the_person_typed() {
    #[derive(Clone, Default)]
    struct Log(Arc<Mutex<Vec<u8>>>);
    impl std::io::Write for Log {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            rb_util::lock(&self.0).extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let (_ui, core) = core();
    let asking = Asking::default();
    let got = Got::default();
    let auth = BasicAuth::new(Box::new(MemoryStore::default()));
    let log = Log::default();
    let writer = log.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .finish();
    tracing::subscriber::with_default(subscriber, || {
        let Ask::Waiting(Some(id)) = asking.ask(
            &core,
            ViewId(1),
            11,
            &site("dev.example.test"),
            Refusal::Refused,
            got.waiter("a"),
        ) else {
            panic!("a new question");
        };
        assert_eq!(
            asking.reply(&core, &auth, &sign_in(id, false)),
            Ok(CoreToShell::Ack)
        );
    });
    let text = String::from_utf8_lossy(&rb_util::lock(&log.0)).into_owned();
    assert!(text.contains("the person is asked"), "{text}");
    assert!(text.contains("the person signed in"), "{text}");
    assert!(
        text.contains("https://dev.example.test:443") && text.contains("Members"),
        "{text}"
    );
    for value in ["ask-test-user", "ask-test-password", "account", "step=2"] {
        assert!(!text.contains(value), "{value} reached the log:\n{text}");
    }
}

/// Security review w35: "Save" stores only for the origin the question asked for. Pointed
/// elsewhere while the question was open, the environment gets nothing - the answer is used once,
/// so a credential typed for one origin is never sent to another.
#[tokio::test]
async fn save_stores_only_for_the_origin_that_was_asked() {
    let (_ui, core) = core_at(Some(root()));
    let asking = Asking::default();
    let got = Got::default();
    let auth = BasicAuth::new(Box::new(MemoryStore::default()));
    let project = crate::instance::file::key(&root());
    let ask = |name: &str| match asking.ask(
        &core,
        ViewId(1),
        11,
        &site("dev.example.test"),
        Refusal::NoCredential,
        got.waiter(name),
    ) {
        Ask::Waiting(Some(id)) => id,
        other => panic!("a new question: {other:?}"),
    };
    // Unchanged: stored for the environment.
    let first = ask("a");
    assert_eq!(
        asking.reply(&core, &auth, &sign_in(first, true)),
        Ok(CoreToShell::Ack)
    );
    assert_eq!(
        auth.statuses(Some(&project), &config())[0],
        rb_ipc::EnvironmentAuthStatus::stored("development", "ask-test-user")
    );
    auth.remove(&project, &config(), "development")
        .expect("removed");
    // Moved while asked: used once, stored nowhere.
    let second = ask("b");
    let mut moved = config();
    moved.environments[0].url = "https://moved.example.test/".to_owned();
    core.host()
        .open_config(moved.clone(), Some(root()))
        .expect("reopens");
    assert_eq!(
        asking.reply(&core, &auth, &sign_in(second, true)),
        Ok(CoreToShell::Ack)
    );
    assert_eq!(
        got.list().last().map(String::as_str),
        Some("b ask-test-user ask-test-password")
    );
    assert!(
        auth.statuses(Some(&project), &moved)
            .iter()
            .all(|status| !status.stored)
    );
    assert!(
        auth.statuses(Some(&project), &config())
            .iter()
            .all(|status| !status.stored)
    );
}
