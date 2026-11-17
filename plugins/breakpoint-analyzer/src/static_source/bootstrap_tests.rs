use super::{imports_bootstrap, scan_project};
use crate::static_source::ProjectFile;
use crate::suggest::Framework;

fn file(path: &str, text: &str) -> ProjectFile {
    ProjectFile {
        path: path.to_owned(),
        text: text.to_owned(),
    }
}

fn widths(files: &[ProjectFile]) -> Vec<(String, u32)> {
    let refs: Vec<&ProjectFile> = files.iter().collect();
    scan_project(&refs, 16.0)
        .declared
        .iter()
        .map(|found| (found.label.clone(), found.width))
        .collect()
}

fn pairs(list: &[(&str, u32)]) -> Vec<(String, u32)> {
    list.iter()
        .map(|(label, width)| ((*label).to_owned(), *width))
        .collect()
}

const DEFAULTS: [(&str, u32); 5] = [
    ("sm", 576),
    ("md", 768),
    ("lg", 992),
    ("xl", 1200),
    ("xxl", 1400),
];

#[test]
fn static_bootstrap_only_imported_gives_the_defaults() {
    let files = [
        file("scss/_variables.scss", "$primary: #0d6efd;"),
        file(
            "scss/custom.scss",
            "// Bootstrap\n$primary: teal;\n@import \"bootstrap/scss/bootstrap\";",
        ),
    ];
    assert_eq!(widths(&files), pairs(&DEFAULTS));
    let refs: Vec<&ProjectFile> = files.iter().collect();
    let found = scan_project(&refs, 16.0);
    assert!(found.declared.iter().all(|declared| {
        declared.path == "scss/custom.scss" && declared.framework == Some(Framework::Bootstrap)
    }));
}

#[test]
fn static_bootstrap_an_overridden_map_gives_the_overridden_values() {
    let scss = "$grid-breakpoints: (\n  xs: 0,\n  sm: 600px,\n  md: 50em,\n  lg: 1000px,\n  \
                xl: 1300px,\n  \"xxl\": 1600px\n) !default;\n@import '~bootstrap/scss/bootstrap';";
    assert_eq!(
        widths(&[file("main.scss", scss)]),
        pairs(&[
            ("sm", 600),
            ("md", 800),
            ("lg", 1000),
            ("xl", 1300),
            ("xxl", 1600)
        ])
    );
}

#[test]
fn static_bootstrap_use_with_passes_the_map() {
    let scss = "@use \"bootstrap\" with (\n  $grid-breakpoints: (xs: 0, tablet: 720px, desk: 1140px),\n  \
                $enable-shadows: true\n);";
    assert_eq!(
        widths(&[file("app.scss", scss)]),
        pairs(&[("tablet", 720), ("desk", 1140)])
    );
}

#[test]
fn static_bootstrap_a_function_is_a_hint_and_no_defaults() {
    let scss = "@import \"bootstrap/scss/functions\";\n\
        $grid-breakpoints: map-merge($grid-breakpoints, (xxxl: 1600px));\n\
        $other: map-get($grid-breakpoints, md);";
    let files = [file("theme.scss", scss)];
    assert!(widths(&files).is_empty());
    let refs: Vec<&ProjectFile> = files.iter().collect();
    let hints = scan_project(&refs, 16.0).hints;
    assert_eq!(hints.len(), 1);
    assert_eq!(hints[0].params["name"], "$grid-breakpoints: map-merge");
    let value = "$grid-breakpoints: (sm: $sm, md: math.div(1536px, 2));";
    let refs = [file("v.scss", value)];
    let refs: Vec<&ProjectFile> = refs.iter().collect();
    let found = scan_project(&refs, 16.0);
    assert!(found.declared.is_empty());
    let names: Vec<&str> = found
        .hints
        .iter()
        .map(|hint| hint.params["name"].as_str())
        .collect();
    assert_eq!(names, vec!["$grid-breakpoints.sm", "$grid-breakpoints.md"]);
}

#[test]
fn static_bootstrap_comments_and_other_variables_are_not_read() {
    let files = [file(
        "a.scss",
        "/* $grid-breakpoints: (sm: 1px); */\n// $grid-breakpoints: (md: 2px);\n\
         $grid-breakpoints-extra: (lg: 3px);\n$url: \"http://x//y\";",
    )];
    assert!(widths(&files).is_empty());
    assert!(!imports_bootstrap(
        "@import 'bootstrap-icons/font/bootstrap-icons';"
    ));
    assert!(imports_bootstrap(
        "@use 'node_modules/bootstrap/scss/bootstrap' as *;"
    ));
    assert!(imports_bootstrap("@forward \"_bootstrap.scss\";"));
    assert!(!imports_bootstrap("@import"));
}
