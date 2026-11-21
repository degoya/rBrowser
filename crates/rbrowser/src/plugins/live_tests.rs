use std::collections::BTreeSet;

use super::{Change, changed_message, diff};

fn set(ids: &[&str]) -> BTreeSet<String> {
    ids.iter().map(|id| (*id).to_owned()).collect()
}

fn plugins(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(id, version)| ((*id).to_owned(), (*version).to_owned()))
        .collect()
}

#[test]
fn a_load_a_replacement_and_an_unload_are_told_apart() {
    let before = plugins(&[
        ("seo", "1.0.0"),
        ("lighthouse", "2.0.0"),
        ("colour", "1.0.0"),
    ]);
    let now = plugins(&[
        ("seo", "1.0.0"),
        ("lighthouse", "2.1.0"),
        ("sitemap", "0.3.0"),
    ]);
    let change = diff(&before, &now);
    assert_eq!(
        change,
        Change {
            added: set(&["sitemap"]),
            replaced: set(&["lighthouse"]),
            removed: set(&["colour"]),
        }
    );
    // What leaves (its commands, its page scripts) and what comes in.
    assert_eq!(change.gone(), set(&["colour", "lighthouse"]));
    assert_eq!(change.came(), set(&["lighthouse", "sitemap"]));
    assert!(!change.is_empty());
}

#[test]
fn the_same_plugins_are_no_change() {
    let same = plugins(&[("seo", "1.0.0"), ("colour", "1.0.0")]);
    let change = diff(&same, &same);
    assert!(change.is_empty());
    assert!(change.gone().is_empty() && change.came().is_empty());
}

#[test]
fn the_first_load_and_the_last_unload() {
    let some = plugins(&[("seo", "1.0.0")]);
    assert_eq!(diff(&[], &some).added, set(&["seo"]));
    assert_eq!(diff(&some, &[]).removed, set(&["seo"]));
}

#[test]
fn the_shell_pages_are_told_the_installed_plugins_changed() {
    assert_eq!(
        changed_message(),
        serde_json::json!({ "pluginsChanged": true })
    );
}

// Security review w27: lasting CDP state an old version set (a vision deficiency, scripts
// disabled, an overlay) does not outlive its update - a replaced plugin is reset like a removed
// one, an added one has nothing to reset.
#[test]
fn a_replaced_plugin_is_reset_like_a_removed_one() {
    let before = plugins(&[("colour", "1.0.0"), ("seo", "1.0.0"), ("a11y", "1.0.0")]);
    let now = plugins(&[("colour", "1.1.0"), ("a11y", "1.0.0"), ("sitemap", "0.3.0")]);
    let change = diff(&before, &now);
    assert_eq!(change.cdp_reset(), set(&["colour", "seo"]));
}
