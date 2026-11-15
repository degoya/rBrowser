use rb_plugin_types::Capability;

use super::{Grants, PLUGIN_SIDECAR_NAMESPACES};

fn parse(table: &str) -> anyhow::Result<Grants> {
    Grants::parse(&toml::from_str(table).expect("toml"))
}

#[test]
fn sidecar_grants_exactly_the_listed_methods() {
    let grants = parse("sidecar = { methods = [\"lighthouse.run\"] }").expect("grants");
    assert!(grants.has(Capability::Sidecar));
    assert_eq!(grants.sidecar_methods, vec!["lighthouse.run".to_owned()]);
    assert!(grants.sidecar_allows("lighthouse.run"));
    assert!(!grants.sidecar_allows("lighthouse.audit"));
    assert!(!grants.sidecar_allows("lighthouse"));
}

#[test]
fn sidecar_needs_its_methods_and_never_the_cores_own() {
    for table in [
        "sidecar = true",
        "sidecar = {}",
        "sidecar = { methods = [] }",
        "sidecar = { methods = [\"run\"] }",
        "sidecar = { methods = [\"a.b c\"] }",
        "sidecar = { methods = [\"lighthouse.run\"], other = 1 }",
    ] {
        assert!(parse(table).is_err(), "{table}");
    }
    // The core's own namespaces, and any namespace a later sidecar module adds, until it is
    // listed for plugins.
    for namespace in [
        "engines",
        "flow",
        "flows",
        "playwright",
        "sidecar",
        "webkit",
        "network",
        "future",
    ] {
        let table = format!("sidecar = {{ methods = [\"{namespace}.anything\"] }}");
        assert!(parse(&table).is_err(), "{table}");
    }
}

#[test]
fn sidecar_namespaces_are_an_allowlist() {
    assert_eq!(PLUGIN_SIDECAR_NAMESPACES, ["lighthouse"]);
    for namespace in PLUGIN_SIDECAR_NAMESPACES {
        let table = format!("sidecar = {{ methods = [\"{namespace}.start\"] }}");
        assert!(parse(&table).is_ok(), "{table}");
    }
}

#[test]
fn cdp_domains_are_only_those_of_the_method_allowlist() {
    let grants = parse("cdp = { domains = [\"DOM\", \"Network\", \"Runtime\"] }").expect("grants");
    assert_eq!(grants.cdp_domains, ["DOM", "Network", "Runtime"]);
    for domain in [
        "Browser",
        "Fetch",
        "Input",
        "Security",
        "Storage",
        "Target",
        "Tethering",
        "SystemInfo",
        "IndexedDB",
        "Debugger",
        "HeapProfiler",
        "dom",
    ] {
        let table = format!("cdp = {{ domains = [\"{domain}\"] }}");
        assert!(parse(&table).is_err(), "{table}");
    }
}

#[test]
fn no_sidecar_grant_allows_no_method() {
    let grants = parse("views = true").expect("grants");
    assert!(!grants.sidecar_allows("lighthouse.run"));
}

#[test]
fn the_window_image_is_a_cdp_permission_of_its_own() {
    // Review S2 of RB-080-64: only a plugin whose manifest asks for it may take the view's
    // window image without a clip.
    let plain = parse("cdp = { domains = [\"Page\"] }").expect("grants");
    assert!(!plain.cdp_window_image);
    let window = parse("cdp = { domains = [\"Page\"], window_image = true }").expect("grants");
    assert!(window.cdp_window_image);
    assert_eq!(window.cdp_domains, ["Page"]);
    for table in [
        "cdp = { domains = [\"Page\"], window_image = \"yes\" }",
        "cdp = { domains = [\"DOM\"], window_image = true }",
        "cdp = { domains = [\"Page\"], window = true }",
    ] {
        assert!(parse(table).is_err(), "{table}");
    }
}
