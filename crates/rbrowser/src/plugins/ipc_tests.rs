use serde_json::{Map, Value, json};

use super::with_shell_keys;

#[test]
fn a_panel_gets_its_plugins_keys_and_the_syntax_colours_switch() {
    let mut own = Map::new();
    own.insert("console.levels".to_owned(), json!(["error"]));
    let user = rb_config::UserSettings::default();
    assert_eq!(
        with_shell_keys(own.clone(), &user),
        json!({ "console.levels": ["error"], "ui.syntaxHighlight": true }),
        "RB-080-65: the panels colour code by default"
    );
    let mut off = rb_config::UserSettings::default();
    off.ui.syntax_highlight = false;
    assert_eq!(
        with_shell_keys(own, &off)["ui.syntaxHighlight"],
        Value::Bool(false)
    );
}
