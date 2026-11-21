use rb_plugin_api::FailureKind;
use serde_json::json;

use super::{Block, Draft, MAX_BLOCKS, MAX_CSS_BYTES, block_text, braces_balance};
use crate::media::Breakpoints;
use crate::testing::view;

fn grid() -> Breakpoints {
    Breakpoints::from_views(&[
        view(1, "mobile", 375),
        view(2, "md", 768),
        view(3, "lg", 1280),
    ])
}

fn block(css: &str, breakpoints: &[&str]) -> Block {
    Block {
        css: css.to_owned(),
        breakpoints: breakpoints.iter().map(|id| (*id).to_owned()).collect(),
    }
}

fn code(draft: &Draft) -> Option<String> {
    draft.check().err().and_then(|failure| failure.code)
}

#[test]
fn a_new_draft_is_on_and_empty() {
    let draft = Draft::default();
    assert!(draft.enabled);
    assert!(draft.blocks.is_empty());
    assert_eq!(draft.sheet(&grid()), "");
}

#[test]
fn a_block_reads_from_json_without_doubles() {
    let read = Block::from_json(&json!({ "css": "a{}", "breakpoints": ["md", "lg", "md"] }))
        .expect("reads");
    assert_eq!(read, block("a{}", &["md", "lg"]));
    assert_eq!(
        read.to_json(),
        json!({ "css": "a{}", "breakpoints": ["md", "lg"] })
    );
    let all = Block::from_json(&json!({ "css": "a{}" })).expect("reads");
    assert!(all.breakpoints.is_empty());
}

#[test]
fn a_block_that_does_not_read_is_refused() {
    for bad in [
        json!({}),
        json!({ "css": 3 }),
        json!({ "css": "a{}", "breakpoints": "md" }),
        json!({ "css": "a{}", "breakpoints": [""] }),
        json!({ "css": "a{}", "breakpoints": [7] }),
        json!({ "css": "a{}", "breakpoints": ["m\nd"] }),
        json!({ "css": "a{}", "breakpoints": ["x".repeat(65)] }),
    ] {
        let failure = Block::from_json(&bad).expect_err("refused");
        assert_eq!(failure.kind, FailureKind::BadData, "{bad}");
        assert_eq!(
            failure.code.as_deref(),
            Some("plugin.live-css.errors.bad_args")
        );
    }
}

#[test]
fn the_sheet_joins_the_blocks_each_in_its_scope() {
    let draft = Draft {
        enabled: true,
        blocks: vec![
            block("body { margin: 0 }", &[]),
            block("body { outline: 3px solid red }", &["md"]),
            block("p { color: red }", &["xl"]),
            block("  \n", &["lg"]),
        ],
    };
    assert_eq!(
        draft.sheet(&grid()),
        "body { margin: 0 }\n@media (min-width: 768px) and (max-width: 1279px) {\n  body { outline: 3px solid red }\n}"
    );
}

#[test]
fn off_the_views_get_nothing_and_the_stylesheet_stays() {
    let draft = Draft {
        enabled: false,
        blocks: vec![block("body { outline: 3px solid red }", &["md"])],
    };
    assert_eq!(draft.sheet(&grid()), "");
    assert_eq!(
        draft.stylesheet(&grid()),
        "@media (min-width: 768px) and (max-width: 1279px) {\n  body { outline: 3px solid red }\n}"
    );
}

#[test]
fn copy_as_media_query_for_md_takes_the_bounds_of_the_breakpoint_config() {
    assert_eq!(
        block_text(&block("body { outline: 3px solid red }", &["md"]), &grid()).as_deref(),
        Some(
            "@media (min-width: 768px) and (max-width: 1279px) {\n  body { outline: 3px solid red }\n}"
        )
    );
    assert_eq!(
        block_text(&block("a{}", &[]), &grid()).as_deref(),
        Some("a{}")
    );
    assert_eq!(block_text(&block("a{}", &["xl"]), &grid()), None);
}

#[test]
fn braces_balance_outside_strings_and_comments() {
    for good in [
        "",
        "a { color: red }",
        "@media print { a { b: c } }",
        "a::after { content: \"}\" }",
        "a::after { content: '{' }",
        "a { b: c } /* } */",
        "a::after { content: \"\\\"}\" }",
        "a { b: \\} }",
    ] {
        assert!(braces_balance(good), "{good}");
    }
    for bad in [
        "a {",
        "}",
        "} body { color: red } @media x {",
        "a { b: c } }",
        "a { b: c } /* unclosed",
    ] {
        assert!(!braces_balance(bad), "{bad}");
    }
}

#[test]
fn a_draft_past_its_limits_is_refused() {
    let many = Draft {
        enabled: true,
        blocks: vec![block("a{}", &[]); MAX_BLOCKS + 1],
    };
    assert_eq!(
        code(&many).as_deref(),
        Some("plugin.live-css.errors.too_many_blocks")
    );
    let long = Draft {
        enabled: true,
        blocks: vec![
            block(&"a".repeat(MAX_CSS_BYTES / 2), &[]),
            block(&"b".repeat(MAX_CSS_BYTES / 2 + 1), &[]),
        ],
    };
    assert_eq!(
        code(&long).as_deref(),
        Some("plugin.live-css.errors.too_long")
    );
}

#[test]
fn unbalanced_braces_and_control_characters_name_their_block() {
    let open = Draft {
        enabled: true,
        blocks: vec![
            block("a{}", &[]),
            block("} body { color: red } @media x {", &["md"]),
        ],
    };
    let failure = open.check().expect_err("refused");
    assert_eq!(
        failure.code.as_deref(),
        Some("plugin.live-css.errors.braces")
    );
    assert_eq!(failure.param("block"), Some("2"));
    let control = Draft {
        enabled: true,
        blocks: vec![block("a{}\u{0}", &[])],
    };
    assert_eq!(
        code(&control).as_deref(),
        Some("plugin.live-css.errors.control")
    );
    let fine = Draft {
        enabled: true,
        blocks: vec![block("a {\r\n\tb: c;\n}", &[])],
    };
    assert!(fine.check().is_ok());
}
