use super::*;

fn rules(lines: &[&str]) -> IgnoreRules {
    IgnoreRules::new(lines.iter().copied()).expect("valid rules")
}

fn defaults() -> IgnoreRules {
    IgnoreRules::with_defaults(std::iter::empty::<&str>()).expect("valid defaults")
}

#[test]
fn the_defaults_ignore_node_modules_git_and_dot_folders_at_any_depth() {
    let ignore = defaults();
    assert!(ignore.is_ignored("node_modules/react/index.js", false));
    assert!(ignore.is_ignored("packages/ui/node_modules/x.css", false));
    assert!(ignore.is_ignored("node_modules", true));
    assert!(ignore.is_ignored(".git/index", false));
    assert!(ignore.is_ignored(".cache/vite/deps.js", false));
    assert!(ignore.is_ignored("src/.hidden/a.css", false));
    assert!(ignore.is_ignored(".rbrowser/config.json", false));
}

#[test]
fn the_defaults_keep_sources_and_dot_files() {
    let ignore = defaults();
    assert!(!ignore.is_ignored("src/app.css", false));
    assert!(!ignore.is_ignored("index.html", false));
    assert!(!ignore.is_ignored("src", true));
    // A dot file is no dot folder: `.env` changes reach the dev server.
    assert!(!ignore.is_ignored(".env", false));
    // A file called node_modules is not the folder.
    assert!(!ignore.is_ignored("docs/node_modules", false));
}

#[test]
fn the_defaults_ignore_editor_leftovers() {
    let ignore = defaults();
    assert!(ignore.is_ignored("src/app.css.swp", false));
    assert!(ignore.is_ignored("src/.app.css.swx", false));
    assert!(ignore.is_ignored("src/app.css~", false));
    assert!(ignore.is_ignored("src/4913", false));
    assert!(ignore.is_ignored("src/.#app.css", false));
}

#[test]
fn a_name_without_a_slash_matches_at_any_depth() {
    let ignore = rules(&["*.log", "dist"]);
    assert!(ignore.is_ignored("debug.log", false));
    assert!(ignore.is_ignored("a/b/debug.log", false));
    assert!(ignore.is_ignored("dist/app.js", false));
    assert!(ignore.is_ignored("packages/x/dist/app.js", false));
    assert!(!ignore.is_ignored("src/distance.js", false));
}

#[test]
fn a_slash_anchors_the_pattern_to_the_root() {
    let ignore = rules(&["/build", "docs/*.md"]);
    assert!(ignore.is_ignored("build/a.js", false));
    assert!(!ignore.is_ignored("src/build/a.js", false));
    assert!(ignore.is_ignored("docs/a.md", false));
    assert!(!ignore.is_ignored("docs/sub/a.md", false));
    assert!(!ignore.is_ignored("x/docs/a.md", false));
}

#[test]
fn a_star_does_not_cross_a_slash_but_two_do() {
    let ignore = rules(&["src/*.tmp", "assets/**/raw"]);
    assert!(ignore.is_ignored("src/a.tmp", false));
    assert!(!ignore.is_ignored("src/sub/a.tmp", false));
    assert!(ignore.is_ignored("assets/raw/x.png", false));
    assert!(ignore.is_ignored("assets/a/b/raw/x.png", false));
}

#[test]
fn a_trailing_slash_matches_folders_only() {
    let ignore = rules(&["cache/"]);
    assert!(ignore.is_ignored("cache", true));
    assert!(ignore.is_ignored("cache/a.json", false));
    assert!(!ignore.is_ignored("cache", false));
}

#[test]
fn a_negation_takes_a_file_back_but_not_out_of_an_ignored_folder() {
    let ignore = rules(&["*.css", "!keep.css", "vendor/", "!vendor/a.js"]);
    assert!(ignore.is_ignored("a.css", false));
    assert!(!ignore.is_ignored("keep.css", false));
    assert!(!ignore.is_ignored("sub/keep.css", false));
    // Git cannot re-include a file whose folder is excluded.
    assert!(ignore.is_ignored("vendor/a.js", false));
}

#[test]
fn the_last_matching_line_wins() {
    let ignore = rules(&["!a.css", "*.css"]);
    assert!(ignore.is_ignored("a.css", false));
}

#[test]
fn comments_blank_lines_and_trailing_spaces_are_skipped() {
    let ignore = rules(&["# a comment", "", "   ", "dist   ", r"\#hash.css"]);
    assert!(ignore.is_ignored("dist/x.js", false));
    assert!(ignore.is_ignored("#hash.css", false));
    assert!(!ignore.is_ignored("a comment", false));
    assert_eq!(ignore.len(), 2);
}

#[test]
fn a_bad_pattern_is_an_error_naming_the_line() {
    let error = IgnoreRules::new(["ok", "src/[z-a].css"]).expect_err("an invalid class");
    assert_eq!(error.line, 2);
    assert_eq!(error.pattern, "src/[z-a].css");
    assert_eq!(error.i18n_key(), "reload.error.badRule");
}

#[test]
fn the_ignore_file_is_read_line_by_line() {
    let text = "# project\nnode_modules\r\n/generated/\n!generated/keep.css\n";
    let lines = parse_ignore_file(text);
    assert_eq!(
        lines,
        vec![
            "node_modules".to_owned(),
            "/generated/".to_owned(),
            "!generated/keep.css".to_owned(),
        ]
    );
}
