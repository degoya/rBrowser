use std::sync::Arc;
use std::time::Instant;

use rb_commands::keymap::{EVENTFLAG_CONTROL_DOWN, EVENTFLAG_SHIFT_DOWN};
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::{Core, CoreSetup, KeyDown};
use crate::host::Host;
use crate::host::state::default_config;
use crate::host::ui::recording::RecordingUi;

fn core() -> Arc<Core> {
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(default_config(), None).expect("opens");
    Core::new(host, ui, Handle::current(), CoreSetup::default())
}

fn leader() -> KeyDown {
    KeyDown {
        flags: EVENTFLAG_CONTROL_DOWN | EVENTFLAG_SHIFT_DOWN,
        windows_key_code: 0x20,
        character: Some(' '),
        unmodified_character: Some(' '),
    }
}

#[test]
fn devtools_browsers_report_the_devtools_focus() {
    assert_eq!(KeySource::Shell.focus(), Focus::Shell);
    assert_eq!(KeySource::View.focus(), Focus::View);
    assert_eq!(KeySource::Devtools.focus(), Focus::Devtools);
}

#[tokio::test]
async fn disable_in_devtools_lets_devtools_keys_through_and_off_takes_the_leader() {
    let core = core();
    let focus = KeySource::Devtools.focus();
    // On (the default): the leader reaches DevTools untouched.
    assert!(core.user_settings().shortcuts.disable_in_devtools);
    assert!(!core.on_key_down(focus, leader(), Instant::now()));
    // A grid view still takes it.
    assert!(core.on_key_down(KeySource::View.focus(), leader(), Instant::now()));
    // Off: rBrowser's shortcuts work while DevTools have focus, at once.
    let mut user = core.user_settings();
    user.shortcuts.disable_in_devtools = false;
    core.replace_user_settings(user);
    let later = Instant::now() + std::time::Duration::from_secs(10);
    assert!(core.on_key_down(focus, leader(), later));
    // On again: through again.
    let mut user = core.user_settings();
    user.shortcuts.disable_in_devtools = true;
    core.replace_user_settings(user);
    assert!(!core.on_key_down(focus, leader(), later));
}
