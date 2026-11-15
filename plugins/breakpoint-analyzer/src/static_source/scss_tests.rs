use super::scan;

fn widths(text: &str, sigil: char) -> Vec<(String, u32)> {
    scan("styles/_vars", text, sigil, 16.0)
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

#[test]
fn static_scss_variables_named_like_breakpoints_are_a_heuristic_source() {
    let scss = "$breakpoint-tablet: 768px;\n$bp-desk: 64em !default;\n\
        $screen-lg-min: 1024px;\n$screen-lg-max: 1279px;\n$bp-calc: $bp-desk + 1;\n\
        // $bp-gone: 1px;\n.a { $bp-local: 500px; color: red }\n$gutter: 30px;";
    assert_eq!(
        widths(scss, '$'),
        pairs(&[
            ("tablet", 768),
            ("desk", 1024),
            ("lg", 1024),
            ("local", 500)
        ])
    );
    let found = scan("v.scss", scss, '$', 16.0);
    assert!(found.declared.iter().all(|declared| {
        declared.heuristic && declared.framework.is_none() && declared.path == "v.scss"
    }));
    assert!(found.hints.is_empty(), "a heuristic source gives no hints");
}

#[test]
fn static_scss_less_uses_the_at_sigil() {
    let less = "@screen-sm-min: 768px;\n@screen-md: 992px;\n@screen-xs-max: (@screen-sm-min - 1);\n\
        @media (min-width: @screen-sm-min) { .a { color: red } }\n@bp-wide: 1440px;";
    assert_eq!(
        widths(less, '@'),
        pairs(&[("sm", 768), ("md", 992), ("wide", 1440)])
    );
    assert!(widths(less, '$').is_empty(), "another sigil reads nothing");
}

#[test]
fn static_scss_odd_names_are_left_out() {
    let scss = "$bp-: 1px; $bp-a.b: 2px; $breakpoints: (sm: 3px); $bp-x y: 4px;";
    assert!(widths(scss, '$').is_empty());
}
