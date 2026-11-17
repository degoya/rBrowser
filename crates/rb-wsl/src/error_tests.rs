use super::*;

#[test]
fn every_error_names_a_key_under_wsl_error() {
    let errors = [
        WslError::NotInstalled,
        WslError::NoDistro,
        WslError::DistroNotFound("Arch".into()),
        WslError::Timeout(3000),
        WslError::Failed {
            status: "exit status: 1".into(),
            detail: "x".into(),
        },
        WslError::Path(PathError::NetworkShare),
        WslError::Handshake(HandshakeError::NonceMismatch),
        WslError::HelperStart("x".into()),
        WslError::HelperCrashed(5),
    ];
    for error in errors {
        assert!(error.i18n_key().starts_with("wsl.error."), "{error}");
    }
}

#[test]
fn a_protocol_mismatch_keeps_its_own_key() {
    let error = WslError::from(HandshakeError::ProtocolMismatch { app: 1, helper: 2 });
    assert_eq!(error.i18n_key(), "wsl.error.protocolMismatch");
}
