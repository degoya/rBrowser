use super::*;
use crate::testing::FakeHost;

fn host() -> FakeHost {
    let mut host = FakeHost::default();
    host.view(1, "chromium")
        .view(2, "firefox")
        .view(3, "chromium");
    host
}

fn ids(views: &[ViewInfo]) -> Vec<ViewId> {
    views.iter().map(|view| view.id).collect()
}

#[test]
fn all_one_or_the_default() {
    let mut host = host();
    let all = resolve(&mut host, &json!({ "target": "all" }), Default::Active).expect("all");
    assert_eq!(ids(&all), [1, 2, 3]);
    let one = resolve(&mut host, &json!({ "target": 3 }), Default::All).expect("one");
    assert_eq!(ids(&one), [3]);
    let active = resolve(&mut host, &json!({}), Default::Active).expect("active");
    assert_eq!(ids(&active), [1]);
    let every = resolve(&mut host, &json!({}), Default::All).expect("every");
    assert_eq!(ids(&every), [1, 2, 3]);
}

#[test]
fn unknown_views_and_odd_targets_are_refused() {
    let mut host = host();
    let missing = resolve(&mut host, &json!({ "target": 9 }), Default::All).expect_err("9");
    assert_eq!(
        missing.code.as_deref(),
        Some("plugin.color-picker.errors.unknown_view")
    );
    for target in [json!("some"), json!(-1), json!(1.5), json!([1])] {
        let odd = resolve(&mut host, &json!({ "target": target }), Default::All);
        assert_eq!(
            odd.expect_err("odd").code.as_deref(),
            Some("plugins.error.invalid"),
            "{target}"
        );
    }
}

#[test]
fn only_chromium_views_are_read() {
    let mut host = host();
    let views = resolve(&mut host, &json!({ "target": "all" }), Default::All).expect("all");
    assert!(supported(&views[0]));
    assert!(!supported(&views[1]));
    assert_eq!(
        unsupported(&views[1])["error"],
        "plugin.color-picker.errors.unsupported_engine"
    );
}
