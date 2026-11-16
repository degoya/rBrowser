use std::path::Path;

use super::*;

#[test]
fn a_path_below_the_root_is_relative_and_slash_separated() {
    let root = Path::new("/home/u/app");
    assert_eq!(
        relative(root, Path::new("/home/u/app/src/app.css")).as_deref(),
        Some("src/app.css")
    );
    assert_eq!(
        relative(root, Path::new("/home/u/app/index.html")).as_deref(),
        Some("index.html")
    );
}

#[test]
fn the_root_itself_and_paths_outside_have_no_relative_form() {
    let root = Path::new("/home/u/app");
    assert_eq!(relative(root, root), None);
    assert_eq!(relative(root, Path::new("/home/u/other/a.css")), None);
    assert_eq!(relative(root, Path::new("/home/u/application/a.css")), None);
}

#[test]
fn the_rules_travel_in_camel_case_with_defaults() {
    let rules: WatchRules =
        serde_json::from_str(r#"{"debounceMs":50}"#).expect("rules with only a debounce");
    assert_eq!(rules.debounce_ms, 50);
    assert!(rules.watch.is_empty() && rules.ignore.is_empty());
    let text = serde_json::to_string(&WatchRules::default()).expect("serialises");
    assert!(text.contains("\"debounceMs\":200"), "{text}");
}

#[test]
fn a_change_serialises_with_a_snake_case_kind() {
    let text =
        serde_json::to_string(&Change::new("a.css", ChangeKind::Modify)).expect("serialises");
    assert_eq!(text, r#"{"path":"a.css","kind":"modify"}"#);
}
