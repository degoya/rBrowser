use serde_json::json;

use super::*;

fn meta() -> BaselineMeta {
    BaselineMeta {
        machine: "DEV-01".to_owned(),
        os: "windows".to_owned(),
        monitor_dpi: Some(144.0),
        dpr: 2.0,
        engine: "chromium".to_owned(),
        engine_version: "156.0.8078.88".to_owned(),
        cef_version: Some("156.0.26+g8ea0c73".to_owned()),
        app_version: "0.5.0".to_owned(),
        url: "http://localhost:5173/".to_owned(),
        slug: "index".to_owned(),
        viewport: Viewport {
            width: 393,
            height: 852,
        },
        full_page: true,
        created_at: 1_700_000_000,
    }
}

fn fields(found: &[MetaMismatch]) -> Vec<&str> {
    found
        .iter()
        .map(|mismatch| mismatch.field.as_str())
        .collect()
}

#[test]
fn the_same_setup_matches_whatever_the_app_version_url_and_date() {
    let mut later = meta();
    later.app_version = "0.6.0".to_owned();
    later.url = "http://localhost:4000/".to_owned();
    later.created_at += 86_400;
    assert!(compare_meta(&meta(), &later).is_empty());
}

#[test]
fn another_engine_version_is_named() {
    let mut now = meta();
    now.engine_version = "157.0.1".to_owned();
    let found = compare_meta(&meta(), &now);
    assert_eq!(
        found,
        [MetaMismatch {
            field: "engineVersion".to_owned(),
            baseline: "156.0.8078.88".to_owned(),
            current: "157.0.1".to_owned(),
        }]
    );
}

#[test]
fn another_density_is_named() {
    let mut now = meta();
    now.dpr = 1.5;
    now.monitor_dpi = Some(96.0);
    assert_eq!(fields(&compare_meta(&meta(), &now)), ["monitorDpi", "dpr"]);
}

#[test]
fn another_machine_or_os_is_recorded_but_no_mismatch() {
    // A teammate's baseline from another machine with the same engine and density is reliable.
    let mut now = meta();
    now.machine = "LAPTOP".to_owned();
    now.os = "linux".to_owned();
    assert!(compare_meta(&meta(), &now).is_empty());
}

#[test]
fn an_unknown_monitor_dpi_or_cef_build_is_no_mismatch() {
    let mut now = meta();
    now.monitor_dpi = None;
    now.cef_version = None;
    assert!(compare_meta(&meta(), &now).is_empty());
}

#[test]
fn the_meta_file_round_trips_with_camel_case_keys() {
    let mut file = MetaFile::default();
    file.baselines.insert("mobile".to_owned(), meta());
    let value = serde_json::to_value(&file).expect("serializes");
    assert_eq!(value["version"], json!(1));
    assert_eq!(
        value["baselines"]["mobile"]["engineVersion"],
        json!("156.0.8078.88")
    );
    assert_eq!(value["baselines"]["mobile"]["monitorDpi"], json!(144.0));
    assert_eq!(
        value["baselines"]["mobile"]["viewport"],
        json!({ "width": 393, "height": 852 })
    );
    let back: MetaFile = serde_json::from_value(value).expect("parses");
    assert_eq!(back, file);
}

#[test]
fn a_meta_entry_with_missing_fields_still_reads() {
    let entry: BaselineMeta =
        serde_json::from_value(json!({ "engine": "chromium" })).expect("parses");
    assert_eq!(entry.engine, "chromium");
    assert_eq!(entry.dpr, 0.0);
}

#[test]
fn the_public_url_drops_credentials_query_and_fragment() {
    assert_eq!(
        public_url("https://user:token@example.com:8443/shop?session=abc#top"),
        "https://example.com:8443/shop"
    );
    assert_eq!(public_url("http://localhost:5173"), "http://localhost:5173");
    assert_eq!(public_url("about:blank"), "about:blank");
}

#[test]
fn the_machine_name_comes_from_the_environment() {
    let windows = |name: &str| (name == "COMPUTERNAME").then(|| "DEV-01".to_owned());
    assert_eq!(machine_name(windows), "DEV-01");
    let linux = |name: &str| (name == "HOSTNAME").then(|| " box \n".to_owned());
    assert_eq!(machine_name(linux), "box");
    assert_eq!(machine_name(|_| None), "unknown");
}

#[test]
fn the_pixel_ratio_is_the_image_width_over_the_viewport() {
    assert_eq!(pixel_ratio(1179, 393), 3.0);
    assert_eq!(pixel_ratio(589, 393), 1.5);
    assert_eq!(pixel_ratio(100, 0), 0.0);
}
