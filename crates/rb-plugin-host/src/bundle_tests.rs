use std::collections::BTreeSet;

use super::{BUNDLE_FILE, BundleStart, BundleState, StartFlags, declared_manifests, start};
use crate::test_support::{ManifestBuilder, signing_key, zip_package};

fn ids(list: &[&str]) -> BTreeSet<String> {
    list.iter().map(|id| (*id).to_owned()).collect()
}

fn bundle() -> BTreeSet<String> {
    ids(&["axe", "console", "fonts", "todo"])
}

fn flags(ran_before: bool, install_all: bool, wizard: bool) -> StartFlags {
    StartFlags {
        ran_before,
        install_all,
        wizard,
    }
}

/// RD-160-05: a fresh installation's first start gets the recommended set, nothing more.
#[test]
fn a_fresh_installation_starts_with_the_recommended_set() {
    let (state, start) = start(
        None,
        &bundle(),
        &ids(&["axe", "console", "gone"]),
        flags(false, false, false),
    );
    assert_eq!(state.chosen, ids(&["axe", "console"]));
    assert_eq!(start, BundleStart::FirstStart);
}

/// Existing installations keep their plugins: before RB-091-04 every start loaded the whole
/// bundle, so an installation that ran before chooses all of it once.
#[test]
fn an_installation_that_ran_before_keeps_every_bundled_plugin() {
    let (state, start) = start(None, &bundle(), &ids(&["axe"]), flags(true, false, true));
    assert_eq!(state.chosen, bundle());
    assert_eq!(start, BundleStart::Kept);
}

/// A later start installs nothing new: a newly bundled plugin waits in the catalogue, an
/// unticked one does not come back (RD-180-14).
#[test]
fn a_later_start_adds_nothing_and_brings_nothing_back() {
    let earlier = BundleState::with(ids(&["axe"]));
    let mut shipped = bundle();
    shipped.insert("new-one".to_owned());
    let (state, start) = start(Some(earlier), &shipped, &shipped, flags(true, false, false));
    assert_eq!(state.chosen, ids(&["axe"]));
    assert_eq!(start, BundleStart::InstalledOnly);
}

/// RB-094-13 (owner): a plugin new in the bundle of an update is named once - neither a chosen
/// one nor one offered before nor one the person took out comes up again.
#[test]
fn a_plugin_new_in_the_bundle_is_named_once() {
    let mut earlier = BundleState::with(ids(&["axe", "console"]));
    earlier.offered = ids(&["fonts"]);
    earlier.unchoose("console");
    let mut shipped = bundle();
    shipped.insert("new-one".to_owned());
    let (mut state, how) = start(Some(earlier), &shipped, &shipped, flags(true, false, false));
    assert_eq!(how, BundleStart::InstalledOnly);
    let new = state.new_in(&shipped);
    assert_eq!(new, ids(&["new-one", "todo"]));
    state.offered.extend(new);
    assert!(state.new_in(&shipped).is_empty(), "named once");
    let dir = tempfile::tempdir().expect("tempdir");
    state.save(dir.path()).expect("save");
    let read = BundleState::load(dir.path()).expect("read");
    assert!(read.new_in(&shipped).is_empty(), "kept across starts");
}

/// The first-run wizard's pending choice is written with the bundle it offers: after its answer
/// the plugins left unticked are not named as new (review of RB-094-13).
#[test]
fn a_wizard_answer_names_nothing_new() {
    let mut pending = BundleState::pending();
    pending.offered = bundle();
    pending.choose("axe");
    let (state, how) = start(
        Some(pending),
        &bundle(),
        &bundle(),
        flags(false, false, true),
    );
    assert_eq!(how, BundleStart::InstalledOnly);
    assert!(state.new_in(&bundle()).is_empty());
}

/// A choice made at a start - the first one, the wizard's pending one, the whole bundle - saw
/// every bundled plugin: nothing in it is new afterwards. A pending choice names nothing.
#[test]
fn a_choice_made_at_a_start_has_nothing_new() {
    for (earlier, flags) in [
        (None, flags(false, false, false)),
        (None, flags(true, false, false)),
        (Some(BundleState::pending()), flags(true, false, true)),
        (
            Some(BundleState::with(ids(&["axe"]))),
            flags(true, true, false),
        ),
    ] {
        let (state, _) = start(earlier, &bundle(), &ids(&["axe"]), flags);
        assert!(state.new_in(&bundle()).is_empty(), "{state:?}");
    }
    assert!(BundleState::pending().new_in(&bundle()).is_empty());
}

/// `--install-all-bundled-plugins` chooses the whole bundle, at any start.
#[test]
fn install_all_chooses_the_whole_bundle() {
    let earlier = BundleState::with(ids(&["axe"]));
    let (state, start) = start(
        Some(earlier),
        &bundle(),
        &BTreeSet::new(),
        flags(true, true, false),
    );
    assert_eq!(state.chosen, bundle());
    assert_eq!(start, BundleStart::All);
}

/// Review 7 (owner: the wizard only marks what gets loaded): a fresh installation whose wizard
/// will ask chooses nothing at its first start; the choice is pending.
#[test]
fn a_fresh_installation_with_the_wizard_waits_for_its_answer() {
    let (state, start) = start(None, &bundle(), &ids(&["axe"]), flags(false, false, true));
    assert!(state.chosen.is_empty());
    assert!(state.pending);
    assert_eq!(start, BundleStart::Pending);
}

/// A later start that finds the choice still pending - the wizard closed without an answer -
/// applies the recommended set.
#[test]
fn a_choice_still_pending_at_a_later_start_gets_the_recommended_set() {
    let (state, start) = start(
        Some(BundleState::pending()),
        &bundle(),
        &ids(&["axe", "gone"]),
        flags(true, false, true),
    );
    assert_eq!(state.chosen, ids(&["axe"]));
    assert!(!state.pending);
    assert_eq!(start, BundleStart::Recommended);
}

/// The wizard's answer - a plugin chosen or taken out, or the recommended set on Skip - ends the
/// pending state.
#[test]
fn an_answer_ends_the_pending_state() {
    let mut chosen = BundleState::pending();
    chosen.choose("todo");
    assert!(!chosen.pending);
    let mut skipped = BundleState::pending();
    skipped.apply_recommended(&bundle(), &ids(&["axe", "gone"]));
    assert_eq!(skipped.chosen, ids(&["axe"]));
    assert!(!skipped.pending);
    let dir = tempfile::tempdir().expect("tempdir");
    BundleState::pending().save(dir.path()).expect("save");
    assert!(BundleState::load(dir.path()).expect("read").pending);
}

#[test]
fn the_state_is_kept_beside_the_catalogue_and_read_back() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(
        BundleState::load(dir.path()).is_none(),
        "no file: a first start"
    );
    let mut state = BundleState::with(ids(&["axe"]));
    state.choose("console");
    state.unchoose("axe");
    state.save(dir.path()).expect("save");
    assert!(dir.path().join(BUNDLE_FILE).is_file());
    let read = BundleState::load(dir.path()).expect("read");
    assert_eq!(read.chosen, ids(&["console"]));
}

/// A file that does not read is no choice at all; the caller counts the installation as one
/// that ran before, so nothing disappears.
#[test]
fn a_broken_state_file_reads_as_none() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join(BUNDLE_FILE), b"{ broken").expect("write");
    assert!(BundleState::load(dir.path()).is_none());
    assert!(BundleState::exists(dir.path()));
}

#[test]
fn the_bundle_s_ids_are_read_from_the_manifests() {
    let dir = tempfile::tempdir().expect("tempdir");
    for id in ["axe", "todo"] {
        let manifest = ManifestBuilder::named(id, id).text(&signing_key(1));
        let package = zip_package(&[("manifest.toml", manifest.as_bytes())]);
        std::fs::write(dir.path().join(format!("{id}-1.0.0.rbplugin")), package).expect("write");
    }
    std::fs::write(dir.path().join("broken.rbplugin"), b"no zip").expect("write");
    let found: BTreeSet<String> = declared_manifests(dir.path())
        .into_values()
        .map(|manifest| manifest.id)
        .collect();
    assert_eq!(found, ids(&["axe", "todo"]));
}
