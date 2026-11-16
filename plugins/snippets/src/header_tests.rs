use super::{Header, HeaderError, MAX_TIMEOUT_MS, MIN_TIMEOUT_MS, World, parse};
use crate::run::RunTarget;

#[test]
fn the_header_is_the_leading_comment_block() {
    let source = "\n// @name Feature flag\n// @description Sets the beta flag\n//   and reloads\n// @world main\n// @target active\n// @timeoutMs 2500\nlocalStorage.beta = '1';\n// @name not header\n";
    let header = parse(source).expect("reads");
    assert_eq!(
        header,
        Header {
            name: Some("Feature flag".to_owned()),
            description: Some("Sets the beta flag".to_owned()),
            world: Some(World::Main),
            target: Some(RunTarget::Active),
            timeout_ms: Some(2500),
        }
    );
}

#[test]
fn a_snippet_without_a_header_has_none() {
    assert_eq!(
        parse("return document.title").expect("reads"),
        Header::default()
    );
    assert_eq!(parse("").expect("reads"), Header::default());
}

#[test]
fn unknown_tags_and_plain_comments_are_ignored() {
    let header =
        parse("// A grid helper\n// @author someone\n// @world isolated\nreturn 1").expect("reads");
    assert_eq!(header.world, Some(World::Isolated));
    assert_eq!(header.name, None);
}

#[test]
fn a_bad_value_names_its_tag() {
    for (source, tag) in [
        ("// @world page\n", "world"),
        ("// @target some\n", "target"),
        ("// @timeoutMs soon\n", "timeoutMs"),
        ("// @timeoutMs 50\n", "timeoutMs"),
        ("// @timeoutMs 999999\n", "timeoutMs"),
        ("// @name\n", "name"),
        ("// @name a\u{7}b\n", "name"),
    ] {
        assert_eq!(
            parse(source),
            Err(HeaderError {
                tag: tag.to_owned()
            }),
            "{source:?}"
        );
    }
}

#[test]
fn the_timeout_bounds_are_accepted() {
    for ms in [MIN_TIMEOUT_MS, MAX_TIMEOUT_MS] {
        let header = parse(&format!("// @timeoutMs {ms}\n")).expect("reads");
        assert_eq!(header.timeout_ms, Some(ms));
    }
}

#[test]
fn long_texts_are_cut() {
    let header = parse(&format!(
        "// @name {}\n// @description {}\n",
        "n".repeat(200),
        "d".repeat(900)
    ))
    .expect("reads");
    assert_eq!(
        header.name.map(|name| name.chars().count()),
        Some(super::MAX_NAME_CHARS)
    );
    assert_eq!(
        header.description.map(|text| text.chars().count()),
        Some(super::MAX_DESCRIPTION_CHARS)
    );
}

#[test]
fn a_block_comment_or_code_ends_the_header() {
    let header = parse("/* @name no */\n// @name later\n").expect("reads");
    assert_eq!(header.name, None);
}

#[test]
fn windows_line_ends_read_alike() {
    let header = parse("// @name Grid\r\n// @world main\r\nreturn 1\r\n").expect("reads");
    assert_eq!(header.name.as_deref(), Some("Grid"));
    assert_eq!(header.world, Some(World::Main));
}

#[test]
fn world_names_read_both_ways() {
    assert_eq!(World::parse("main"), Some(World::Main));
    assert_eq!(World::parse("isolated"), Some(World::Isolated));
    assert_eq!(World::parse("Main"), None);
    assert_eq!(World::Main.as_str(), "main");
    assert_eq!(World::Isolated.as_str(), "isolated");
}
