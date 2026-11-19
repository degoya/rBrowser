use std::collections::BTreeSet;

use rb_config::ProjectConfig;
use serde_json::json;

use rb_plugin_host::BundleStartFlags;

use super::{chosen_at_start, missing, named_plugins, update_choice_in};

fn ids(list: &[&str]) -> BTreeSet<String> {
    list.iter().map(|id| (*id).to_owned()).collect()
}

/// `--install-all-bundled-plugins`, the user settings there before, the wizard will ask.
fn flags(install_all: bool, ran_before: bool, wizard: bool) -> BundleStartFlags {
    BundleStartFlags {
        ran_before,
        install_all,
        wizard,
    }
}

/// RB-091-04: a fresh installation's first start chooses the recommended set and keeps it; a
/// later start adds nothing, not even a plugin new in the bundle.
#[test]
fn the_first_start_chooses_and_a_later_one_keeps_the_choice() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bundle = ids(&["axe", "todo"]);
    let first = chosen_at_start(
        dir.path(),
        &bundle,
        &ids(&["axe"]),
        flags(false, false, false),
    );
    assert_eq!(first, ids(&["axe"]));
    let newer_bundle = ids(&["axe", "todo", "fresh"]);
    let later = chosen_at_start(
        dir.path(),
        &newer_bundle,
        &newer_bundle,
        flags(false, false, false),
    );
    assert_eq!(later, ids(&["axe"]));
    let all = chosen_at_start(
        dir.path(),
        &newer_bundle,
        &BTreeSet::new(),
        flags(true, false, false),
    );
    assert_eq!(all, newer_bundle, "--install-all-bundled-plugins");
}

/// An installation that ran before RB-091-04 keeps the whole bundle; so does one whose choice
/// file does not read - nothing disappears.
#[test]
fn an_installation_that_ran_before_keeps_everything() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bundle = ids(&["axe", "todo"]);
    assert_eq!(
        chosen_at_start(
            dir.path(),
            &bundle,
            &BTreeSet::new(),
            flags(false, true, false)
        ),
        bundle
    );
    let broken = tempfile::tempdir().expect("tempdir");
    std::fs::write(broken.path().join(rb_plugin_host::BUNDLE_FILE), b"{").expect("write");
    assert_eq!(
        chosen_at_start(
            broken.path(),
            &bundle,
            &BTreeSet::new(),
            flags(false, false, false)
        ),
        bundle
    );
}

/// RD-180-14: a plugin taken out of the choice stays out at the next start.
#[test]
fn a_removed_plugin_does_not_come_back() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bundle = ids(&["axe", "todo"]);
    chosen_at_start(dir.path(), &bundle, &bundle, flags(false, false, false));
    update_choice_in(dir.path(), &bundle, |state| state.unchoose("todo")).expect("saved");
    assert_eq!(
        chosen_at_start(dir.path(), &bundle, &bundle, flags(false, false, false)),
        ids(&["axe"])
    );
    update_choice_in(dir.path(), &bundle, |state| state.choose("todo")).expect("saved");
    assert_eq!(
        chosen_at_start(dir.path(), &bundle, &bundle, flags(false, false, false)),
        bundle
    );
}

/// The plugins a project's config names: a section or a `checks` entry under a first-party
/// plugin's id; anything else is no plugin.
#[test]
fn the_config_names_plugins_by_their_sections() {
    let mut config = ProjectConfig::default();
    config
        .extra
        .insert("a11y-axe".to_owned(), json!({ "tags": ["wcag2a"] }));
    config
        .extra
        .insert("notAPlugin".to_owned(), json!({ "x": 1 }));
    config
        .checks
        .extra
        .insert("coverage".to_owned(), json!({ "threshold": 3 }));
    let named = named_plugins(&config);
    assert_eq!(named, ids(&["a11y-axe", "coverage"]));
    let installed = ids(&["a11y-axe"]);
    assert_eq!(missing(&named, &installed), ["coverage"]);
    assert!(missing(&named, &named).is_empty());
}

/// Review 4: a choice file that cannot be read is replaced by what this start loads, changed -
/// one install never shrinks the choice to that plugin.
#[test]
fn a_change_without_a_readable_choice_starts_from_what_is_loaded() {
    let dir = tempfile::tempdir().expect("tempdir");
    let loaded = ids(&["axe", "todo"]);
    let state =
        update_choice_in(dir.path(), &loaded, |state| state.choose("fonts")).expect("saved");
    assert_eq!(state.chosen, ids(&["axe", "fonts", "todo"]));
    std::fs::write(dir.path().join(rb_plugin_host::BUNDLE_FILE), b"{").expect("broken");
    let state =
        update_choice_in(dir.path(), &loaded, |state| state.unchoose("todo")).expect("saved");
    assert_eq!(state.chosen, ids(&["axe"]));
}

/// Review 7: with the wizard on, a fresh installation loads no optional plugin until the wizard
/// answers; a later start without an answer gets the recommended set.
#[test]
fn the_wizard_s_first_start_waits_and_a_later_one_recommends() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bundle = ids(&["axe", "todo"]);
    let recommended = ids(&["axe"]);
    let first = chosen_at_start(dir.path(), &bundle, &recommended, flags(false, false, true));
    assert!(first.is_empty());
    assert!(
        rb_plugin_host::BundleState::load(dir.path())
            .expect("saved")
            .pending
    );
    let later = chosen_at_start(dir.path(), &bundle, &recommended, flags(false, true, false));
    assert_eq!(later, recommended);
    // The wizard's answer ends the wait: its plugins, nothing else, at the next start.
    let answered = tempfile::tempdir().expect("tempdir");
    chosen_at_start(
        answered.path(),
        &bundle,
        &recommended,
        flags(false, false, true),
    );
    update_choice_in(answered.path(), &BTreeSet::new(), |state| {
        state.choose("todo")
    })
    .expect("saved");
    assert_eq!(
        chosen_at_start(
            answered.path(),
            &bundle,
            &recommended,
            flags(false, true, false)
        ),
        ids(&["todo"])
    );
}
