use serde_json::{Map, Value, json};

use super::with_shell_keys;

#[test]
fn a_panel_gets_its_plugins_keys_and_the_syntax_colours_switch() {
    let mut own = Map::new();
    own.insert("console.levels".to_owned(), json!(["error"]));
    let user = rb_config::UserSettings::default();
    assert_eq!(
        with_shell_keys(own.clone(), &user),
        json!({
            "console.levels": ["error"],
            "ui.syntaxHighlight": true,
            "plugins.panelMessageMaxBytes": 4 * 1024 * 1024,
        }),
        "RB-080-65: the panels colour code by default"
    );
    let mut off = rb_config::UserSettings::default();
    off.ui.syntax_highlight = false;
    assert_eq!(
        with_shell_keys(own, &off)["ui.syntaxHighlight"],
        Value::Bool(false)
    );
}

/// RB-090-44: the shell's bridge and the panels get the message limit in effect, held to its
/// range.
#[test]
fn a_panel_gets_the_message_limit_in_effect() {
    let mut user = rb_config::UserSettings::default();
    user.plugins.panel_message_max_bytes = 1024 * 1024;
    assert_eq!(
        with_shell_keys(Map::new(), &user)["plugins.panelMessageMaxBytes"],
        json!(1024 * 1024)
    );
    user.plugins.panel_message_max_bytes = 1;
    assert_eq!(
        with_shell_keys(Map::new(), &user)["plugins.panelMessageMaxBytes"],
        json!(64 * 1024)
    );
}
