use crate::{API_VERSION, Capability, PACKAGE, PluginKind, WIT, versioned};

#[test]
fn the_wit_package_is_this_version() {
    assert!(
        WIT.contains(&format!("package {PACKAGE}@{API_VERSION};")),
        "the WIT package line names {PACKAGE}@{API_VERSION}"
    );
    assert_eq!(versioned("http"), "rbrowser:plugin/http@0.1.0");
}

#[test]
fn every_kind_is_a_world_exporting_its_interface() {
    for kind in PluginKind::ALL {
        let world = format!("world {} {{", kind.world());
        let start = WIT
            .find(&world)
            .unwrap_or_else(|| panic!("{world} missing"));
        let body = &WIT[start..];
        let end = body.find('}').expect("world closes");
        assert!(
            body[..end].contains(&format!("export {};", kind.export())),
            "{} exports {}",
            kind.world(),
            kind.export()
        );
    }
}

#[test]
fn every_capability_interface_is_in_the_contract() {
    for capability in Capability::ALL {
        for interface in capability.interfaces() {
            assert!(
                WIT.contains(&format!("interface {interface} {{")),
                "interface {interface} missing"
            );
            // Every world may import it (RB-099-24: the additive ones too).
            assert_eq!(
                WIT.matches(&format!("import {interface};")).count(),
                PluginKind::ALL.len(),
                "every world imports {interface}"
            );
        }
    }
}

#[test]
fn the_contract_is_mit_licensed() {
    assert!(WIT.contains("SPDX-License-Identifier: MIT"));
}
