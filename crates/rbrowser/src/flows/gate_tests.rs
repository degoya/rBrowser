use std::path::{Path, PathBuf};

use rb_config::trust::{TrustStore, trust_state, update_trust_store};

use super::*;

struct Project {
    root: tempfile::TempDir,
    settings: tempfile::TempDir,
}

impl Project {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("a project");
        let rbrowser = root.path().join(".rbrowser");
        std::fs::create_dir_all(rbrowser.join("flows")).expect("folders");
        std::fs::write(
            rbrowser.join("config.json"),
            r#"{ "version": 1, "breakpoints": [{ "id": "mobile", "width": 375, "height": 812 }] }"#,
        )
        .expect("config");
        std::fs::write(rbrowser.join("flows/login.ts"), "await page.goto(env);\n").expect("flow");
        Self {
            root,
            settings: tempfile::tempdir().expect("settings"),
        }
    }

    fn root(&self) -> &Path {
        self.root.path()
    }

    fn store(&self) -> PathBuf {
        self.settings.path().join(rb_config::trust::TRUST_FILE)
    }

    /// What the shell's dialog does: trusts the project as it is now.
    fn trust(&self) {
        let hash = trust_state(self.root(), &TrustStore::default())
            .expect("a state")
            .hash;
        update_trust_store(&self.store(), |store| {
            store.grant(self.root(), &hash, "2026-11-11T10:00:00Z");
        })
        .expect("writes the store");
    }
}

#[test]
fn an_untrusted_project_runs_no_flow() {
    let project = Project::new();
    assert_eq!(
        trusted_hash(project.root(), Some(&project.store())),
        Err(FlowError::Untrusted)
    );
    assert_eq!(
        trusted_hash(project.root(), None),
        Err(FlowError::Untrusted)
    );
    assert!(FlowError::Untrusted.to_string().contains("trust.show"));
}

#[test]
fn a_trusted_project_runs_until_a_flow_changes() {
    let project = Project::new();
    project.trust();
    let hash = trusted_hash(project.root(), Some(&project.store())).expect("trusted");
    assert!(still_trusted(project.root(), Some(&project.store()), &hash).is_ok());
    std::fs::write(
        project.root().join(".rbrowser/flows/login.ts"),
        "await page.goto('https://evil.test');\n",
    )
    .expect("changes the flow");
    assert_eq!(
        still_trusted(project.root(), Some(&project.store()), &hash),
        Err(FlowError::Untrusted)
    );
    assert_eq!(
        trusted_hash(project.root(), Some(&project.store())),
        Err(FlowError::Untrusted)
    );
}

#[test]
fn a_broken_store_trusts_nothing() {
    let project = Project::new();
    std::fs::write(project.store(), "{ not json").expect("writes");
    assert_eq!(
        trusted_hash(project.root(), Some(&project.store())),
        Err(FlowError::Untrusted)
    );
}
