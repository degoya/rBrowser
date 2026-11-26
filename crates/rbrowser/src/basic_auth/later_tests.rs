use std::sync::{Arc, Mutex};

use rb_engine::ViewId;

use super::*;
use crate::basic_auth::ask::tests::{Got, config, core_at, root, shell_scripts, sign_in, site};
use crate::basic_auth::store::{Credential, MemoryStore};
use rb_ipc::CoreToShell;

/// The jobs `answer_later` hands to the blocking pool, run when the test says.
#[derive(Clone, Default)]
struct Jobs(Arc<Mutex<Vec<Job>>>);

impl Jobs {
    fn spawn(&self) -> impl FnOnce(Job) + use<> {
        let jobs = self.clone();
        move |job| rb_util::lock(&jobs.0).push(job)
    }

    fn run(&self) -> usize {
        let jobs = std::mem::take(&mut *rb_util::lock(&self.0));
        let count = jobs.len();
        jobs.into_iter().for_each(|job| job());
        count
    }
}

fn owned(challenge: &Challenge<'_>) -> OwnedChallenge {
    OwnedChallenge {
        origin_url: challenge.origin_url.to_owned(),
        is_proxy: challenge.is_proxy,
        host: challenge.host.to_owned(),
        port: challenge.port,
        realm: challenge.realm.to_owned(),
        scheme: challenge.scheme.to_owned(),
    }
}

/// RB-099-05/RB-099-04: the store may block (a locked keyring asks first), so CEF's IO thread
/// only hands the challenge over and returns; the stored credential answers when the job ran.
#[tokio::test]
async fn the_handler_returns_at_once_and_the_store_answers_later() {
    let (_ui, core) = core_at(Some(root()));
    let auth: &'static BasicAuth =
        Box::leak(Box::new(BasicAuth::new(Box::new(MemoryStore::default()))));
    let asking: &'static Asking = Box::leak(Box::default());
    let project = crate::instance::file::key(&root());
    let stored = Credential::new("stored-user", "stored-password-1").expect("valid");
    auth.set(&project, &config(), "staging", &stored)
        .expect("stored");
    let got = Got::default();
    let jobs = Jobs::default();
    let staging = Challenge {
        origin_url: "https://staging.example.test/shop/",
        ..site("staging.example.test")
    };
    answer_later(
        Arc::clone(&core),
        auth,
        asking,
        Requester::View(ViewId(1)),
        11,
        owned(&staging),
        got.waiter("a"),
        jobs.spawn(),
    );
    assert!(
        got.list().is_empty(),
        "nothing is read or answered on CEF's IO thread"
    );
    assert_eq!(jobs.run(), 1);
    assert_eq!(got.list(), vec!["a stored-user stored-password-1"]);
}

/// Without a stored credential the job asks the person; the answer comes from the dialog.
#[tokio::test]
async fn without_a_stored_credential_the_job_asks_the_person() {
    let (ui, core) = core_at(Some(root()));
    let auth: &'static BasicAuth =
        Box::leak(Box::new(BasicAuth::new(Box::new(MemoryStore::default()))));
    let asking: &'static Asking = Box::leak(Box::default());
    let got = Got::default();
    let jobs = Jobs::default();
    answer_later(
        Arc::clone(&core),
        auth,
        asking,
        Requester::View(ViewId(1)),
        11,
        owned(&site("dev.example.test")),
        got.waiter("a"),
        jobs.spawn(),
    );
    assert!(
        shell_scripts(&ui).is_empty(),
        "not asked before the store said"
    );
    jobs.run();
    let scripts = shell_scripts(&ui);
    assert_eq!(scripts.len(), 1);
    assert!(got.list().is_empty(), "waits for the person");
    // The first question of a fresh `Asking` is number 1.
    let id = 1;
    assert!(rb_util::lock(&asking.prompts).environment(id).is_some());
    assert_eq!(
        asking.reply(&core, auth, &sign_in(id, false)),
        Ok(CoreToShell::Ack)
    );
    assert_eq!(got.list(), vec!["a ask-test-user ask-test-password"]);
}

/// Review w35: a windowless render view (a sitemap capture) is answered from the store or not at
/// all - nobody watches it, so nobody is asked.
#[tokio::test]
async fn a_render_view_is_never_asked() {
    let (ui, core) = core_at(Some(root()));
    let auth: &'static BasicAuth =
        Box::leak(Box::new(BasicAuth::new(Box::new(MemoryStore::default()))));
    let asking: &'static Asking = Box::leak(Box::default());
    let got = Got::default();
    let jobs = Jobs::default();
    answer_later(
        Arc::clone(&core),
        auth,
        asking,
        Requester::Render,
        21,
        owned(&site("dev.example.test")),
        got.waiter("render"),
        jobs.spawn(),
    );
    jobs.run();
    assert_eq!(got.list(), vec!["render cancelled"]);
    assert!(shell_scripts(&ui).is_empty());
    assert!(rb_util::lock(&asking.prompts).is_empty());
}

/// Review w35: a grid view's popup is asked in the bar, named by the view that opened it.
#[tokio::test]
async fn a_popup_is_asked_under_its_opener() {
    let (ui, core) = core_at(Some(root()));
    let auth: &'static BasicAuth =
        Box::leak(Box::new(BasicAuth::new(Box::new(MemoryStore::default()))));
    let asking: &'static Asking = Box::leak(Box::default());
    let got = Got::default();
    let jobs = Jobs::default();
    answer_later(
        Arc::clone(&core),
        auth,
        asking,
        Requester::Popup(ViewId(2)),
        31,
        owned(&site("dev.example.test")),
        got.waiter("popup"),
        jobs.spawn(),
    );
    jobs.run();
    let scripts = shell_scripts(&ui);
    assert_eq!(scripts.len(), 1);
    assert!(scripts[0].contains("\"views\":[2]"), "{}", scripts[0]);
    assert!(got.list().is_empty(), "waits for the person");
}

/// Review w35: the view closes while its store read runs - its request is cancelled and never
/// opens a question for a dead browser.
#[tokio::test]
async fn a_view_closed_during_the_store_read_is_not_asked() {
    let (ui, core) = core_at(Some(root()));
    let auth: &'static BasicAuth =
        Box::leak(Box::new(BasicAuth::new(Box::new(MemoryStore::default()))));
    let asking: &'static Asking = Box::leak(Box::default());
    let got = Got::default();
    let jobs = Jobs::default();
    answer_later(
        Arc::clone(&core),
        auth,
        asking,
        Requester::View(ViewId(1)),
        11,
        owned(&site("dev.example.test")),
        got.waiter("a"),
        jobs.spawn(),
    );
    asking.view_closed(&core, 11);
    jobs.run();
    assert_eq!(got.list(), vec!["a cancelled"]);
    assert!(shell_scripts(&ui).is_empty());
    assert!(rb_util::lock(&asking.prompts).is_empty());
}
