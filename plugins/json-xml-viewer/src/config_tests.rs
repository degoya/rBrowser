use rb_plugin_api::test_support::MemoryHost;
use serde_json::json;

use super::*;

fn host_with(settings: &[(&str, &str)]) -> MemoryHost {
    let mut host = MemoryHost::new();
    for (key, value) in settings {
        host.settings.insert((*key).to_owned(), (*value).to_owned());
    }
    host
}

#[test]
fn without_settings_the_defaults_hold() {
    let config = Config::read(&mut host_with(&[]));
    assert_eq!(config, Config::default());
    assert_eq!(
        config.to_json(),
        json!({
            "enabled": true,
            "maxBytes": 20_971_520,
            "defaultView": "tree",
            "expandDepth": 2,
            "theme": "auto",
        })
    );
}

#[test]
fn the_settings_are_read_by_property_name() {
    let config = Config::read(&mut host_with(&[
        ("enabled", "false"),
        ("maxBytes", "1048576"),
        ("defaultView", "\"raw\""),
        // Removed with RB-070-05's dead key: page scripts reach only Chromium views.
        ("inFirefox", "true"),
        ("expandDepth", "5"),
        ("theme", "\"dark\""),
    ]));
    assert_eq!(
        config,
        Config {
            enabled: false,
            max_bytes: 1_048_576,
            default_view: "raw",
            expand_depth: 5,
            theme: "dark",
        }
    );
}

#[test]
fn a_value_out_of_range_or_of_the_wrong_kind_is_the_default() {
    let config = Config::read(&mut host_with(&[
        ("enabled", "\"yes\""),
        ("maxBytes", "12"),
        ("defaultView", "\"pretty\""),
        ("expandDepth", "-1"),
        ("theme", "\"<script>\""),
    ]));
    assert_eq!(config, Config::default());
    let config = Config::read(&mut host_with(&[
        ("maxBytes", "1e12"),
        ("expandDepth", "99"),
    ]));
    assert_eq!(config, Config::default());
}
