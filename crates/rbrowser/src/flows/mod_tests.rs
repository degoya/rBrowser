use rb_commands::{Binding, McpCoverage};
use rb_engine::ViewId;
use rb_mcp::testing::view;
use serde_json::json;

use super::app::resolve_views;
use super::*;

#[test]
fn the_commands_sit_on_leader_f_and_name_their_mcp_tool_or_why_not() {
    let defs = definitions().expect("valid ids");
    let rows: Vec<(String, String, String)> = defs
        .iter()
        .map(|def| {
            let chord = match def.default_bindings.as_slice() {
                [Binding::Leader(chord)] => format!("{:?}", chord.keys),
                other => format!("{other:?}"),
            };
            let mcp = match &def.mcp {
                McpCoverage::Covered(tool) => format!("covered({tool})"),
                McpCoverage::Omitted(reason) => format!("omitted({reason})"),
            };
            (def.id.as_str().to_owned(), chord, mcp)
        })
        .collect();
    let keys: Vec<&str> = rows.iter().map(|(id, _, _)| id.as_str()).collect();
    assert_eq!(keys, [RUN, RUN_ALL, RERUN_LAST, CANCEL, OPEN_RESULTS]);
    assert!(rows[0].1.contains("'F'") && rows[0].1.contains("'R'"));
    assert!(rows[1].1.contains("'A'"));
    assert!(rows[2].1.contains("'L'"));
    assert!(rows[3].1.contains("'X'"));
    assert!(rows[4].1.contains("'O'"));
    for (id, _, mcp) in &rows[..3] {
        assert_eq!(mcp, "covered(flow_run)", "{id}");
    }
    assert!(rows[3].2.starts_with("omitted(cancelling"));
    assert!(
        defs.iter()
            .all(|def| def.group == rb_commands::Group::Flows)
    );
}

#[test]
fn views_are_named_by_id_breakpoint_or_label() {
    let views = vec![
        view(1, "mobile", "about:blank"),
        view(2, "desktop", "about:blank"),
        view(3, "mobile", "about:blank"),
    ];
    assert_eq!(resolve_views(&views, &json!(null)), Ok(None));
    assert_eq!(
        resolve_views(&views, &json!([2, "MOBILE"])),
        Ok(Some(vec![ViewId(2), ViewId(1), ViewId(3)]))
    );
    assert_eq!(
        resolve_views(&views, &json!(["1", "mobile"])),
        Ok(Some(vec![ViewId(1), ViewId(3)]))
    );
    assert!(matches!(
        resolve_views(&views, &json!(["watch"])),
        Err(FlowError::NoViews(_))
    ));
}

#[test]
fn errors_carry_a_code_and_the_untrusted_one_points_at_the_trust_dialog() {
    assert_eq!(FlowError::Untrusted.code(), "flows.untrusted");
    assert!(FlowError::Untrusted.to_string().contains(TRUST_HINT));
    assert_eq!(
        FlowError::BadDir("x".to_owned()).settings_key(),
        Some("automation.flows.dir")
    );
    assert_eq!(
        FlowError::NoName(vec!["a".to_owned(), "b".to_owned()]).to_string(),
        "name the flow to run; the project has: a, b"
    );
}
