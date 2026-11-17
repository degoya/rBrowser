use super::{imports_tailwind, scan_project};
use crate::static_source::ProjectFile;

fn file(path: &str, text: &str) -> ProjectFile {
    ProjectFile {
        path: path.to_owned(),
        text: text.to_owned(),
    }
}

fn widths(files: &[ProjectFile]) -> Vec<(String, u32, String)> {
    let refs: Vec<&ProjectFile> = files.iter().collect();
    scan_project(&refs, 16.0)
        .declared
        .iter()
        .map(|found| (found.label.clone(), found.width, found.path.clone()))
        .collect()
}

fn triples(list: &[(&str, u32, &str)]) -> Vec<(String, u32, String)> {
    list.iter()
        .map(|(label, width, path)| ((*label).to_owned(), *width, (*path).to_owned()))
        .collect()
}

#[test]
fn static_tailwind4_an_entry_brings_the_defaults() {
    let files = [file(
        "src/app.css",
        "@import \"tailwindcss\";\n.a { color: red }",
    )];
    assert_eq!(
        widths(&files),
        triples(&[
            ("sm", 640, "src/app.css"),
            ("md", 768, "src/app.css"),
            ("lg", 1024, "src/app.css"),
            ("xl", 1280, "src/app.css"),
            ("2xl", 1536, "src/app.css"),
        ])
    );
}

#[test]
fn static_tailwind4_initial_and_two_own_values_give_only_those_two() {
    let css = "@import 'tailwindcss';\n\
        @theme {\n  --breakpoint-*: initial;\n  --breakpoint-tablet: 48rem;\n  \
        --breakpoint-desktop: 1200px;\n  --color-brand: oklch(0.6 0.1 180);\n}";
    assert_eq!(
        widths(&[file("app.css", css)]),
        triples(&[("tablet", 768, "app.css"), ("desktop", 1200, "app.css")])
    );
}

#[test]
fn static_tailwind4_own_values_change_and_remove_single_defaults() {
    let css = "@import \"tailwindcss\" prefix(tw);\n\
        @theme inline { --breakpoint-sm: initial; --breakpoint-md: 50rem !important; \
        --breakpoint-3xl: 120rem; /* --breakpoint-4xl: 1px; */ }";
    assert_eq!(
        widths(&[file("app.css", css)]),
        triples(&[
            ("lg", 1024, "app.css"),
            ("xl", 1280, "app.css"),
            ("2xl", 1536, "app.css"),
            ("md", 800, "app.css"),
            ("3xl", 1920, "app.css"),
        ])
    );
}

#[test]
fn static_tailwind4_a_theme_file_beside_the_entry_counts_after_it() {
    let files = [
        file(
            "src/theme.css",
            "@theme { --*: initial; --breakpoint-wide: 90rem; }",
        ),
        file(
            "src/main.css",
            "@import \"tailwindcss\";\n@import \"./theme.css\";",
        ),
        file(
            "public/built.css",
            ":root { --breakpoint-sm: 40rem; } @media (width >= 40rem) {}",
        ),
    ];
    assert_eq!(widths(&files), triples(&[("wide", 1440, "src/theme.css")]));
}

#[test]
fn static_tailwind4_a_value_that_is_no_literal_is_a_hint() {
    let css = "@import \"tailwindcss\"; @theme { --breakpoint-*: initial; \
               --breakpoint-md: var(--width-md); --breakpoint-lg: calc(64rem + 1px); \
               --breakpoint-xl: 80rem }";
    let files = [file("app.css", css)];
    assert_eq!(widths(&files), triples(&[("xl", 1280, "app.css")]));
    let refs: Vec<&ProjectFile> = files.iter().collect();
    let hints = scan_project(&refs, 16.0).hints;
    let names: Vec<&str> = hints
        .iter()
        .map(|hint| hint.params["name"].as_str())
        .collect();
    assert_eq!(names, vec!["--breakpoint-md", "--breakpoint-lg"]);
    assert_eq!(hints[0].key, "plugin.breakpoint-analyzer.hints.dynamic");
}

#[test]
fn static_tailwind4_plain_css_and_built_css_declare_nothing() {
    let files = [
        file("a.css", "/* @import \"tailwindcss\"; */ .a {}"),
        file("b.css", "@import url('tailwindcss-animate'); @themes {}"),
        file("c.css", "@theme;"),
    ];
    assert!(widths(&files).is_empty());
    assert!(imports_tailwind("@import url(\"tailwindcss\");"));
    assert!(imports_tailwind(
        "@import 'tailwindcss/theme.css' layer(theme);"
    ));
    assert!(!imports_tailwind("@import 'tailwindcss-forms';"));
    assert!(!imports_tailwind("@import"));
}

#[test]
fn static_tailwind4_survives_unterminated_blocks() {
    let files = [file(
        "app.css",
        "@import \"tailwindcss\"; @theme { --breakpoint-sm: 1px",
    )];
    assert_eq!(
        widths(&files)[4],
        ("sm".to_owned(), 1, "app.css".to_owned())
    );
}
