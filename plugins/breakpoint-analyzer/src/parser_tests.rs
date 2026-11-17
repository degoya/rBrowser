use super::{Boundary, Side, scan, scan_media_list};

fn changes(css: &str) -> Vec<(u32, Side)> {
    scan(css, 16.0)
        .boundaries
        .into_iter()
        .map(|Boundary { change, side }| (change, side))
        .collect()
}

#[test]
fn min_width_changes_at_its_value() {
    assert_eq!(
        changes("@media (min-width: 768px) { .a { color: red } }"),
        vec![(768, Side::Min)]
    );
    assert_eq!(
        changes("@media(min-width:767.5px){}"),
        vec![(768, Side::Min)]
    );
}

#[test]
fn max_width_changes_one_pixel_above() {
    assert_eq!(
        changes("@media (max-width: 767px) {} @media (max-width: 575.98px) {}"),
        vec![(768, Side::Max), (576, Side::Max)]
    );
}

#[test]
fn range_syntax_in_every_direction() {
    assert_eq!(
        changes("@media (width >= 768px) {} @media (width > 767px) {}"),
        vec![(768, Side::Min), (768, Side::Min)]
    );
    assert_eq!(
        changes("@media (width < 768px) {} @media (width <= 767.98px) {}"),
        vec![(768, Side::Max), (768, Side::Max)]
    );
    assert_eq!(
        changes("@media (768px <= width) {} @media (640px > width) {}"),
        vec![(768, Side::Min), (640, Side::Max)]
    );
    assert_eq!(
        changes("@media (400px <= width < 800px) {}"),
        vec![(400, Side::Min), (800, Side::Max)]
    );
}

#[test]
fn em_and_rem_use_the_root_font_size() {
    assert_eq!(
        changes("@media (min-width: 48em) {} @media (width >= 40rem) {}"),
        vec![(768, Side::Min), (640, Side::Min)]
    );
    let scanned = scan("@media (min-width: 40rem) {}", 20.0);
    assert_eq!(scanned.boundaries[0].change, 800);
}

#[test]
fn combined_and_listed_conditions() {
    let scanned = scan(
        "@media screen and (min-width: 768px) and (max-width: 1023px), print and (min-width: 1280px) {}",
        16.0,
    );
    assert_eq!(scanned.queries, 2);
    assert_eq!(
        scanned
            .boundaries
            .iter()
            .map(|boundary| boundary.change)
            .collect::<Vec<_>>(),
        vec![768, 1024, 1280]
    );
}

#[test]
fn nested_rules_count_at_any_depth() {
    let css = "@layer utilities { .sm\\:flex { @media (width >= 40rem) { display: flex } } }\n\
               @supports (display: grid) { @media (min-width: 1024px) { .g { display: grid } } }";
    assert_eq!(changes(css), vec![(640, Side::Min), (1024, Side::Min)]);
}

#[test]
fn not_swaps_the_side() {
    assert_eq!(
        changes("@media not all and (min-width: 768px) {}"),
        vec![(768, Side::Max)]
    );
}

#[test]
fn container_queries_are_only_a_hint() {
    let scanned = scan(
        "@container card (min-width: 400px) { .a { color: red } } @media (min-width: 640px) {}",
        16.0,
    );
    assert_eq!(scanned.containers, 1);
    assert_eq!(scanned.boundaries.len(), 1);
    assert_eq!(scanned.boundaries[0].change, 640);
}

#[test]
fn comments_strings_and_other_features_are_ignored() {
    let css = "/* @media (min-width: 111px) {} */ .a::after { content: \"@media (min-width: 222px)\" }\n\
               @media print {} @media (prefers-reduced-motion: reduce) {} @media (min-height: 600px) {}\n\
               @media (orientation: landscape) and (min-width: 20vw) {} @media (min-width: 0) {}";
    let scanned = scan(css, 16.0);
    assert!(scanned.boundaries.is_empty(), "{scanned:?}");
    assert_eq!(scanned.queries, 0);
}

#[test]
fn import_media_lists_count() {
    assert_eq!(
        changes("@import url(\"wide.css\") screen and (min-width: 1200px);"),
        vec![(1200, Side::Min)]
    );
    assert_eq!(
        scan_media_list("(max-width: 599px)", 16.0).boundaries,
        vec![Boundary {
            change: 600,
            side: Side::Max
        }]
    );
}

#[test]
fn absurd_widths_and_broken_css_do_not_panic() {
    assert!(changes("@media (min-width: 99999px) {}").is_empty());
    assert!(changes("@media (min-width: -5px) {}").is_empty());
    assert!(changes("@media (min-width: 1e400px) {}").is_empty());
    for broken in [
        "@",
        "@media",
        "@media (",
        "/*",
        "\"",
        "\\",
        "@media (width >=) {}",
        "((((()",
    ] {
        let _ = scan(broken, 16.0);
    }
}
