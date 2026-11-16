use std::fs;
use std::path::Path;

use rb_config::{ChromiumExtensionSettings, UnpackedExtension};

use super::*;
use crate::extensions::start::plan;

fn extension(dir: &Path, name: &str) -> String {
    let folder = dir.join(name);
    fs::create_dir_all(&folder).expect("folder");
    fs::write(
        folder.join("manifest.json"),
        format!(r#"{{ "manifest_version": 3, "name": "{name}", "version": "2.1" }}"#),
    )
    .expect("manifest");
    folder.to_string_lossy().into_owned()
}

fn settings(paths: &[&str]) -> ChromiumExtensionSettings {
    ChromiumExtensionSettings {
        unpacked: paths
            .iter()
            .map(|path| UnpackedExtension {
                path: (*path).to_owned(),
                enabled: true,
            })
            .collect(),
        ..ChromiumExtensionSettings::default()
    }
}

#[test]
fn the_status_names_each_folder_and_whether_this_start_loaded_it() {
    let dir = tempfile::tempdir().expect("dir");
    let a = extension(dir.path(), "a");
    let b = extension(dir.path(), "b");
    let gone = dir.path().join("gone").to_string_lossy().into_owned();
    let started = plan(&settings(&[&a]));
    let now_settings = settings(&[&a, &b, &gone]);
    let now = plan(&now_settings);
    let answer = status_answer(&now_settings, &started, &now, None);
    assert_eq!(answer["enabled"], true);
    assert_eq!(
        answer["restartNeeded"], true,
        "b loads only after a restart"
    );
    assert_eq!(answer["webStore"], true);
    assert_eq!(answer["checkUpdatesAtStart"], false);
    let rows = answer["extensions"].as_array().expect("rows");
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0]["name"], "a");
    assert_eq!(rows[0]["version"], "2.1");
    assert_eq!(rows[0]["loaded"], true);
    assert_eq!(rows[1]["loaded"], false);
    assert_eq!(rows[2]["error"]["code"], "extensions.folder_missing");
    assert_eq!(answer["projectRecommendations"], json!([]));
}

#[test]
fn the_status_lists_the_offers_and_which_web_store_ones_are_installable() {
    let empty = ChromiumExtensionSettings::default();
    let offers = [
        Offer::Store {
            written: "abc".to_owned(),
            id: None,
        },
        Offer::Store {
            written: "nhdogjmejiglipccpnnnanhbledajbpd".to_owned(),
            id: Some("nhdogjmejiglipccpnnnanhbledajbpd".to_owned()),
        },
        Offer::Refused {
            written: "../x".to_owned(),
            error: FolderError::OutsideProject,
        },
    ];
    let answer = status_answer(
        &empty,
        &StartPlan::default(),
        &StartPlan::default(),
        Some(&offers),
    );
    assert_eq!(answer["restartNeeded"], false);
    assert_eq!(
        answer["projectRecommendations"],
        json!([
            { "store": "abc", "installable": false },
            {
                "store": "nhdogjmejiglipccpnnnanhbledajbpd",
                "id": "nhdogjmejiglipccpnnnanhbledajbpd",
                "installable": true
            },
            { "path": "../x", "error": {
                "code": "extensions.path_outside_project",
                "message": "the path is not a folder inside the project"
            } }
        ])
    );
}

/// RB-080-23: with the Web Store off nothing is installable, and an installed Web Store
/// extension is a row of its own.
#[test]
fn the_status_names_the_installed_web_store_extensions() {
    let off = ChromiumExtensionSettings {
        allow_web_store: false,
        ..ChromiumExtensionSettings::default()
    };
    let offers = [Offer::Store {
        written: "nhdogjmejiglipccpnnnanhbledajbpd".to_owned(),
        id: Some("nhdogjmejiglipccpnnnanhbledajbpd".to_owned()),
    }];
    let dir = tempfile::tempdir().expect("dir");
    let folder = dir.path().join("x");
    let installed = crate::extensions::store::InstalledExtension {
        id: "nhdogjmejiglipccpnnnanhbledajbpd".to_owned(),
        version: "7.6.4".to_owned(),
        name: "Vue".to_owned(),
        folder: folder.clone(),
    };
    let now = StartPlan {
        web_store: vec![installed],
        ..StartPlan::default()
    };
    let answer = status_answer(&off, &StartPlan::default(), &now, Some(&offers));
    assert_eq!(answer["webStore"], false);
    assert_eq!(answer["projectRecommendations"][0]["installable"], false);
    let rows = answer["extensions"].as_array().expect("rows");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["source"], "webStore");
    assert_eq!(rows[0]["id"], "nhdogjmejiglipccpnnnanhbledajbpd");
    assert_eq!(rows[0]["version"], "7.6.4");
    assert_eq!(rows[0]["path"], json!(folder.to_string_lossy()));
    assert_eq!(rows[0]["loaded"], false, "not loaded by this start");
}

/// RB-080-23: one failing Web Store install stops nothing after it; the folders and ids
/// behind it are still added, and the failures are counted with their reasons.
#[test]
fn adding_goes_on_after_a_failed_install_and_counts_both() {
    let a = "a".repeat(32);
    let b = "b".repeat(32);
    let offers = [
        Offer::Store {
            written: a.clone(),
            id: Some(a.clone()),
        },
        Offer::Folder {
            written: "tools/hello".to_owned(),
            folder: std::path::PathBuf::from("/p/tools/hello"),
            name: "Hello".to_owned(),
        },
        Offer::Store {
            written: b.clone(),
            id: Some(b.clone()),
        },
        Offer::Store {
            written: "no-id".to_owned(),
            id: None,
        },
        Offer::Refused {
            written: "../x".to_owned(),
            error: FolderError::OutsideProject,
        },
    ];
    let mut installed = Vec::new();
    let mut added = Vec::new();
    let done = add_each(
        &offers,
        true,
        |folder| {
            added.push(folder.to_owned());
            Ok(())
        },
        |id| {
            installed.push(id.to_owned());
            if id == a {
                Err("the download failed: 404".to_owned())
            } else {
                Ok(())
            }
        },
    );
    assert_eq!(
        done,
        Recommended {
            added: 2,
            failed: vec![(a.clone(), "the download failed: 404".to_owned())],
        }
    );
    assert_eq!(installed, [a.clone(), b.clone()]);
    assert_eq!(added, ["/p/tools/hello"]);
    let done = add_each(&offers, false, |_| Ok(()), |_| panic!("the store is off"));
    assert_eq!(
        done,
        Recommended {
            added: 1,
            failed: vec![]
        }
    );
    let done = add_each(
        &offers,
        true,
        |_| Err("refused".to_owned()),
        |_| Err("offline".to_owned()),
    );
    assert_eq!(done.added, 0);
    assert_eq!(done.failed.len(), 3);
}

#[test]
fn the_notice_after_adding_names_the_counts_and_the_first_failure() {
    let note = |done: Recommended| match recommended_notice(&done) {
        CoreToShell::Notification(note) => note,
        other => panic!("a notification, not {other:?}"),
    };
    let nothing = note(Recommended::default());
    assert_eq!(nothing.message_key, NOTHING_KEY);
    let all = note(Recommended {
        added: 2,
        failed: vec![],
    });
    assert_eq!(all.message_key, ADDED_RECOMMENDED_KEY);
    assert_eq!(all.args["count"], "2");
    assert_eq!(all.level, NotificationLevel::Info);
    let some = note(Recommended {
        added: 1,
        failed: vec![
            ("a".repeat(32), "offline".to_owned()),
            ("b".repeat(32), "404".to_owned()),
        ],
    });
    assert_eq!(some.message_key, ADDED_RECOMMENDED_PARTLY_KEY);
    assert_eq!(some.args["added"], "1");
    assert_eq!(some.args["failed"], "2");
    assert_eq!(some.args["first"], format!("{}: offline", "a".repeat(32)));
    assert_eq!(some.level, NotificationLevel::Warning);
    assert_eq!(some.settings_key.as_deref(), Some(UNPACKED_KEY));
    let none = note(Recommended {
        added: 0,
        failed: vec![("x".to_owned(), "offline".to_owned())],
    });
    assert_eq!(none.message_key, ADDED_RECOMMENDED_PARTLY_KEY);
    assert_eq!(none.level, NotificationLevel::Error);
}

#[test]
fn a_notice_links_the_folders_setting() {
    let CoreToShell::Notification(note) = notice(
        NotificationLevel::Info,
        ADDED_KEY,
        &[("name", "Hello".to_owned())],
    ) else {
        panic!("a notification");
    };
    assert_eq!(note.settings_key.as_deref(), Some(UNPACKED_KEY));
    assert_eq!(note.args["name"], "Hello");
}
