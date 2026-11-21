use serde_json::json;

use super::*;
use crate::{CoreToShell, ShellToCore, parse_request};

#[test]
fn a_preview_request_reads_in_camel_case_under_setup() {
    let request = parse_request(
        &json!({
            "type": "setup",
            "request": {
                "op": "preview",
                "requestId": 7,
                "root": "\\\\wsl.localhost\\Ubuntu\\home\\me\\site",
                "url": "http://localhost:5173",
                "template": "devices",
                "devices": ["iphone-15", "ipad-air"]
            }
        })
        .to_string(),
    )
    .expect("parses");
    assert_eq!(
        request,
        ShellToCore::Setup {
            request: SetupRequest::Preview {
                request_id: 7,
                root: "\\\\wsl.localhost\\Ubuntu\\home\\me\\site".to_owned(),
                url: Some("http://localhost:5173".to_owned()),
                template: SetupTemplate::Devices,
                devices: vec!["iphone-15".to_owned(), "ipad-air".to_owned()],
            }
        }
    );
}

#[test]
fn the_short_requests_need_no_fields() {
    for (op, expected) in [("info", SetupRequest::Info), ("close", SetupRequest::Close)] {
        let parsed =
            parse_request(&json!({ "type": "setup", "request": { "op": op } }).to_string())
                .expect("parses");
        assert_eq!(parsed, ShellToCore::Setup { request: expected }, "{op}");
    }
    let write = parse_request(
        &json!({ "type": "setup", "request": { "op": "write", "requestId": 8, "proposalId": 4 } })
            .to_string(),
    )
    .expect("parses");
    assert_eq!(
        write,
        ShellToCore::Setup {
            request: SetupRequest::Write {
                request_id: 8,
                proposal_id: 4
            }
        }
    );
}

#[test]
fn basics_carry_only_what_the_person_chose() {
    let parsed = parse_request(
        &json!({ "type": "setup", "request": { "op": "basics", "requestId": 9, "layout": "canvas" } }).to_string(),
    )
    .expect("parses");
    assert_eq!(
        parsed,
        ShellToCore::Setup {
            request: SetupRequest::Basics {
                request_id: 9,
                layout: Some(SetupLayout::Canvas),
                sync: None,
            }
        }
    );
}

#[test]
fn an_unknown_template_is_refused() {
    let text = json!({
        "type": "setup",
        "request": { "op": "preview", "root": "C:\\site", "template": "everything" }
    })
    .to_string();
    assert!(parse_request(&text).is_err());
}

#[test]
fn the_answers_are_tagged_by_op() {
    let info = CoreToShell::Setup {
        answer: SetupAnswer::Info {
            enabled: true,
            project: Some(SetupProject {
                root: "C:\\site".to_owned(),
                has_config: false,
            }),
            written: None,
        },
    };
    assert_eq!(
        serde_json::to_value(&info).expect("serializes"),
        json!({
            "type": "setup",
            "answer": {
                "op": "info",
                "enabled": true,
                "project": { "root": "C:\\site", "hasConfig": false }
            }
        })
    );
    let failed = CoreToShell::Setup {
        answer: SetupAnswer::Failed {
            request_id: 7,
            step: SetupStep::Preview,
            message_key: "firstRun.project.error.exists".to_owned(),
            args: [("path".to_owned(), "C:\\site".to_owned())].into(),
        },
    };
    let value = serde_json::to_value(&failed).expect("serializes");
    assert_eq!(value["answer"]["op"], "failed");
    assert_eq!(value["answer"]["step"], "preview");
    // The answer names the request it answers, so the page drops a stale one.
    assert_eq!(value["answer"]["requestId"], 7);
    assert_eq!(
        value["answer"]["messageKey"],
        "firstRun.project.error.exists"
    );
    let back: CoreToShell = serde_json::from_value(value).expect("reads back");
    assert_eq!(back, failed);
}

#[test]
fn a_proposal_carries_the_config_as_it_will_be_written() {
    let proposal = CoreToShell::Setup {
        answer: SetupAnswer::Proposal {
            request_id: 7,
            proposal_id: 2,
            root: "C:\\site".to_owned(),
            config: json!({ "version": 1, "breakpoints": [] }),
            source: SetupSource::Default,
            sheets: None,
            reason: None,
        },
    };
    let value = serde_json::to_value(&proposal).expect("serializes");
    assert_eq!(value["answer"]["proposalId"], 2);
    assert_eq!(value["answer"]["requestId"], 7);
    assert_eq!(value["answer"]["source"], "default");
    assert_eq!(value["answer"]["config"]["version"], 1);
    // Neither field without the analyser.
    assert!(value["answer"].get("sheets").is_none());
    assert!(value["answer"].get("reason").is_none());
}

/// RB-091-08: the analyser's success names the stylesheets, its fallback the reason - a code the
/// page turns into `firstrun.project.reason.<code>`, with its values.
#[test]
fn a_proposal_says_what_the_analyser_did() {
    let found = SetupAnswer::Proposal {
        request_id: 3,
        proposal_id: 1,
        root: "C:\\site".to_owned(),
        config: json!({ "version": 1 }),
        source: SetupSource::Analyzer,
        sheets: Some(4),
        reason: None,
    };
    let value = serde_json::to_value(&found).expect("serializes");
    assert_eq!(value["sheets"], 4);
    let fell_back = SetupAnswer::Proposal {
        request_id: 4,
        proposal_id: 2,
        root: "C:\\site".to_owned(),
        config: json!({ "version": 1 }),
        source: SetupSource::Default,
        sheets: None,
        reason: Some(SetupReason {
            code: SetupReasonCode::PluginMissing,
            params: [("code".to_owned(), "plugins.error.not_available".to_owned())].into(),
        }),
    };
    let value = serde_json::to_value(&fell_back).expect("serializes");
    assert_eq!(
        value["reason"],
        json!({ "code": "plugin_missing", "params": { "code": "plugins.error.not_available" } })
    );
    let back: SetupAnswer = serde_json::from_value(value).expect("reads back");
    assert_eq!(back, fell_back);
    for (code, text) in [
        (SetupReasonCode::NoPage, "no_page"),
        (SetupReasonCode::Unreachable, "unreachable"),
        (SetupReasonCode::Timeout, "timeout"),
        (SetupReasonCode::NoCss, "no_css"),
        (SetupReasonCode::PluginMissing, "plugin_missing"),
        (SetupReasonCode::PluginFailed, "plugin_failed"),
        (SetupReasonCode::BadOutput, "bad_output"),
        (SetupReasonCode::NothingFound, "nothing_found"),
        (SetupReasonCode::Invalid, "invalid"),
    ] {
        assert_eq!(serde_json::to_value(code).expect("serializes"), text);
    }
}

#[test]
fn a_request_that_does_its_work_later_needs_its_number() {
    let text =
        json!({ "type": "setup", "request": { "op": "write", "proposalId": 4 } }).to_string();
    assert!(parse_request(&text).is_err());
}
