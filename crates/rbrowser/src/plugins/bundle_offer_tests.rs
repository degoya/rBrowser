use super::{Row, row};

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
