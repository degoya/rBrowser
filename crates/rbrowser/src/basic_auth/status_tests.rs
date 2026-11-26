use std::sync::Arc;

use rb_config::UserSettings;
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
use crate::host::Host;
use crate::host::state::default_config;
use crate::host::ui::recording::{RecordingUi, UiCall};

fn core() -> (Arc<RecordingUi>, Arc<Core>) {
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(default_config(), None).expect("opens");
    let setup = CoreSetup {
        user: UserSettings::default(),
        ..CoreSetup::default()
    };
    let core = Core::new(host, ui.clone(), Handle::current(), setup);
    (ui, core)
}

/// RB-094-29: every environment is answered; without a project none has a credential.
#[tokio::test]
async fn the_shell_asks_for_every_environment() {
    let (_ui, core) = core();
    let answer = ipc(&core, &ShellToCore::GetBasicAuthStatus).expect("this module's");
    let expected: Vec<_> = core
        .host()
        .config()
        .environments
        .iter()
        .map(|environment| rb_ipc::EnvironmentAuthStatus::none(&environment.id))
        .collect();
    assert_eq!(
        answer,
        Ok(CoreToShell::BasicAuthStatus {
            environments: expected,
            store: crate::basic_auth::store::kind(),
        })
    );
    assert!(ipc(&core, &ShellToCore::GetState).is_none());
}

#[tokio::test]
async fn a_change_reaches_every_shell_page() {
    let (ui, core) = core();
    push(&core);
    assert!(
        ui.calls().iter().any(
            |call| matches!(call, UiCall::Shell(script) if script.contains("basicAuthStatus"))
        ),
        "the status is sent to the shell"
    );
}

/// Review w35: the store may wait for an unlock prompt, so the status and the dialog's answer
/// are answered on a blocking thread, never on the thread the query came in on.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_store_is_never_read_on_the_query_thread() {
    let (_ui, core) = core();
    assert!(is_off_thread(&ShellToCore::GetBasicAuthStatus));
    assert!(is_off_thread(&ShellToCore::AuthPromptReply(
        rb_ipc::AuthPromptReply {
            id: 1,
            sign_in: None
        }
    )));
    assert!(!is_off_thread(&ShellToCore::GetState));
    let (sent, answered) = tokio::sync::oneshot::channel();
    let caller = std::thread::current().id();
    dispatch_off_thread(&core, ShellToCore::GetBasicAuthStatus, move |answer| {
        let _ = sent.send((answer, std::thread::current().id()));
    });
    let (answer, thread) = answered.await.expect("answered");
    assert!(matches!(answer, Ok(CoreToShell::BasicAuthStatus { .. })));
    assert_ne!(thread, caller);
}
