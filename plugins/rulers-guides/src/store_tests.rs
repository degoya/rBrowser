use super::*;
use crate::model::{Axis, GuideUnit, Preset};
use rb_plugin_api::Capability;
use rb_plugin_api::test_support::MemoryHost;

fn host() -> MemoryHost {
    MemoryHost::new()
        .grant(Capability::ProjectRead)
        .grant(Capability::OutWrite)
}

fn set(host: &mut MemoryHost, key: &str, value: &str) {
    host.settings.insert(key.to_owned(), value.to_owned());
}

#[test]
fn without_settings_the_defaults_apply() {
    let mut host = host();
    let shared = shared(&mut host);
    assert_eq!(
        shared,
        Shared {
            columns: default_columns(),
            ..Shared::default()
        }
    );
    assert_eq!(
        prefs(&mut host),
        Prefs {
            unit: GuideUnit::Px,
            snap_px: DEFAULT_SNAP_PX
        }
    );
}

#[test]
fn the_settings_make_the_shared_layer() {
    let mut host = host();
    set(&mut host, "visible", "true");
    set(&mut host, "unit", "\"percent\"");
    set(&mut host, "snapPx", "4");
    set(
        &mut host,
        "guides",
        r#"[{"id":1,"axis":"x","unit":"percent","value":50}]"#,
    );
    set(
        &mut host,
        "columns",
        r#"{"mobile":{"columns":4,"gutter":16},"*":{"preset":"bootstrap"}}"#,
    );
    set(&mut host, "baseline.lineHeight", "24");
    set(&mut host, "baseline.offset", "4");
    let shared = shared(&mut host);
    assert!(shared.visible.rulers);
    assert!(!shared.visible.columns);
    assert_eq!(shared.guides[0].axis, Axis::X);
    assert_eq!(shared.columns["*"].preset, Some(Preset::Bootstrap));
    assert_eq!(shared.columns["mobile"].columns, Some(4));
    assert_eq!(shared.baseline.line_height, 24.0);
    assert_eq!(shared.baseline.offset, 4.0);
    assert_eq!(
        prefs(&mut host),
        Prefs {
            unit: GuideUnit::Percent,
            snap_px: 4
        }
    );
}

#[test]
fn a_setting_out_of_range_is_left_out_and_logged() {
    let mut host = host();
    set(
        &mut host,
        "guides",
        r#"[{"id":1,"axis":"x","unit":"percent","value":500}]"#,
    );
    set(&mut host, "columns", r#"{"a b":{"columns":4}}"#);
    set(&mut host, "baseline.lineHeight", "0");
    set(&mut host, "snapPx", "1000");
    set(&mut host, "unit", "\"em\"");
    assert_eq!(
        shared(&mut host),
        Shared {
            columns: default_columns(),
            ..Shared::default()
        }
    );
    assert_eq!(prefs(&mut host).snap_px, DEFAULT_SNAP_PX);
    assert_eq!(prefs(&mut host).unit, GuideUnit::Px);
    assert!(host.logs.iter().any(|(level, _)| level == "warn"));
}

#[test]
fn overrides_are_saved_and_read_back() {
    let mut host = host();
    let overrides = Overrides {
        guides: Some(vec![crate::model::Guide {
            id: 1,
            axis: Axis::Y,
            unit: GuideUnit::Px,
            value: 320.0,
        }]),
        ..Overrides::default()
    };
    save(&mut host, &overrides).expect("saved");
    let written = host.out.get(STATE_FILE).expect("state.json").clone();
    let mut later = host.with_file(STATE_PATH, &written);
    assert_eq!(load(&mut later), Loaded::Stored(overrides));
}

#[test]
fn no_state_file_yet_is_no_override() {
    assert_eq!(load(&mut host()), Loaded::Stored(Overrides::default()));
}

#[test]
fn a_broken_state_file_counts_as_none_and_is_logged() {
    let mut host = host().with_file(
        STATE_PATH,
        br#"{"version":1,"overrides":{"guides":[{"id":1,"axis":"y","unit":"percent","value":1}]}}"#,
    );
    assert_eq!(load(&mut host), Loaded::Stored(Overrides::default()));
    let mut other = self::host().with_file(STATE_PATH, b"not json");
    assert_eq!(load(&mut other), Loaded::Stored(Overrides::default()));
    assert_eq!(host.logs.len(), 1);
}

#[test]
fn a_file_of_a_later_version_is_not_read() {
    let mut host = host().with_file(STATE_PATH, br#"{"version":2,"overrides":{}}"#);
    assert_eq!(load(&mut host), Loaded::Stored(Overrides::default()));
}

#[test]
fn without_the_project_the_state_is_unavailable() {
    // Not granted here; in the app, a window without a project answers `not_available` alike.
    assert_eq!(load(&mut MemoryHost::new()), Loaded::Unavailable);
    assert!(save(&mut MemoryHost::new(), &Overrides::default()).is_err());
}

#[test]
fn the_page_copy_stands_in_only_when_the_file_cannot_be_read() {
    let page = Overrides {
        baseline: Some(crate::model::Baseline {
            line_height: 10.0,
            offset: 0.0,
        }),
        ..Overrides::default()
    };
    assert_eq!(
        overrides(&mut MemoryHost::new(), Some(page.clone())),
        page.clone()
    );
    assert_eq!(overrides(&mut host(), Some(page)), Overrides::default());
}
