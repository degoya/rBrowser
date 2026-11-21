use std::sync::Arc;

use rb_commands::{Binding, McpCoverage};
use rb_config::EngineKind;
use rb_engine::ViewId;
use rb_ipc::{CoreToShell, NotificationLevel};
use rb_safari::ios::fake::{DEVICE, FakeIosTools, SIMULATOR};
use rb_safari::ios::{Device, Inventory, Simulator};
use rb_safari::testing::FakeSafari;
use serde_json::json;
use tokio::runtime::Handle;

use super::*;
use crate::breakpoint_keys::engine_pick;
use crate::engines::ios::tests::{setup_on, start};
use crate::engines::tests::until;

fn inventory() -> Inventory {
    let sim = |udid: &str, name: &str, version: Vec<u32>| Simulator {
        udid: udid.to_owned(),
        name: name.to_owned(),
        runtime: format!("iOS {}.{}", version[0], version[1]),
        version,
        booted: false,
    };
    Inventory {
        xcode: true,
        simulators: vec![
            sim(SIMULATOR, "iPhone 16", vec![18, 2]),
            sim(
                "11111111-2222-3333-4444-555555555555",
                "iPhone 16",
                vec![17, 5],
            ),
            sim(
                "AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE",
                "iPhone SE (3rd generation)",
                vec![17, 5],
            ),
        ],
        devices: vec![Device {
            udid: DEVICE.to_owned(),
            name: "iPhone".to_owned(),
        }],
    }
}

#[test]
fn the_command_is_leader_b_i_and_covered_by_ios_devices() {
    let def = definition().expect("a definition");
    assert_eq!(def.id.as_str(), DEVICES);
    assert_eq!(def.title_key, "commands.ios.devices");
    assert_eq!(
        def.default_bindings,
        ["Ctrl+Shift+Space B I"
            .parse::<Binding>()
            .expect("a binding")]
    );
    assert_eq!(def.mcp, McpCoverage::Covered(DEVICES_TOOL.to_owned()));
}

#[test]
fn the_menu_rows_name_one_simulator_per_model_and_the_device() {
    let rows = device_items(&inventory(), Some("iPhone 16"), true, None);
    let picks: Vec<Option<&str>> = rows.iter().map(|row| row.ios_device.as_deref()).collect();
    // A model writes its name (it holds on every Mac); a device its UDID.
    assert_eq!(
        picks,
        [
            Some("iPhone 16"),
            Some("iPhone SE (3rd generation)"),
            Some(DEVICE)
        ]
    );
    assert!(rows.iter().all(|row| row.engine == EngineKind::Ios));
    assert!(rows[0].checked && !rows[1].checked && !rows[2].checked);
    assert!(
        rows[0].label.contains("iPhone 16 (iOS 18.2)") || rows[0].label == ROW_KEY,
        "{}",
        rows[0].label
    );
    // Not on iOS: nothing is checked.
    let rows = device_items(&inventory(), Some("iPhone 16"), false, None);
    assert!(rows.iter().all(|row| !row.checked));
    // A reason disables every row.
    let rows = device_items(&inventory(), None, false, Some("no".to_owned()));
    assert!(rows.iter().all(|row| !row.available()));
}

#[test]
fn the_menu_lists_at_most_its_rows() {
    let mut many = inventory();
    many.simulators = (0..40)
        .map(|n| Simulator {
            udid: format!("{n:08X}-2222-3333-4444-555555555555"),
            name: format!("iPhone {n}"),
            runtime: "iOS 18.2".to_owned(),
            version: vec![18, 2],
            booted: false,
        })
        .collect();
    assert_eq!(device_items(&many, None, false, None).len(), MAX_ROWS);
}

#[test]
fn a_mac_with_neither_way_gets_a_notice_with_both_and_the_setting() {
    let Some(CoreToShell::Notification(notice)) = empty_notice(&Inventory::default()) else {
        panic!("a notice");
    };
    assert_eq!(notice.level, NotificationLevel::Warning);
    assert_eq!(notice.message_key, "engines.ios.no.way");
    assert_eq!(notice.settings_key.as_deref(), Some("engines.ios.mode"));
    let bare = Inventory {
        xcode: true,
        ..Inventory::default()
    };
    assert!(matches!(
        empty_notice(&bare),
        Some(CoreToShell::Notification(notice)) if notice.message_key == "engines.ios.no.simulator"
    ));
    assert!(empty_notice(&inventory()).is_none());
}

#[test]
fn a_bad_device_name_is_refused() {
    assert!(matches!(
        parse(&json!({ "device": "iPhone\n16" })),
        Err(CommandError::InvalidArgs(_))
    ));
    assert!(matches!(
        parse(&json!({ "bogus": 1 })),
        Err(CommandError::InvalidArgs(_))
    ));
    let args = parse(&json!({ "device": "iPhone 16", "view": 3 })).expect("args");
    assert_eq!(args.device.as_deref(), Some("iPhone 16"));
    assert_eq!(args.view, Some(ViewId(3)));
    assert!(
        parse(&serde_json::Value::Null)
            .expect("none")
            .device
            .is_none()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_badge_menu_lists_the_devices_under_ios() {
    let core = setup_on(Handle::current());
    let fake = FakeSafari::start().await;
    let tools = Arc::new(FakeIosTools::with_xcode());
    start(&core, &fake, &tools);
    until(|| {
        engine_pick::engine_pick(&core, ViewId(3))
            .is_some_and(|items| items.iter().any(|item| item.ios_device.is_some()))
    })
    .await;
    let items = engine_pick::engine_pick(&core, ViewId(3)).expect("the view");
    let ios = items
        .iter()
        .position(|item| item.engine == EngineKind::Ios && item.ios_device.is_none())
        .expect("the iOS row");
    let row = &items[ios + 1];
    assert_eq!(row.ios_device.as_deref(), Some("iPhone 16"));
    assert_eq!(
        engine_pick::command_args(row, ViewId(3)),
        json!({ "view": 3, "engine": "ios", "iosDevice": "iPhone 16" })
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn ios_devices_answers_the_inventory_and_the_default_target() {
    let core = setup_on(Handle::current());
    let fake = FakeSafari::start().await;
    let tools = Arc::new(FakeIosTools::with_device());
    start(&core, &fake, &tools);
    let answer = devices_tool(&core).await.expect("an answer");
    assert_eq!(answer["supported"], true);
    assert_eq!(answer["xcode"], false);
    assert_eq!(answer["mode"], "auto");
    assert_eq!(answer["devices"][0]["udid"], DEVICE);
    assert_eq!(answer["default"]["udid"], DEVICE);
    assert_eq!(answer["default"]["simulator"], false);
    tools.set(Inventory::default());
    let answer = devices_tool(&core).await.expect("an answer");
    assert_eq!(answer["code"], "engines.ios.no.way");
    assert_eq!(answer["setting"], "engines.ios.mode");
}
