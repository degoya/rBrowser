use std::path::Path;

use super::{MAX_FILE_BYTES, collect, read_regular};

fn write(root: &Path, path: &str, text: &str) {
    let file = root.join(path);
    std::fs::create_dir_all(file.parent().expect("a parent")).expect("folder");
    std::fs::write(file, text).expect("written");
}

fn defaults() -> Vec<String> {
    rb_config::DEFAULT_STATIC_GLOBS.map(str::to_owned).to_vec()
}

fn everything(_: &str) -> bool {
    true
}

fn paths(root: &Path, globs: &[String], granted: &dyn Fn(&str) -> bool) -> Vec<String> {
    collect(root, globs, granted)
        .into_iter()
        .map(|file| file.path)
        .collect()
}

/// A project with sources, dependencies, hidden folders and other files.
fn project() -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("temp dir");
    let at = root.path();
    write(at, "tailwind.config.js", "module.exports = {}");
    write(at, "src/app.css", "@import \"tailwindcss\";");
    write(at, "src/styles/_vars.scss", "$bp-md: 768px;");
    write(at, "src/legacy/theme.less", "@screen-sm-min: 768px;");
    write(at, "src/main.ts", "export {}");
    write(
        at,
        "node_modules/bootstrap/scss/_variables.scss",
        "$grid-breakpoints: ();",
    );
    write(at, "packages/web/node_modules/x/x.css", "a {}");
    write(at, ".git/hooks/x.css", "a {}");
    write(at, ".rbrowser/config.local.json", "{}");
    write(at, ".cache/out.css", "a {}");
    write(at, "dist/.hidden.css", "a {}");
    root
}

#[test]
fn static_files_are_the_matching_sources_outside_dependencies_and_dot_folders() {
    let root = project();
    assert_eq!(
        paths(root.path(), &defaults(), &everything),
        vec![
            "tailwind.config.js",
            "src/app.css",
            "src/legacy/theme.less",
            "src/styles/_vars.scss",
        ]
    );
    let files = collect(root.path(), &defaults(), &everything);
    assert_eq!(files[0].text, "module.exports = {}");
}

#[test]
fn static_the_plugin_s_grant_bounds_the_configured_globs() {
    let root = project();
    // The config asks for every file; the plugin's declaration allows only CSS.
    let wide = vec!["**".to_owned()];
    let only_css = |path: &str| path.ends_with(".css");
    assert_eq!(paths(root.path(), &wide, &only_css), vec!["src/app.css"]);
    // The config narrows: only SCSS below src/styles.
    let narrow = vec!["src/styles/**/*.scss".to_owned()];
    assert_eq!(
        paths(root.path(), &narrow, &everything),
        vec!["src/styles/_vars.scss"]
    );
    // Nothing granted, nothing read.
    assert!(paths(root.path(), &defaults(), &|_| false).is_empty());
}

#[test]
fn static_globs_that_climb_out_or_name_a_drive_are_left_out() {
    let root = project();
    for glob in [
        "../**/*.css",
        "/etc/**",
        "C:/**/*.css",
        "src\\**\\*.css",
        "",
        "src/[",
    ] {
        assert!(
            paths(root.path(), &[glob.to_owned()], &everything).is_empty(),
            "{glob}"
        );
    }
    let mixed = vec!["../x".to_owned(), "src/*.css".to_owned()];
    assert_eq!(paths(root.path(), &mixed, &everything), vec!["src/app.css"]);
}

#[test]
fn static_rbrowser_ignore_leaves_out_what_it_names() {
    let root = project();
    write(
        root.path(),
        ".rbrowser/ignore",
        "# generated\nlegacy/\n/tailwind.config.js\n*.scss\n!src/styles/_vars.scss\n\n",
    );
    assert_eq!(
        paths(root.path(), &defaults(), &everything),
        vec!["src/app.css"]
    );
}

#[test]
fn static_large_and_non_text_files_are_skipped() {
    let root = tempfile::tempdir().expect("temp dir");
    let size = usize::try_from(MAX_FILE_BYTES).expect("small") + 1;
    write(root.path(), "big.css", &"a".repeat(size));
    std::fs::write(root.path().join("binary.css"), [0xff, 0xfe, 0x00]).expect("written");
    write(root.path(), "ok.css", "a {}");
    assert_eq!(paths(root.path(), &defaults(), &everything), vec!["ok.css"]);
}

#[cfg(unix)]
#[test]
fn static_links_are_never_followed_out_of_the_project() {
    let outside = tempfile::tempdir().expect("temp dir");
    write(outside.path(), "secret/app.css", "@import \"tailwindcss\";");
    write(outside.path(), "secret.scss", "$bp-md: 1px;");
    let root = tempfile::tempdir().expect("temp dir");
    write(root.path(), "src/own.css", "a {}");
    std::os::unix::fs::symlink(
        outside.path().join("secret"),
        root.path().join("src/linked"),
    )
    .expect("folder link");
    std::os::unix::fs::symlink(
        outside.path().join("secret.scss"),
        root.path().join("src/linked.scss"),
    )
    .expect("file link");
    assert_eq!(
        paths(root.path(), &defaults(), &everything),
        vec!["src/own.css"]
    );
}

#[test]
fn static_a_missing_root_reads_nothing() {
    let root = tempfile::tempdir().expect("temp dir");
    let gone = root.path().join("gone");
    assert!(paths(&gone, &defaults(), &everything).is_empty());
}

/// The read opens the file once and judges it by that handle: a path that became a link after
/// the walk looked at it is not read, and no more than the limit is ever read (charge-4 review,
/// finding 7).
#[test]
fn static_a_file_is_read_through_its_own_handle_within_the_limit() {
    let root = tempfile::tempdir().expect("temp dir");
    write(root.path(), "ok.css", "a {}");
    assert_eq!(
        read_regular(&root.path().join("ok.css"), 16).as_deref(),
        Some("a {}")
    );
    write(root.path(), "five.css", "abcde");
    assert_eq!(read_regular(&root.path().join("five.css"), 4), None);
    assert_eq!(read_regular(&root.path().join("missing.css"), 16), None);
    assert_eq!(read_regular(root.path(), 16), None, "a folder is no file");
    #[cfg(unix)]
    {
        let outside = tempfile::tempdir().expect("temp dir");
        write(outside.path(), "secret.css", "secret");
        let link = root.path().join("swapped.css");
        std::os::unix::fs::symlink(outside.path().join("secret.css"), &link).expect("link");
        assert_eq!(read_regular(&link, 16), None);
    }
}
