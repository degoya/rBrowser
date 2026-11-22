use rb_settings::Category;

use super::*;

fn registry() -> SettingsRegistry {
    let (registry, issues) = crate::control::settings::settings_registry();
    assert!(issues.is_empty(), "the schemas read: {issues:?}");
    registry
}

#[test]
fn a_key_opens_its_field_and_wins_over_a_category_of_its_name() {
    let registry = registry();
    assert_eq!(
        resolve(&registry, "sync.scroll"),
        Some(Target::Key("sync.scroll"))
    );
    // `breakpoints` is a key and a category: the key wins, as in the deep links.
    assert_eq!(
        resolve(&registry, "breakpoints"),
        Some(Target::Key("breakpoints"))
    );
}

#[test]
fn a_category_opens_the_category() {
    let registry = registry();
    assert_eq!(
        resolve(&registry, "capture"),
        Some(Target::Category(Category::Capture))
    );
    // `emulation` is a category and the prefix of `emulation.*`: the category wins.
    assert_eq!(
        resolve(&registry, "emulation"),
        Some(Target::Category(Category::Emulation))
    );
}

#[test]
fn a_section_id_opens_its_section() {
    let registry = registry();
    assert_eq!(
        resolve(&registry, "sitemap"),
        Some(Target::Section {
            category: Category::Capture,
            section: "sitemap"
        })
    );
    assert_eq!(
        resolve(&registry, "env-compare"),
        Some(Target::Section {
            category: Category::Capture,
            section: "env-compare"
        })
    );
    // A section id in two categories opens the first in navigation order.
    assert_eq!(
        resolve(&registry, "trust"),
        Some(Target::Section {
            category: Category::Project,
            section: "trust"
        })
    );
}

#[test]
fn a_key_prefix_opens_the_section_or_category_its_keys_share() {
    let registry = registry();
    assert_eq!(
        resolve(&registry, "visual"),
        Some(Target::Section {
            category: Category::Capture,
            section: "visual-regression"
        })
    );
    assert_eq!(
        resolve(&registry, "pageApi"),
        Some(Target::Section {
            category: Category::Sync,
            section: "page-api"
        })
    );
    // `settings.*` sits in two sections of General: the category.
    assert_eq!(
        resolve(&registry, "settings"),
        Some(Target::Category(Category::General))
    );
    // `wsl.*` sits in Reload and in Automation: no one place.
    assert_eq!(resolve(&registry, "wsl"), None);
    // A prefix is whole segments: `sync.scr` names no key.
    assert_eq!(resolve(&registry, "sync.scr"), None);
}

#[test]
fn anything_else_opens_nothing() {
    let registry = registry();
    assert_eq!(resolve(&registry, "no.such"), None);
    assert_eq!(resolve(&registry, ""), None);
    assert_eq!(resolve(&registry, "\"><script>"), None);
}

#[test]
fn the_route_carries_the_section_for_an_area() {
    let registry = registry();
    assert_eq!(route_of(&registry, "sync.scroll"), "settings/sync.scroll");
    assert_eq!(route_of(&registry, "capture"), "settings/capture");
    assert_eq!(
        route_of(&registry, "sitemap"),
        "settings/capture?section=sitemap"
    );
    assert_eq!(
        route_of(&registry, "visual"),
        "settings/capture?section=visual-regression"
    );
    assert_eq!(route_of(&registry, "no.such"), "settings?missing=no.such");
}

/// RB-094-26: a link that opens nothing opens the settings at their start with the link named,
/// never silently another page; what is no key is not put in the URL.
#[test]
fn a_link_that_opens_nothing_is_named_in_the_route() {
    let registry = registry();
    assert_eq!(
        route_of(&registry, "api-client.noSuch"),
        "settings?missing=api-client.noSuch"
    );
    assert_eq!(route_of(&registry, "\"><script>"), "settings?missing=");
    assert_eq!(route_of(&registry, ""), "settings?missing=");
    assert_eq!(
        route_of(&registry, &"a".repeat(201)),
        "settings?missing=",
        "longer than a key"
    );
    assert_eq!(
        not_found_route(PENDING, "api-client.benchRepeats"),
        "settings?pending=api-client.benchRepeats"
    );
}

/// RB-094-26: with the plugins' keys in the registry a plugin key opens its field and a plugin's
/// prefix its section - the owner's link `api-client.benchRepeats` in Network > API client.
#[test]
fn a_plugin_key_opens_its_field_and_a_plugin_prefix_its_section() {
    let api_client = serde_json::from_str(include_str!(
        "../../../../plugins/api-client/settings.schema.json"
    ))
    .expect("JSON");
    let registry =
        crate::control::plugin_keys::with_plugins(&[("api-client".to_owned(), api_client)]);
    assert_eq!(
        resolve(&registry, "api-client.benchRepeats"),
        Some(Target::Key("api-client.benchRepeats"))
    );
    assert_eq!(
        route_of(&registry, "api-client.benchRepeats"),
        "settings/api-client.benchRepeats"
    );
    assert_eq!(
        resolve(&registry, "api-client"),
        Some(Target::Section {
            category: Category::Network,
            section: "api-client"
        })
    );
    // Without the plugin's keys (the registry before RB-094-26) the link opened nothing.
    assert_eq!(resolve(&self::registry(), "api-client.benchRepeats"), None);
}

/// Review of RB-094-26: `pending` only for a key of an installed plugin while a load runs or
/// waits to run; a link of no plugin, or any link while the first-run wizard holds the load
/// back, is `missing` - else the page would wait for keys that never come.
#[test]
fn only_an_installed_plugin_s_key_is_pending_and_only_while_it_loads() {
    let installed = |id: &str| id == "api-client";
    assert_eq!(
        not_found_query("api-client.benchRepeats", installed, true),
        PENDING
    );
    assert_eq!(
        not_found_query("api-client.benchRepeats", installed, false),
        MISSING,
        "no load runs: held by the wizard, or done"
    );
    assert_eq!(not_found_query("no.such.key", installed, true), MISSING);
    assert_eq!(not_found_query("api-client", installed, true), PENDING);
    assert_eq!(not_found_query("", installed, true), MISSING);
}

/// RB-094-27: a secret key is no place of the settings window - neither as a key nor under a
/// prefix -, and its link opens the panel its `x-panel` names: `api-client.secrets` the API
/// client at Variables. Without the plugin's keys it opens nothing.
#[test]
fn a_secret_key_opens_its_panel_and_never_the_window() {
    let api_client = serde_json::from_str(include_str!(
        "../../../../plugins/api-client/settings.schema.json"
    ))
    .expect("JSON");
    let registry =
        crate::control::plugin_keys::with_plugins(&[("api-client".to_owned(), api_client)]);
    assert_eq!(resolve(&registry, "api-client.secrets"), None);
    assert_eq!(
        panel_of(&registry, "api-client.secrets"),
        Some(PanelLink {
            plugin: "api-client",
            panel: "api-client",
            tab: Some("variables")
        })
    );
    // The section stays the API client's: its other keys are there.
    assert_eq!(
        resolve(&registry, "api-client"),
        Some(Target::Section {
            category: Category::Network,
            section: "api-client"
        })
    );
    // A key that is no secret is the window's, not a panel's.
    assert_eq!(panel_of(&registry, "api-client.benchRepeats"), None);
    assert_eq!(panel_of(&registry, "api-client"), None);
    assert_eq!(panel_of(&self::registry(), "api-client.secrets"), None);
}

/// A secret key that names no panel opens nothing, and a prefix of secret keys only is no area.
#[test]
fn a_secret_key_without_a_panel_opens_nothing() {
    let fragment = serde_json::json!({
        "type": "object",
        "properties": {
            "vault": {
                "type": "object",
                "default": {},
                "x-category": "network",
                "x-section": "api-client",
                "x-order": 900,
                "x-scope": ["local"],
                "x-local-only": true,
                "x-protected": true,
                "x-secret": true
            }
        }
    });
    let registry =
        crate::control::plugin_keys::with_plugins(&[("secret-only".to_owned(), fragment)]);
    assert!(registry.get("secret-only.vault").is_some());
    assert_eq!(resolve(&registry, "secret-only.vault"), None);
    assert_eq!(panel_of(&registry, "secret-only.vault"), None);
    assert_eq!(resolve(&registry, "secret-only"), None);
}
