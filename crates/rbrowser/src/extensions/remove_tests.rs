use rb_ipc::{CoreToShell, NotificationLevel};

use super::*;
use crate::extensions::store::{ExtensionStore, POINTER, STAGING};

const ID: &str = "nhdogjmejiglipccpnnnanhbledajbpd";

fn setup() -> (tempfile::TempDir, ExtensionStore) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = ExtensionStore::new(dir.path().join("extensions"));
    (dir, store)
}

/// An installed extension as the store leaves it: the version folder and the pointer.
fn installed(store: &ExtensionStore, id: &str, version: &str) {
    let folder = store.root().join(id).join(version);
    std::fs::create_dir_all(&folder).expect("folder");
    std::fs::write(
        folder.join("manifest.json"),
        format!(
            r#"{{"manifest_version":3,"name":"Vue.js devtools","version":"{version}","content_scripts":[]}}"#
        ),
    )
    .expect("manifest");
    std::fs::write(
        store.root().join(id).join(POINTER),
        format!(r#"{{"version":"{version}"}}"#),
    )
    .expect("pointer");
}

fn started_with(store: &ExtensionStore, id: &str) -> StartPlan {
    StartPlan {
        web_store: store.installed_one(id).into_iter().collect(),
        ..StartPlan::default()
    }
}

#[test]
fn a_removal_takes_the_folder_and_says_whether_this_start_loaded_it() {
    let (_dir, store) = setup();
    installed(&store, ID, "7.6.4");
    let removed = remove_in(&store, &StartPlan::default(), ID).expect("removed");
    assert_eq!(
        removed,
        Removed {
            id: ID.to_owned(),
            name: "Vue.js devtools".to_owned(),
            version: Some("7.6.4".to_owned()),
            loaded: false,
        }
    );
    assert!(!store.root().join(ID).exists());
    assert!(store.installed().is_empty());

    installed(&store, ID, "7.6.4");
    let started = started_with(&store, ID);
    let url = format!("https://chromewebstore.google.com/detail/vue/{ID}");
    let removed = remove_in(&store, &started, &url).expect("removed by its store URL");
    assert!(
        removed.loaded,
        "this start loaded it: gone at the next start"
    );
}

#[test]
fn a_folder_without_a_readable_pointer_goes_too() {
    let (_dir, store) = setup();
    std::fs::create_dir_all(store.root().join(ID).join("1.0.0")).expect("folder");
    let removed = remove_in(&store, &StartPlan::default(), ID).expect("removed");
    assert_eq!(removed.name, ID, "no manifest to name it by");
    assert_eq!(removed.version, None);
    assert!(!store.root().join(ID).exists());
}

#[test]
fn nothing_installed_or_no_reference_is_refused() {
    let (_dir, store) = setup();
    let error = remove_in(&store, &StartPlan::default(), ID).expect_err("nothing there");
    assert!(error.contains("not installed"), "{error}");
    // A staging leftover of a crashed install (a pid no process has) is swept, but nothing
    // was installed.
    let leftover = store
        .root()
        .join(STAGING)
        .join(format!("{ID}-{}-0", u32::MAX));
    std::fs::create_dir_all(&leftover).expect("staging");
    let error = remove_in(&store, &StartPlan::default(), ID).expect_err("a leftover only");
    assert!(error.contains("not installed"), "{error}");
    assert!(!leftover.exists(), "the leftover is swept all the same");
    let error = remove_in(&store, &StartPlan::default(), "abc").expect_err("no reference");
    assert!(error.contains("not an extension id"), "{error}");
    let error =
        remove_in(&store, &StartPlan::default(), "https://evil.test/x").expect_err("no store URL");
    assert!(error.contains("not an extension id"), "{error}");
}

#[test]
fn the_rows_name_each_installed_extension_and_whether_this_start_loaded_it() {
    let (_dir, store) = setup();
    installed(&store, ID, "7.6.4");
    let other = "a".repeat(32);
    installed(&store, &other, "1.2.3");
    let started = started_with(&store, ID);
    let rows = rows(&store.installed(), &started);
    assert_eq!(
        rows,
        [
            WebStoreExtension {
                id: other.clone(),
                name: "Vue.js devtools".to_owned(),
                version: "1.2.3".to_owned(),
                loaded: false,
            },
            WebStoreExtension {
                id: ID.to_owned(),
                name: "Vue.js devtools".to_owned(),
                version: "7.6.4".to_owned(),
                loaded: true,
            },
        ]
    );
}

#[test]
fn the_notice_after_a_removal_names_it_and_links_the_switch() {
    let removed = Removed {
        id: ID.to_owned(),
        name: "Vue.js devtools".to_owned(),
        version: Some("7.6.4".to_owned()),
        loaded: true,
    };
    let CoreToShell::Notification(note) = removed_notice(&removed) else {
        panic!("a notification");
    };
    assert_eq!(note.level, NotificationLevel::Info);
    assert_eq!(note.message_key, REMOVED_KEY);
    assert_eq!(
        note.args.get("name").map(String::as_str),
        Some("Vue.js devtools")
    );
    assert_eq!(note.settings_key.as_deref(), Some(web::ALLOW_KEY));
}
