use std::fs;
use std::path::PathBuf;

use rb_config::trust::{TRUST_FILE, TrustStore, load_trust_store, trust_state, update_trust_store};
use rb_ipc::{CoreToShell, NotificationLevel};

use super::*;
use crate::recorder::flow_file::DEFAULT_FLOWS_DIR;

const NOW: &str = "2026-11-19T10:00:00Z";

struct Project {
    root: tempfile::TempDir,
    settings: tempfile::TempDir,
}

impl Project {
    /// A project with two flows, `login.ts` and `old.ts`.
    fn new() -> Self {
        let root = tempfile::tempdir().expect("a project");
        let rbrowser = root.path().join(".rbrowser");
        fs::create_dir_all(rbrowser.join("flows")).expect("folders");
        fs::write(rbrowser.join("config.json"), r#"{ "version": 1 }"#).expect("config");
        fs::write(rbrowser.join("flows/login.ts"), "await page.goto(env);\n").expect("flow");
        fs::write(rbrowser.join("flows/old.ts"), "await page.goto(env);\n").expect("flow");
        Self {
            root,
            settings: tempfile::tempdir().expect("settings"),
        }
    }

    fn store(&self) -> PathBuf {
        self.settings.path().join(TRUST_FILE)
    }

    fn trust(&self) {
        let root = self.root.path();
        let hash = trust_state(root, &TrustStore::default())
            .expect("a state")
            .hash;
        update_trust_store(&self.store(), |store| store.grant(root, &hash, NOW))
            .expect("writes the store");
    }

    fn trusted(&self) -> bool {
        let store = load_trust_store(&self.store()).unwrap_or_default();
        trust_state(self.root.path(), &store).is_ok_and(|state| state.trusted)
    }

    fn delete(&self, name: &str, running: Option<&str>) -> Result<DeletedFlow, DeleteError> {
        let store = self.store();
        delete_flow(
            &Target {
                root: self.root.path(),
                dir: DEFAULT_FLOWS_DIR,
                name,
                store: Some(&store),
                now: NOW,
            },
            &running.map(str::to_owned).into_iter().collect::<Vec<_>>(),
        )
    }

    fn exists(&self, name: &str) -> bool {
        self.root.path().join(".rbrowser/flows").join(name).exists()
    }
}

#[test]
fn a_deleted_flow_is_gone_and_the_project_stays_trusted() {
    let project = Project::new();
    project.trust();
    let deleted = project.delete("old", None).expect("deleted");
    assert_eq!(deleted.path, ".rbrowser/flows/old.ts");
    assert!(
        deleted.trust_kept,
        "the person's click is the decision for this one file"
    );
    assert!(!project.exists("old.ts"));
    assert!(project.exists("login.ts"), "the other flows stay");
    assert!(
        project.trusted(),
        "the other flows run without a new trust prompt"
    );
}

#[test]
fn the_last_flow_goes_too_and_the_trust_stays() {
    let project = Project::new();
    project.trust();
    project.delete("old", None).expect("deleted");
    let deleted = project.delete("login", None).expect("deleted");
    assert!(deleted.trust_kept);
    assert!(project.trusted());
}

#[test]
fn names_that_are_no_flow_of_the_list_delete_nothing() {
    let project = Project::new();
    project.trust();
    for bad in [
        "../config",
        "flows/old",
        "",
        ".old",
        "old.",
        "a\\b",
        "C:old",
    ] {
        assert!(
            matches!(project.delete(bad, None), Err(DeleteError::BadName(_))),
            "{bad:?}"
        );
    }
    assert_eq!(
        project.delete("gone", None),
        Err(DeleteError::NotFound("gone".to_owned()))
    );
    assert!(project.exists("login.ts") && project.exists("old.ts"));
    assert!(project.root.path().join(".rbrowser/config.json").exists());
    assert!(project.trusted());
}

#[test]
fn a_folder_outside_the_flows_folder_is_refused() {
    let project = Project::new();
    let store = project.store();
    let outside = delete_flow(
        &Target {
            root: project.root.path(),
            dir: ".rbrowser",
            name: "config",
            store: Some(&store),
            now: NOW,
        },
        &[],
    );
    assert!(matches!(outside, Err(DeleteError::BadDir(_))));
    assert!(project.root.path().join(".rbrowser/config.json").exists());
}

#[test]
fn the_running_flow_waits_for_its_end() {
    let project = Project::new();
    project.trust();
    let refused = project.delete("login", Some("login"));
    assert_eq!(refused, Err(DeleteError::Running("login".to_owned())));
    let why = DeleteError::Running("login".to_owned()).to_string();
    assert!(
        why.contains("running") && why.contains("flows.cancel"),
        "{why}"
    );
    assert!(project.exists("login.ts"));
    // Another flow running is no reason.
    assert!(project.delete("old", Some("login")).is_ok());
}

#[test]
fn an_untrusted_project_stays_untrusted() {
    let project = Project::new();
    let deleted = project.delete("old", None).expect("deleted");
    assert!(!deleted.trust_kept);
    assert!(
        !project.trusted(),
        "a delete grants no trust it did not have"
    );
    assert!(
        load_trust_store(&project.store())
            .unwrap_or_default()
            .records
            .is_empty()
    );
}

#[test]
fn a_project_changed_since_its_decision_stays_untrusted() {
    // `only_this_removed` holds a change beside the delete (`recorder::flow_file`'s tests); here
    // a mock came with a pull after the decision, and the delete moves nothing.
    let project = Project::new();
    project.trust();
    let mocks = project.root.path().join(".rbrowser/mocks");
    fs::create_dir_all(&mocks).expect("folder");
    fs::write(mocks.join("evil.json"), "{}").expect("mock");
    let deleted = project.delete("old", None).expect("deleted");
    assert!(!deleted.trust_kept);
    assert!(!project.trusted());
}

#[cfg(unix)]
#[test]
fn a_linked_flows_folder_is_refused() {
    let project = Project::new();
    let real = project.root.path().join(".rbrowser/flows");
    fs::remove_dir_all(&real).expect("removes");
    let outside = tempfile::tempdir().expect("outside");
    fs::write(outside.path().join("old.ts"), "keep").expect("victim");
    std::os::unix::fs::symlink(outside.path(), &real).expect("link");
    assert!(matches!(
        project.delete("old", None),
        Err(DeleteError::Linked(_))
    ));
    assert!(outside.path().join("old.ts").exists());
}

#[test]
fn the_notice_names_the_flow_and_why_it_stayed() {
    let done = outcome_notice(
        "old",
        &Ok(DeletedFlow {
            path: ".rbrowser/flows/old.ts".to_owned(),
            trust_kept: true,
        }),
    );
    let CoreToShell::Notification(done) = done else {
        panic!("a notice");
    };
    assert_eq!(done.level, NotificationLevel::Info);
    assert_eq!(done.message_key, notice::DELETED);
    assert_eq!(done.args.get("flow").map(String::as_str), Some("old"));
    let refused = outcome_notice("login", &Err(DeleteError::Running("login".to_owned())));
    let CoreToShell::Notification(refused) = refused else {
        panic!("a notice");
    };
    assert_eq!(refused.level, NotificationLevel::Error);
    assert_eq!(refused.message_key, notice::DELETE_FAILED);
    assert!(
        refused
            .args
            .get("error")
            .is_some_and(|error| error.contains("running"))
    );
}

/// Code review, wave 090-w24: a flow a Run all still waits to start is not deleted either.
#[test]
fn a_flow_waiting_in_a_run_all_is_kept() {
    let project = Project::new();
    let store = project.store();
    std::fs::write(
        project.root.path().join(".rbrowser/flows/later.ts"),
        "// later\n",
    )
    .expect("writes a flow");
    let busy = ["first".to_owned(), "later".to_owned()];
    let kept = delete_flow(
        &Target {
            root: project.root.path(),
            dir: DEFAULT_FLOWS_DIR,
            name: "later",
            store: Some(&store),
            now: NOW,
        },
        &busy,
    );
    assert!(matches!(kept, Err(DeleteError::Running(_))));
    assert!(project.exists("later.ts"));
}
