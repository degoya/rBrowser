use serde_json::json;

use super::*;
use crate::fake::FakeHost;

#[test]
fn defaults_without_settings() {
    let mut host = FakeHost::default();
    assert_eq!(Settings::read(&mut host), Settings::default());
}

#[test]
fn values_are_held_to_the_schema_bounds() {
    let mut host = FakeHost::default();
    host.set("minTextPx", "2")
        .set("minInputPx", "99")
        .set("minLineHeight", "1.4")
        .set("maxWeightsPerFamily", "0")
        .set("maxFileKb", "\"big\"")
        .set("fluidMinRatio", "1.3");
    let settings = Settings::read(&mut host);
    assert!((settings.min_text_px - 6.0).abs() < f64::EPSILON);
    assert!((settings.min_input_px - 32.0).abs() < f64::EPSILON);
    assert!((settings.min_line_height - 1.4).abs() < f64::EPSILON);
    assert_eq!(settings.max_weights, 1);
    assert_eq!(settings.max_file_kb, 100);
    assert!((settings.fluid_min_ratio - 1.3).abs() < f64::EPSILON);
}

#[test]
fn allowed_hosts_are_cleaned_and_matched() {
    let hosts = allowed_hosts(&json!([
        " Fonts.GSTATIC.com ",
        "*.typekit.net",
        "bad host",
        "",
        "*.",
        42
    ]));
    assert_eq!(hosts, ["fonts.gstatic.com", "*.typekit.net"]);
    let settings = Settings {
        allowed_hosts: hosts,
        ..Settings::default()
    };
    assert!(settings.host_allowed("fonts.gstatic.com"));
    assert!(settings.host_allowed("use.typekit.net"));
    assert!(settings.host_allowed("typekit.net"));
    assert!(!settings.host_allowed("fonts.googleapis.com"));
    assert!(!settings.host_allowed("eviltypekit.net"));
}

#[test]
fn at_most_the_cap_of_hosts() {
    let many: Vec<String> = (0..100).map(|n| format!("h{n}.example")).collect();
    assert_eq!(allowed_hosts(&json!(many)).len(), MAX_HOSTS);
}
