use super::Capability;

#[test]
fn every_capability_parses_from_its_id() {
    for capability in Capability::ALL {
        assert_eq!(capability.id().parse::<Capability>(), Ok(capability));
    }
    assert!("filesystem".parse::<Capability>().is_err());
}

#[test]
fn an_interface_maps_back_to_its_capability() {
    assert_eq!(
        Capability::for_interface("project"),
        Some(Capability::ProjectRead)
    );
    assert_eq!(Capability::for_interface("out"), Some(Capability::OutWrite));
    assert_eq!(Capability::for_interface("host"), None);
    assert_eq!(Capability::NetworkRules.interface(), None);
    assert_eq!(Capability::ConsoleLogs.interface(), None);
    assert_eq!(Capability::Status.interface(), None);
    assert_eq!("status".parse::<Capability>(), Ok(Capability::Status));
    assert_eq!(
        "console-logs".parse::<Capability>(),
        Ok(Capability::ConsoleLogs)
    );
}

#[test]
fn the_serde_form_is_the_manifest_id() {
    let json = serde_json::to_string(&Capability::ProjectRead).expect("json");
    assert_eq!(json, "\"project-read\"");
    assert_eq!(
        Capability::OutWrite.description_key(),
        "plugins.capability.out-write"
    );
}

#[test]
fn project_write_is_a_capability_of_its_own_interface() {
    assert_eq!(
        "project-write".parse::<Capability>(),
        Ok(Capability::ProjectWrite)
    );
    assert_eq!(Capability::ProjectWrite.interface(), Some("project-write"));
    assert_eq!(
        Capability::for_interface("project-write"),
        Some(Capability::ProjectWrite)
    );
    assert!(Capability::ALL.contains(&Capability::ProjectWrite));
}
