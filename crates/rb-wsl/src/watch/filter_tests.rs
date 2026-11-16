use super::*;

fn filter(watch: &[&str], ignore: &[&str]) -> Filter {
    Filter::new(&WatchRules {
        watch: watch.iter().map(|s| (*s).to_owned()).collect(),
        ignore: ignore.iter().map(|s| (*s).to_owned()).collect(),
        debounce_ms: 0,
    })
    .expect("valid rules")
}

#[test]
fn without_watch_globs_every_file_not_ignored_counts() {
    let filter = filter(&[], &[]);
    assert!(filter.accepts("src/app.css", false));
    assert!(filter.accepts("index.html", false));
    assert!(!filter.accepts("node_modules/x/y.js", false));
    assert!(!filter.accepts(".git/HEAD", false));
}

#[test]
fn watch_globs_narrow_the_files() {
    let filter = filter(&["src/**", "public/*.html"], &[]);
    assert!(filter.accepts("src/a/b.css", false));
    assert!(filter.accepts("public/index.html", false));
    assert!(!filter.accepts("public/img/a.png", false));
    assert!(!filter.accepts("README.md", false));
    // Ignores still apply inside a watched folder.
    assert!(!filter.accepts("src/node_modules/a.js", false));
}

#[test]
fn config_ignores_apply_after_the_defaults() {
    let filter = filter(&[], &["dist/", "!.env.local"]);
    assert!(!filter.accepts("dist/a.js", false));
    assert!(filter.accepts(".env.local", false));
}

#[test]
fn ignored_folders_are_not_walked_but_the_rbrowser_folder_is() {
    let filter = filter(&["src/**"], &["dist/"]);
    assert!(filter.walks("src", true));
    // Watch globs never prune a folder: `public` may hold nothing watched, but its walk is cheap
    // and a glob like `**/*.css` would need it.
    assert!(filter.walks("public", true));
    assert!(!filter.walks("dist", true));
    assert!(!filter.walks("node_modules", true));
    assert!(!filter.walks(".git", true));
    assert!(filter.walks(".rbrowser", true));
    assert!(!filter.walks(".rbrowser/cache", true));
}

#[test]
fn the_ignore_file_always_counts_and_is_marked() {
    let filter = filter(&["src/**"], &[]);
    assert!(filter.accepts(IGNORE_FILE, false));
    assert!(is_ignore_file(IGNORE_FILE));
    assert!(!filter.accepts(".rbrowser/config.json", false));
}

#[test]
fn a_bad_watch_glob_is_an_error() {
    let error = Filter::new(&WatchRules {
        watch: vec!["src/[z-a]".to_owned()],
        ..WatchRules::default()
    })
    .expect_err("an invalid class");
    assert_eq!(error.pattern, "src/[z-a]");
}
