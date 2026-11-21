use serde_json::json;

use super::*;
use crate::{CoreToShell, ShellToCore, parse_request};

fn round_trip_shell(value: serde_json::Value) -> ShellToCore {
    let message = parse_request(&value.to_string()).expect("parses");
    assert_eq!(serde_json::to_value(&message).expect("serializes"), value);
    message
}

#[test]
fn the_requests_are_tagged_by_op_inside_bug_report() {
    assert_eq!(
        round_trip_shell(json!({ "type": "bugReport", "request": { "op": "getPreview" } })),
        ShellToCore::BugReport {
            request: BugReportRequest::GetPreview
        }
    );
    assert_eq!(
        round_trip_shell(json!({
            "type": "bugReport",
            "request": { "op": "decide", "previewId": 4, "save": true }
        })),
        ShellToCore::BugReport {
            request: BugReportRequest::Decide {
                preview_id: 4,
                save: true
            }
        }
    );
    assert_eq!(
        round_trip_shell(json!({
            "type": "bugReport",
            "request": { "op": "getMarkdown", "report": "2026-11-20_14-03-07" }
        })),
        ShellToCore::BugReport {
            request: BugReportRequest::GetMarkdown {
                report: "2026-11-20_14-03-07".to_owned()
            }
        }
    );
}

#[test]
fn an_unknown_op_or_a_missing_field_is_refused() {
    for bad in [
        json!({ "type": "bugReport", "request": { "op": "save" } }),
        json!({ "type": "bugReport", "request": { "op": "decide", "previewId": 4 } }),
        json!({ "type": "bugReport", "request": { "op": "getMarkdown" } }),
        json!({ "type": "bugReport" }),
    ] {
        assert!(parse_request(&bad.to_string()).is_err(), "{bad}");
    }
}

#[test]
fn the_answers_are_tagged_by_kind_inside_bug_report() {
    let preview = CoreToShell::BugReport {
        answer: BugReportAnswer::Preview(ReportPreview {
            preview_id: 4,
            folder: "C:/p/.rbrowser/out/reports/2026-11-20_14-03-07".to_owned(),
            markdown: "# Bug report\n".to_owned(),
            images: 3,
            annotate: true,
        }),
    };
    let value = json!({
        "type": "bugReport",
        "answer": {
            "kind": "preview",
            "previewId": 4,
            "folder": "C:/p/.rbrowser/out/reports/2026-11-20_14-03-07",
            "markdown": "# Bug report\n",
            "images": 3,
            "annotate": true
        }
    });
    assert_eq!(serde_json::to_value(&preview).expect("serializes"), value);
    assert_eq!(
        serde_json::from_value::<CoreToShell>(value).expect("parses"),
        preview
    );
    let markdown = CoreToShell::BugReport {
        answer: BugReportAnswer::Markdown {
            report: "2026-11-20_14-03-07".to_owned(),
            text: "# Bug report\n".to_owned(),
        },
    };
    assert_eq!(
        serde_json::to_value(&markdown).expect("serializes"),
        json!({
            "type": "bugReport",
            "answer": {
                "kind": "markdown",
                "report": "2026-11-20_14-03-07",
                "text": "# Bug report\n"
            }
        })
    );
}
