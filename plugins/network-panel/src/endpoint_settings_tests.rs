use std::collections::BTreeSet;

use super::{EndpointSettings, Limits};
use crate::fake_host::FakeHost;

#[test]
fn without_settings_fetch_and_xhr_count_with_the_default_limits() {
    let settings = EndpointSettings::read(&mut FakeHost::default());
    assert_eq!(
        settings.types,
        BTreeSet::from(["Fetch".to_owned(), "XHR".to_owned()])
    );
    assert!(settings.templates.is_empty());
    assert_eq!(settings.limits, Limits::default());
    assert_eq!(settings.limits.slow_ms, 1000.0);
}

#[test]
fn the_settings_are_read_and_held_to_their_bounds() {
    let mut host = FakeHost::default();
    host.set(
        "endpoints.resourceTypes",
        r#"["Document", "Fetch", "Nonsense"]"#,
    );
    host.set("endpoints.patterns", r#"["/shop/{slug}", "bad", 7]"#);
    host.set("endpoints.slowMs", "250");
    host.set("endpoints.largeBytes", "999999999999");
    host.set("endpoints.nPlusOne", "1");
    host.set("endpoints.breakpointDifference", "3");
    let settings = EndpointSettings::read(&mut host);
    assert_eq!(
        settings.types,
        BTreeSet::from(["Document".to_owned(), "Fetch".to_owned()])
    );
    assert_eq!(settings.templates.len(), 1);
    assert_eq!(settings.limits.slow_ms, 250.0);
    assert_eq!(settings.limits.large_bytes, 64.0 * 1024.0 * 1024.0);
    assert_eq!(settings.limits.n_plus_one, 2);
    assert_eq!(settings.limits.breakpoint_difference, 3);
}

#[test]
fn an_empty_type_list_counts_every_type() {
    let mut host = FakeHost::default();
    host.set("endpoints.resourceTypes", "[]");
    assert!(EndpointSettings::read(&mut host).types.is_empty());
}
