use super::{Breakpoints, Scope, wrap};
use crate::testing::view;

fn grid() -> Breakpoints {
    // Out of order on purpose, and `md` twice (two views of one breakpoint).
    Breakpoints::from_views(&[
        view(3, "lg", 1280),
        view(1, "mobile", 375),
        view(2, "md", 768),
        view(4, "md", 768),
    ])
}

fn ids(selected: &[&str]) -> Vec<String> {
    selected.iter().map(|id| (*id).to_owned()).collect()
}

fn query(text: &str) -> Scope {
    Scope::Query(text.to_owned())
}

#[test]
fn the_breakpoints_are_the_grids_once_each_by_width() {
    let grid = grid();
    let listed: Vec<(&str, u32)> = grid
        .list()
        .iter()
        .map(|bp| (bp.id.as_str(), bp.width))
        .collect();
    assert_eq!(listed, [("mobile", 375), ("md", 768), ("lg", 1280)]);
    assert!(grid.knows("md"));
    assert!(!grid.knows("xl"));
}

#[test]
fn a_view_without_breakpoint_or_width_is_left_out() {
    let mut nameless = view(1, "", 375);
    nameless.label = String::new();
    let grid = Breakpoints::from_views(&[nameless, view(2, "zero", 0), view(3, "md", 768)]);
    assert_eq!(grid.list().len(), 1);
}

#[test]
fn md_runs_from_its_width_to_below_the_next() {
    assert_eq!(
        grid().scope(&ids(&["md"])),
        query("(min-width: 768px) and (max-width: 1279px)")
    );
}

#[test]
fn the_smallest_has_no_lower_bound_and_the_largest_no_upper() {
    assert_eq!(grid().scope(&ids(&["mobile"])), query("(max-width: 767px)"));
    assert_eq!(grid().scope(&ids(&["lg"])), query("(min-width: 1280px)"));
}

#[test]
fn neighbours_join_and_gaps_become_a_list() {
    assert_eq!(
        grid().scope(&ids(&["mobile", "md"])),
        query("(max-width: 1279px)")
    );
    assert_eq!(
        grid().scope(&ids(&["lg", "mobile"])),
        query("(max-width: 767px), (min-width: 1280px)")
    );
}

#[test]
fn none_or_every_breakpoint_is_all_and_only_unknown_ones_is_nowhere() {
    assert_eq!(grid().scope(&[]), Scope::All);
    assert_eq!(grid().scope(&ids(&["mobile", "md", "lg"])), Scope::All);
    assert_eq!(grid().scope(&ids(&["xl"])), Scope::Nowhere);
    assert_eq!(
        grid().scope(&ids(&["xl", "lg"])),
        query("(min-width: 1280px)")
    );
}

#[test]
fn breakpoints_of_one_width_share_its_range() {
    let grid = Breakpoints::from_views(&[
        view(1, "mobile", 375),
        view(2, "md", 768),
        view(3, "tablet", 768),
        view(4, "lg", 1280),
    ]);
    assert_eq!(
        grid.scope(&ids(&["tablet"])),
        query("(min-width: 768px) and (max-width: 1279px)")
    );
}

#[test]
fn wrap_indents_the_rules_inside_the_query() {
    assert_eq!(
        wrap(
            "(max-width: 767px)",
            "body { outline: 3px solid red }\n\na { color: red }"
        ),
        "@media (max-width: 767px) {\n  body { outline: 3px solid red }\n\n  a { color: red }\n}"
    );
}
