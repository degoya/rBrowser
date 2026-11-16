use rb_commands::{Binding, McpCoverage, builtin};
use rb_ipc::{CoreToShell, ShellToCore, encode_response};
use rb_settings::Category;
use serde_json::Value;

use super::*;

#[test]
fn get_about_is_answered_with_the_facts() {
    let Some(CoreToShell::About(info)) = answer(&ShellToCore::GetAbout) else {
        panic!("getAbout must be answered with `about`");
    };
    assert_eq!(info, facts());
}

#[test]
fn other_messages_are_not_this_modules() {
    assert!(answer(&ShellToCore::GetState).is_none());
}

/// The acceptance criterion: `app_info` over MCP gives the same versions as the page.
#[test]
fn app_info_answers_what_the_page_shows() {
    let Some(page) = answer(&ShellToCore::GetAbout) else {
        panic!("getAbout must be answered");
    };
    let mut shown: Value =
        serde_json::from_str(&encode_response(&page).expect("the answer encodes"))
            .expect("the answer is JSON");
    shown.as_object_mut().expect("an object").remove("type");
    assert_eq!(app_info(), shown);
    for field in ["version", "commit", "cef", "chromium", "platform", "apis"] {
        assert!(app_info().get(field).is_some(), "app_info lacks {field}");
    }
}

#[test]
fn the_category_the_command_opens_exists() {
    assert_eq!(Category::from_id(CATEGORY), Some(Category::About));
}

/// `app.about` is registered with `Ctrl+Shift+Space , A` and covered by `app_info`.
#[test]
fn the_command_is_defined_with_its_chord_and_tool() {
    let def = builtin::definitions()
        .into_iter()
        .find(|def| def.id.as_str() == COMMAND)
        .expect("app.about is a core command");
    let chord: Binding = "Ctrl+Shift+Space , A".parse().expect("a chord");
    assert_eq!(def.default_bindings, [chord]);
    assert_eq!(def.mcp, McpCoverage::Covered(APP_INFO_TOOL.to_owned()));
}
