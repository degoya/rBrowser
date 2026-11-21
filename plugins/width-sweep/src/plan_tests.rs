use std::collections::BTreeMap;

use super::*;

fn issue(kind: &str, selector: &str) -> Issue {
    Issue {
        kind: kind.to_owned(),
        selector: selector.to_owned(),
        path: format!("html > body > {selector}"),
        other: None,
    }
}

/// Samples of `widths` where `present` says which issues the page shows.
fn sampled(widths: &[u32], present: impl Fn(u32) -> Vec<Issue>) -> Samples {
    widths
        .iter()
        .map(|&width| (width, present(width).into_iter().collect()))
        .collect()
}

#[test]
fn the_coarse_pass_runs_from_the_first_width_to_the_last_in_steps() {
    let widths = coarse(Span {
        from: 320,
        to: 1920,
        step: 10,
    });
    assert_eq!(widths.len(), 161);
    assert_eq!(widths.first(), Some(&320));
    assert_eq!(widths.last(), Some(&1920));
    assert!(widths.windows(2).all(|pair| pair[1] - pair[0] == 10));
}

#[test]
fn the_last_width_is_swept_even_off_the_step() {
    let span = Span {
        from: 320,
        to: 345,
        step: 10,
    };
    assert_eq!(coarse(span), vec![320, 330, 340, 345]);
    let one = Span {
        from: 500,
        to: 500,
        step: 10,
    };
    assert_eq!(coarse(one), vec![500]);
}

#[test]
fn refinement_probes_every_pixel_only_between_widths_that_differ() {
    let hero = issue("overflow", ".hero img");
    let coarse_widths = coarse(Span {
        from: 680,
        to: 760,
        step: 10,
    });
    let samples = sampled(&coarse_widths, |width| {
        if (700..=740).contains(&width) {
            vec![hero.clone()]
        } else {
            Vec::new()
        }
    });
    let fine = refine(&samples, 1000);
    let expected: Vec<u32> = (691..=699).chain(741..=749).collect();
    assert_eq!(fine, expected);
}

#[test]
fn refinement_stops_at_its_limit_and_skips_neighbours() {
    let samples = sampled(&[100, 101, 110, 120], |width| {
        if width == 101 || width == 120 {
            vec![issue("overlap", "nav > a")]
        } else {
            Vec::new()
        }
    });
    // 100 -> 101 has no pixel between, 101 -> 110 eight, 110 -> 120 nine.
    assert_eq!(refine(&samples, 1000).len(), 17);
    assert_eq!(refine(&samples, 5), vec![102, 103, 104, 105, 106]);
}

#[test]
fn an_issue_present_in_one_run_of_widths_is_one_range() {
    let hero = issue("overflow", ".hero img");
    let widths: Vec<u32> = (690..=750).collect();
    let samples = sampled(&widths, |width| {
        if (700..=740).contains(&width) {
            vec![hero.clone()]
        } else {
            Vec::new()
        }
    });
    let found = ranges(&samples);
    assert_eq!(
        found,
        vec![Range {
            issue: hero,
            from: 700,
            to: 740
        }]
    );
}

#[test]
fn an_issue_back_after_a_gap_is_two_ranges_and_issues_are_apart() {
    let text = issue("text-clipped", "h1.title");
    let nav = issue("overlap", "nav > a");
    let samples = sampled(&[400, 410, 420, 430, 440], |width| {
        let mut found = Vec::new();
        if width == 400 || width >= 430 {
            found.push(text.clone());
        }
        if width == 420 {
            found.push(nav.clone());
        }
        found
    });
    let found = ranges(&samples);
    assert_eq!(found.len(), 3);
    assert!(found.contains(&Range {
        issue: text.clone(),
        from: 400,
        to: 400
    }));
    assert!(found.contains(&Range {
        issue: text,
        from: 430,
        to: 440
    }));
    assert!(found.contains(&Range {
        issue: nav,
        from: 420,
        to: 420
    }));
}

#[test]
fn a_clean_page_has_no_range() {
    let samples = sampled(&[320, 330, 340], |_| Vec::new());
    assert!(ranges(&samples).is_empty());
    assert!(refine(&samples, 1000).is_empty());
    let empty: Samples = BTreeMap::new();
    assert!(ranges(&empty).is_empty());
}
