use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use rb_plugin_host::{PluginManifest, parse_manifest};

use super::{Row, row, with_choice};

fn manifest(id: &str) -> PluginManifest {
    let text = format!(
        "manifest_version = 1\nid = \"{id}\"\nname = \"{id}\"\nversion = \"1.2.3\"\n\
         api_version = \"0.1.0\"\nkinds = [\"feature-plugin\"]\nmcp_prefix = \"{id}\"\n\
         key_id = \"test-v1\"\npublic_key = \"AAAA\"\ndescription = \"{id}\"\n\n\
         [locales]\nen = \"locales/en.json\"\n"
    );
    parse_manifest(text.as_bytes()).expect("a valid manifest")
}

/// The wizard's row of a plugin (RB-091-05 builds against these names).
#[test]
fn a_row_names_what_the_wizard_shows() {
    let capabilities = ["http".to_owned()];
    let row = row(Row {
        id: "seo",
        name: "SEO",
        description: "Checks a page",
        version: "1.2.0",
        size: 4096,
        capabilities: &capabilities,
        installed: false,
        origin: "bundle",
    });
    assert_eq!(row["id"], "seo");
    assert_eq!(row["name"], "SEO");
    assert_eq!(row["description"], "Checks a page");
    assert_eq!(row["category"], "checks");
    assert_eq!(row["sizeBytes"], 4096);
    assert_eq!(row["recommended"], true);
    assert_eq!(row["origin"], "bundle");
    assert_eq!(row["grants"], serde_json::json!(["http"]));
    assert_eq!(row["installed"], false);
    assert_eq!(row["version"], "1.2.0");
}

/// RB-091-08: while the first choice holds the load, the wizard lists the bundle from the
/// packages' manifests - each installed when the answered choice holds it, so Back shows the real
/// state before the load is done.
#[test]
fn the_bundle_without_the_load_says_what_the_choice_holds() {
    let manifests: BTreeMap<PathBuf, PluginManifest> = [
        (PathBuf::from("plugins/seo-1.2.3.rbplugin"), manifest("seo")),
        (PathBuf::from("plugins/axe-1.2.3.rbplugin"), manifest("axe")),
    ]
    .into();
    let chosen: BTreeSet<String> = ["seo".to_owned()].into();
    let found = with_choice(manifests.clone(), &chosen);
    assert_eq!(found.keys().collect::<Vec<_>>(), ["axe", "seo"]);
    assert!(found["seo"].installed);
    assert!(!found["axe"].installed);
    assert_eq!(
        found["seo"].path,
        PathBuf::from("plugins/seo-1.2.3.rbplugin")
    );
    // A pending choice holds nothing yet.
    assert!(
        with_choice(manifests, &BTreeSet::new())
            .values()
            .all(|plugin| !plugin.installed)
    );
}
