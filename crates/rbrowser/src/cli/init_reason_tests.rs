use rb_plugin_types::codes;

use super::*;
use crate::analyzer::plugins::PluginError;

fn plugin(code: &str) -> AnalyzerError {
    AnalyzerError::Plugin(PluginError {
        code: code.to_owned(),
        params: vec![("service".to_owned(), "breakpoint-analyzer".to_owned())],
    })
}

/// RB-091-08: each failure has its own reason, never one text for all.
#[test]
fn each_failure_of_the_analysis_names_its_own_reason() {
    let cases = [
        (
            AnalyzerError::Collect("no stylesheet was found".to_owned()),
            Reason::NoCss,
        ),
        (plugin(codes::NOT_AVAILABLE), Reason::PluginMissing),
        (plugin(codes::KEY_UNTRUSTED), Reason::PluginMissing),
        (plugin(codes::UNTRUSTED_WORKSPACE), Reason::PluginMissing),
        (plugin(codes::BAD_OUTPUT), Reason::BadOutput),
        (plugin(codes::TIMED_OUT), Reason::PluginFailed),
        (
            AnalyzerError::Config("breakpoints/0: width".to_owned()),
            Reason::Invalid,
        ),
        (AnalyzerError::NoUrl, Reason::NoPage),
    ];
    for (error, reason) in cases {
        let found = NoProposal::of(&error);
        assert_eq!(found.reason, reason, "{error}");
        // The detail is the English text the command line printed before.
        assert_eq!(found.detail, error.to_string());
    }
    assert_eq!(
        NoProposal::of(&plugin(codes::NOT_AVAILABLE))
            .params
            .get("code")
            .map(String::as_str),
        Some(codes::NOT_AVAILABLE)
    );
}

#[test]
fn a_page_that_does_not_answer_in_time_is_a_timeout() {
    let refused = NoProposal::unreachable("http://localhost:5999/: connection refused");
    assert_eq!(refused.reason, Reason::Unreachable);
    assert!(refused.detail.contains("connection refused"), "{refused:?}");
    for text in [
        "http://localhost:5173/: timeout: global",
        "http://localhost:5173/: operation timed out",
        "http://localhost:5173/: Timeout",
    ] {
        assert_eq!(
            NoProposal::unreachable(text).reason,
            Reason::Timeout,
            "{text}"
        );
    }
}

#[test]
fn the_command_line_says_why_it_falls_back() {
    let line = NoProposal::of(&plugin(codes::NOT_AVAILABLE)).sentence();
    assert!(
        line.starts_with("rbrowser init: the analyser had no proposal: "),
        "{line}"
    );
    assert!(line.contains("plugin is not loaded"), "{line}");
    assert!(line.contains(codes::NOT_AVAILABLE), "{line}");
    assert!(
        line.ends_with("writing a mobile and a desktop view"),
        "{line}"
    );
}
