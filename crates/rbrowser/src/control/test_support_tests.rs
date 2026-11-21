use std::sync::Arc;

use rb_commands::validate::bindings_conflict;
use rb_commands::{Binding, CommandId};
use tokio::runtime::Handle;

use super::{free_chord, plugin_bindings};
use crate::cdp::fake::FakeBrowser;
use crate::cdp::lock;
use crate::control::{Core, CoreSetup};
use crate::host::Host;
use crate::host::state::default_config;
use crate::host::ui::recording::RecordingUi;

/// RB-094-10: `V J` is the JSON/XML viewer's; the free chord is never a plugin's or a
/// command's.
#[tokio::test]
async fn a_free_chord_is_held_by_no_plugin_and_no_command() {
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(default_config(), None).expect("opens");
    let core = Core::new(host, ui, Handle::current(), CoreSetup::default());
    let plugins = plugin_bindings();
    let viewer: Binding = "Ctrl+Shift+Space V J".parse().expect("chord");
    assert!(plugins.contains(&viewer), "json-xml-viewer binds V J");
    let chord = free_chord(&core, "view.back");
    let binding: Binding = chord.parse().expect("chord");
    assert!(!plugins.iter().any(|held| bindings_conflict(held, &binding)));
    let id = CommandId::new("view.back").expect("id");
    assert!(lock(&core.registry).check_for(&id, &binding).is_ok());
}
