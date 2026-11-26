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
    assert_eq!(
        keys,
        [
            RUN,
            RUN_ALL,
            RERUN_LAST,
            CANCEL,
            OPEN_RESULTS,
            PANEL,
            DELETE,
            RUN_ALL_VISIBLE
        ]
    );
    // RB-099-10: every flow in the views the grid shows, `F G`.
    assert!(rows[7].1.contains("'F'") && rows[7].1.contains("'G'"));
    assert_eq!(rows[7].2, "covered(flow_run)");
    assert!(rows[0].1.contains("'F'") && rows[0].1.contains("'R'"));
    assert!(rows[1].1.contains("'A'"));
    assert!(rows[2].1.contains("'L'"));
    assert!(rows[3].1.contains("'X'"));
    assert!(rows[4].1.contains("'O'"));
    // RB-090-60: `F F`, the Flows panel; `P W` is the PWA panel's.
    assert!(rows[5].1.contains("'F'") && !rows[5].1.contains("'W'"));
    assert_eq!(rows[5].2, "covered(flow_list)");
    for (id, _, mcp) in &rows[..3] {
        assert_eq!(mcp, "covered(flow_run)", "{id}");
    }
    assert!(rows[3].2.starts_with("omitted(cancelling"));
    // RB-090-91: `F D` deletes a flow; an agent never deletes project files.
    assert!(rows[6].1.contains("'F'") && rows[6].1.contains("'D'"));
    assert_eq!(rows[6].2, format!("omitted({DELETE_OMITTED})"));
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

#[test]
fn no_flow_runs_while_a_recording_takes_steps() {
    // A flow's navigations in the active view would become the recording's steps (RB-090-71);
    // the name is the recording's that takes steps now, none while paused or idle.
    assert_eq!(
        super::gate::not_while_recording(Some("login")),
        Err(FlowError::Recording("login".to_owned()))
    );
    assert_eq!(
        FlowError::Recording("login".to_owned()).code(),
        "flows.recording"
    );
    assert!(
        FlowError::Recording("login".to_owned())
            .to_string()
            .contains("recorder.stop")
    );
    assert_eq!(super::gate::not_while_recording(None), Ok(()));
}
