use std::collections::BTreeSet;

use super::*;

fn words(key: &str, args: &[(&str, &str)]) -> String {
    let args: String = args
        .iter()
        .map(|(name, value)| format!(" {name}={value}"))
        .collect();
    format!("{key}{args}")
}

fn view(id: u32, label: &str) -> ViewFacts {
    ViewFacts {
        id,
        label: label.to_owned(),
        breakpoint: Some(label.to_owned()),
        width: 390,
        height: 844,
        dpr: 3.0,
        engine: "chromium".to_owned(),
        url: "https://example.test/cart?id=***".to_owned(),
        title: "Cart".to_owned(),
        emulation: vec![("mobile".to_owned(), "true".to_owned())],
    }
}

fn report() -> Report {
    Report {
        created: "2026-11-20_14-03-07".to_owned(),
        version: Some("0.9.3".to_owned()),
        environment: Some((
            "staging".to_owned(),
            "https://staging.example.test".to_owned(),
        )),
        note: Some("The cart is empty after login".to_owned()),
        views: vec![view(1, "mobile"), view(2, "desktop")],
        show_pages: true,
        show_emulation: true,
        show_engines: true,
        images: vec![
            Image {
                view: Some(1),
                path: Some("screens/2026-11-20/mobile view.png".to_owned()),
                error: None,
            },
            Image {
                view: Some(2),
                path: None,
                error: Some("firefox: screenshot is not supported".to_owned()),
            },
        ],
        overview: Some(Image {
            view: None,
            path: Some("screens/overview.png".to_owned()),
            error: None,
        }),
        console: Some(vec![ConsoleProblem {
            level: Level::Error,
            text: "Boom".to_owned(),
            source: Some("https://example.test/app.js:3".to_owned()),
            count: 2,
            views: BTreeSet::from([1, 2]),
        }]),
        requests: Some(vec![FailedRequest {
            method: "GET".to_owned(),
            url: "https://example.test/api?token=***".to_owned(),
            status: Some(401),
            failure: None,
            count: 1,
            views: BTreeSet::from([1]),
        }]),
    }
}

#[test]
fn the_report_names_every_part_with_relative_images() {
    let text = render(&report(), &words);
    assert!(text.starts_with("# title\n"), "{text}");
    for expected in [
        "- created: `2026-11-20_14-03-07`",
        "- version: `0.9.3`",
        "- environment: `staging` `https://staging.example.test`",
        "### view id=1 `mobile`",
        "- page: `https://example.test/cart?id=***`",
        "- breakpoint: `mobile` `390 x 844 @3x`",
        "- emulation: `mobile: true`",
        "- engine: `chromium`",
        "![mobile](screens/2026-11-20/mobile%20view.png)",
        "no_image reason=firefox: screenshot is not supported",
        "![overview](screens/overview.png)",
        "## console count=1",
        "[level_error] x2 (views 1, 2) Boom\n    https://example.test/app.js:3",
        "## requests count=1",
        "401 GET https://example.test/api?token=*** x1 (views 1)",
        "## note\n\n```text\nThe cart is empty after login\n```",
    ] {
        assert!(text.contains(expected), "{expected:?} missing in\n{text}");
    }
}

#[test]
fn a_part_left_out_is_not_shown() {
    let report = Report {
        version: None,
        environment: None,
        note: None,
        show_pages: false,
        show_emulation: false,
        show_engines: false,
        images: Vec::new(),
        overview: None,
        console: None,
        requests: None,
        ..report()
    };
    let text = render(&report, &words);
    for absent in [
        "version",
        "environment",
        "page:",
        "breakpoint",
        "engine",
        "![",
        "## console",
        "## requests",
        "## note",
    ] {
        assert!(!text.contains(absent), "{absent} in\n{text}");
    }
    let empty = Report {
        console: Some(Vec::new()),
        requests: Some(Vec::new()),
        ..Report::default()
    };
    let text = render(&empty, &words);
    assert!(text.contains("console_none"));
    assert!(text.contains("requests_none"));
}

/// A page's text cannot become Markdown or HTML in the ticket: no image that calls home, no
/// link, no closed fence.
#[test]
fn hostile_page_text_stays_code() {
    let hostile =
        "```\n![x](https://tracker.example/p.png) <img src=x onerror=alert(1)> [a](javascript:x)";
    let mut report = report();
    report.views[0].title = hostile.to_owned();
    report.views[0].label = "m`o]bile)".to_owned();
    if let Some(console) = report.console.as_mut() {
        console[0].text = hostile.to_owned();
    }
    report.note = Some(hostile.to_owned());
    let text = render(&report, &words);
    // Outside code nothing of it is left: drop every code span and block and look again.
    let mut outside = String::new();
    let mut in_block: Option<String> = None;
    for line in text.lines() {
        let fence: String = line.chars().take_while(|c| *c == '`').collect();
        match &in_block {
            Some(open) if line.trim_end() == open.as_str() => in_block = None,
            Some(_) => {}
            None if fence.len() >= 3 => in_block = Some(fence),
            None => {
                outside.push_str(&strip_spans(line));
                outside.push('\n');
            }
        }
    }
    assert!(in_block.is_none(), "every block is closed:\n{text}");
    for bad in ["tracker.example", "<img", "javascript:", "](https"] {
        assert!(!outside.contains(bad), "{bad} outside code in\n{outside}");
    }
    assert!(
        text.contains("![mobile](screens/"),
        "the alt text loses its brackets"
    );
}

/// The line without its inline code spans (a run of n backticks up to the next run of n).
fn strip_spans(line: &str) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(start) = rest.find('`') {
        out.push_str(&rest[..start]);
        let run = rest[start..].chars().take_while(|c| *c == '`').count();
        let after = &rest[start + run..];
        let fence = "`".repeat(run);
        match after.find(&fence) {
            Some(end) => rest = &after[end + run..],
            None => {
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

#[test]
fn code_spans_and_blocks_outlast_their_content() {
    assert_eq!(code("a`b"), "``a`b``");
    assert_eq!(code("`x`"), "`` `x` ``");
    assert_eq!(code("line\nbreak"), "`line break`");
    assert_eq!(code("  "), "`-`");
    assert_eq!(block("x ```` y"), "`````text\nx ```` y\n`````\n");
    assert_eq!(block("plain"), "```text\nplain\n```\n");
}
