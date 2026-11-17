use std::time::{Duration, Instant};

use super::*;
use crate::registry::tests::{def, noop};
use crate::{Group, Modifiers};

fn key(text: &str) -> Shortcut {
    text.parse().expect(text)
}

fn id(text: &str) -> CommandId {
    CommandId::new(text).expect(text)
}

fn registry() -> CommandRegistry {
    let mut registry = CommandRegistry::new();
    for (command, group, bindings) in [
        ("sync.toggle", Group::Sync, &["Ctrl+Shift+Space S S"][..]),
        ("sync.skip_next", Group::Sync, &["Ctrl+Shift+Space S N"][..]),
        (
            "view.breakpoint_next",
            Group::Breakpoints,
            &["Ctrl+Shift+Space ]", "Ctrl+Shift+Space B N"][..],
        ),
        (
            "view.set_breakpoint.2",
            Group::Breakpoints,
            &["Ctrl+Shift+Space B 2", "Ctrl+Shift+2"][..],
        ),
        (PALETTE_COMMAND, Group::View, &["Ctrl+Shift+Space V P"][..]),
    ] {
        registry
            .register(def(command, group, bindings), noop())
            .expect(command);
    }
    registry
}

#[test]
fn leader_shows_the_groups_then_s_s_toggles_sync() {
    let registry = registry();
    let mut machine = ChordMachine::default();
    let now = Instant::now();
    let KeyOutcome::Show(root) = machine.on_key(&registry, Focus::View, LEADER, now) else {
        panic!("the leader shows the overlay");
    };
    let root_keys: Vec<&str> = root
        .entries
        .iter()
        .map(|entry| match entry {
            WhichKeyEntry::Group { key, .. } | WhichKeyEntry::Command { key, .. } => key.as_str(),
        })
        .collect();
    assert_eq!(root_keys, ["B", "S", "V", "]"]);
    assert_eq!(root.typed, "Ctrl+Shift+Space");
    let KeyOutcome::Show(sync) = machine.on_key(&registry, Focus::View, key("S"), now) else {
        panic!("S opens the sync group");
    };
    assert_eq!(sync.typed, "Ctrl+Shift+Space S");
    assert_eq!(sync.entries.len(), 2);
    assert_eq!(
        machine.on_key(&registry, Focus::View, key("S"), now),
        KeyOutcome::Run(id("sync.toggle"))
    );
    assert!(!machine.is_pending());
}

#[test]
fn the_leader_twice_opens_the_palette() {
    let registry = registry();
    let mut machine = ChordMachine::default();
    let now = Instant::now();
    machine.on_key(&registry, Focus::Shell, LEADER, now);
    assert_eq!(
        machine.on_key(&registry, Focus::Shell, LEADER, now),
        KeyOutcome::Run(id(PALETTE_COMMAND))
    );
}

#[test]
fn direct_shortcuts_run_and_everything_else_passes_through() {
    let registry = registry();
    let mut machine = ChordMachine::default();
    let now = Instant::now();
    assert_eq!(
        machine.on_key(&registry, Focus::View, key("Ctrl+Shift+2"), now),
        KeyOutcome::Run(id("view.set_breakpoint.2"))
    );
    for text in ["Ctrl+Alt+Q", "Ctrl+Shift+Left", "A", "Ctrl+T", "F5"] {
        assert_eq!(
            machine.on_key(&registry, Focus::View, key(text), now),
            KeyOutcome::PassThrough,
            "{text} reaches the page"
        );
    }
}

#[test]
fn devtools_focus_turns_shortcuts_off_unless_the_setting_says_otherwise() {
    let registry = registry();
    let mut machine = ChordMachine::default();
    let now = Instant::now();
    assert_eq!(
        machine.on_key(&registry, Focus::Devtools, LEADER, now),
        KeyOutcome::PassThrough
    );
    assert!(!machine.is_pending());
    assert_eq!(
        machine.on_key(&registry, Focus::External, key("Ctrl+Shift+2"), now),
        KeyOutcome::PassThrough
    );
    machine.set_settings(ChordSettings {
        disable_in_devtools: false,
        ..ChordSettings::default()
    });
    assert!(matches!(
        machine.on_key(&registry, Focus::Devtools, LEADER, now),
        KeyOutcome::Show(_)
    ));
}

#[test]
fn altgr_and_shift_after_the_leader_count_as_the_typed_key() {
    let registry = registry();
    let mut machine = ChordMachine::default();
    let now = Instant::now();
    machine.on_key(&registry, Focus::View, LEADER, now);
    // `]` on a German layout is AltGr+9: the host passes the typed character.
    let altgr_bracket = Shortcut {
        modifiers: Modifiers {
            ctrl: true,
            alt: true,
            ..Modifiers::NONE
        },
        key: Key::Char(']'),
    };
    assert_eq!(
        machine.on_key(&registry, Focus::View, altgr_bracket, now),
        KeyOutcome::Run(id("view.breakpoint_next"))
    );
    machine.on_key(&registry, Focus::View, LEADER, now);
    assert!(matches!(
        machine.on_key(&registry, Focus::View, key("Shift+S"), now),
        KeyOutcome::Show(_)
    ));
}

#[test]
fn escape_and_unknown_keys_hide_the_overlay() {
    let registry = registry();
    let mut machine = ChordMachine::default();
    let now = Instant::now();
    machine.on_key(&registry, Focus::View, LEADER, now);
    assert_eq!(
        machine.on_key(&registry, Focus::View, key("Escape"), now),
        KeyOutcome::Hide
    );
    machine.on_key(&registry, Focus::View, LEADER, now);
    assert_eq!(
        machine.on_key(&registry, Focus::View, key("Q"), now),
        KeyOutcome::Hide,
        "an unknown key is swallowed while the overlay is up"
    );
}

#[test]
fn by_default_the_leader_waits_until_escape_or_a_key() {
    let registry = registry();
    let mut machine = ChordMachine::default();
    assert_eq!(ChordSettings::default().leader_timeout, None);
    let now = Instant::now();
    machine.on_key(&registry, Focus::View, LEADER, now);
    let hour = Duration::from_secs(3600);
    assert_eq!(
        machine.on_tick(now + hour),
        None,
        "no timeout: the overlay stays"
    );
    assert!(machine.is_pending());
    assert!(matches!(
        machine.on_key(&registry, Focus::View, key("S"), now + hour),
        KeyOutcome::Show(_)
    ));
    assert_eq!(
        machine.on_key(&registry, Focus::View, key("S"), now + hour * 2),
        KeyOutcome::Run(id("sync.toggle"))
    );
}

#[test]
fn a_timeout_when_set_hides_the_overlay_and_ends_the_chord() {
    let registry = registry();
    let mut machine = ChordMachine::new(ChordSettings {
        leader_timeout: Some(Duration::from_millis(4000)),
        ..ChordSettings::default()
    });
    let now = Instant::now();
    machine.on_key(&registry, Focus::View, LEADER, now);
    assert_eq!(machine.on_tick(now + Duration::from_millis(100)), None);
    assert_eq!(
        machine.on_tick(now + Duration::from_millis(4100)),
        Some(KeyOutcome::Hide)
    );
    assert!(!machine.is_pending());
    machine.on_key(&registry, Focus::View, LEADER, now);
    assert_eq!(
        machine.on_key(
            &registry,
            Focus::View,
            key("S"),
            now + Duration::from_millis(4500)
        ),
        KeyOutcome::PassThrough,
        "after the timeout a key is an ordinary key again"
    );
}

#[test]
fn a_combination_of_its_own_ends_the_chord() {
    let registry = registry();
    let mut machine = ChordMachine::default();
    let now = Instant::now();
    machine.on_key(&registry, Focus::View, LEADER, now);
    assert_eq!(
        machine.on_key(&registry, Focus::View, key("Ctrl+Shift+2"), now),
        KeyOutcome::Run(id("view.set_breakpoint.2"))
    );
    machine.on_key(&registry, Focus::View, LEADER, now);
    assert_eq!(
        machine.on_key(&registry, Focus::View, key("Ctrl+C"), now),
        KeyOutcome::PassThrough
    );
}

#[test]
fn which_key_serializes_for_the_shell() {
    let registry = registry();
    let which = registry.which_key(&[Key::Char('S')]);
    let json = serde_json::to_value(&which).expect("serializes");
    assert_eq!(json["typed"], "Ctrl+Shift+Space S");
    assert_eq!(json["entries"][0]["kind"], "command");
    assert_eq!(json["entries"][0]["key"], "N");
    assert_eq!(json["entries"][0]["id"], "sync.skip_next");
    assert_eq!(json["entries"][0]["titleKey"], "commands.sync.skip_next");
    let root = serde_json::to_value(registry.which_key(&[])).expect("serializes");
    assert_eq!(root["entries"][0]["kind"], "group");
    assert_eq!(root["entries"][0]["group"], "breakpoints");
    assert_eq!(root["entries"][0]["titleKey"], "commands.group.breakpoints");
}

#[test]
fn ctrl_0_on_the_canvas_board_runs_the_zoom_reset_and_nowhere_else() {
    // RB-080-94: the reservation list's one context entry.
    let mut registry = registry();
    let now = Instant::now();
    let mut machine = ChordMachine::default();
    // Without the command registered the key stays the browser's.
    assert_eq!(
        machine.on_key(&registry, Focus::Board, key("Ctrl+0"), now),
        KeyOutcome::PassThrough
    );
    registry
        .register(
            def(
                "canvas.zoom.reset",
                Group::Layout,
                &["Ctrl+Shift+Space L 0"],
            ),
            noop(),
        )
        .expect("registers");
    assert_eq!(
        machine.on_key(&registry, Focus::Board, key("Ctrl+0"), now),
        KeyOutcome::Run(id("canvas.zoom.reset"))
    );
    // In a view, the shell or DevTools Ctrl+0 is the page's zoom.
    for focus in [Focus::View, Focus::Shell, Focus::Devtools] {
        assert_eq!(
            machine.on_key(&registry, focus, key("Ctrl+0"), now),
            KeyOutcome::PassThrough,
            "{focus:?}"
        );
    }
    // The other reserved digits stay reserved on the board.
    assert_eq!(
        machine.on_key(&registry, Focus::Board, key("Ctrl+1"), now),
        KeyOutcome::PassThrough
    );
    // The leader works on the board as in the toolbar.
    assert!(matches!(
        machine.on_key(&registry, Focus::Board, LEADER, now),
        KeyOutcome::Show(_)
    ));
    // Ctrl+0 is no default and no user binding: the editor refuses it.
    assert!(
        registry
            .register(
                def(
                    "canvas.zoom.other",
                    Group::Layout,
                    &["Ctrl+Shift+Space L 9", "Ctrl+0"]
                ),
                noop(),
            )
            .is_err()
    );
}
