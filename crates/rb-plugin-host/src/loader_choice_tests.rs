use std::collections::BTreeSet;
use std::path::PathBuf;

use super::{Candidate, Choice, Declared, choose};
use crate::loader::PluginSource;

fn declared(id: &str, version: &str, key: &str) -> Option<Declared> {
    Some(Declared {
        id: id.to_owned(),
        version: semver::Version::parse(version).expect("version"),
        public_key: key.to_owned(),
    })
}

fn candidate(source: PluginSource, file: &str, declared: Option<Declared>) -> Candidate {
    Candidate {
        source,
        path: PathBuf::from(file),
        declared,
    }
}

fn ids(list: &[&str]) -> BTreeSet<String> {
    list.iter().map(|id| (*id).to_owned()).collect()
}

fn loaded(choice: &Choice) -> Vec<(PluginSource, &str, Option<&str>)> {
    choice
        .load
        .iter()
        .map(|(source, path, fallback)| {
            (
                *source,
                path.to_str().expect("utf-8"),
                fallback.as_deref().and_then(std::path::Path::to_str),
            )
        })
        .collect()
}

/// RB-091-04: without a choice (the tests, a folder loaded on its own) every bundled package
/// loads, as before.
#[test]
fn without_a_choice_every_bundled_package_loads() {
    let choice = choose(
        vec![
            candidate(
                PluginSource::Bundled,
                "a.rbplugin",
                declared("a", "1.0.0", "k"),
            ),
            candidate(
                PluginSource::Bundled,
                "b.rbplugin",
                declared("b", "1.0.0", "k"),
            ),
        ],
        None,
    );
    assert_eq!(
        loaded(&choice),
        [
            (PluginSource::Bundled, "a.rbplugin", None),
            (PluginSource::Bundled, "b.rbplugin", None)
        ]
    );
    assert!(choice.available.is_empty());
}

/// RB-091-04 (rDownloader RD-160-05): only the chosen bundled plugins load; the others stay
/// available for the wizard and the catalogue.
#[test]
fn only_the_chosen_bundled_plugins_load_and_the_rest_is_available() {
    let choice = choose(
        vec![
            candidate(
                PluginSource::Bundled,
                "a.rbplugin",
                declared("a", "1.0.0", "k"),
            ),
            candidate(
                PluginSource::Bundled,
                "b.rbplugin",
                declared("b", "1.0.0", "k"),
            ),
            candidate(
                PluginSource::User,
                "c.rbplugin",
                declared("c", "1.0.0", "x"),
            ),
        ],
        Some(&ids(&["a"])),
    );
    assert_eq!(
        loaded(&choice),
        [
            (PluginSource::Bundled, "a.rbplugin", None),
            (PluginSource::User, "c.rbplugin", None)
        ]
    );
    assert_eq!(choice.available, [PathBuf::from("b.rbplugin")]);
}

/// RB-091-04 (RD-1260-02): a newer copy from the repository that declares the bundle's key
/// stands in for the bundled package, which is held back as its fallback.
#[test]
fn a_newer_copy_of_the_same_key_stands_in_for_the_bundled_package() {
    let choice = choose(
        vec![
            candidate(
                PluginSource::Bundled,
                "a.rbplugin",
                declared("a", "1.0.0", "k"),
            ),
            candidate(
                PluginSource::User,
                "a-user.rbplugin",
                declared("a", "1.1.0", "k"),
            ),
        ],
        Some(&ids(&["a"])),
    );
    assert_eq!(
        loaded(&choice),
        [(PluginSource::User, "a-user.rbplugin", Some("a.rbplugin"))]
    );
    // Held back, not dropped: the loader takes it when the copy is refused (review 2).
    assert!(choice.superseded.is_empty());
}

/// An application update that ships a newer bundle than the copy the repository gave wins, and
/// the same version is the bundle's.
#[test]
fn a_bundle_as_new_or_newer_than_the_copy_wins() {
    for copy in ["1.0.0", "0.9.0"] {
        let choice = choose(
            vec![
                candidate(
                    PluginSource::Bundled,
                    "a.rbplugin",
                    declared("a", "1.0.0", "k"),
                ),
                candidate(
                    PluginSource::User,
                    "a-user.rbplugin",
                    declared("a", copy, "k"),
                ),
            ],
            Some(&ids(&["a"])),
        );
        assert_eq!(
            loaded(&choice),
            [(PluginSource::Bundled, "a.rbplugin", None)]
        );
        assert_eq!(choice.superseded, [PathBuf::from("a-user.rbplugin")]);
    }
}

/// A copy signed by another key never stands in: the bundled package loads first and the copy
/// is refused as a duplicate, as before RB-091-04.
#[test]
fn a_copy_of_another_key_never_stands_in() {
    let choice = choose(
        vec![
            candidate(
                PluginSource::Bundled,
                "a.rbplugin",
                declared("a", "1.0.0", "k"),
            ),
            candidate(
                PluginSource::User,
                "a-user.rbplugin",
                declared("a", "9.0.0", "other"),
            ),
        ],
        Some(&ids(&["a"])),
    );
    assert_eq!(
        loaded(&choice),
        [
            (PluginSource::Bundled, "a.rbplugin", None),
            (PluginSource::User, "a-user.rbplugin", None)
        ]
    );
}

/// A package whose manifest does not read loads, so its refusal is listed.
#[test]
fn an_unreadable_bundled_package_still_loads_to_be_refused() {
    let choice = choose(
        vec![
            candidate(PluginSource::Bundled, "broken.rbplugin", None),
            candidate(PluginSource::Project, "p.rbplugin", None),
        ],
        Some(&BTreeSet::new()),
    );
    assert_eq!(
        loaded(&choice),
        [
            (PluginSource::Bundled, "broken.rbplugin", None),
            (PluginSource::Project, "p.rbplugin", None)
        ]
    );
}
