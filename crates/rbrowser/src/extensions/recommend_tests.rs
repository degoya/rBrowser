use std::fs;
use std::path::Path;

use rb_config::{ChromiumExtensionSettings, ExtensionRecommendation, UnpackedExtension};

use super::*;

fn extension(root: &Path, relative: &str, name: &str) -> String {
    let folder = root.join(relative);
    fs::create_dir_all(&folder).expect("folder");
    fs::write(
        folder.join("manifest.json"),
        format!(r#"{{ "manifest_version": 3, "name": "{name}", "version": "1.0" }}"#),
    )
    .expect("manifest");
    folder.to_string_lossy().into_owned()
}

fn by_path(path: &str) -> ExtensionRecommendation {
    ExtensionRecommendation {
        id: None,
        url: None,
        path: Some(path.to_owned()),
    }
}

fn by_id(id: &str) -> ExtensionRecommendation {
    ExtensionRecommendation {
        id: Some(id.to_owned()),
        url: None,
        path: None,
    }
}

#[test]
fn a_project_folder_is_offered_with_its_name() {
    let dir = tempfile::tempdir().expect("dir");
    let folder = extension(dir.path(), "tools/my-extension", "Mine");
    let offers = offers(
        dir.path(),
        &[by_path("tools/my-extension")],
        &ChromiumExtensionSettings::default(),
        &[],
    );
    assert_eq!(
        offers,
        [Offer::Folder {
            written: "tools/my-extension".to_owned(),
            folder: folder.into(),
            name: "Mine".to_owned(),
        }]
    );
}

#[test]
fn a_folder_the_person_has_already_is_not_offered_again() {
    let dir = tempfile::tempdir().expect("dir");
    let folder = extension(dir.path(), "ext", "Mine");
    let installed = ChromiumExtensionSettings {
        unpacked: vec![UnpackedExtension {
            path: folder,
            enabled: false,
        }],
        ..ChromiumExtensionSettings::default()
    };
    assert!(offers(dir.path(), &[by_path("ext")], &installed, &[]).is_empty());
}

#[test]
fn a_path_out_of_the_project_or_no_extension_is_refused_not_offered() {
    let dir = tempfile::tempdir().expect("dir");
    let root = dir.path().join("project");
    fs::create_dir_all(root.join("empty")).expect("folder");
    extension(dir.path(), "outside", "Out");
    let offers = offers(
        &root,
        &[by_path("../outside"), by_path("empty")],
        &ChromiumExtensionSettings::default(),
        &[],
    );
    assert_eq!(
        offers,
        [
            Offer::Refused {
                written: "../outside".to_owned(),
                error: FolderError::OutsideProject,
            },
            Offer::Refused {
                written: "empty".to_owned(),
                error: FolderError::NoManifest,
            },
        ]
    );
}

#[test]
fn web_store_entries_are_named_with_their_id_when_it_is_one() {
    let dir = tempfile::tempdir().expect("dir");
    let url = ExtensionRecommendation {
        id: None,
        url: Some("https://chromewebstore.google.com/detail/x/abc".to_owned()),
        path: None,
    };
    let store_url = ExtensionRecommendation {
        id: None,
        url: Some(
            "https://chromewebstore.google.com/detail/x/ljjemllljcmogpfapbkkighbhhppjdbg"
                .to_owned(),
        ),
        path: None,
    };
    let offers = offers(
        dir.path(),
        &[
            by_id("nhdogjmejiglipccpnnnanhbledajbpd"),
            url,
            store_url,
            by_id(""),
        ],
        &ChromiumExtensionSettings::default(),
        &[],
    );
    assert_eq!(
        offers,
        [
            Offer::Store {
                written: "nhdogjmejiglipccpnnnanhbledajbpd".to_owned(),
                id: Some("nhdogjmejiglipccpnnnanhbledajbpd".to_owned()),
            },
            Offer::Store {
                written: "https://chromewebstore.google.com/detail/x/abc".to_owned(),
                id: None,
            },
            Offer::Store {
                written:
                    "https://chromewebstore.google.com/detail/x/ljjemllljcmogpfapbkkighbhhppjdbg"
                        .to_owned(),
                id: Some("ljjemllljcmogpfapbkkighbhhppjdbg".to_owned()),
            },
        ],
        "an entry naming nothing is left out"
    );
}

/// RB-080-23: an installed Web Store extension is not offered again.
#[test]
fn an_installed_web_store_extension_is_not_offered() {
    let dir = tempfile::tempdir().expect("dir");
    let offers = offers(
        dir.path(),
        &[by_id("nhdogjmejiglipccpnnnanhbledajbpd")],
        &ChromiumExtensionSettings::default(),
        &["nhdogjmejiglipccpnnnanhbledajbpd".to_owned()],
    );
    assert!(offers.is_empty());
}

#[test]
fn nothing_is_offered_unless_trusted_and_both_switches_are_on() {
    let on = ChromiumExtensionSettings::default();
    assert!(should_offer(true, &on));
    assert!(
        !should_offer(false, &on),
        "an untrusted project offers nothing"
    );
    let no_offers = ChromiumExtensionSettings {
        offer_project_recommendations: false,
        ..on.clone()
    };
    assert!(!should_offer(true, &no_offers));
    let off = ChromiumExtensionSettings {
        enabled: false,
        ..on
    };
    assert!(!should_offer(true, &off));
}

#[test]
fn adding_a_folder_appends_it_or_switches_its_entry_back_on() {
    let start = vec![UnpackedExtension {
        path: "C:\\ext\\a".to_owned(),
        enabled: false,
    }];
    let again = with_folder(&start, "c:\\EXT\\a");
    assert_eq!(
        again,
        [UnpackedExtension {
            path: "C:\\ext\\a".to_owned(),
            enabled: true,
        }]
    );
    let more = with_folder(&start, "C:\\ext\\b");
    assert_eq!(more.len(), 2);
    assert_eq!(more[1].path, "C:\\ext\\b");
    assert!(more[1].enabled);
}

#[test]
fn the_offer_notice_changes_only_with_the_offers() {
    let a = vec![Offer::Store {
        id: None,
        written: "x".to_owned(),
    }];
    let b = vec![Offer::Store {
        id: None,
        written: "y".to_owned(),
    }];
    assert_eq!(
        fingerprint(Path::new("/p"), &a),
        fingerprint(Path::new("/p"), &a)
    );
    assert_ne!(
        fingerprint(Path::new("/p"), &a),
        fingerprint(Path::new("/p"), &b)
    );
    assert_ne!(
        fingerprint(Path::new("/p"), &a),
        fingerprint(Path::new("/q"), &a)
    );
}
