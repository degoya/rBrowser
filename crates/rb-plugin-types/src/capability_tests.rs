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

#[test]
fn consent_probe_is_a_capability_without_an_import() {
    assert_eq!(
        "consent-probe".parse::<Capability>(),
        Ok(Capability::ConsentProbe)
    );
    assert_eq!(Capability::ConsentProbe.interface(), None);
    assert!(Capability::ALL.contains(&Capability::ConsentProbe));
    assert_eq!(
        Capability::ConsentProbe.description_key(),
        "plugins.capability.consent-probe"
    );
}

#[test]
fn the_api_client_is_a_capability_without_an_import() {
    assert_eq!(
        "api-client".parse::<Capability>(),
        Ok(Capability::ApiClient)
    );
    assert_eq!(Capability::ApiClient.interface(), None);
    assert!(Capability::ALL.contains(&Capability::ApiClient));
    assert_eq!(
        Capability::ApiClient.description_key(),
        "plugins.capability.api-client"
    );
}

#[test]
fn the_load_test_is_a_capability_without_an_import() {
    assert_eq!("load-test".parse::<Capability>(), Ok(Capability::LoadTest));
    assert_eq!(Capability::LoadTest.interface(), None);
    assert!(Capability::ALL.contains(&Capability::LoadTest));
    assert_eq!(
        Capability::LoadTest.description_key(),
        "plugins.capability.load-test"
    );
}
