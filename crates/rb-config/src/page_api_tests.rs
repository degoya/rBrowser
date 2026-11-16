use super::*;

#[test]
fn both_are_on_by_default_with_64_kib_and_100_a_second() {
    assert_eq!(
        PageApiConfig::default(),
        PageApiConfig {
            enabled: true,
            ua_suffix: true
        }
    );
    assert_eq!(
        BusConfig::default(),
        BusConfig {
            enabled: true,
            max_payload_bytes: 65_536,
            max_per_second: 100
        }
    );
}

#[test]
fn reads_camel_case_and_fills_what_is_missing() {
    let page: PageApiConfig =
        serde_json::from_str(r#"{ "uaSuffix": false }"#).expect("a page API section");
    assert!(page.enabled);
    assert!(!page.ua_suffix);
    let bus: BusConfig =
        serde_json::from_str(r#"{ "enabled": false, "maxPerSecond": 10 }"#).expect("a bus section");
    assert_eq!(
        bus,
        BusConfig {
            enabled: false,
            max_payload_bytes: 65_536,
            max_per_second: 10
        }
    );
}

#[test]
fn the_schema_holds_the_limits() {
    let schema = serde_json::to_value(schemars::schema_for!(BusConfig)).expect("a schema");
    let properties = &schema["properties"];
    assert_eq!(properties["maxPayloadBytes"]["minimum"], 1);
    assert_eq!(properties["maxPayloadBytes"]["maximum"], 196_608);
    assert_eq!(properties["maxPerSecond"]["minimum"], 1);
    assert_eq!(properties["maxPerSecond"]["maximum"], 1000);
}
