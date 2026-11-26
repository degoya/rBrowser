//! `project-range` and `out-stream` (RB-099-24): `project.read`'s and `out.write`'s rules, one
//! call's limit per call, and a streamed file that shows whole or not at all.

use std::path::Path;
use std::sync::Arc;

use rb_plugin_api::MAX_IO_BYTES;
use rb_plugin_types::codes;

use crate::context::PluginContext;
use crate::manifest::Grants;
use crate::test_support::ScriptedServices;

const BOTH: &str = "project-read = true\nout-write = true";

fn plugin(table: &str, root: &Path) -> PluginContext {
    let grants = Grants::parse(&toml::from_str(table).expect("toml")).expect("grants");
    PluginContext::new(
        "sample",
        "1.0.0",
        grants,
        Arc::new(ScriptedServices::default()),
    )
    .with_project(Some(root.to_path_buf()))
}

/// The plugin's out folder, made.
fn out_folder(root: &Path) -> std::path::PathBuf {
    let folder = root.join(".rbrowser/out/sample");
    std::fs::create_dir_all(&folder).expect("folder");
    folder
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
async fn a_file_over_one_read_is_read_in_ranges() {
    let project = tempfile::tempdir().expect("tempdir");
    let out = project.path().join(".rbrowser/out");
    std::fs::create_dir_all(&out).expect("folder");
    let big: Vec<u8> = (0..MAX_IO_BYTES + 100).map(|i| (i % 251) as u8).collect();
    std::fs::write(out.join("shot.png"), &big).expect("write");
    let context = plugin(BOTH, project.path());
    let whole = context
        .project_read(".rbrowser/out/shot.png")
        .await
        .expect_err("over one read");
    assert_eq!(whole.code(), Some(codes::TOO_LARGE));

    let at = MAX_IO_BYTES as u64 - 10;
    let (bytes, size) = context
        .project_read_range(".rbrowser/out/shot.png", at, 1000)
        .await
        .expect("ranged");
    assert_eq!(size, big.len() as u64);
    assert_eq!(bytes, big[MAX_IO_BYTES - 10..], "fewer at the end");
    let (none, _) = context
        .project_read_range(".rbrowser/out/shot.png", size + 5, 10)
        .await
        .expect("past the end");
    assert!(none.is_empty());
    let over = u32::try_from(MAX_IO_BYTES + 1).expect("fits");
    let refused = context
        .project_read_range(".rbrowser/out/shot.png", 0, over)
        .await
        .expect_err("one call's limit");
    assert_eq!(refused.code(), Some(codes::TOO_LARGE));
}

#[tokio::test]
async fn a_range_holds_the_read_rules() {
    let project = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(project.path().join(".rbrowser")).expect("folder");
    std::fs::write(project.path().join(".rbrowser/config.local.json"), b"{}").expect("write");
    std::fs::write(project.path().join("secret.txt"), b"key").expect("write");
    let context = plugin(BOTH, project.path());
    for (path, code) in [
        ("../outside.png", codes::PATH_OUTSIDE),
        (".rbrowser/../secret.txt", codes::PATH_OUTSIDE),
        (".rbrowser/config.local.json", codes::PATH_NOT_GRANTED),
        ("secret.txt", codes::PATH_NOT_GRANTED),
    ] {
        let refused = context
            .project_read_range(path, 0, 16)
            .await
            .expect_err(path);
        assert_eq!(refused.code(), Some(code), "{path}");
    }
    let bare = plugin("out-write = true", project.path());
    let refused = bare
        .project_read_range(".rbrowser/a.json", 0, 1)
        .await
        .expect_err("no project-read");
    assert_eq!(refused.code(), Some(codes::CAPABILITY_DENIED));
}

#[cfg(unix)]
#[tokio::test]
async fn a_range_follows_no_link() {
    let project = tempfile::tempdir().expect("tempdir");
    let elsewhere = tempfile::tempdir().expect("tempdir");
    std::fs::write(elsewhere.path().join("key"), b"secret").expect("write");
    let rbrowser = project.path().join(".rbrowser");
    std::fs::create_dir_all(&rbrowser).expect("folder");
    std::os::unix::fs::symlink(elsewhere.path().join("key"), rbrowser.join("key.png"))
        .expect("link");
    std::os::unix::fs::symlink(elsewhere.path(), rbrowser.join("out")).expect("link");
    let context = plugin(BOTH, project.path());
    for path in [".rbrowser/key.png", ".rbrowser/out/key"] {
        let refused = context
            .project_read_range(path, 0, 6)
            .await
            .expect_err(path);
        assert_eq!(refused.code(), Some(codes::PATH_OUTSIDE), "{path}");
    }
}

#[tokio::test]
async fn a_streamed_file_appears_whole_at_the_commit() {
    let project = tempfile::tempdir().expect("tempdir");
    let folder = out_folder(project.path());
    std::fs::create_dir_all(folder.join("edits")).expect("folder");
    std::fs::write(folder.join("edits/a.png"), b"old").expect("write");
    let context = plugin(BOTH, project.path());
    let mut staged = context
        .out_stream_begin("edits/a.png")
        .await
        .expect("begun");
    context
        .out_stream_append(&mut staged, b"first,")
        .await
        .expect("appended");
    context
        .out_stream_append(&mut staged, b"second")
        .await
        .expect("appended");
    // Until the commit the target holds the old file; the parts lie in one hidden file.
    assert_eq!(
        std::fs::read(folder.join("edits/a.png")).expect("read"),
        b"old"
    );
    let during = names(&folder.join("edits"));
    assert_eq!(during.len(), 2, "{during:?}");
    assert!(
        during
            .iter()
            .any(|name| name.starts_with('.') && name.ends_with(".rbrowser-tmp")),
        "{during:?}"
    );
    context.out_stream_commit(staged).await.expect("committed");
    assert_eq!(
        std::fs::read(folder.join("edits/a.png")).expect("read"),
        b"first,second"
    );
    assert_eq!(names(&folder.join("edits")), ["a.png"]);
}

#[tokio::test]
async fn an_abandoned_stream_leaves_nothing() {
    let project = tempfile::tempdir().expect("tempdir");
    let folder = out_folder(project.path());
    std::fs::write(folder.join("kept.png"), b"old").expect("write");
    let context = plugin(BOTH, project.path());
    // Dropped, as the end of a plugin call drops the store holding it.
    let mut staged = context.out_stream_begin("kept.png").await.expect("begun");
    context
        .out_stream_append(&mut staged, b"half of it")
        .await
        .expect("appended");
    drop(staged);
    let fresh = context.out_stream_begin("new.png").await.expect("begun");
    drop(fresh);
    assert_eq!(names(&folder), ["kept.png"]);
    assert_eq!(
        std::fs::read(folder.join("kept.png")).expect("read"),
        b"old"
    );
}

#[tokio::test]
async fn a_stream_holds_the_out_rules_and_one_calls_limit() {
    let project = tempfile::tempdir().expect("tempdir");
    let folder = out_folder(project.path());
    let context = plugin(BOTH, project.path());
    for path in ["../escape.png", "/etc/passwd", "a/../../b.png"] {
        let refused = context.out_stream_begin(path).await.expect_err(path);
        assert_eq!(refused.code(), Some(codes::PATH_OUTSIDE), "{path}");
    }
    let mut staged = context.out_stream_begin("big.png").await.expect("begun");
    let over = vec![0u8; MAX_IO_BYTES + 1];
    let refused = context
        .out_stream_append(&mut staged, &over)
        .await
        .expect_err("one call's limit");
    assert_eq!(refused.code(), Some(codes::TOO_LARGE));
    drop(staged);
    assert!(names(&folder).is_empty());
    let read_only = plugin("project-read = true", project.path());
    let refused = read_only
        .out_stream_begin("a.png")
        .await
        .expect_err("no out-write");
    assert_eq!(refused.code(), Some(codes::CAPABILITY_DENIED));
}

#[cfg(unix)]
#[tokio::test]
async fn a_stream_never_writes_through_a_link() {
    let project = tempfile::tempdir().expect("tempdir");
    let elsewhere = tempfile::tempdir().expect("tempdir");
    let folder = out_folder(project.path());
    std::os::unix::fs::symlink(elsewhere.path(), folder.join("linked")).expect("link");
    std::os::unix::fs::symlink(elsewhere.path().join("x.png"), folder.join("x.png")).expect("link");
    let context = plugin(BOTH, project.path());
    for path in ["linked/a.png", "x.png"] {
        let refused = context.out_stream_begin(path).await.expect_err(path);
        assert_eq!(refused.code(), Some(codes::PATH_OUTSIDE), "{path}");
    }
    assert!(names(elsewhere.path()).is_empty());
}
