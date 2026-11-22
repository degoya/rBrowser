use super::*;
use crate::{Key, Modifiers, Shortcut};

fn shortcut(text: &str) -> Shortcut {
    text.parse().expect(text)
}

fn reason(text: &str) -> Option<String> {
    let list = reserved_list().expect("parses");
    list.find(&shortcut(text), Platform::Windows)
        .map(|entry| entry.reason.clone())
}

#[test]
fn the_shipped_list_parses() {
    let list = reserved_list().expect("data/reserved-shortcuts.json parses");
    assert_eq!(list.version, 1);
    assert!(list.entries.len() >= 100, "the full list");
    let first = &list.entries[0];
    assert_eq!(first.combo, "Ctrl+T");
    assert_eq!(first.source, ReservedSource::Browser);
    assert_eq!(first.platforms, [Platform::Windows, Platform::Linux]);
    assert!(
        list.entries
            .iter()
            .any(|entry| entry.combo == "Ctrl+Alt+<any>" && entry.source == ReservedSource::Altgr)
    );
}

#[test]
fn every_entry_is_well_formed() {
    let list = reserved_list().expect("parses");
    for entry in &list.entries {
        assert!(
            ComboPattern::parse(&entry.combo).is_some(),
            "{}: combo does not parse",
            entry.combo
        );
        assert!(
            entry.reason_key.starts_with("shortcuts.reserved.")
                && entry.reason_key.len() > "shortcuts.reserved.".len(),
            "{}: reason key {}",
            entry.combo,
            entry.reason_key
        );
        assert!(!entry.reason.is_empty(), "{}: no reason", entry.combo);
        assert!(!entry.platforms.is_empty(), "{}: no platform", entry.combo);
    }
}

#[test]
fn every_reason_key_has_its_english_text() {
    let part: serde_json::Value = serde_json::from_str(include_str!("../../../locales/en.json"))
        .expect("the catalogue parses");
    let list = reserved_list().expect("parses");
    for entry in &list.entries {
        let name = entry
            .reason_key
            .strip_prefix("shortcuts.reserved.")
            .expect("prefixed");
        assert_eq!(
            part["shortcuts"]["reserved"][name].as_str(),
            Some(entry.reason.as_str()),
            "{}",
            entry.reason_key
        );
    }
}

#[test]
fn the_path_is_fixed() {
    assert_eq!(
        RESERVED_SHORTCUTS_PATH,
        "crates/rb-commands/data/reserved-shortcuts.json"
    );
}

#[test]
fn the_acceptance_examples_name_their_reason() {
    let ctrl_t = reason("Ctrl+T").expect("Ctrl+T is reserved");
    assert!(ctrl_t.starts_with("Browser: new tab"), "{ctrl_t}");
    let altgr_q = reason("Ctrl+Alt+Q").expect("Ctrl+Alt+Q is reserved");
    assert!(altgr_q.starts_with("AltGr character @"), "{altgr_q}");
}

#[test]
fn the_browser_combinations_of_the_job_are_reserved() {
    let mut combos: Vec<String> = "TWNLRFPSDHJUKG"
        .chars()
        .map(|c| format!("Ctrl+{c}"))
        .collect();
    combos.extend((1..=9).map(|d| format!("Ctrl+{d}")));
    combos.extend(
        [
            "Ctrl+Tab",
            "Ctrl+Plus",
            "Ctrl+-",
            "Ctrl+0",
            "F1",
            "F3",
            "F5",
            "F6",
            "F7",
            "F10",
            "F11",
            "F12",
            "Alt+Left",
            "Alt+Right",
            "Alt+Home",
        ]
        .map(String::from),
    );
    combos.extend("IJCKPTNB".chars().map(|c| format!("Ctrl+Shift+{c}")));
    combos.push("Ctrl+Shift+Delete".into());
    for combo in combos {
        assert!(reason(&combo).is_some(), "{combo} is not reserved");
    }
}

#[test]
fn text_editing_system_accesskey_and_altgr_are_reserved() {
    for combo in [
        "Ctrl+A",
        "Ctrl+C",
        "Ctrl+V",
        "Ctrl+X",
        "Ctrl+Z",
        "Ctrl+Y",
        "Ctrl+Backspace",
        "Ctrl+Left",
        "Ctrl+Shift+Left",
        "Shift+Right",
        "Home",
        "End",
        "Alt+Tab",
        "Alt+F4",
        "Ctrl+Escape",
        "Win+E",
        "Ctrl+Shift+Alt+Win+L",
        "Alt+M",
        "Alt+Shift+M",
        "Ctrl+Alt+E",
        "Ctrl+Alt+7",
        "Ctrl+Alt+F1",
        "Ctrl+Shift+Alt+9",
        "A",
        "Shift+A",
        "Space",
        "Ctrl+.",
    ] {
        assert!(reason(combo).is_some(), "{combo} is not reserved");
    }
}

#[test]
fn the_free_combinations_stay_free() {
    for combo in [
        "Ctrl+Shift+Space",
        "Ctrl+Shift+1",
        "Ctrl+Shift+9",
        "F4",
        "F13",
        "Ctrl+Shift+F9",
    ] {
        assert_eq!(reason(combo), None, "{combo} is reserved");
    }
}

#[test]
fn specific_entries_win_over_patterns() {
    let list = reserved_list().expect("parses");
    let entry = list
        .find(&shortcut("Ctrl+Alt+Q"), Platform::Windows)
        .expect("reserved");
    assert_eq!(entry.reason_key, "shortcuts.reserved.altgr_at");
    let entry = list
        .find(&shortcut("Ctrl+Alt+K"), Platform::Windows)
        .expect("reserved");
    assert_eq!(entry.reason_key, "shortcuts.reserved.altgr");
}

#[test]
fn placeholders_match_their_keys() {
    let pattern = |text: &str| ComboPattern::parse(text).expect(text);
    let key = |text: &str| shortcut(text).key;
    assert!(pattern("Ctrl+<letter>").key.matches(key("Q")));
    assert!(!pattern("Ctrl+<letter>").key.matches(key("1")));
    assert!(pattern("Ctrl+<digit>").key.matches(key("1")));
    assert!(pattern("<arrow>").key.matches(Key::Down));
    assert!(!pattern("<arrow>").key.matches(Key::Home));
    assert!(pattern("<char>").key.matches(Key::Char(',')));
    assert!(!pattern("<char>").key.matches(Key::F(2)));
    assert!(pattern("Win+<any>").key.matches(Key::F(2)));
    assert_eq!(
        pattern("Ctrl+Shift+Alt+Win+<any>").modifiers,
        Modifiers {
            ctrl: true,
            shift: true,
            alt: true,
            meta: true
        }
    );
    assert!(ComboPattern::parse("Ctrl+<nothing>").is_none());
}

#[test]
fn linux_reserves_the_browser_the_desktop_and_text_editing() {
    // RB-097-05: the browsers and text editing as on Windows, the desktops' own combinations.
    let list = reserved_list().expect("parses");
    let linux = |text: &str| list.find(&shortcut(text), Platform::Linux);
    for (combo, source) in [
        ("Ctrl+T", ReservedSource::Browser),
        ("Ctrl+F8", ReservedSource::Devtools),
        ("Ctrl+C", ReservedSource::TextEditing),
        ("Alt+K", ReservedSource::Accesskey),
        ("Ctrl+Alt+Q", ReservedSource::Altgr),
        ("Win+A", ReservedSource::Os),
        ("Shift+Win+S", ReservedSource::Os),
        ("Ctrl+Shift+Alt+Win+J", ReservedSource::Os),
        ("Alt+Tab", ReservedSource::Os),
        ("Alt+F1", ReservedSource::Os),
        ("Alt+F2", ReservedSource::Os),
        ("Alt+F4", ReservedSource::Os),
        ("Alt+F7", ReservedSource::Os),
        ("Alt+F10", ReservedSource::Os),
        ("Alt+Space", ReservedSource::Os),
        ("Ctrl+Alt+T", ReservedSource::Os),
        ("Ctrl+Alt+Left", ReservedSource::Os),
        ("Ctrl+Alt+Down", ReservedSource::Os),
        ("Ctrl+Shift+Alt+Right", ReservedSource::Os),
        ("Ctrl+Alt+Delete", ReservedSource::Os),
        ("Ctrl+F1", ReservedSource::Os),
    ] {
        let entry = linux(combo).unwrap_or_else(|| panic!("{combo} is not reserved on Linux"));
        assert_eq!(entry.source, source, "{combo}: {}", entry.reason);
    }
    // The desktop's own entries speak of the Linux desktop, never of Windows.
    for combo in ["Win+A", "Alt+Tab", "Alt+F4", "Ctrl+Alt+T", "Shift+F10"] {
        let reason = &linux(combo).expect("reserved").reason;
        assert!(reason.starts_with("Linux desktop: "), "{combo}: {reason}");
    }
    // The plain keys hold as everywhere.
    for combo in ["A", "Enter", "Tab", "Escape", "Ctrl+Tab"] {
        assert!(linux(combo).is_some(), "{combo} is not reserved on Linux");
    }
}

#[test]
fn linux_leaves_the_leader_and_the_breakpoint_keys_free() {
    let list = reserved_list().expect("parses");
    for combo in ["Ctrl+Shift+Space", "Ctrl+Shift+1", "Ctrl+Shift+9"] {
        assert!(
            list.find(&shortcut(combo), Platform::Linux).is_none(),
            "{combo} is reserved on Linux"
        );
    }
}

#[test]
fn linux_holds_every_windows_reservation_but_the_windows_systems_own() {
    // What Windows reserves beyond the system keys holds on Linux too, so the defaults a
    // Windows build may bind are the ones a Linux build may bind.
    let list = reserved_list().expect("parses");
    for entry in list
        .entries
        .iter()
        .filter(|entry| entry.platforms.contains(&Platform::Windows))
        .filter(|entry| entry.source != ReservedSource::Os)
    {
        assert!(
            entry.platforms.contains(&Platform::Linux),
            "{} is reserved on Windows only",
            entry.combo
        );
    }
}

#[test]
fn ctrl_0_stays_reserved_but_is_the_canvas_boards_zoom_reset() {
    // RB-080-94, the owner's one exception: Ctrl+0 on the canvas board is 100 %.
    let list = reserved_list().expect("parses");
    let ctrl_0 = shortcut("Ctrl+0");
    let entry = list.find(&ctrl_0, Platform::Windows).expect("reserved");
    assert_eq!(entry.source, ReservedSource::Browser);
    assert_eq!(
        list.context_command(&ctrl_0, Platform::Windows, KeyContext::CanvasBoard),
        Some("canvas.zoom.reset")
    );
    // Only Ctrl+0: the other digits and the zoom steps stay the browser's everywhere.
    for other in ["Ctrl+1", "Ctrl+Plus", "Ctrl+-", "Ctrl+Shift+0"] {
        assert_eq!(
            list.context_command(&shortcut(other), Platform::Windows, KeyContext::CanvasBoard),
            None,
            "{other}"
        );
    }
    let exceptions: Vec<&str> = list
        .entries
        .iter()
        .filter(|entry| entry.except.is_some())
        .map(|entry| entry.combo.as_str())
        .collect();
    assert_eq!(exceptions, ["Ctrl+0"]);
}

#[test]
fn the_platform_of_the_build_is_its_own() {
    // macOS (RB-090-15) and Linux (RB-097-05) have their own lists; Windows is the rest.
    let expected = if cfg!(target_os = "macos") {
        Platform::Macos
    } else if cfg!(target_os = "linux") {
        Platform::Linux
    } else {
        Platform::Windows
    };
    assert_eq!(Platform::current(), expected);
}

#[test]
fn macos_reserves_the_command_key_the_system_and_text_editing() {
    let list = reserved_list().expect("parses");
    let mac = |text: &str| list.find(&shortcut(text), Platform::Macos);
    // Cmd is the meta modifier (`Win` in the text form).
    for (combo, source) in [
        ("Win+Q", ReservedSource::Os),
        ("Win+H", ReservedSource::Os),
        ("Win+Tab", ReservedSource::Os),
        ("Win+Space", ReservedSource::Os),
        ("Ctrl+Space", ReservedSource::Os),
        ("Ctrl+Left", ReservedSource::Os),
        ("Ctrl+Up", ReservedSource::Os),
        ("Shift+Win+4", ReservedSource::Os),
        ("Win+T", ReservedSource::Browser),
        ("Win+W", ReservedSource::Browser),
        ("Win+N", ReservedSource::Browser),
        ("Win+L", ReservedSource::Browser),
        ("Win+R", ReservedSource::Browser),
        ("Alt+Win+I", ReservedSource::Devtools),
        ("Win+C", ReservedSource::TextEditing),
        ("Win+V", ReservedSource::TextEditing),
        ("Win+Z", ReservedSource::TextEditing),
        ("Ctrl+A", ReservedSource::TextEditing),
        ("Alt+E", ReservedSource::TextEditing),
        ("Ctrl+Alt+K", ReservedSource::Accesskey),
        ("Win+J", ReservedSource::Os),
        ("Ctrl+Shift+Alt+Win+J", ReservedSource::Os),
    ] {
        let entry = mac(combo).unwrap_or_else(|| panic!("{combo} is not reserved on macOS"));
        assert_eq!(entry.source, source, "{combo}: {}", entry.reason);
    }
    // Typing and the plain keys hold on both platforms.
    for combo in [
        "A",
        "Shift+A",
        "Enter",
        "Tab",
        "Escape",
        "Backspace",
        "Left",
        "Ctrl+Tab",
    ] {
        assert!(mac(combo).is_some(), "{combo} is not reserved on macOS");
        assert!(
            list.find(&shortcut(combo), Platform::Windows).is_some(),
            "{combo}"
        );
    }
}

#[test]
fn macos_leaves_the_leader_and_the_breakpoint_keys_free() {
    let list = reserved_list().expect("parses");
    for combo in ["Ctrl+Shift+Space", "Ctrl+Shift+1", "Ctrl+Shift+9"] {
        assert!(
            list.find(&shortcut(combo), Platform::Macos).is_none(),
            "{combo} is reserved on macOS"
        );
    }
}

#[test]
fn no_macos_entry_speaks_of_windows() {
    let list = reserved_list().expect("parses");
    for entry in list
        .entries
        .iter()
        .filter(|entry| entry.platforms.contains(&Platform::Macos))
    {
        assert!(
            !entry.reason.contains("Windows") && !entry.reason.contains("AltGr"),
            "{}: {}",
            entry.combo,
            entry.reason
        );
    }
}
