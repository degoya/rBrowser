use std::path::PathBuf;

use serde_json::json;

use super::*;

#[test]
fn two_commands_in_the_capture_group_on_free_leader_keys() {
    let defs = definitions().expect("valid ids");
    let ids: Vec<&str> = defs.iter().map(|def| def.id.as_str()).collect();
    assert_eq!(ids, ["report.create", "report.open_folder"]);
    assert_eq!(defs[0].title_key, "commands.report.create");
    assert_eq!(defs[0].mcp, McpCoverage::Covered("bug_report".to_owned()));
    assert!(matches!(defs[1].mcp, McpCoverage::Omitted(_)));
    let keys: Vec<Vec<Key>> = defs
        .iter()
        .map(|def| match def.default_bindings.as_slice() {
            [Binding::Leader(chord)] => chord.keys.clone(),
            other => panic!("one leader chord, not {other:?}"),
        })
        .collect();
    assert_eq!(keys[0], [Key::Char('C'), Key::Char('U')]);
    assert_eq!(keys[1], [Key::Char('C'), Key::Char('Y')]);
    assert!(defs.iter().all(|def| def.group == Group::Capture));
}

#[test]
fn the_arguments_name_views_and_a_note() {
    assert_eq!(parse_args(&Value::Null), Ok(CreateArgs::default()));
    let one = parse_args(&json!({ "target": 2, "note": "cart empty" })).expect("valid");
    assert_eq!(one.views(), Ok(Some(vec![ViewId(2)])));
    assert_eq!(one.note.as_deref(), Some("cart empty"));
    let many = parse_args(&json!({ "target": [1, 3] })).expect("valid");
    assert_eq!(many.views(), Ok(Some(vec![ViewId(1), ViewId(3)])));
    let all = parse_args(&json!({ "target": "ALL" })).expect("valid");
    assert_eq!(all.views(), Ok(None));
}

#[test]
fn wrong_arguments_are_refused() {
    for args in [
        json!({ "target": "mobile" }),
        json!({ "target": [] }),
        json!({ "target": -1 }),
        json!({ "nope": 1 }),
        json!({ "note": 3 }),
        json!({ "note": "x".repeat(MAX_NOTE_BYTES + 1) }),
    ] {
        assert!(parse_args(&args).is_err(), "{args}");
    }
}

#[test]
fn the_answer_is_paths_only() {
    let folder = PathBuf::from("/p/.rbrowser/out/reports/2026-11-20_14-03-07");
    let written = Written {
        markdown: folder.join("report.md"),
        zip: folder.join("report.zip"),
        folder,
        text: "# Bug report: SECRET-TEXT\n".to_owned(),
    };
    let answer = answer(&written);
    let keys: Vec<&str> = answer
        .as_object()
        .expect("an object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, ["path", "markdown", "zip"]);
    assert!(
        !answer.to_string().contains("SECRET-TEXT"),
        "never the content: {answer}"
    );
    assert!(
        answer["path"]
            .as_str()
            .is_some_and(|path| path.contains(".rbrowser/out/reports/")),
        "{answer}"
    );
}

#[test]
fn errors_keep_their_kind() {
    let invalid: CommandError = ReportError::Invalid("no".to_owned()).into();
    assert!(matches!(invalid, CommandError::InvalidArgs(_)));
    let failed: CommandError = ReportError::Failed("no".to_owned()).into();
    assert!(matches!(failed, CommandError::Failed(_)));
    let cancelled: CommandError = ReportError::Cancelled.into();
    assert!(matches!(cancelled, CommandError::Failed(_)));
}

/// RB-093-04: the screenshot editor reads `.rbrowser/out/` only, so `report.annotate` opens it
/// only on a report written there - a report in the person's own folder is not the newest
/// screenshot it would show.
#[test]
fn the_editor_opens_only_on_a_report_below_the_projects_out_folder() {
    let root = Path::new("/p");
    let inside = root.join(".rbrowser/out/reports/2026-11-20_14-03-07");
    assert!(editor_sees(&inside, Some(root)));
    assert!(!editor_sees(
        &PathBuf::from("/tickets/2026-11-20_14-03-07"),
        Some(root)
    ));
    assert!(!editor_sees(&inside, None), "no project, no editor");
    assert!(!editor_sees(
        &PathBuf::from("/p/.rbrowser/outside/r"),
        Some(root)
    ));
}

#[test]
fn a_saved_report_offers_its_folder_and_a_copy_of_its_markdown() {
    let folder = PathBuf::from("/p/.rbrowser/out/reports/2026-11-20_14-03-07");
    let (action, copy) = buttons(&folder);
    assert_eq!(action.command, OPEN_FOLDER);
    assert_eq!(action.label_key, OPEN_LABEL_KEY);
    assert_eq!(copy.label_key, COPY_LABEL_KEY);
    assert_eq!(
        copy.report, "2026-11-20_14-03-07",
        "the name, never the text"
    );
}
