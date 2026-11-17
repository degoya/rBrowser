use super::super::fixtures::{entry, index_of};
use super::{Compatibility, compatibility, merge_offers, update_for};

fn app(version: &str) -> semver::Version {
    semver::Version::parse(version).expect("version")
}

#[test]
fn the_same_id_from_two_repositories_stays_two_offers_in_list_order() {
    let official = index_of(4, vec![entry("seo", "1.0.0"), entry("todo", "2.0.0")]);
    let mine = index_of(9, vec![entry("seo", "1.5.0")]);
    let catalog = merge_offers(
        &[
            ("official".to_owned(), &official),
            ("mine".to_owned(), &mine),
        ],
        &app("1.0.0"),
    );
    let ids: Vec<&str> = catalog.iter().map(|entry| entry.id.as_str()).collect();
    assert_eq!(ids, ["seo", "todo"]);
    let seo = &catalog[0];
    assert!(seo.has_several_origins());
    let origins: Vec<(&str, &str)> = seo
        .offers
        .iter()
        .map(|offer| (offer.repository.as_str(), offer.entry.version.as_str()))
        .collect();
    assert_eq!(origins, [("official", "1.0.0"), ("mine", "1.5.0")]);
    assert!(!catalog[1].has_several_origins());
}

#[test]
fn each_repository_offers_its_newest_version_that_runs_here() {
    let mut too_new = entry("seo", "3.0.0");
    too_new.min_app_version = Some("9.0.0".to_owned());
    let official = index_of(
        2,
        vec![entry("seo", "1.0.0"), entry("seo", "2.0.0"), too_new],
    );
    let catalog = merge_offers(&[("official".to_owned(), &official)], &app("1.0.0"));
    let offer = &catalog[0].offers[0];
    assert_eq!(offer.entry.version, "2.0.0");
    assert_eq!(offer.compatibility, Compatibility::Compatible);
}

#[test]
fn an_offer_that_runs_nowhere_here_keeps_its_reason() {
    let mut newer_app = entry("seo", "1.0.0");
    newer_app.min_app_version = Some("9.0.0".to_owned());
    assert_eq!(
        compatibility(&newer_app, &app("1.0.0")),
        Compatibility::NeedsNewerApp {
            min_app_version: "9.0.0".to_owned()
        }
    );
    let mut other_contract = entry("seo", "1.0.0");
    other_contract.api_version = "99.0.0".to_owned();
    assert_eq!(
        compatibility(&other_contract, &app("1.0.0")),
        Compatibility::ApiVersion {
            api_version: "99.0.0".to_owned()
        }
    );
    let official = index_of(2, vec![newer_app]);
    let catalog = merge_offers(&[("official".to_owned(), &official)], &app("1.0.0"));
    assert!(!catalog[0].offers[0].compatibility.is_compatible());
}

#[test]
fn an_update_comes_from_the_repository_the_plugin_was_installed_from_only() {
    let official = index_of(4, vec![entry("seo", "1.1.0")]);
    let mine = index_of(9, vec![entry("seo", "5.0.0")]);
    let catalog = merge_offers(
        &[
            ("official".to_owned(), &official),
            ("mine".to_owned(), &mine),
        ],
        &app("1.0.0"),
    );
    let seo = &catalog[0];
    let installed = app("1.0.0");
    let update = update_for(seo, "official", &installed).expect("an update");
    assert_eq!(update.entry.version, "1.1.0");
    assert_eq!(update.repository, "official");
    assert!(update_for(seo, "official", &app("1.1.0")).is_none());
    assert!(update_for(seo, "elsewhere", &installed).is_none());
}

#[test]
fn the_catalogue_serialises_with_its_compatibility_tag() {
    let official = index_of(4, vec![entry("seo", "1.1.0")]);
    let catalog = merge_offers(&[("official".to_owned(), &official)], &app("1.0.0"));
    let json = serde_json::to_value(&catalog).expect("encode");
    assert_eq!(json[0]["offers"][0]["compatibility"]["state"], "compatible");
    assert_eq!(json[0]["offers"][0]["repository"], "official");
    assert_eq!(
        json[0]["offers"][0]["entry"]["sha256"],
        super::super::fixtures::SHA256
    );
}
