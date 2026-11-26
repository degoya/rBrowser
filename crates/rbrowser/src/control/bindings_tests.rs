use std::path::Path;
use std::sync::{Arc, mpsc};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
use crate::control::test_support::free_chord;
use crate::host::Host;
use crate::host::state::default_config;
use crate::host::ui::recording::RecordingUi;

fn core_at(path: &Path) -> Arc<Core> {
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(default_config(), None).expect("opens");
    Core::new(
        host,
        ui,
        Handle::current(),
        CoreSetup {
            user_path: Some(path.to_path_buf()),
            ..CoreSetup::default()
        },
    )
}

fn saved(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).expect("written")).expect("json")
}

/// RB-099-19, the lost update: a setting change in progress (`settings_window::set_setting`)
/// holds the one-change lock, has read the user file and writes it back with its key. A binding
/// change at that moment waits for it - before, it wrote at once, and the setting's write put
/// the old bindings back.
#[tokio::test(flavor = "multi_thread")]
async fn a_binding_change_waits_for_a_setting_change_in_progress() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("settings.json");
    rb_config::write_json(&path, &json!({})).expect("an empty user file");
    let core = core_at(&path);
    let chord = free_chord(&core, "view.back");

    let one_change = lock(&crate::settings_window::ONE_CHANGE);
    let mut read = rb_config::read_json(&path).expect("read");

    let (tx, rx) = mpsc::channel();
    let (binder, texts, runtime) = (core.clone(), vec![chord.clone()], Handle::current());
    let binding = std::thread::spawn(move || {
        let _runtime = runtime.enter();
        let answer = binder.set_bindings("view.back", &texts).map(drop);
        let _ = tx.send(());
        answer
    });
    // A give-up wait, no bound: without the lock the binding is on disk by now.
    let wrote_beside = rx.recv_timeout(Duration::from_millis(200)).is_ok();
    read["ui"] = json!({ "theme": "dark" });
    rb_config::write_json(&path, &read).expect("the setting's write");
    drop(one_change);
    binding.join().expect("joined").expect("set");

    assert!(
        !wrote_beside,
        "the binding was written beside the setting change"
    );
    let saved = saved(&path);
    assert_eq!(saved["ui"]["theme"], "dark", "{saved}");
    assert_eq!(
        saved["shortcuts"]["bindings"]["view.back"],
        json!([chord]),
        "{saved}"
    );
}

/// RB-099-19: the real setting change and a binding change from two threads at once - both end
/// up in the file and in effect, whichever came first.
#[tokio::test(flavor = "multi_thread")]
async fn a_setting_and_a_binding_from_two_threads_both_hold() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("settings.json");
    let core = core_at(&path);
    let chord = free_chord(&core, "view.back");
    let runtime = Handle::current();
    let (setter, binder) = (core.clone(), core.clone());
    let setting = {
        let runtime = runtime.clone();
        std::thread::spawn(move || {
            let _runtime = runtime.enter();
            crate::settings_window::set_setting(
                &setter,
                "ui.theme",
                &json!("dark"),
                rb_settings::Scope::User,
                crate::settings_window::Caller::Shell,
            )
        })
    };
    let texts = vec![chord.clone()];
    let binding = std::thread::spawn(move || {
        let _runtime = runtime.enter();
        binder.set_bindings("view.back", &texts).map(drop)
    });
    assert_eq!(setting.join().expect("joined"), CoreToShell::Ack);
    binding.join().expect("joined").expect("set");

    let saved = saved(&path);
    assert_eq!(saved["ui"]["theme"], "dark", "{saved}");
    assert_eq!(
        saved["shortcuts"]["bindings"]["view.back"],
        json!([chord.clone()]),
        "{saved}"
    );
    let user = core.user_settings();
    assert_eq!(user.shortcuts.bindings.get("view.back"), Some(&vec![chord]));
}
