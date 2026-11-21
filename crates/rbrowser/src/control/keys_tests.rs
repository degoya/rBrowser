use super::*;

#[test]
fn only_a_key_down_becomes_a_key() {
    assert_eq!(KeyDown::from_event(false, 0, 0x41, 97, 97), None);
    let key = KeyDown::from_event(true, 6, 0x20, 32, 32).expect("a key-down");
    assert_eq!(
        key,
        KeyDown {
            flags: 6,
            windows_key_code: 0x20,
            character: Some(' '),
            unmodified_character: Some(' '),
        }
    );
}

#[test]
fn a_broken_code_or_character_is_no_key_code_or_character() {
    let key = KeyDown::from_event(true, 0, -5, 0xD800, 65).expect("a key-down");
    assert_eq!(key.windows_key_code, 0);
    assert_eq!(key.character, None, "a lone surrogate is no character");
    assert_eq!(key.unmodified_character, Some('A'));
}

/// The leader's ticker runs only while a chord is open (RB-094-10).
#[tokio::test]
async fn the_ticker_runs_only_while_a_chord_is_open() {
    use std::sync::Arc;
    use std::sync::atomic::Ordering;

    use rb_commands::keymap::{EVENTFLAG_CONTROL_DOWN, EVENTFLAG_SHIFT_DOWN};

    use crate::cdp::fake::FakeBrowser;
    use crate::control::CoreSetup;
    use crate::host::Host;
    use crate::host::state::default_config;
    use crate::host::ui::recording::RecordingUi;

    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, tokio::runtime::Handle::current());
    host.open_config(default_config(), None).expect("opens");
    let core = Core::new(
        host,
        ui,
        tokio::runtime::Handle::current(),
        CoreSetup::default(),
    );
    assert!(!core.ticking.load(Ordering::Acquire), "no chord, no ticker");
    let leader = KeyDown {
        flags: EVENTFLAG_CONTROL_DOWN | EVENTFLAG_SHIFT_DOWN,
        windows_key_code: 0x20,
        character: Some(' '),
        unmodified_character: Some(' '),
    };
    let escape = KeyDown {
        flags: 0,
        windows_key_code: 0x1B,
        character: None,
        unmodified_character: None,
    };
    assert!(core.on_key_down(Focus::View, leader, Instant::now()));
    assert!(core.ticking.load(Ordering::Acquire), "the open chord ticks");
    assert!(core.on_key_down(Focus::View, escape, Instant::now()));
    for _ in 0..100 {
        if !core.ticking.load(Ordering::Acquire) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("the ticker still runs after the chord closed");
}
