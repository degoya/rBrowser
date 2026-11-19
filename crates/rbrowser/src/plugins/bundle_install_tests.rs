use rb_ipc::PluginGrantDetails;

use super::{
    INVALID_CODE, grants_within, note_installed, progress_message, valid_ids, with_session,
};

fn grants() -> PluginGrantDetails {
    PluginGrantDetails {
        cdp_domains: vec!["DOM".to_owned(), "CSS".to_owned()],
        project_globs: vec!["src/**".to_owned()],
        http_rate_per_second: Some(5),
        risky: vec!["cdp:DOM".to_owned()],
        ..PluginGrantDetails::default()
    }
}

/// RD-1260-02: the repository's version installs without a review only when it asks for
/// nothing the bundled one does not.
#[test]
fn the_same_or_fewer_grants_are_within_more_are_not() {
    let shipped = grants();
    assert!(grants_within(&shipped, &shipped));
    let fewer = PluginGrantDetails {
        cdp_domains: vec!["CSS".to_owned()],
        http_rate_per_second: Some(2),
        ..PluginGrantDetails::default()
    };
    assert!(grants_within(&fewer, &shipped));
    for wider in [
        PluginGrantDetails {
            evaluate: true,
            ..grants()
        },
        PluginGrantDetails {
            cdp_domains: vec!["Network".to_owned()],
            ..grants()
        },
        PluginGrantDetails {
            http_rate_per_second: Some(9),
            ..grants()
        },
        PluginGrantDetails {
            window_image: true,
            ..grants()
        },
        PluginGrantDetails {
            write_globs: vec!["**".to_owned()],
            ..grants()
        },
        PluginGrantDetails {
            sidecar_methods: vec!["lighthouse.run".to_owned()],
            ..grants()
        },
    ] {
        assert!(!grants_within(&wider, &shipped), "{wider:?}");
    }
    let no_http = PluginGrantDetails {
        http_rate_per_second: None,
        ..grants()
    };
    assert!(!grants_within(&shipped, &no_http), "http on top");
}

/// The wizard's request: between 1 and 256 plugin ids, each once, in its order.
#[test]
fn the_ids_are_checked_and_deduplicated() {
    let ids =
        valid_ids(vec!["seo".to_owned(), "todo".to_owned(), "seo".to_owned()]).expect("valid");
    assert_eq!(ids, ["seo", "todo"]);
    assert_eq!(valid_ids(Vec::new()).expect_err("empty").1, INVALID_CODE);
    assert_eq!(
        valid_ids(vec!["Not An Id".to_owned()]).expect_err("bad").1,
        INVALID_CODE
    );
    assert!(
        valid_ids(vec!["seo".to_owned(); 300]).is_ok(),
        "the same id 300 times is one"
    );
    let many: Vec<String> = (0..257).map(|index| format!("p{index}")).collect();
    assert!(valid_ids(many).is_err());
}

#[test]
fn a_progress_event_is_a_plugin_panel_message() {
    let message = progress_message("seo", 1, 3, "failed", Some("plugins.bundle.error.offline"));
    assert_eq!(
        message,
        serde_json::json!({ "pluginInstallProgress": {
            "id": "seo", "done": 1, "total": 3, "state": "failed",
            "error": "plugins.bundle.error.offline"
        } })
    );
    let message = progress_message("seo", 0, 3, "installing", None);
    assert!(message["pluginInstallProgress"].get("error").is_none());
}

/// Review 6: what this session installed is the base of the next update check.
#[test]
fn a_session_install_raises_the_base_of_the_next_check() {
    let shipped = rb_plugin_host::Shipped {
        id: "session-raise".to_owned(),
        version: "1.0.0".parse().expect("version"),
        fingerprint: "f".repeat(64),
        capabilities: Default::default(),
    };
    assert_eq!(with_session(shipped.clone()).version.to_string(), "1.0.0");
    note_installed("session-raise", "1.1.0");
    assert_eq!(with_session(shipped).version.to_string(), "1.1.0");
}
