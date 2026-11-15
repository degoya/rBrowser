use super::{ProjectFile, Source, literal_px, scan, strip_comments, width_of};

fn file(path: &str, text: &str) -> ProjectFile {
    ProjectFile {
        path: path.to_owned(),
        text: text.to_owned(),
    }
}

#[test]
fn static_files_are_read_by_their_kind() {
    let files = [
        file(
            "tailwind.config.ts",
            "export default { theme: { screens: { md: '700px' } } }",
        ),
        file(
            "web/tailwind.config.mjs",
            "export default { theme: { screens: { lg: '900px' } } }",
        ),
        file(
            "tailwind.config.json",
            "{ \"theme\": { \"screens\": { \"x\": \"1px\" } } }",
        ),
        file(
            "my.tailwind.config.js",
            "module.exports = { theme: { screens: { y: '2px' } } }",
        ),
        file("src/vars.scss", "$bp-wide: 1500px;"),
        file("src/vars.less", "@screen-huge: 2000px;"),
        file("src/vars.sass", "$bp-gone: 3px"),
        file("src/app.css", "$bp-none: 4px;"),
    ];
    let found = scan(&files, 16.0);
    let declared: Vec<(&str, u32, &str, bool)> = found
        .declared
        .iter()
        .map(|d| (d.label.as_str(), d.width, d.path.as_str(), d.heuristic))
        .collect();
    assert_eq!(
        declared,
        vec![
            ("md", 700, "tailwind.config.ts", false),
            ("lg", 900, "web/tailwind.config.mjs", false),
            ("wide", 1500, "src/vars.scss", true),
            ("huge", 2000, "src/vars.less", true),
        ]
    );
    assert_eq!(
        found.sources,
        vec![
            Source {
                url: "tailwind.config.ts".to_owned(),
                count: 1
            },
            Source {
                url: "web/tailwind.config.mjs".to_owned(),
                count: 1
            },
            Source {
                url: "src/vars.scss".to_owned(),
                count: 1
            },
            Source {
                url: "src/vars.less".to_owned(),
                count: 1
            },
        ]
    );
}

#[test]
fn static_no_files_declare_nothing() {
    let found = scan(&[], 16.0);
    assert!(found.declared.is_empty() && found.hints.is_empty() && found.sources.is_empty());
}

#[test]
fn static_widths_are_sensible_boundaries() {
    assert_eq!(width_of(767.2), Some(768));
    assert_eq!(width_of(0.0), None);
    assert_eq!(width_of(-5.0), None);
    assert_eq!(width_of(20_000.0), None);
    assert_eq!(width_of(f64::NAN), None);
    assert_eq!(literal_px("'48rem'", 16.0), Some(768.0));
    assert_eq!(literal_px("\"30em\"", 20.0), Some(600.0));
    assert_eq!(literal_px("768", 16.0), None);
    assert_eq!(literal_px("'", 16.0), None);
}

#[test]
fn static_comments_go_and_strings_stay() {
    assert_eq!(
        strip_comments("a /* b */ c // d\ne \"/* f */\" 'g // h'", true),
        "a   c \ne \"/* f */\" 'g // h'"
    );
    assert_eq!(strip_comments("a // b", false), "a // b");
    assert_eq!(strip_comments("a /* never closed", false), "a  ");
}
