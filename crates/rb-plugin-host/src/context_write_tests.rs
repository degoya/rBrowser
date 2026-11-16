//! `project-write` (RB-110-13): only the manifest's write globs, never `.git/` or `.rbrowser/`,
//! only in a trusted workspace, atomically, and only over the bytes the plugin read - a file
//! changed on disk is a conflict, never overwritten.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use rb_plugin_types::codes;

use super::PluginContext;
use crate::manifest::Grants;
use crate::test_support::ScriptedServices;

/// Writing needs the read glob too (security review RB-110-13, finding 2).
const WRITE: &str =
    "project-read = { globs = [\"**/*.todo\"] }\nproject-write = { globs = [\"**/*.todo\"] }";

fn grants(table: &str) -> Grants {
    Grants::parse(&toml::from_str(table).expect("toml")).expect("grants")
}

/// A plugin with `table` in a project at `root`, trusted or not.
fn plugin(table: &str, root: &Path, trusted: bool) -> PluginContext {
    let services = Arc::new(ScriptedServices::default());
    services.trusted.store(trusted, Ordering::SeqCst);
    PluginContext::new("sample", "1.0.0", grants(table), services)
        .with_project(Some(root.to_path_buf()))
}

fn project() -> tempfile::TempDir {
    let project = tempfile::tempdir().expect("tempdir");
    std::fs::write(project.path().join("TODO.todo"), b"old\n").expect("write");
    project
}

fn names(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(folder)
        .expect("dir")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

#[tokio::test]
async fn an_untrusted_workspace_is_never_written() {
    let project = project();
    let context = plugin(WRITE, project.path(), false);
    let refused = context
        .project_write("TODO.todo", b"new\n", Some(b"old\n"))
        .await
        .expect_err("untrusted");
    assert_eq!(refused.code(), Some(codes::UNTRUSTED_WORKSPACE));
    assert_eq!(
        std::fs::read(project.path().join("TODO.todo")).expect("read"),
        b"old\n"
    );
}

#[tokio::test]
async fn a_file_still_as_read_is_replaced_atomically() {
    let project = project();
    let context = plugin(WRITE, project.path(), true);
    context
        .project_write("TODO.todo", b"new\n", Some(b"old\n"))
        .await
        .expect("written");
    assert_eq!(
        std::fs::read(project.path().join("TODO.todo")).expect("read"),
        b"new\n"
    );
    // No temporary file stays behind.
    assert_eq!(names(project.path()), ["TODO.todo"]);
}

#[tokio::test]
async fn a_file_changed_on_disk_is_a_conflict_and_stays_as_it_is() {
    let project = project();
    let context = plugin(WRITE, project.path(), true);
    let conflict = context
        .project_write("TODO.todo", b"mine\n", Some(b"what I read\n"))
        .await
        .expect_err("changed");
    assert_eq!(conflict.code(), Some(codes::WRITE_CONFLICT));
    assert_eq!(conflict.param("path"), Some("TODO.todo"));
    assert_eq!(
        std::fs::read(project.path().join("TODO.todo")).expect("read"),
        b"old\n"
    );
    // A new file must not exist yet; one that does is a conflict too.
    let exists = context
        .project_write("TODO.todo", b"mine\n", None)
        .await
        .expect_err("exists");
    assert_eq!(exists.code(), Some(codes::WRITE_CONFLICT));
    // A file the plugin read that is gone now is one as well.
    let gone = context
        .project_write("docs.todo", b"x", Some(b"y"))
        .await
        .expect_err("gone");
    assert_eq!(gone.code(), Some(codes::WRITE_CONFLICT));
    assert_eq!(names(project.path()), ["TODO.todo"]);
}

#[tokio::test]
async fn a_new_file_is_created_in_an_existing_folder() {
    let project = project();
    std::fs::create_dir(project.path().join("docs")).expect("dir");
    let context = plugin(WRITE, project.path(), true);
    context
        .project_write("docs/plan.todo", b"\xe2\x98\x90 a\n", None)
        .await
        .expect("created");
    assert_eq!(
        std::fs::read(project.path().join("docs/plan.todo")).expect("read"),
        "\u{2610} a\n".as_bytes()
    );
    // No folder is made on the way.
    let refused = context
        .project_write("new/plan.todo", b"x", None)
        .await
        .expect_err("no folder");
    assert_eq!(refused.code(), Some(codes::NOT_FOUND));
    assert!(!project.path().join("new").exists());
}

#[tokio::test]
async fn only_the_write_globs_are_written_never_git_or_the_config() {
    let project = project();
    for folder in [".git", ".rbrowser", "sub/.GIT"] {
        std::fs::create_dir_all(project.path().join(folder)).expect("dir");
    }
    std::fs::write(project.path().join("main.rs"), b"fn main() {}").expect("write");
    let context = plugin(WRITE, project.path(), true);
    for path in [
        "main.rs",
        "TODO.todo.bak",
        ".git/x.todo",
        "sub/.GIT/x.todo",
        ".rbrowser/x.todo",
        ".RBROWSER/x.todo",
    ] {
        let refused = context
            .project_write(path, b"x", None)
            .await
            .expect_err(path);
        assert_eq!(refused.code(), Some(codes::PATH_NOT_WRITABLE), "{path}");
    }
    for path in [
        "../x.todo",
        "/x.todo",
        "C:\\x.todo",
        "a/../../x.todo",
        "TODO~1.TOD",
    ] {
        let refused = context
            .project_write(path, b"x", None)
            .await
            .expect_err(path);
        assert_eq!(refused.code(), Some(codes::PATH_OUTSIDE), "{path}");
    }
    assert!(names(&project.path().join(".git")).is_empty());
    assert!(names(&project.path().join(".rbrowser")).is_empty());
    assert_eq!(
        std::fs::read(project.path().join("main.rs")).expect("read"),
        b"fn main() {}"
    );
}

#[tokio::test]
async fn reading_grants_no_writing_and_too_much_is_refused() {
    let project = project();
    let reader = plugin(
        "project-read = { globs = [\"**/*.todo\"] }",
        project.path(),
        true,
    );
    let refused = reader
        .project_write("TODO.todo", b"x", Some(b"old\n"))
        .await
        .expect_err("not granted");
    assert_eq!(refused.code(), Some(codes::CAPABILITY_DENIED));

    let context = plugin(WRITE, project.path(), true);
    let big = vec![b'x'; rb_plugin_api::MAX_IO_BYTES + 1];
    let refused = context
        .project_write("TODO.todo", &big, Some(b"old\n"))
        .await
        .expect_err("too large");
    assert_eq!(refused.code(), Some(codes::TOO_LARGE));
}

#[cfg(unix)]
#[tokio::test]
async fn a_link_is_never_written_through() {
    let project = project();
    let elsewhere = tempfile::tempdir().expect("tempdir");
    let target = elsewhere.path().join("secret.todo");
    std::fs::write(&target, b"old\n").expect("write");
    std::os::unix::fs::symlink(&target, project.path().join("linked.todo")).expect("link");
    std::os::unix::fs::symlink(elsewhere.path(), project.path().join("folder")).expect("link");
    let context = plugin(WRITE, project.path(), true);
    for path in ["linked.todo", "folder/secret.todo", "folder/new.todo"] {
        let refused = context
            .project_write(path, b"new\n", Some(b"old\n"))
            .await
            .expect_err(path);
        assert_eq!(refused.code(), Some(codes::PATH_OUTSIDE), "{path}");
    }
    assert_eq!(std::fs::read(&target).expect("read"), b"old\n");
    assert_eq!(names(elsewhere.path()), ["secret.todo"]);
}

#[cfg(unix)]
#[tokio::test]
async fn a_hard_link_elsewhere_keeps_its_bytes() {
    // The rename replaces the project's name; a second name of the same file outside the
    // project is not written through.
    let project = project();
    let elsewhere = tempfile::tempdir().expect("tempdir");
    let other = elsewhere.path().join("other.todo");
    std::fs::hard_link(project.path().join("TODO.todo"), &other).expect("hard link");
    let context = plugin(WRITE, project.path(), true);
    context
        .project_write("TODO.todo", b"new\n", Some(b"old\n"))
        .await
        .expect("written");
    assert_eq!(std::fs::read(&other).expect("read"), b"old\n");
}

#[tokio::test]
async fn an_empty_folder_lists_the_root() {
    let project = project();
    std::fs::create_dir_all(project.path().join("docs")).expect("dir");
    std::fs::create_dir_all(project.path().join(".rbrowser")).expect("dir");
    std::fs::write(project.path().join("main.rs"), b"").expect("write");
    std::fs::write(project.path().join(".rbrowser/config.local.json"), b"{}").expect("write");
    let context = plugin(
        "project-read = { globs = [\"**/*.todo\"] }",
        project.path(),
        false,
    );
    assert_eq!(
        context.project_list("").await.expect("root"),
        [".rbrowser/", "TODO.todo", "docs/"]
    );
    let narrow = plugin(
        "project-read = { globs = [\"docs/*.todo\"] }",
        project.path(),
        false,
    );
    assert_eq!(
        narrow.project_list("").await.expect("root"),
        [".rbrowser/", "docs/"]
    );
}

#[test]
fn a_write_grant_without_its_read_glob_does_not_load() {
    // A file it may not read is never written, so `expected` tells nothing about it: the
    // manifest that would ask for that is refused before any call (code review RB-110-13); the
    // host's write check refuses such a path besides.
    let table = "project-write = { globs = [\"**/*.todo\"] }";
    assert!(Grants::parse(&toml::from_str(table).expect("toml")).is_err());
}

#[tokio::test]
async fn a_read_only_file_is_refused_and_kept() {
    let project = project();
    let file = project.path().join("TODO.todo");
    let mut permissions = std::fs::metadata(&file).expect("meta").permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&file, permissions).expect("read-only");
    let context = plugin(WRITE, project.path(), true);
    let refused = context
        .project_write("TODO.todo", b"new\n", Some(b"old\n"))
        .await
        .expect_err("read-only");
    assert_eq!(refused.code(), Some(codes::PATH_NOT_WRITABLE));
    assert_eq!(std::fs::read(&file).expect("read"), b"old\n");
    assert_eq!(names(project.path()), ["TODO.todo"]);
}

#[cfg(unix)]
#[tokio::test]
async fn the_new_file_keeps_the_old_ones_permissions() {
    use std::os::unix::fs::PermissionsExt as _;
    let project = project();
    let file = project.path().join("TODO.todo");
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).expect("mode");
    let context = plugin(WRITE, project.path(), true);
    context
        .project_write("TODO.todo", b"new\n", Some(b"old\n"))
        .await
        .expect("written");
    let mode = std::fs::metadata(&file).expect("meta").permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
}

#[cfg(unix)]
#[tokio::test]
async fn a_file_the_user_may_not_write_is_refused_though_its_folder_is_writable() {
    // The rename would replace a file only the folder lets us replace (another owner's 0644, or
    // a deny-write ACL on Windows): the host asks the file itself first (code review RB-110-13).
    use std::os::unix::fs::PermissionsExt as _;
    let project = project();
    let file = project.path().join("TODO.todo");
    // Owner r--, group -w-: not read-only by the mode, but not writable for its owner.
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o420)).expect("mode");
    if std::fs::OpenOptions::new().write(true).open(&file).is_ok() {
        // Root writes any file: this state cannot be made, nothing to test.
        return;
    }
    let context = plugin(WRITE, project.path(), true);
    let refused = context
        .project_write("TODO.todo", b"new\n", Some(b"old\n"))
        .await
        .expect_err("not writable");
    assert_eq!(refused.code(), Some(codes::PATH_NOT_WRITABLE));
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).expect("mode");
    assert_eq!(std::fs::read(&file).expect("read"), b"old\n");
    assert_eq!(names(project.path()), ["TODO.todo"]);
}

#[tokio::test]
async fn a_long_name_the_file_system_takes_is_written() {
    // The temporary file's name does not repeat the file's, so a name near the file system's
    // limit (255 bytes) is written, not refused as too long (code review RB-110-13).
    let project = project();
    let name = format!("{}.todo", "n".repeat(245));
    std::fs::write(project.path().join(&name), b"old\n").expect("write");
    let context = plugin(WRITE, project.path(), true);
    context
        .project_write(&name, b"new\n", Some(b"old\n"))
        .await
        .expect("written");
    assert_eq!(
        std::fs::read(project.path().join(&name)).expect("read"),
        b"new\n"
    );
}
