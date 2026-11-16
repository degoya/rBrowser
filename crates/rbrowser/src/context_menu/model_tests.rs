use std::collections::BTreeSet;

use serde_json::json;

use super::*;

fn state() -> MenuState {
    MenuState {
        view: ViewId(2),
        point: Some((30, 40)),
        menu_enabled: true,
        attached: true,
        detached: false,
        devtools_open: false,
        open_on_inspect: true,
        emulation: None,
        recording: None,
        checks_busy: false,
        checks: vec![PluginCommand {
            command: "plugin.seo.check".to_owned(),
            title: "SEO check".to_owned(),
        }],
        panels: vec![PluginPanel {
            plugin: "network-panel".to_owned(),
            panel: "network".to_owned(),
            title: "Network".to_owned(),
        }],
        known: [
            "breakpoints.analyze",
            "debug.vitals",
            "panels.sitemap",
            "view.remove",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        footer_visible: true,
        forced: crate::force_state::ForcedMarks::default(),
    }
}

/// Every entry in menu order, each as (id, label, enabled, command).
fn flat(items: &[Item]) -> Vec<(i32, String, bool, String)> {
    let mut rows = Vec::new();
    for item in items {
        match item {
            Item::Entry(entry) => rows.push((
                entry.id,
                label(&entry.label),
                entry.enabled,
                match &entry.action {
                    Action::Run { command, .. } => command.clone(),
                    Action::SkipNextClick => "<skip>".to_owned(),
                },
            )),
            Item::Separator => rows.push((0, "-".to_owned(), true, String::new())),
            Item::Menu { items, .. } => rows.extend(flat(items)),
        }
    }
    rows
}

fn label(label: &Label) -> String {
    match label {
        Label::Key(key) => (*key).to_owned(),
        Label::Text(text) => format!("'{text}'"),
        Label::KeyOr { key, .. } => key.clone(),
    }
}

fn entry<'a>(items: &'a [Item], key: &str) -> &'a Entry {
    flat(items)
        .iter()
        .find(|(_, label, _, _)| label == key)
        .and_then(|(id, _, _, _)| find(items, *id))
        .unwrap_or_else(|| panic!("no entry {key}"))
}

fn args(items: &[Item], key: &str) -> Value {
    match &entry(items, key).action {
        Action::Run { args, .. } => args.clone(),
        Action::SkipNextClick => Value::Null,
    }
}

#[test]
fn the_block_has_the_job_files_order_with_ids_from_the_user_range() {
    let items = build(&state());
    let top: Vec<String> = items
        .iter()
        .map(|item| match item {
            Item::Entry(entry) => label(&entry.label),
            Item::Separator => "-".to_owned(),
            Item::Menu { label: text, .. } => format!("{}>", label(text)),
        })
        .collect();
    assert_eq!(
        top,
        [
            "contextmenu.inspect",
            "contextmenu.devtools_open",
            "contextmenu.screenshot.menu>",
            "contextmenu.video.menu>",
            "contextmenu.checks.menu>",
            "contextmenu.panels.menu>",
            "-",
            "sync.menu.skipNextClick",
            "popout.menu.out",
            "contextmenu.remove_view",
        ]
    );
    let ids: Vec<i32> = flat(&items)
        .iter()
        .filter(|(id, ..)| *id != 0)
        .map(|(id, ..)| *id)
        .collect();
    assert_eq!(ids[0], FIRST_ID);
    assert!(ids.windows(2).all(|pair| pair[1] == pair[0] + 1), "{ids:?}");
    assert!(ids.iter().all(|id| *id <= LAST_ID));
    let last = *ids.last().expect("entries");
    assert_eq!(find(&items, last).map(|entry| entry.id), Some(last));
    assert!(find(&items, last + 1).is_none());
    assert!(
        find(&items, 28_000).is_none(),
        "the pre-0.8 ids are not ours"
    );
}

#[test]
fn every_entry_runs_the_registrys_command_for_the_clicked_view() {
    let items = build(&state());
    assert_eq!(
        args(&items, "contextmenu.inspect"),
        json!({ "view": 2, "x": 30, "y": 40, "dip": true })
    );
    assert_eq!(
        args(&items, "contextmenu.devtools_open"),
        json!({ "target": 2 })
    );
    assert_eq!(
        entry(&items, "contextmenu.screenshot.this_viewport").action,
        Action::Run {
            command: "capture.views".to_owned(),
            args: json!({ "target": 2 })
        }
    );
    assert_eq!(
        entry(&items, "contextmenu.screenshot.this_full_page").action,
        Action::Run {
            command: "capture.full_page".to_owned(),
            args: json!({ "target": 2 })
        }
    );
    assert_eq!(
        args(&items, "contextmenu.screenshot.element"),
        json!({ "target": 2, "at": { "x": 30, "y": 40, "dip": true } })
    );
    let all: Vec<(String, Value)> = [
        "contextmenu.screenshot.all_viewport",
        "contextmenu.screenshot.all_full_page",
        "contextmenu.screenshot.overview",
    ]
    .iter()
    .map(|key| match &entry(&items, key).action {
        Action::Run { command, args } => (command.clone(), args.clone()),
        Action::SkipNextClick => panic!("a command"),
    })
    .collect();
    assert_eq!(
        all,
        [
            ("capture.all_viewport".to_owned(), json!({})),
            ("capture.full_page".to_owned(), json!({})),
            ("capture.overview".to_owned(), json!({})),
        ]
    );
    assert_eq!(
        entry(&items, "contextmenu.video.record").action,
        Action::Run {
            command: "capture.video.toggle".to_owned(),
            args: json!({ "target": 2 })
        }
    );
    assert_eq!(
        entry(&items, "contextmenu.video.record_all").action,
        Action::Run {
            command: "capture.video.toggle_all".to_owned(),
            args: json!({})
        }
    );
    assert_eq!(
        entry(&items, "contextmenu.checks.run_all").action,
        Action::Run {
            command: "checks.run_all".to_owned(),
            args: json!({})
        }
    );
    assert_eq!(
        entry(&items, "'SEO check'").action,
        Action::Run {
            command: "plugin.seo.check".to_owned(),
            args: json!({ "target": 2 })
        }
    );
    assert_eq!(
        entry(&items, "contextmenu.checks.analyze").action,
        Action::Run {
            command: "breakpoints.analyze".to_owned(),
            args: Value::Null
        },
        "the analyzer window opens for Null only"
    );
    assert_eq!(
        entry(&items, "contextmenu.checks.vitals").action,
        Action::Run {
            command: "debug.vitals".to_owned(),
            args: json!({})
        }
    );
    assert_eq!(
        entry(&items, "contextmenu.panels.findings").action,
        Action::Run {
            command: "panels.findings".to_owned(),
            args: json!({})
        }
    );
    assert_eq!(
        entry(&items, "contextmenu.panels.sitemap").action,
        Action::Run {
            command: "panels.sitemap".to_owned(),
            args: json!({})
        }
    );
    assert_eq!(
        entry(&items, "'Network'").action,
        Action::Run {
            command: "plugins.panels".to_owned(),
            args: json!({ "plugin": "network-panel", "panel": "network", "view": 2 })
        }
    );
    assert_eq!(
        entry(&items, "sync.menu.skipNextClick").action,
        Action::SkipNextClick
    );
    assert_eq!(
        entry(&items, "popout.menu.out").action,
        Action::Run {
            command: "view.pop_out".to_owned(),
            args: json!({ "view": 2 })
        }
    );
    assert_eq!(
        entry(&items, "contextmenu.remove_view").action,
        Action::Run {
            command: "view.remove".to_owned(),
            args: json!({ "view": 2 })
        }
    );
    // The checks submenu keeps the job file's order: run all, the plugins, then the core's.
    let checks = flat(&items)
        .into_iter()
        .map(|(_, label, ..)| label)
        .filter(|label| label.starts_with("contextmenu.checks.") || label.starts_with('\''))
        .filter(|label| label != "'Network'")
        .collect::<Vec<_>>();
    assert_eq!(
        checks,
        [
            "contextmenu.checks.run_all",
            "'SEO check'",
            "contextmenu.checks.vitals",
            "contextmenu.checks.analyze",
        ]
    );
}

#[test]
fn the_texts_follow_the_state_open_close_record_stop_pop_in() {
    let mut state = state();
    state.devtools_open = true;
    state.detached = true;
    let items = build(&state);
    assert_eq!(
        entry(&items, "contextmenu.devtools_close").action,
        Action::Run {
            command: "devtools.toggle".to_owned(),
            args: json!({})
        }
    );
    assert_eq!(
        entry(&items, "popout.menu.in").action,
        Action::Run {
            command: "view.pop_in".to_owned(),
            args: json!({ "view": 2 })
        }
    );
    state.recording = Some(vec![ViewId(2), ViewId(3)]);
    let items = build(&state);
    assert!(entry(&items, "contextmenu.video.stop").enabled);
    assert!(entry(&items, "contextmenu.video.stop_all").enabled);
    assert_eq!(
        entry(&items, "contextmenu.video.stop").action,
        Action::Run {
            command: "capture.video.toggle".to_owned(),
            args: json!({ "target": 2 })
        }
    );
}

#[test]
fn what_cannot_run_now_is_disabled_not_hidden() {
    let mut state = state();
    state.attached = false;
    state.checks_busy = true;
    state.recording = Some(vec![ViewId(1)]);
    let items = build(&state);
    let disabled: Vec<String> = flat(&items)
        .into_iter()
        .filter(|(id, _, enabled, _)| *id != 0 && !enabled)
        .map(|(_, label, ..)| label)
        .collect();
    assert_eq!(
        disabled,
        [
            "contextmenu.inspect",
            "contextmenu.devtools_open",
            "contextmenu.screenshot.this_viewport",
            "contextmenu.screenshot.this_full_page",
            "contextmenu.screenshot.element",
            "contextmenu.video.record",
            "contextmenu.checks.run_all",
            "'SEO check'",
        ]
    );
    assert!(
        entry(&items, "contextmenu.video.stop_all").enabled,
        "another view's recording stops from here"
    );
    assert!(entry(&items, "contextmenu.screenshot.all_viewport").enabled);
}

#[test]
fn optional_entries_wait_for_their_commands_and_plugins() {
    let mut state = state();
    state.known = BTreeSet::new();
    state.checks.clear();
    state.panels.clear();
    let labels: Vec<String> = flat(&build(&state))
        .into_iter()
        .map(|(_, label, ..)| label)
        .collect();
    for absent in [
        "contextmenu.remove_view",
        "contextmenu.checks.analyze",
        "contextmenu.checks.vitals",
        "contextmenu.panels.sitemap",
        "'SEO check'",
        "'Network'",
    ] {
        assert!(
            !labels.contains(&absent.to_owned()),
            "{absent} in {labels:?}"
        );
    }
    assert!(labels.contains(&"contextmenu.panels.findings".to_owned()));
    assert!(labels.contains(&"contextmenu.checks.run_all".to_owned()));
}

/// Without CEF's parameters there is no point: the entries that need one are disabled, the
/// rest stays.
#[test]
fn without_a_point_the_point_entries_are_disabled() {
    let mut state = state();
    state.point = None;
    let items = build(&state);
    let inspect = entry(&items, "contextmenu.inspect");
    assert!(!inspect.enabled);
    assert_eq!(
        inspect.action,
        Action::Run {
            command: "devtools.inspect_at".to_owned(),
            args: json!({ "view": 2 })
        }
    );
    assert!(!entry(&items, "contextmenu.screenshot.element").enabled);
    assert!(entry(&items, "contextmenu.devtools_open").enabled);
    assert!(entry(&items, "contextmenu.screenshot.this_viewport").enabled);
}

/// The setting off leaves what the menu had before 0.8: "Do not sync the next click" and the
/// pop-out entries (RB-080-14), and RB-080-41's "Remove view".
#[test]
fn the_setting_off_leaves_the_pre_0_8_entries() {
    let mut state = state();
    state.menu_enabled = false;
    let items = build(&state);
    assert_eq!(
        items,
        vec![
            Item::Entry(Entry {
                id: FIRST_ID,
                label: Label::Key("sync.menu.skipNextClick"),
                enabled: true,
                mark: Mark::None,
                action: Action::SkipNextClick,
            }),
            Item::Entry(Entry {
                id: FIRST_ID + 1,
                label: Label::Key("popout.menu.out"),
                enabled: true,
                mark: Mark::None,
                action: Action::Run {
                    command: "view.pop_out".to_owned(),
                    args: json!({ "view": 2 }),
                },
            }),
            Item::Entry(Entry {
                id: FIRST_ID + 2,
                label: Label::Key("contextmenu.remove_view"),
                enabled: true,
                mark: Mark::None,
                action: Action::Run {
                    command: "view.remove".to_owned(),
                    args: json!({ "view": 2 }),
                },
            }),
        ]
    );
    state.known.remove("view.remove");
    assert_eq!(build(&state).len(), 2);
}

/// Every submenu of the block.
fn menu_ids(items: &[Item], out: &mut Vec<i32>) {
    for item in items {
        if let Item::Menu { id, items, .. } = item {
            out.push(*id);
            menu_ids(items, out);
        }
    }
}

/// A submenu's id is its own, given at build time from the submenus' range - which the block
/// must not outgrow: past it ids would repeat.
#[test]
fn submenus_take_ids_of_their_own_from_their_range() {
    let mut ids = Vec::new();
    menu_ids(&build(&state()), &mut ids);
    assert_eq!(ids.len(), 4, "screenshot, video, checks, panels");
    let unique: BTreeSet<i32> = ids.iter().copied().collect();
    assert_eq!(unique.len(), ids.len(), "{ids:?}");
    assert!(
        ids.iter()
            .all(|id| (FIRST_MENU_ID..=LAST_MENU_ID).contains(id)),
        "{ids:?}"
    );
    assert_eq!(LAST_MENU_ID - FIRST_MENU_ID + 1, 100);
    assert_eq!(LAST_MENU_ID + 1, 28_000, "below the pre-0.8 entries");
}

/// A label from outside the catalogue is no markup: `&` is doubled, control characters go.
#[test]
fn a_label_from_outside_the_catalogue_is_plain() {
    assert_eq!(plain_label("R&D office"), "R&&D office");
    assert_eq!(plain_label("a && b"), "a &&&& b");
    assert_eq!(plain_label("tab\there\nline\u{7}"), "tabhereline");
    assert_eq!(plain_label("Europe/Berlin"), "Europe/Berlin");
}

/// `devtools.openOnInspect` off: Inspect element runs only where DevTools are open for the
/// view - disabled otherwise, not hidden (RB-080-45).
#[test]
fn inspect_is_disabled_when_it_may_not_open_devtools_and_they_are_closed() {
    let mut off = state();
    off.open_on_inspect = false;
    assert!(!entry(&build(&off), "contextmenu.inspect").enabled);
    off.devtools_open = true;
    assert!(entry(&build(&off), "contextmenu.inspect").enabled);
    assert!(entry(&build(&state()), "contextmenu.inspect").enabled);
}
