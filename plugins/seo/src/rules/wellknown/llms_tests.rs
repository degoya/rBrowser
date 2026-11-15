use super::*;

#[test]
fn a_file_in_the_format_has_no_problem_and_gives_its_links() {
    let parsed = parse(
        "# Shop\n\n> A shop for lamps.\n\nSome details.\n\n## Docs\n\n\
- [Guide](https://shop.test/guide.md): how to order\n- [API](/api.md)\n\n\
```\n# not a heading\n- not a link\n```\n\n## Optional\n\n* [Blog](https://shop.test/blog)\n",
    );
    assert_eq!(parsed.problems, []);
    assert_eq!(
        parsed.links,
        [
            "https://shop.test/guide.md",
            "/api.md",
            "https://shop.test/blog"
        ]
    );
}

#[test]
fn a_file_without_h1_is_named_first() {
    let parsed = parse("> A summary.\n\n## Docs\n\n- [Guide](/guide.md)\n");
    assert_eq!(parsed.problems, [Problem::NoH1]);
}

#[test]
fn a_late_or_second_h1_no_summary_bad_items_and_empty_sections_are_named() {
    let parsed = parse(
        "Intro\n# Shop\nNo quote.\n# Again\n## Docs\n- Guide at /guide.md\n- [x](y) trailing\n## Empty\nText only.\n",
    );
    assert_eq!(
        parsed.problems,
        [
            Problem::H1NotFirst(2),
            Problem::NoSummary,
            Problem::H1Multiple(4),
            Problem::BadItem(6),
            Problem::BadItem(7),
            Problem::EmptySection(5, "Docs".to_owned()),
            Problem::EmptySection(8, "Empty".to_owned()),
        ]
    );
}

#[test]
fn an_h1_alone_lacks_its_summary() {
    assert_eq!(parse("# Shop\n").problems, [Problem::NoSummary]);
}
