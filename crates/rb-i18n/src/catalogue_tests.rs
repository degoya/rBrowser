use super::*;

#[test]
fn a_nested_catalogue_is_read_by_dotted_keys() {
    let catalogue = Catalogue::from_json(
        r#"{"toolbar": {"reload": "Reload"}, "grid": {"views": "{count, plural, one {# view} other {# views}}"}}"#,
    )
    .expect("valid");
    assert_eq!(catalogue.get("toolbar.reload"), Some("Reload"));
    assert_eq!(catalogue.get("toolbar"), None);
    assert_eq!(catalogue.len(), 2);
    assert_eq!(
        catalogue.keys().collect::<Vec<_>>(),
        ["grid.views", "toolbar.reload"]
    );
    assert!(Catalogue::from_json("{}").expect("empty").is_empty());
}

#[test]
fn a_key_given_twice_is_refused_also_across_nesting() {
    let twice = Catalogue::from_json(r#"{"toolbar": {"reload": "A", "reload": "B"}}"#);
    assert_eq!(
        twice,
        Err(I18nError::Duplicate("toolbar.reload".to_owned()))
    );
    let across = Catalogue::from_json(r#"{"toolbar.reload": "A", "toolbar": {"reload": "B"}}"#);
    assert_eq!(
        across,
        Err(I18nError::Duplicate("toolbar.reload".to_owned()))
    );
}

#[test]
fn bad_shapes_keys_and_messages_are_refused() {
    assert!(matches!(
        Catalogue::from_json("[]"),
        Err(I18nError::Json(_))
    ));
    assert!(matches!(Catalogue::from_json("{"), Err(I18nError::Json(_))));
    assert!(matches!(
        Catalogue::from_json(r#"{"toolbar": {"reload": 3}}"#),
        Err(I18nError::Json(_))
    ));
    assert_eq!(
        Catalogue::from_json(r#"{"toolbar": "Toolbar"}"#),
        Err(I18nError::BadKey("toolbar".to_owned()))
    );
    assert_eq!(
        Catalogue::from_json(r#"{"Toolbar": {"reload": "Reload"}}"#),
        Err(I18nError::BadKey("Toolbar.reload".to_owned()))
    );
    assert!(matches!(
        Catalogue::from_json(r#"{"toolbar": {"reload": "Re{load"}}"#),
        Err(I18nError::BadMessage { key, .. }) if key == "toolbar.reload"
    ));
    assert!(matches!(
        Catalogue::from_json(r#"{"toolbar": {"reload": "  "}}"#),
        Err(I18nError::BadMessage { key, .. }) if key == "toolbar.reload"
    ));
}

#[test]
fn merging_adds_new_keys_keeps_equal_ones_and_refuses_a_conflict() {
    let mut base = Catalogue::from_json(r#"{"a": {"x": "X"}}"#).expect("valid");
    let more = Catalogue::from_json(r#"{"a": {"x": "X", "y": "Y"}}"#).expect("valid");
    base.merge(&more).expect("no conflict");
    assert_eq!(base.len(), 2);
    let other = Catalogue::from_json(r#"{"a": {"y": "Why"}}"#).expect("valid");
    assert_eq!(
        base.merge(&other),
        Err(I18nError::Conflict("a.y".to_owned()))
    );
    assert_eq!(
        base.get("a.y"),
        Some("Y"),
        "a refused merge changes nothing"
    );
}

#[test]
fn a_plugin_catalogue_stays_in_its_namespace() {
    let json =
        br#"{"plugin": {"seo-essentials": {"title": "SEO", "panel": {"empty": "Nothing"}}}}"#;
    let catalogue = plugin_catalogue("seo-essentials", json).expect("valid");
    assert_eq!(catalogue.get("plugin.seo-essentials.title"), Some("SEO"));
    assert_eq!(
        plugin_catalogue("seo", json),
        Err(I18nError::ForeignKey {
            plugin: "seo".to_owned(),
            key: "plugin.seo-essentials.panel.empty".to_owned(),
        })
    );
    let core = br#"{"toolbar": {"reload": "Hijacked"}}"#;
    assert!(matches!(
        plugin_catalogue("seo", core),
        Err(I18nError::ForeignKey { .. })
    ));
    for id in ["", "Seo", "seo.x", "seo x", "-seo"] {
        assert_eq!(
            plugin_catalogue(id, b"{}"),
            Err(I18nError::BadPluginId(id.to_owned()))
        );
    }
    assert!(matches!(
        plugin_catalogue("seo", b"\xff"),
        Err(I18nError::Json(_))
    ));
}

#[test]
fn a_plugin_catalogue_has_a_size_limit() {
    let big = format!(
        r#"{{"plugin": {{"seo": {{"text": "{}"}}}}}}"#,
        "x".repeat(MAX_PLUGIN_LOCALE_BYTES)
    );
    assert_eq!(
        plugin_catalogue("seo", big.as_bytes()),
        Err(I18nError::TooLarge {
            limit: MAX_PLUGIN_LOCALE_BYTES
        })
    );
}
