use rb_config::{Geolocation, NetworkConditions};
use serde_json::{Value, json};

use super::*;
use crate::emulation::cdp::emulation_calls;

fn berlin() -> Geolocation {
    Geolocation {
        latitude: 52.52,
        longitude: 13.4,
        accuracy: 50.0,
    }
}

fn with(change: impl FnOnce(&mut Emulation)) -> Emulation {
    let mut emulation = Emulation::default();
    change(&mut emulation);
    emulation
}

/// The calls as (method, params).
type Sent = Vec<(&'static str, Value)>;
/// (name, before, next, calls).
type FieldCase = (&'static str, Emulation, Emulation, Sent);
/// (granted, url, has a position, force, held by another view, calls, granted after).
type PermissionCase = (
    Option<&'static str>,
    &'static str,
    bool,
    bool,
    Vec<&'static str>,
    Sent,
    Option<&'static str>,
);

fn sent(calls: &[CdpCall]) -> Sent {
    calls
        .iter()
        .map(|call| (call.method, call.params.clone()))
        .collect()
}

/// One row per field: the emulation before, after, and the calls an incremental apply sends.
#[test]
fn each_extra_maps_to_its_cdp_call() {
    let unset = Emulation::default();
    let cases: Vec<FieldCase> = vec![
        (
            "geolocation set",
            unset.clone(),
            with(|e| e.geolocation = Some(berlin())),
            vec![(
                "Emulation.setGeolocationOverride",
                json!({ "latitude": 52.52, "longitude": 13.4, "accuracy": 50.0 }),
            )],
        ),
        (
            "geolocation cleared",
            with(|e| e.geolocation = Some(berlin())),
            unset.clone(),
            vec![("Emulation.clearGeolocationOverride", json!({}))],
        ),
        (
            "time zone set",
            unset.clone(),
            with(|e| e.timezone = Some("America/New_York".into())),
            vec![(
                "Emulation.setTimezoneOverride",
                json!({ "timezoneId": "America/New_York" }),
            )],
        ),
        (
            "time zone cleared",
            with(|e| e.timezone = Some("Asia/Tokyo".into())),
            unset.clone(),
            vec![("Emulation.setTimezoneOverride", json!({ "timezoneId": "" }))],
        ),
        (
            "cpu throttled",
            unset.clone(),
            with(|e| e.cpu_throttling = Some(4.0)),
            vec![("Emulation.setCPUThrottlingRate", json!({ "rate": 4.0 }))],
        ),
        (
            "cpu back to full speed",
            with(|e| e.cpu_throttling = Some(4.0)),
            unset.clone(),
            vec![("Emulation.setCPUThrottlingRate", json!({ "rate": 1.0 }))],
        ),
        ("nothing changed", unset.clone(), unset.clone(), vec![]),
    ];
    for (name, before, next, expected) in cases {
        let calls = extras_calls(&before, &next, false);
        assert_eq!(sent(&calls), expected, "{name}");
    }
}

#[test]
fn a_full_apply_sends_every_set_extra_again_and_nothing_unset() {
    let next = with(|e| {
        e.geolocation = Some(berlin());
        e.timezone = Some("Asia/Tokyo".into());
        e.cpu_throttling = Some(6.0);
    });
    let methods: Vec<&str> = extras_calls(&next, &next, true)
        .iter()
        .map(|call| call.method)
        .collect();
    assert_eq!(
        methods,
        [
            "Emulation.setGeolocationOverride",
            "Emulation.setTimezoneOverride",
            "Emulation.setCPUThrottlingRate",
        ],
        "after a reconnect the session has none of them"
    );
    let unset = Emulation::default();
    assert!(extras_calls(&unset, &unset, true).is_empty());
}

#[test]
fn the_extras_ride_on_the_emulation_calls() {
    let next = with(|e| e.timezone = Some("Asia/Tokyo".into()));
    let methods: Vec<&str> = emulation_calls(None, &next, true)
        .iter()
        .map(|call| call.method)
        .collect();
    assert!(
        methods.contains(&"Emulation.setTimezoneOverride"),
        "{methods:?}"
    );
}

#[test]
fn offline_is_the_network_conditions_and_print_the_media_type() {
    let offline = with(|e| {
        e.network = Some(NetworkConditions {
            offline: true,
            ..NetworkConditions::default()
        });
    });
    let calls = emulation_calls(None, &offline, false);
    let network = calls
        .iter()
        .find(|call| call.method == "Network.emulateNetworkConditions")
        .expect("sent");
    assert_eq!(network.params["offline"], true);
    let online = emulation_calls(Some(&offline), &Emulation::default(), false);
    let network = online
        .iter()
        .find(|call| call.method == "Network.emulateNetworkConditions")
        .expect("lifted without a new view");
    assert_eq!(network.params["offline"], false);
    let mut print = Emulation::default();
    print.media.media_type = Some(rb_config::MediaType::Print);
    let calls = emulation_calls(None, &print, false);
    let media = calls
        .iter()
        .find(|call| call.method == "Emulation.setEmulatedMedia")
        .expect("sent");
    assert_eq!(media.params["media"], "print");
}

#[test]
fn the_permission_follows_the_origin_of_a_view_with_a_position() {
    let grant = |origin: &str| {
        (
            "Browser.grantPermissions",
            json!({ "permissions": ["geolocation"], "origin": origin }),
        )
    };
    let revoke = |origin: &str| {
        (
            "Browser.setPermission",
            json!({
                "permission": { "name": "geolocation" },
                "setting": "prompt",
                "origin": origin,
            }),
        )
    };
    let a = "https://a.test";
    let b = "https://b.test";
    let cases: Vec<PermissionCase> = vec![
        (
            None,
            "https://a.test/x",
            true,
            false,
            vec![],
            vec![grant(a)],
            Some(a),
        ),
        (
            Some(a),
            "https://a.test/y",
            true,
            false,
            vec![],
            vec![],
            Some(a),
        ),
        (
            Some(a),
            "https://a.test/y",
            true,
            true,
            vec![],
            vec![grant(a)],
            Some(a),
        ),
        (
            Some(a),
            "https://b.test/",
            true,
            false,
            vec![],
            vec![revoke(a), grant(b)],
            Some(b),
        ),
        (
            Some(a),
            "https://b.test/",
            true,
            false,
            vec![a],
            vec![grant(b)],
            Some(b),
        ),
        (
            Some(a),
            "https://a.test/",
            false,
            false,
            vec![],
            vec![revoke(a)],
            None,
        ),
        (
            Some(a),
            "https://a.test/",
            false,
            false,
            vec![a],
            vec![],
            None,
        ),
        (None, "about:blank", true, false, vec![], vec![], None),
        (None, "https://a.test/", false, true, vec![], vec![], None),
    ];
    for (granted, url, position, force, held, calls, after) in cases {
        let plan = geolocation_permission(granted, url, position, force, &held);
        assert_eq!(
            sent(&plan.calls),
            calls,
            "{granted:?} {url} {position} {force}"
        );
        assert_eq!(plan.granted.as_deref(), after, "{granted:?} {url}");
    }
}

/// A grant without `browserContextId` lands on the browser's default context, so every view of
/// another session misses it and the default session holds it for all its pages (charge-4
/// review, finding 4): the calls name the view's own context when the browser gave one.
#[test]
fn the_permission_calls_name_the_views_browser_context() {
    let mut plan =
        geolocation_permission(Some("https://a.test"), "https://b.test/", true, false, &[]);
    in_context(&mut plan.calls, Some("CTX-7"));
    assert_eq!(plan.calls.len(), 2);
    for call in &plan.calls {
        assert_eq!(call.params["browserContextId"], "CTX-7", "{}", call.method);
    }
    let mut plan = geolocation_permission(None, "https://b.test/", true, false, &[]);
    in_context(&mut plan.calls, None);
    assert!(plan.calls[0].params.get("browserContextId").is_none());
}
