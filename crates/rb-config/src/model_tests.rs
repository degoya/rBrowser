use super::*;
use crate::{ColorScheme, EngineKind, NetworkPreset, NetworkThrottle, ReloadMode, Zoom};

/// The example of the plan (section "Projekt-Config"), verbatim.
const PLAN_EXAMPLE: &str = r##"{
  "$schema": "./schema/config.v1.json",
  "version": 1,
  "startUrl": "http://localhost:5173",
  "environments": [
    { "id": "local", "url": "http://localhost:5173", "color": "#2e7d32" },
    { "id": "staging", "url": "https://staging.example.com", "color": "#f9a825" }
  ],
  "sessions": ["default", "admin"],
  "layouts": { "default": "grid-main", "presets": { "grid-main": { "mode": "grid" }, "focus": { "mode": "focus" } } },
  "breakpoints": [
    { "id": "mobile", "label": "iPhone 15", "width": 393, "height": 852, "dpr": 3, "touch": true,
      "engine": "chromium", "userAgent": null, "colorScheme": "light", "reducedMotion": false,
      "locale": "de-DE", "network": null, "headers": {}, "session": "default", "syncNavigation": true,
      "shortcut": "Alt+M" },
    { "id": "desktop-ff", "width": 1440, "height": 900, "engine": "firefox" }
  ],
  "layout": { "columns": 3, "rows": 2, "gap": 8,
              "cells": [{ "breakpoint": "mobile", "col": 1, "row": 1, "colSpan": 1, "rowSpan": 2, "zoom": "fit" }] },
  "sync": { "navigation": true, "scroll": true, "click": true, "input": true, "keyboard": true, "hover": true,
            "leaderOnlySubmit": true },
  "reload": { "watch": ["src/**"], "ignore": ["node_modules/**"], "debounceMs": 200, "mode": "auto" },
  "network": { "hostResolverRules": null, "ignoreCertificateErrors": false },
  "visual": { "threshold": 0.1, "fullPage": true },
  "checks": { "axe": { "tags": ["wcag22aa"] }, "lighthouse": { "categories": ["performance"] } }
}"##;

#[test]
fn the_plan_example_deserializes() {
    let config: ProjectConfig = serde_json::from_str(PLAN_EXAMPLE).expect("plan example parses");
    assert_eq!(config.version, 1);
    assert_eq!(config.start_url.as_deref(), Some("http://localhost:5173"));
    assert_eq!(config.environments.len(), 2);
    assert_eq!(config.environments[1].color.as_deref(), Some("#f9a825"));
    assert_eq!(config.sessions, ["default", "admin"]);
    let presets: Vec<&String> = config.layouts.presets.keys().collect();
    assert_eq!(presets, ["grid-main", "focus"], "config order is kept");
    assert_eq!(config.layouts.presets["focus"].mode, LayoutMode::Focus);

    let mobile = &config.breakpoints[0];
    assert_eq!((mobile.width, mobile.height), (Some(393), Some(852)));
    assert_eq!(mobile.emulation.dpr, Some(3.0));
    assert_eq!(mobile.emulation.touch, Some(true));
    assert_eq!(mobile.emulation.color_scheme, Some(ColorScheme::Light));
    assert_eq!(mobile.emulation.locale.as_deref(), Some("de-DE"));
    assert_eq!(mobile.shortcut.as_deref(), Some("Alt+M"));
    assert!(mobile.sync_navigation);
    assert_eq!(config.breakpoints[1].engine, EngineKind::Firefox);
    assert!(config.breakpoints[1].sync_navigation, "defaults to true");

    let cell = &config.layout.cells[0];
    assert_eq!(
        (cell.col, cell.row, cell.col_span, cell.row_span),
        (1, 1, 1, 2)
    );
    assert_eq!(cell.zoom, Zoom::Fit);
    assert!(config.sync.leader_only_submit);
    // Missing in the fixture: the default (RB-020-01).
    assert_eq!(config.sync.hover_throttle_ms, 100);
    assert_eq!(config.sync.desync_timeout_ms, 750);
    assert!(!config.sync.context_menu);
    assert_eq!(config.sync.navigation_settle_ms, 1000);
    assert!(!config.sync.navigation_include_hash);
    assert!(config.sync.detect_browsersync);
    assert_eq!(config.sync.scroll_mode, ScrollMode::Anchor);
    assert_eq!(config.sync.scroll_debounce_ms, 150);
    assert!(config.sync.forms && !config.sync.focus);
    assert!(!config.sync.activate_on_focus);
    assert_eq!(config.reload.mode, ReloadMode::Auto);
    assert_eq!(config.reload.debounce_ms, 200);
    assert!(!config.network.ignore_certificate_errors);
    // `checks.axe` is no core key any more (the axe plugin's `a11y-axe.tags` took it over): an
    // old value is kept as it stands.
    assert_eq!(
        config.checks.extra["axe"],
        serde_json::json!({ "tags": ["wcag22aa"] })
    );
}

#[test]
fn plugin_sections_survive_as_values() {
    let json = r#"{ "version": 1, "my-plugin": { "on": true } }"#;
    let config: ProjectConfig = serde_json::from_str(json).expect("parses");
    assert_eq!(config.extra["my-plugin"], serde_json::json!({ "on": true }));
    let back = serde_json::to_value(&config).expect("serializes");
    assert_eq!(back["my-plugin"], serde_json::json!({ "on": true }));
}

#[test]
fn zoom_is_fit_or_a_factor() {
    let cell: LayoutCell =
        serde_json::from_str(r#"{ "breakpoint": "b", "col": 1, "row": 1, "zoom": 0.5 }"#)
            .expect("parses");
    assert_eq!(cell.zoom, Zoom::Factor(0.5));
    assert_eq!(cell.col_span, 1);
    assert_eq!(
        serde_json::to_value(Zoom::Fit).expect("serializes"),
        serde_json::json!("fit")
    );
}

#[test]
fn network_throttle_is_a_preset_or_custom() {
    let preset: NetworkThrottle = serde_json::from_str(r#""slow-3g""#).expect("preset");
    assert_eq!(preset, NetworkThrottle::Preset(NetworkPreset::Slow3g));
    let custom: NetworkThrottle =
        serde_json::from_str(r#"{ "latencyMs": 40, "downloadKbps": 1000, "uploadKbps": 500 }"#)
            .expect("custom");
    match custom {
        NetworkThrottle::Custom(conditions) => assert_eq!(conditions.latency_ms, 40),
        other => panic!("expected custom, got {other:?}"),
    }
}

#[test]
fn the_visual_section_reads_its_keys_and_writes_only_the_set_ones() {
    let config: ProjectConfig = serde_json::from_str(
        r#"{ "version": 1, "visual": { "threshold": 0.2, "slugStrategy": "hostPath",
             "lfsCheck": false, "failOnMetaMismatch": true } }"#,
    )
    .expect("parses");
    assert_eq!(config.visual.threshold, 0.2);
    assert!(config.visual.full_page, "defaults to true");
    assert_eq!(config.visual.slug_strategy, crate::SlugStrategy::HostPath);
    assert!(!config.visual.lfs_check && config.visual.fail_on_meta_mismatch);
    let defaults = serde_json::to_value(VisualConfig::default()).expect("serializes");
    assert_eq!(
        defaults,
        serde_json::json!({ "threshold": 0.1, "fullPage": true })
    );
    let bad = r#"{ "version": 1, "visual": { "slugStrategy": "custom" } }"#;
    assert!(serde_json::from_str::<ProjectConfig>(bad).is_err());
}

#[test]
fn the_static_source_reads_the_default_globs_unless_the_project_names_its_own() {
    let defaults = AnalyzerConfig::default();
    assert!(defaults.static_scan);
    assert_eq!(
        defaults.static_globs,
        DEFAULT_STATIC_GLOBS.map(str::to_owned)
    );
    let own: ProjectConfig = serde_json::from_value(serde_json::json!({
        "version": 1,
        "analyzer": { "static": false, "staticGlobs": ["src/**/*.scss"] }
    }))
    .expect("valid");
    assert!(!own.analyzer.static_scan);
    assert_eq!(own.analyzer.static_globs, vec!["src/**/*.scss"]);
}

#[test]
fn an_environment_takes_its_interval_reload_in_camel_case() {
    let env: Environment = serde_json::from_value(serde_json::json!({
        "id": "staging", "url": "https://staging.example.com", "reloadIntervalSec": 30
    }))
    .expect("valid");
    assert_eq!(env.reload_interval_sec, Some(30));
    assert_eq!(
        serde_json::to_value(&env).expect("serialises")["reloadIntervalSec"],
        30
    );
}
