use std::fs;

use rb_config::trust::{TrustStore, load_trust_store, trust_state, update_trust_store};

use super::*;

const NOW: &str = "2026-11-17T10:00:00Z";

struct Project {
    root: tempfile::TempDir,
    settings: tempfile::TempDir,
}

impl Project {
    /// A project with one flow, `login.ts`.
    fn new() -> Self {
        let root = tempfile::tempdir().expect("a project");
        let rbrowser = root.path().join(".rbrowser");
        fs::create_dir_all(rbrowser.join("flows")).expect("folders");
        fs::write(rbrowser.join("config.json"), r#"{ "version": 1 }"#).expect("config");
        fs::write(rbrowser.join("flows/login.ts"), "await page.goto(env);\n").expect("flow");
        Self {
            root,
            settings: tempfile::tempdir().expect("settings"),
        }
    }

    fn store(&self) -> PathBuf {
        self.settings.path().join(rb_config::trust::TRUST_FILE)
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

    fn write(&self, name: &str, source: &str, overwrite: bool) -> Result<SavedFlow, FlowFileError> {
        let store = self.store();
        write_flow(
            &Target {
                root: self.root.path(),
                dir: DEFAULT_FLOWS_DIR,
                name,
                store: Some(&store),
                now: NOW,
            },
            source,
            overwrite,
        )
    }

    fn read(&self, name: &str) -> String {
        fs::read_to_string(self.root.path().join(".rbrowser/flows").join(name)).expect("the flow")
    }
}

#[test]
fn a_persons_flow_goes_into_the_flows_folder_and_keeps_the_trust() {
    let project = Project::new();
    project.trust();
    let saved = project
        .write("checkout", "await page.goto(env);\n", false)
        .expect("written");
    assert_eq!(saved.path, ".rbrowser/flows/checkout.ts");
    assert!(
        saved.trust_kept,
        "the person's click is the decision for this one file"
    );
    assert_eq!(project.read("checkout.ts"), "await page.goto(env);\n");
    assert!(project.trusted());
}

#[test]
fn an_existing_flow_is_replaced_only_with_the_persons_word() {
    let project = Project::new();
    project.trust();
    let refused = project.write("login", "// new\n", false);
    assert_eq!(
        refused,
        Err(FlowFileError::Exists(".rbrowser/flows/login.ts".to_owned()))
    );
    assert_eq!(project.read("login.ts"), "await page.goto(env);\n");
    let saved = project.write("login", "// new\n", true).expect("replaced");
    assert!(saved.trust_kept);
    assert_eq!(project.read("login.ts"), "// new\n");
    assert!(project.trusted());
}

#[test]
fn an_untrusted_project_gets_the_file_and_asks_as_before() {
    let project = Project::new();
    let saved = project
        .write("checkout", "// flow\n", false)
        .expect("written");
    assert!(!saved.trust_kept);
    assert!(
        !project.trusted(),
        "a flow export grants no trust it did not have"
    );
    assert!(
        load_trust_store(&project.store())
            .unwrap_or_default()
            .records
            .is_empty()
    );
}

#[test]
fn a_bad_name_or_folder_writes_nothing() {
    let project = Project::new();
    assert!(matches!(
        project.write("../x", "", false),
        Err(FlowFileError::BadName(_))
    ));
    let store = project.store();
    let outside = write_flow(
        &Target {
            root: project.root.path(),
            dir: ".rbrowser/out",
            name: "x",
            store: Some(&store),
            now: NOW,
        },
        "",
        false,
    );
    assert!(matches!(outside, Err(FlowFileError::BadDir(_))));
    assert!(!project.root.path().join(".rbrowser/out/x.ts").exists());
}

fn state(paths: &[&str], unpinned: &[&str]) -> TrustState {
    TrustState {
        trusted: true,
        hash: "h".to_owned(),
        blocked: paths
            .iter()
            .map(|path| BlockedItem {
                kind: ProtectedKind::Flows,
                path: (*path).to_owned(),
            })
            .collect(),
        unpinned: unpinned.iter().map(|path| (*path).to_owned()).collect(),
    }
}

#[test]
fn only_the_one_file_is_trusted_along_never_another_change() {
    let before = state(&[".rbrowser/flows/login.ts"], &[]);
    let one_more = state(
        &[".rbrowser/flows/checkout.ts", ".rbrowser/flows/login.ts"],
        &[],
    );
    assert!(only_this_changed(
        &before,
        &one_more,
        ".rbrowser/flows/checkout.ts"
    ));
    // The same set: the file replaced an existing one.
    assert!(only_this_changed(
        &before,
        &before,
        ".rbrowser/flows/login.ts"
    ));
    let and_a_mock = state(
        &[
            ".rbrowser/flows/checkout.ts",
            ".rbrowser/flows/login.ts",
            ".rbrowser/mocks/evil.json",
        ],
        &[],
    );
    assert!(!only_this_changed(
        &before,
        &and_a_mock,
        ".rbrowser/flows/checkout.ts"
    ));
    let gone = state(&[".rbrowser/flows/checkout.ts"], &[]);
    assert!(!only_this_changed(
        &before,
        &gone,
        ".rbrowser/flows/checkout.ts"
    ));
    let linked = state(
        &[".rbrowser/flows/checkout.ts", ".rbrowser/flows/login.ts"],
        &[".rbrowser/flows/out"],
    );
    assert!(!only_this_changed(
        &before,
        &linked,
        ".rbrowser/flows/checkout.ts"
    ));
}

#[test]
fn a_flow_in_a_subfolder_counts_by_its_top_level_entry() {
    assert_eq!(
        top_level_entry(".rbrowser/flows/team/checkout.ts"),
        ".rbrowser/flows/team"
    );
    assert_eq!(
        top_level_entry(".rbrowser/flows/checkout.ts"),
        ".rbrowser/flows/checkout.ts"
    );
}

#[test]
fn the_mcp_export_has_no_way_into_the_flows_folder() {
    // `recorder_export` (MCP, `export::recorder_export`) writes only through `write_export` into
    // `automation.recorderExport.dir`, which may not lie in a protected folder; only the shell's
    // panel request reaches this module.
    for source in [include_str!("export.rs"), include_str!("mcp.rs")] {
        assert!(
            !source.contains("flow_file"),
            "the MCP path names flow_file"
        );
    }
    let root = tempfile::tempdir().expect("a project");
    fs::create_dir_all(root.path().join(".rbrowser/flows")).expect("folders");
    let protected: Vec<&str> = rb_config::trust::PROTECTED_DIRS
        .iter()
        .map(|(_, name)| *name)
        .collect();
    assert!(
        rb_sync::recorder::ExportsDir::open(root.path(), ".rbrowser/flows", &protected).is_err()
    );
}

#[cfg(unix)]
#[test]
fn a_linked_flows_folder_or_rbrowser_folder_is_refused() {
    // A repository can ship `.rbrowser/flows` or `.rbrowser` as a link out of the project.
    for linked in [".rbrowser/flows", ".rbrowser"] {
        let project = Project::new();
        let real = project.root.path().join(linked);
        fs::remove_dir_all(&real).expect("removes");
        let outside = tempfile::tempdir().expect("outside");
        std::os::unix::fs::symlink(outside.path(), &real).expect("link");
        assert!(
            matches!(
                project.write("checkout", "await page.goto(env);\n", false),
                Err(FlowFileError::Linked(_))
            ),
            "{linked}"
        );
        assert!(
            fs::read_dir(outside.path())
                .expect("listed")
                .next()
                .is_none(),
            "nothing written through {linked}"
        );
    }
}

#[cfg(unix)]
#[test]
fn a_flow_file_that_is_a_link_is_not_written_through() {
    // "Replace" on a flow that is a link to a file elsewhere leaves that file alone.
    let project = Project::new();
    let outside = tempfile::tempdir().expect("outside");
    let target = outside.path().join("victim.txt");
    fs::write(&target, "keep").expect("victim");
    let flow = project.root.path().join(".rbrowser/flows/checkout.ts");
    std::os::unix::fs::symlink(&target, &flow).expect("link");
    assert!(matches!(
        project.write("checkout", "await page.goto(env);\n", true),
        Err(FlowFileError::Linked(_))
    ));
    assert_eq!(fs::read_to_string(&target).expect("reads"), "keep");
}
