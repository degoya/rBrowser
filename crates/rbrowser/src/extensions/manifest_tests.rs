use super::*;

fn manifest(extra: &str) -> String {
    format!(r#"{{ "manifest_version": 3, "name": "Hello", "version": "1.0.2"{extra} }}"#)
}

#[test]
fn a_manifest_v3_reads_with_its_name_and_version() {
    let read = parse(&manifest("")).expect("valid");
    assert_eq!(read.name, "Hello");
    assert_eq!(read.version, "1.0.2");
    assert_eq!(
        read.id, None,
        "an unpacked folder without a key gets Chromium's id"
    );
}

#[test]
fn the_id_comes_from_the_key_as_chromium_computes_it() {
    // Encoded here, so no key-shaped literal sits in the source (the secret scan).
    let key = base64::engine::general_purpose::STANDARD.encode(b"rbrowser extension test key");
    let read = parse(&manifest(&format!(r#", "key": "{key}""#))).expect("valid");
    assert_eq!(read.id.as_deref(), Some("kojkngdmfhoklkoabcpepbnddlehfphl"));
}

#[test]
fn a_key_that_is_no_base64_is_refused() {
    assert!(matches!(
        parse(&manifest(r#", "key": "not base64!""#)),
        Err(ManifestError::BadKey)
    ));
}

#[test]
fn only_manifest_v3_is_accepted() {
    for (text, found) in [
        (
            r#"{ "manifest_version": 2, "name": "a", "version": "1" }"#,
            "2",
        ),
        (r#"{ "name": "a", "version": "1" }"#, "none"),
        (
            r#"{ "manifest_version": "3", "name": "a", "version": "1" }"#,
            "\"3\"",
        ),
    ] {
        match parse(text) {
            Err(ManifestError::NotV3(seen)) => assert_eq!(seen, found, "{text}"),
            other => panic!("{text}: {other:?}"),
        }
    }
}

#[test]
fn a_manifest_that_is_no_json_object_is_refused() {
    assert!(matches!(parse("{ nope"), Err(ManifestError::NotJson(_))));
    assert!(matches!(parse("[1]"), Err(ManifestError::NotJson(_))));
}

#[test]
fn the_name_must_be_text() {
    for name in [r#""""#, r#""   ""#, "7", "null"] {
        let text = format!(r#"{{ "manifest_version": 3, "name": {name}, "version": "1" }}"#);
        assert!(matches!(parse(&text), Err(ManifestError::NoName)), "{name}");
    }
}

#[test]
fn versions_follow_chromiums_rules() {
    for good in ["1", "0.1", "1.2.3.4", "65535.0", "10.20.30"] {
        assert!(is_valid_version(good), "{good}");
    }
    for bad in [
        "",
        "1.",
        ".1",
        "1..2",
        "1.2.3.4.5",
        "65536",
        "01",
        "1.02",
        "1.a",
        "-1",
        "1 .2",
        "v1",
    ] {
        assert!(!is_valid_version(bad), "{bad}");
    }
    let text = r#"{ "manifest_version": 3, "name": "a", "version": "1.x" }"#;
    assert!(matches!(parse(text), Err(ManifestError::BadVersion(v)) if v == "1.x"));
}

/// `debugger` would let the extension drive every target over CDP, rBrowser's own pages
/// included: refused, as a permission and as an optional one.
#[test]
fn the_debugger_permission_is_refused() {
    for extra in [
        r#", "permissions": ["storage", "debugger"]"#,
        r#", "optional_permissions": ["debugger"]"#,
    ] {
        assert!(
            matches!(
                parse(&manifest(extra)),
                Err(ManifestError::Refused("debugger"))
            ),
            "{extra}"
        );
    }
    assert!(parse(&manifest(r#", "permissions": ["storage", "tabs"]"#)).is_ok());
}

#[test]
fn a_localised_name_is_kept_as_written() {
    let text = r#"{ "manifest_version": 3, "name": "__MSG_appName__", "version": "1" }"#;
    assert_eq!(parse(text).expect("valid").name, "__MSG_appName__");
}

#[test]
fn every_error_has_a_stable_code() {
    let errors = [
        ManifestError::NotJson("x".to_owned()),
        ManifestError::NotV3("2".to_owned()),
        ManifestError::NoName,
        ManifestError::BadVersion("x".to_owned()),
        ManifestError::BadKey,
        ManifestError::Refused("debugger"),
    ];
    let mut codes: Vec<&str> = errors.iter().map(ManifestError::code).collect();
    codes.sort_unstable();
    codes.dedup();
    assert_eq!(codes.len(), errors.len());
    assert!(codes.iter().all(|code| code.starts_with("extensions.")));
}
