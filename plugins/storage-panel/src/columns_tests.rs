use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, ViewInfo};
use serde_json::json;

use super::*;

fn view(id: ViewId, breakpoint: &str, engine: &str) -> ViewInfo {
    ViewInfo {
        id,
        label: format!("View {id}"),
        breakpoint: breakpoint.to_owned(),
        engine: engine.to_owned(),
        width: 400,
        height: 800,
        dpr: 1.0,
        url: "http://127.0.0.1:18767/storage/index.html".to_owned(),
        title: "Storage fixture".to_owned(),
    }
}

const CONFIG: &str = r#"{"version":1,"sessions":["admin"],"breakpoints":[
  {"id":"mobile","width":375},{"id":"tablet","width":768,"session":"default"},
  {"id":"desktop","width":1280,"session":"admin"},{"id":"fx","width":800,"engine":"firefox"}]}"#;

fn host() -> MemoryHost {
    let mut host = MemoryHost::new()
        .grant(Capability::Views)
        .grant(Capability::ProjectRead)
        .with_file(CONFIG_PATH, CONFIG.as_bytes());
    host.views = vec![
        view(1, "mobile", "chromium"),
        view(2, "tablet", "chromium"),
        view(3, "desktop", "chromium"),
        view(4, "fx", "firefox"),
        view(5, "", "chromium"),
    ];
    host
}

#[test]
fn every_view_is_a_column_with_its_breakpoints_session() {
    let columns = columns(&mut host());
    let sessions: Vec<(ViewId, &str)> = columns
        .iter()
        .map(|column| (column.id, column.session.as_str()))
        .collect();
    assert_eq!(
        sessions,
        [
            (1, "default"),
            (2, "default"),
            (3, "admin"),
            (4, "default"),
            (5, "default")
        ]
    );
    assert_eq!(columns[0].origin.as_deref(), Some("http://127.0.0.1:18767"));
    assert!(columns[2].supported());
    assert!(!columns[3].supported(), "a Firefox view is shown, not read");
    assert_eq!(columns[3].to_json("unsupported")["status"], "unsupported");
}

#[test]
fn without_a_config_every_view_is_in_the_default_session() {
    let mut host = host();
    host.project.clear();
    assert!(
        columns(&mut host)
            .iter()
            .all(|c| c.session == DEFAULT_SESSION)
    );
}

#[test]
fn targets_are_all_or_one_view() {
    let column = &columns(&mut host())[1];
    assert_eq!(Target::parse(&json!(null)), Some(Target::All));
    assert_eq!(Target::parse(&json!("all")), Some(Target::All));
    assert_eq!(Target::parse(&json!(2)), Some(Target::View(2)));
    assert!(Target::parse(&json!("active")).is_none());
    assert!(Target::parse(&json!(-1)).is_none());
    assert!(Target::View(2).covers(column));
    assert!(!Target::View(3).covers(column));
    assert!(Target::All.covers(column));
}
