use rb_commands::{Binding, McpCoverage};

use super::*;
use crate::engines::android::devices::parse_devices;

const LISTING: &str = "List of devices attached\n\
    S1 device model:Pixel_8\n\
    S2 unauthorized usb:1\n";

#[test]
fn the_commands_have_their_chords_and_their_mcp_side() {
    let defs = definitions().expect("valid");
    let add = defs.iter().find(|def| def.id.as_str() == ADD).expect("add");
    assert_eq!(
        add.default_bindings,
        vec!["Ctrl+Shift+Space V ;".parse::<Binding>().expect("a chord")]
    );
    assert_eq!(add.mcp, McpCoverage::Covered(ADD_TOOL.to_owned()));
    assert_eq!(add.title_key, "commands.view.add_android");
    let install = defs
        .iter()
        .find(|def| def.id.as_str() == INSTALL)
        .expect("install");
    assert_eq!(
        install.default_bindings,
        vec!["Ctrl+Shift+Space X S".parse::<Binding>().expect("a chord")]
    );
    // The download is the person's decision: MCP never reaches it.
    assert!(matches!(install.mcp, McpCoverage::Omitted(_)));
}

/// `V J`, the job's chord, is the JSON viewer plugin's `toggle_raw`; the command takes `V ;`.
#[test]
fn the_chords_are_free_of_the_bundled_plugins() {
    let held = crate::control::test_support::plugin_bindings();
    for def in definitions().expect("valid") {
        for binding in &def.default_bindings {
            assert!(
                !held
                    .iter()
                    .any(|plugin| rb_commands::validate::bindings_conflict(plugin, binding)),
                "{} {binding}",
                def.id.as_str()
            );
        }
    }
}

#[test]
fn without_a_serial_the_one_ready_device_is_taken() {
    let devices = parse_devices(LISTING);
    assert_eq!(pick(None, &devices).map(|d| d.serial.as_str()), Ok("S1"));
    assert_eq!(
        pick(Some("S1"), &devices).map(|d| d.serial.as_str()),
        Ok("S1")
    );
}

#[test]
fn a_device_that_is_not_there_or_not_ready_is_refused_with_the_reason() {
    let devices = parse_devices(LISTING);
    let refused = pick(Some("S2"), &devices).expect_err("unauthorized");
    assert_eq!(refused.code, "android.device_not_ready");
    assert_eq!(
        refused.message,
        rb_i18n::t_with("engines.android.unauthorized", &[("serial", "S2")])
    );
    let missing = pick(Some("S9"), &devices).expect_err("missing");
    assert_eq!(missing.code, "android.no_device");
    let bad = pick(Some("-a"), &devices).expect_err("bad");
    assert_eq!(bad.code, "invalid_arguments");
    let none = pick(None, &[]).expect_err("none");
    assert_eq!(none.code, "android.no_device");
    assert_eq!(none.message, no_device_message(cfg!(windows)));
    assert_eq!(none.setting, None);
}

// RB-096-10: an empty list names what keeps a connected phone from adb - the cable and its USB
// mode, USB debugging, on Windows the maker's driver, Samsung's Auto Blocker.
#[test]
fn no_device_names_the_steps_that_bring_one_to_adb() {
    let windows = no_device_message(true);
    for key in [
        "engines.android.noDevice",
        "engines.android.noDeviceUsb",
        "engines.android.noDeviceDriver",
        "engines.android.noDeviceSamsung",
    ] {
        assert!(windows.contains(&rb_i18n::t(key)), "{key}: {windows}");
    }
    assert!(windows.contains("File transfer"), "{windows}");
    assert!(windows.contains("Auto Blocker"), "{windows}");
    let elsewhere = no_device_message(false);
    assert!(!elsewhere.contains(&rb_i18n::t("engines.android.noDeviceDriver")));
    assert!(elsewhere.contains(&rb_i18n::t("engines.android.noDeviceSamsung")));
}

#[test]
fn several_ready_devices_need_a_serial() {
    let devices = parse_devices("List of devices attached\nA device\nB device\n");
    let refused = pick(None, &devices).expect_err("several");
    assert_eq!(refused.code, "invalid_arguments");
    assert!(refused.message.contains('A') && refused.message.contains('B'));
}

#[test]
fn the_device_list_answers_state_and_model_and_never_a_secret() {
    let devices = parse_devices(LISTING);
    let answer = devices_answer(true, None, None, &devices);
    assert_eq!(answer["enabled"], true);
    assert_eq!(answer["devices"][0]["serial"], "S1");
    assert_eq!(answer["devices"][0]["ready"], true);
    assert_eq!(answer["devices"][0]["breakpoint"], "android-S1");
    assert_eq!(answer["devices"][1]["state"], "unauthorized");
    assert_eq!(answer["devices"][1]["ready"], false);
    let off = devices_answer(false, None, Some("off"), &[]);
    assert_eq!(off["reason"], "off");
    assert_eq!(off["adb"], serde_json::Value::Null);
}

#[test]
fn a_refusal_becomes_a_tool_failure_with_its_setting() {
    let refusal = Refusal {
        code: "android.disabled",
        message: "off".to_owned(),
        setting: Some(ENABLED_SETTING),
    };
    let failure = rb_mcp::ToolFailure::from(refusal);
    assert_eq!(failure.code, "android.disabled");
    assert_eq!(failure.settings_key.as_deref(), Some(ENABLED_SETTING));
}
