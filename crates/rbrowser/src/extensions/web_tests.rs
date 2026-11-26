use rb_ipc::{CoreToShell, NotificationLevel};
use serde_json::{Value, json};

use super::*;

#[test]
fn the_install_and_remove_tools_take_an_id_or_a_store_url() {
    let id = "nhdogjmejiglipccpnnnanhbledajbpd";
    assert_eq!(
        tool_args(&json!({ "id": id }), INSTALL_TOOL).expect("id"),
        id
    );
    let url = format!("https://chromewebstore.google.com/detail/vue/{id}");
    assert_eq!(
        tool_args(&json!({ "id": url }), INSTALL_TOOL).expect("url"),
        url
    );
    assert_eq!(
        tool_args(&json!({ "id": format!(" {id} ") }), REMOVE_TOOL).expect("trimmed"),
        id
    );
    for bad in [
        json!({}),
        json!(null),
        json!({ "id": "" }),
        json!({ "id": "abc" }),
        json!({ "id": 7 }),
        json!({ "id": "https://evil.test/x" }),
    ] {
        for tool in [INSTALL_TOOL, REMOVE_TOOL] {
            let error = tool_args(&bad, tool).expect_err("refused");
            assert_eq!(error.code, "args.invalid", "{bad}");
            assert!(error.message.contains(tool), "{}", error.message);
        }
    }
}

/// RB-080-30: the one extraction of a written reference, the commands' `ref` and the tools'
/// `id` alike: trimmed, never empty, nothing but a string.
#[test]
fn a_reference_argument_is_read_trimmed_once() {
    assert_eq!(
        ref_arg(
            &json!({ "ref": " nhdogjmejiglipccpnnnanhbledajbpd " }),
            "ref"
        )
        .as_deref(),
        Some("nhdogjmejiglipccpnnnanhbledajbpd")
    );
    assert_eq!(ref_arg(&json!({ "id": "x" }), "id").as_deref(), Some("x"));
    assert_eq!(ref_arg(&json!({ "ref": "   " }), "ref"), None);
    assert_eq!(ref_arg(&json!({ "ref": "" }), "ref"), None);
    assert_eq!(ref_arg(&json!({}), "ref"), None);
    assert_eq!(ref_arg(&Value::Null, "ref"), None);
    assert_eq!(ref_arg(&json!({ "ref": 3 }), "ref"), None);
    assert_eq!(ref_arg(&json!({ "id": "x" }), "ref"), None);
}

#[test]
fn a_web_store_notice_links_the_switch() {
    let CoreToShell::Notification(note) = notice(
        NotificationLevel::Info,
        INSTALLED_KEY,
        &[("name", "Vue".to_owned())],
    ) else {
        panic!("a notification");
    };
    assert_eq!(note.settings_key.as_deref(), Some(ALLOW_KEY));
    assert_eq!(note.message_key, INSTALLED_KEY);
}

/// RB-090-85: a person's update check names its start with the number of extensions.
#[test]
fn an_update_check_names_its_start() {
    let CoreToShell::Notification(notification) = checking_notice(2) else {
        panic!("a notification");
    };
    assert_eq!(notification.message_key, CHECKING_KEY);
    assert_eq!(notification.level, NotificationLevel::Info);
    assert_eq!(notification.args["count"], "2");
    assert_eq!(notification.settings_key.as_deref(), Some(ALLOW_KEY));
}
