use rb_config::{ColorScheme, EngineKind, Orientation, ProjectConfig};
use rb_engine::{Emulation, ViewId, ViewInfo};
use rb_sync::{BusSettings, PageEmulation};

use super::*;

fn info() -> ViewInfo {
    ViewInfo {
        id: ViewId(3),
        label: "Tablet".into(),
        breakpoint: Some("tablet".into()),
        cell_breakpoint: Some("tablet".into()),
        engine: EngineKind::Chromium,
        width: 768,
        height: 1024,
        dpr: 2.0,
        scale: 0.5,
        url: "https://app.test/".into(),
        title: String::new(),
        loading: false,
        session: "default".into(),
        cell: None,
    }
}

fn emulation() -> Emulation {
    Emulation {
        width: 768,
        height: 1024,
        dpr: 2.0,
        mobile: true,
        touch: true,
        orientation: Some(Orientation::Portrait),
        color_scheme: Some(ColorScheme::Dark),
        reduced_motion: Some(true),
        locale: Some("de-DE".into()),
        user_agent: Some("secret-ua".into()),
        ..Emulation::default()
    }
}

fn on() -> PageSwitches {
    PageSwitches {
        api: true,
        ua_suffix: true,
        bus: BusSettings::default(),
    }
}

#[test]
fn the_page_learns_label_size_index_engine_and_emulation() {
    let state = page_state(&info(), 1, &emulation(), &on());
    assert_eq!(state.info.id, 3);
    assert_eq!(state.info.label, "Tablet");
    assert_eq!((state.info.width, state.info.height), (768, 1024));
    assert_eq!(state.info.index, 1);
    assert_eq!(state.info.engine, "chromium");
    assert_eq!(
        state.info.emulation,
        PageEmulation {
            dpr: 2.0,
            mobile: true,
            touch: true,
            orientation: Some("portrait".into()),
            color_scheme: Some("dark".into()),
            reduced_motion: Some(true),
            locale: Some("de-DE".into()),
        }
    );
    assert!(state.bus.enabled);
}

#[test]
fn the_bus_is_off_without_the_page_api() {
    let switches = PageSwitches { api: false, ..on() };
    assert!(!page_state(&info(), 0, &emulation(), &switches).bus.enabled);
    assert!(!switches.bus_in_effect().enabled);
}

#[test]
fn the_switches_follow_the_config() {
    let mut config = ProjectConfig::default();
    config.page_api.ua_suffix = false;
    config.bus.max_per_second = 5000;
    let switches = Overrides::default().apply(&config);
    assert!(switches.api);
    assert!(!switches.ua_suffix);
    assert_eq!(switches.bus.max_per_second, 1000, "clamped to the range");
}

#[test]
fn a_toggle_overrides_until_the_config_changes_that_key() {
    let mut config = ProjectConfig::default();
    let mut overrides = Overrides::default();
    assert!(!overrides.toggle(Switch::Bus, &config));
    assert!(!overrides.apply(&config).bus.enabled);
    // Another key changes: the override holds.
    config.page_api.ua_suffix = false;
    assert!(!overrides.apply(&config).bus.enabled);
    // The key itself changes in the config: the config wins again.
    config.bus.enabled = false;
    assert!(!overrides.apply(&config).bus.enabled);
    config.bus.enabled = true;
    assert!(overrides.apply(&config).bus.enabled);
    assert!(!overrides.toggle(Switch::Api, &config));
    // The config has the suffix off: the first toggle switches it on.
    assert!(overrides.toggle(Switch::UaSuffix, &config));
    assert!(!overrides.toggle(Switch::UaSuffix, &config));
}

#[test]
fn the_main_world_source_wraps_the_bundle_in_the_state() {
    let state = page_state(&info(), 0, &emulation(), &on());
    let source = main_world_source("console.log(1);", &state).expect("a bundle");
    assert!(source.starts_with("(function(__rbPageBoot){\nconsole.log(1);\n})({"));
    assert!(source.ends_with("});"));
    assert!(
        !source.contains("secret-ua"),
        "the user agent is not page info"
    );
    assert!(main_world_source("  \n", &state).is_none());
}

#[test]
fn a_label_cannot_break_out_of_the_boot() {
    let mut view = info();
    view.label = "x\"});alert(1);//\u{2028}</script>".into();
    let state = page_state(&view, 0, &emulation(), &on());
    let source = main_world_source("b();", &state).expect("a bundle");
    let boot = source
        .strip_prefix("(function(__rbPageBoot){\nb();\n})(")
        .and_then(|rest| rest.strip_suffix(");"))
        .expect("the wrapper");
    let parsed: rb_sync::PageState = serde_json::from_str(boot).expect("the boot is JSON");
    assert_eq!(parsed.info.label, view.label);
}
