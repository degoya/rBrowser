use rb_plugin_api::{FailureKind, PluginHost, codes};

use super::{check_path, discover, read, write};
use crate::config::Settings;
use crate::fake_host::FakeHost;

fn project() -> FakeHost {
    FakeHost::with(&[
        ("TODO.todo", "\u{2610} a\n"),
        ("docs/plan.todo", "\u{2610} b\n"),
        ("docs/deep/er/x.todo", "\u{2610} c\n"),
        ("node_modules/pkg/readme.todo", "\u{2610} vendored\n"),
        ("NODE_MODULES/other.todo", "\u{2610} vendored too\n"),
        (".git/info.todo", "\u{2610} git\n"),
        (".rbrowser/notes.todo", "\u{2610} config\n"),
        ("src/app.todo", "\u{2610} d\n"),
    ])
}

#[test]
fn every_todo_file_outside_the_excluded_folders_is_found() {
    let mut host = project();
    let found = discover(&mut host, &Settings::default()).expect("found");
    assert_eq!(
        found.paths,
        [
            "TODO.todo",
            "docs/deep/er/x.todo",
            "docs/plan.todo",
            "src/app.todo"
        ]
    );
    assert!(!found.truncated);
    // The root first, and never a folder that is excluded, `.git` or `.rbrowser`.
    assert_eq!(host.listed[0], "");
    for skipped in ["node_modules", "NODE_MODULES", ".git", ".rbrowser"] {
        assert!(!host.listed.iter().any(|dir| dir == skipped), "{skipped}");
    }
}

#[test]
fn hidden_and_git_ignored_folders_are_never_listed() {
    let mut host = FakeHost::with(&[
        ("TODO.todo", "\u{2610} a\n"),
        (".gitignore", "/storage/framework/*\nlogs/\n"),
        (".idea/x.todo", "\u{2610} editor\n"),
        (".cache/a/b/c.todo", "\u{2610} cache\n"),
        ("app/.hidden/y.todo", "\u{2610} hidden below\n"),
        ("app/logs/z.todo", "\u{2610} ignored\n"),
        ("storage/framework/views/v.todo", "\u{2610} ignored\n"),
        ("storage/framework/cache/.gitignore", "*\n!data/\n"),
        ("storage/notes.todo", "\u{2610} kept\n"),
        ("docs/.gitignore", "private.todo\nskip/\n"),
        ("docs/skip/s.todo", "\u{2610} ignored\n"),
        ("docs/keep/k.todo", "\u{2610} kept\n"),
        // A todo file git ignores is still found when its folder is searched.
        ("docs/private.todo", "\u{2610} kept\n"),
    ]);
    let found = discover(&mut host, &Settings::default()).expect("found");
    assert_eq!(
        found.paths,
        [
            "TODO.todo",
            "docs/keep/k.todo",
            "docs/private.todo",
            "storage/notes.todo"
        ]
    );
    for skipped in [
        ".idea",
        ".cache",
        "app/.hidden",
        "app/logs",
        "storage/framework/views",
        "docs/skip",
    ] {
        assert!(!host.listed.iter().any(|dir| dir == skipped), "{skipped}");
    }
}

#[test]
fn a_folder_the_files_setting_names_is_searched_though_hidden_or_ignored() {
    let mut host = FakeHost::with(&[
        (".gitignore", "notes/\n"),
        (".plans/a.todo", "\u{2610} a\n"),
        ("notes/b.todo", "\u{2610} b\n"),
        (".other/c.todo", "\u{2610} c\n"),
    ]);
    let settings = Settings {
        files: vec![".plans/*.todo".to_owned(), "notes/**/*.todo".to_owned()],
        ..Settings::default()
    };
    let found = discover(&mut host, &settings).expect("found");
    assert_eq!(found.paths, [".plans/a.todo", "notes/b.todo"]);
    assert!(!host.listed.iter().any(|dir| dir == ".other"));
}

#[test]
fn a_git_ignore_that_does_not_read_is_left_aside() {
    // Not UTF-8, or larger than read: its `logs/` leaves out nothing.
    let mut too_large = b"logs/\n".to_vec();
    too_large.resize(crate::ignore::MAX_BYTES + 1, b'#');
    for bytes in [b"logs/\n\xff\xfe".to_vec(), too_large] {
        let mut host = FakeHost::with(&[("logs/a.todo", "\u{2610} a\n")]);
        host.files.insert(".gitignore".to_owned(), bytes);
        let found = discover(&mut host, &Settings::default()).expect("found");
        assert_eq!(found.paths, ["logs/a.todo"]);
    }
}

#[test]
fn the_files_setting_narrows_the_search() {
    let mut host = project();
    let settings = Settings {
        files: vec!["docs/*.todo".to_owned()],
        ..Settings::default()
    };
    let found = discover(&mut host, &settings).expect("found");
    assert_eq!(found.paths, ["docs/plan.todo"]);
}

#[test]
fn without_a_project_the_search_says_so() {
    let mut host = project();
    host.no_project = true;
    let failure = discover(&mut host, &Settings::default()).expect_err("no project");
    assert_eq!(failure.code(), Some(codes::NOT_AVAILABLE));
}

#[test]
fn a_file_that_is_no_utf8_or_too_large_is_not_read() {
    let mut host = project();
    host.files.insert("bad.todo".to_owned(), vec![0xff, 0xfe]);
    let failure = read(&mut host, "bad.todo").expect_err("encoding");
    assert_eq!(failure.code(), Some("plugin.todo.errors.encoding"));
    host.files
        .insert("big.todo".to_owned(), vec![b'a'; super::MAX_FILE_BYTES + 1]);
    let failure = read(&mut host, "big.todo").expect_err("too large");
    assert_eq!(failure.code(), Some("plugin.todo.errors.too_large"));
    let (bytes, text) = read(&mut host, "TODO.todo").expect("read");
    assert_eq!(bytes, text.as_bytes());
}

#[test]
fn only_todo_paths_are_named() {
    for path in ["", "notes.txt", "TODO"] {
        let failure = check_path(path).expect_err(path);
        assert_eq!(failure.kind, FailureKind::BadData, "{path}");
    }
    assert!(check_path("docs/plan.todo").is_ok());
}

#[test]
fn an_untrusted_workspace_gets_the_plugins_own_sentence() {
    let mut host = project();
    host.trusted = false;
    let failure = write(
        &mut host,
        "TODO.todo",
        b"x",
        Some("\u{2610} a\n".as_bytes()),
    )
    .expect_err("untrusted");
    assert_eq!(failure.code(), Some("plugin.todo.errors.untrusted"));
    host.trusted = true;
    let conflict = write(&mut host, "TODO.todo", b"x", Some(b"stale")).expect_err("conflict");
    assert_eq!(conflict.code(), Some(codes::WRITE_CONFLICT));
    write(
        &mut host,
        "TODO.todo",
        b"x",
        Some("\u{2610} a\n".as_bytes()),
    )
    .expect("written");
    assert_eq!(host.project_read("TODO.todo").expect("read"), b"x");
}
