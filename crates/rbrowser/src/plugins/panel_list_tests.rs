use rb_config::{PluginId, UserSettings};
use serde_json::json;

use super::*;

fn hiding(ids: &[&str]) -> UserSettings {
    let mut user = UserSettings::default();
    user.plugins.panels_hidden = ids.iter().map(|id| PluginId((*id).to_owned())).collect();
    user
}

#[test]
fn the_event_asks_the_pages_for_the_list_again() {
    assert_eq!(changed_message(), json!({ "panelsChanged": true }));
}

/// RB-080-97: the first time is news - a page that asked while the plugins loaded holds no list
/// at all; after that only what the panels offered depend on: the hidden panels and the project
/// plugins' trust (a window loads its plugins once).
#[test]
fn the_first_time_and_then_only_hidden_panels_or_trust_are_news() {
    let mut last = None;
    assert!(take(&mut last, Inputs::of(&UserSettings::default(), false)));
    assert!(!take(
        &mut last,
        Inputs::of(&UserSettings::default(), false)
    ));
    // Any other setting written: nothing to tell.
    let mut other = UserSettings::default();
    other.ui.syntax_highlight = !other.ui.syntax_highlight;
    other.plugins.show_bundled = !other.plugins.show_bundled;
    assert!(!take(&mut last, Inputs::of(&other, false)));
    assert!(take(&mut last, Inputs::of(&hiding(&["console"]), false)));
    assert!(!take(&mut last, Inputs::of(&hiding(&["console"]), false)));
    assert!(take(&mut last, Inputs::of(&hiding(&["console"]), true)));
    assert!(take(&mut last, Inputs::of(&hiding(&[]), true)));
    // The order of the list is no change.
    let _ = take(&mut last, Inputs::of(&hiding(&["a", "b"]), true));
    assert!(!take(&mut last, Inputs::of(&hiding(&["b", "a"]), true)));
}
