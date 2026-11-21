use rb_config::ConfigFile;
use rb_engine::ViewHost as _;
use serde_json::{Value, json};

use super::super::view_edit::{ADD_SIZE, run};
use super::*;
use crate::breakpoint_keys::FreeSize;
use crate::host::ui::recording::UiCall;
use crate::layout::test_support::{Setup, config, setup_in};
use crate::page_window::PageWindow;

/// The test config on disk as `config.json` and a core over it.
fn project() -> (tempfile::TempDir, Setup) {
    let dir = tempfile::tempdir().expect("a temporary folder");
    let root = dir.path().to_path_buf();
    std::fs::create_dir_all(root.join(".rbrowser")).expect("the config folder");
    let value = serde_json::to_value(config()).expect("serializes");
    rb_config::write_config_file(&root, ConfigFile::Project, &value).expect("the project file");
    let setup = setup_in(config(), Some(root));
    (dir, setup)
}

fn local(dir: &tempfile::TempDir) -> Value {
    rb_config::read_json(&ConfigFile::Local.path(dir.path())).expect("the local file")
}

fn open(core: &Core) -> Vec<String> {
    core.host()
        .views()
        .into_iter()
        .filter_map(|view| view.cell_breakpoint)
        .collect()
}

/// What went to the shell's pages.
fn scripts(setup: &Setup) -> Vec<String> {
    setup
        .ui
        .calls()
        .into_iter()
        .filter_map(|call| match call {
            UiCall::Shell(script) => Some(script),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_device_adds_a_view_of_its_size_and_pixel_ratio() {
    let (dir, setup) = project();
    let core = &setup.core;
    let device = crate::emulation::device_list()
        .into_iter()
        .next()
        .expect("a built-in device preset");
    let answer = run(core, ADD_SIZE, &json!({ "device": device.id })).expect("added");
    assert_eq!(answer["breakpoint"], json!(device.id));
    assert_eq!(answer["device"], json!(device.id));
    assert_eq!(answer["width"], json!(device.viewport.width));
    assert_eq!(answer["height"], json!(device.viewport.height));
    assert_eq!(answer["dpr"], json!(device.dpr));
    assert_eq!(answer["added"], json!(true));
    // The breakpoint names the preset: its size, pixel ratio, user agent and touch all apply.
    let written = &local(&dir)["breakpoints"][0];
    assert_eq!(*written, json!({ "id": device.id, "device": device.id }));
    assert!(open(core).contains(&device.id), "{:?}", open(core));
    let view = core
        .host()
        .views()
        .into_iter()
        .find(|view| view.cell_breakpoint.as_deref() == Some(device.id.as_str()))
        .expect("the device's view");
    // The grid gives the cell its row's height; the width is the device's.
    assert_eq!(view.width, device.viewport.width);
    // Once more: the breakpoint is there already, nothing is added.
    let again = run(core, ADD_SIZE, &json!({ "device": device.id })).expect("open");
    assert_eq!(again["added"], json!(false));
    // Under a name of its own.
    let named = run(
        core,
        ADD_SIZE,
        &json!({ "device": device.id, "name": "phone-2" }),
    )
    .expect("named");
    assert_eq!(named["breakpoint"], json!("phone-2"));
    assert_eq!(
        local(&dir)["breakpoints"][1],
        json!({ "id": "phone-2", "device": device.id })
    );
}

#[tokio::test]
async fn a_device_is_refused_when_unknown_or_beside_a_size() {
    let (_dir, setup) = project();
    let core = &setup.core;
    let device = crate::emulation::device_list()
        .into_iter()
        .next()
        .expect("a built-in device preset");
    for args in [
        json!({ "device": "no-such-device" }),
        json!({ "device": device.id, "width": 100, "height": 100 }),
        json!({ "device": device.id, "dpr": 2 }),
        json!({ "width": 100 }),
    ] {
        let error = run(core, ADD_SIZE, &args).expect_err("refused");
        assert!(
            matches!(error, CommandError::InvalidArgs(_)),
            "{args}: {error}"
        );
    }
    assert_eq!(open(core).len(), 3, "nothing was opened");
}

#[tokio::test]
async fn without_arguments_the_picker_opens_in_the_palette_and_custom_opens_the_bar() {
    let (_dir, setup) = project();
    let core = &setup.core;
    assert_eq!(
        run(core, ADD_SIZE, &Value::Null).expect("opens"),
        Value::Null
    );
    let windows: Vec<String> = setup
        .ui
        .calls()
        .into_iter()
        .filter_map(|call| match call {
            UiCall::PageWindow(PageWindow::Palette, url) => Some(url),
            _ => None,
        })
        .collect();
    assert_eq!(windows.len(), 1, "the palette's popup is shown");
    assert!(windows[0].ends_with("#/palette/new-view"), "{windows:?}");
    let sent = scripts(&setup);
    assert!(
        sent.iter()
            .any(|script| script.contains("uiCommand") && script.contains("\"view.add_size\"")),
        "{sent:?}"
    );
    assert!(
        !sent.iter().any(|script| script.contains(ADD_SIZE_CUSTOM)),
        "{sent:?}"
    );
    // "Custom...": the size input in the toolbar, no popup.
    assert_eq!(
        run(core, ADD_SIZE, &json!({ "custom": true })).expect("opens"),
        Value::Null
    );
    assert!(
        scripts(&setup)
            .iter()
            .any(|script| script.contains("uiCommand") && script.contains(ADD_SIZE_CUSTOM)),
    );
    assert!(
        setup.ui.calls().contains(&UiCall::FocusShell),
        "the bar's fields take the typing at once"
    );
    let popups = setup
        .ui
        .calls()
        .into_iter()
        .filter(|call| matches!(call, UiCall::PageWindow(..)))
        .count();
    assert_eq!(popups, 1, "the bar opens without the palette");
}

#[tokio::test]
async fn an_added_view_is_named_in_the_status_bar_with_its_size() {
    let (_dir, setup) = project();
    let core = &setup.core;
    run(
        core,
        ADD_SIZE,
        &json!({ "width": 1000, "height": 1000, "name": "testsetestt" }),
    )
    .expect("added");
    let events: Vec<String> = setup
        .ui
        .calls()
        .into_iter()
        .filter_map(|call| match call {
            UiCall::StatusEvent(script) => Some(script),
            _ => None,
        })
        .collect();
    let said = events
        .iter()
        .find(|script| script.contains(KEY_ADDED))
        .unwrap_or_else(|| panic!("no message about the new view: {events:?}"));
    assert!(said.contains("testsetestt"), "{said}");
    assert!(said.contains("1000"), "{said}");
}

// Review of RB-080-46: a user's device id only has to be non-empty.
#[test]
fn a_device_id_that_is_no_breakpoint_id_gets_one_made_from_it() {
    let device = |id: &str, label: Option<&str>| {
        let mut device = crate::emulation::device_list()
            .into_iter()
            .next()
            .expect("a built-in device preset");
        device.id = id.to_owned();
        device.label = label.map(str::to_owned);
        device
    };
    let breakpoint = |id: &str, device: Option<&str>| {
        let mut breakpoint = FreeSize::new(10, 10, None).expect("a size").breakpoint();
        breakpoint.id = id.to_owned();
        breakpoint.device = device.map(str::to_owned);
        breakpoint
    };
    let phone = device("My Phone (2024)!", None);
    assert_eq!(device_breakpoint_id(&phone, &[]), "My-Phone-2024");
    assert!(breakpoint_id(&device_breakpoint_id(&phone, &[])).is_ok());
    // Nothing usable in the id: the label, then `device`.
    assert_eq!(
        device_breakpoint_id(&device("???", Some("B\u{fc}ro Tablet")), &[]),
        "B-ro-Tablet"
    );
    assert_eq!(device_breakpoint_id(&device("#", None), &[]), "device");
    let long = device(&"x".repeat(80), None);
    assert!(breakpoint_id(&format!("{}-99", device_breakpoint_id(&long, &[]))).is_ok());
    // A breakpoint of that id for another thing is not taken over; this device's own is.
    let taken = [
        breakpoint("My-Phone-2024", None),
        breakpoint("My-Phone-2024-2", Some("other")),
    ];
    assert_eq!(device_breakpoint_id(&phone, &taken), "My-Phone-2024-3");
    let own = [
        breakpoint("My-Phone-2024", None),
        breakpoint("My-Phone-2024-2", Some(&phone.id)),
    ];
    assert_eq!(device_breakpoint_id(&phone, &own), "My-Phone-2024-2");
}

// Review of RB-080-46: the answer and the notice say what is really open.
#[tokio::test]
async fn an_existing_breakpoint_is_described_as_it_is_and_another_size_under_its_name_is_refused() {
    let (_dir, setup) = project();
    let core = &setup.core;
    let notices = |setup: &Setup| {
        setup
            .ui
            .calls()
            .into_iter()
            .filter(
                |call| matches!(call, UiCall::StatusEvent(script) if script.contains(KEY_ADDED)),
            )
            .count()
    };
    let tablet = core
        .host()
        .config()
        .breakpoints
        .iter()
        .find(|breakpoint| breakpoint.id == "tablet")
        .map(|breakpoint| size_of(breakpoint, &[]))
        .expect("the test config has a tablet");
    // Another size under the name of an open breakpoint: refused, nothing said.
    let error = run(
        core,
        ADD_SIZE,
        &json!({ "width": tablet.0 + 1, "height": tablet.1, "name": "tablet" }),
    )
    .expect_err("refused");
    assert!(
        matches!(&error, CommandError::InvalidArgs(text) if text.contains("tablet")),
        "{error}"
    );
    let device = crate::emulation::device_list()
        .into_iter()
        .next()
        .expect("a device");
    let error = run(
        core,
        ADD_SIZE,
        &json!({ "device": device.id, "name": "tablet" }),
    )
    .expect_err("refused");
    assert!(matches!(error, CommandError::InvalidArgs(_)), "{error}");
    assert_eq!(notices(&setup), 0);
    // Its own size: it is open already - nothing changes, no notice.
    let same = run(
        core,
        ADD_SIZE,
        &json!({ "width": tablet.0, "height": tablet.1, "name": "tablet" }),
    )
    .expect("open");
    assert_eq!(same["added"], json!(false));
    assert_eq!(same["changed"], json!(false));
    assert_eq!(
        (same["width"].as_u64(), same["height"].as_u64()),
        (Some(u64::from(tablet.0)), Some(u64::from(tablet.1)))
    );
    assert_eq!(notices(&setup), 0);
    // Hidden, it opens again: that is news, with its own size.
    run(
        core,
        super::super::view_edit::REMOVE,
        &json!({ "breakpoint": "tablet" }),
    )
    .expect("removed");
    let again = run(
        core,
        ADD_SIZE,
        &json!({ "width": tablet.0, "height": tablet.1, "name": "tablet" }),
    )
    .expect("opened");
    assert_eq!(again["added"], json!(false));
    assert_eq!(again["changed"], json!(true));
    assert_eq!(notices(&setup), 1);
}

/// RB-095-02: an Android device is a breakpoint of its own engine at the device's size, with
/// its label; adding it again opens the same one whatever size the device reports now.
#[tokio::test]
async fn an_engine_view_is_a_breakpoint_of_that_engine_at_its_size() {
    let (dir, setup) = project();
    let core = &setup.core;
    let size = FreeSize::new(412, 915, Some(2.625)).expect("a size");
    let answer = add_engine_view(
        core,
        "android-R58N123",
        size,
        rb_config::EngineKind::Android,
        "Pixel 8 (device size)",
    )
    .expect("added");
    assert_eq!(answer["breakpoint"], json!("android-R58N123"));
    assert_eq!(answer["added"], json!(true));
    assert_eq!(
        local(&dir)["breakpoints"][0],
        json!({
            "id": "android-R58N123",
            "label": "Pixel 8 (device size)",
            "width": 412,
            "height": 915,
            "emulation": { "dpr": 2.625 },
            "engine": "android",
        })
    );
    // Its view is the Android engine's, which this test core does not run: the breakpoint is
    // shown (not hidden), and the engine opens it.
    assert!(
        !local(&dir)["layout"]["hidden"]
            .as_array()
            .is_some_and(|hidden| hidden.contains(&json!("android-R58N123")))
    );
    let rotated = FreeSize::new(915, 412, Some(2.625)).expect("a size");
    let again = add_engine_view(
        core,
        "android-R58N123",
        rotated,
        rb_config::EngineKind::Android,
        "Pixel 8 (device size)",
    )
    .expect("the same breakpoint");
    assert_eq!(again["added"], json!(false));
}
