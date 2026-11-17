use super::*;
use serde_json::json;

#[test]
fn a_set_local_only_key_is_removed_with_exactly_one_warning() {
    let mut project =
        json!({ "network": { "ignoreCertificateErrors": true, "hostResolverRules": "x" } });
    let warnings = strip_local_only(&mut project);
    assert_eq!(project, json!({ "network": { "hostResolverRules": "x" } }));
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].path, "/network/ignoreCertificateErrors");
    assert_eq!(warnings[0].key, WARNING_KEY);
}

#[test]
fn every_local_only_key_counts_once() {
    let mut project =
        json!({ "network": { "ignoreCertificateErrors": true, "proxy": "proxy:3128" } });
    let warnings = strip_local_only(&mut project);
    assert_eq!(project, json!({ "network": {} }));
    let paths: Vec<&str> = warnings.iter().map(|w| w.path.as_str()).collect();
    assert_eq!(
        paths,
        ["/network/ignoreCertificateErrors", "/network/proxy"]
    );
}

#[test]
fn a_spelled_out_default_is_removed_silently() {
    let mut project = json!({ "network": { "ignoreCertificateErrors": false, "proxy": null } });
    assert!(strip_local_only(&mut project).is_empty());
    assert_eq!(project, json!({ "network": {} }));
}

#[test]
fn a_config_without_the_section_is_untouched() {
    let mut project = json!({ "version": 1 });
    assert!(strip_local_only(&mut project).is_empty());
    assert_eq!(project, json!({ "version": 1 }));
}

#[test]
fn trust_gated_keys_leave_any_project_file_with_a_warning_naming_key_and_file() {
    let mut local = json!({
        "network": {
            "ignoreCertificateErrors": true,
            "proxy": "evil:3128",
            "hostResolverRules": "MAP * 10.0.0.1"
        },
        "sync": { "hover": false }
    });
    let warnings = strip_trust_gated(&mut local, "config.local.json");
    assert_eq!(local, json!({ "network": {}, "sync": { "hover": false } }));
    let paths: Vec<&str> = warnings.iter().map(|w| w.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "/network/ignoreCertificateErrors",
            "/network/proxy",
            "/network/hostResolverRules"
        ]
    );
    for warning in &warnings {
        assert_eq!(warning.key, TRUST_WARNING_KEY);
        assert!(warning.message.contains("config.local.json"), "{warning:?}");
    }
    assert!(warnings[1].message.contains("network.proxy"));
}
