use std::collections::BTreeSet;

use rb_engine::ViewId;

use super::*;

fn set(classes: &[PseudoClass]) -> BTreeSet<PseudoClass> {
    classes.iter().copied().collect()
}

fn names(changes: &[NodeChange]) -> Vec<(i64, Vec<&'static str>)> {
    changes
        .iter()
        .map(|change| (change.node, change.classes.clone()))
        .collect()
}

fn registry_with_session() -> Registry {
    let mut registry = Registry::default();
    registry.open(ViewId(1), "F1".to_owned(), 1);
    registry
}

#[test]
fn the_classes_read_with_and_without_the_colon_and_name_cdps_words() {
    assert_eq!(PseudoClass::parse("hover"), Some(PseudoClass::Hover));
    assert_eq!(
        PseudoClass::parse(":focus-within"),
        Some(PseudoClass::FocusWithin)
    );
    assert_eq!(
        PseudoClass::parse(" Focus-Visible "),
        Some(PseudoClass::FocusVisible)
    );
    assert_eq!(PseudoClass::parse("checked"), None);
    assert_eq!(PseudoClass::parse(""), None);
    let words: Vec<&str> = PseudoClass::ALL.iter().map(|class| class.name()).collect();
    assert_eq!(
        words,
        [
            "hover",
            "active",
            "focus",
            "focus-visible",
            "focus-within",
            "visited",
            "target"
        ]
    );
}

/// `li:hover > ul` opens only when the `li` hovers too: a real hover reaches every ancestor,
/// and so do `:active` and `:focus-within`; `:focus` and the rest stay on the element.
#[test]
fn hover_active_and_focus_within_reach_the_ancestors_the_rest_stays_on_the_element() {
    let mut registry = registry_with_session();
    let changes = registry.apply(
        ViewId(1),
        10,
        vec![10, 9, 8],
        set(&[PseudoClass::Hover, PseudoClass::Focus]),
    );
    assert_eq!(
        names(&changes),
        [
            (8, vec!["hover"]),
            (9, vec!["hover"]),
            (10, vec!["hover", "focus"])
        ]
    );
    let changes = registry.apply(
        ViewId(1),
        10,
        vec![10, 9, 8],
        set(&[
            PseudoClass::Active,
            PseudoClass::FocusWithin,
            PseudoClass::Visited,
        ]),
    );
    assert_eq!(
        names(&changes),
        [
            (8, vec!["active", "focus-within"]),
            (9, vec!["active", "focus-within"]),
            (10, vec!["active", "focus-within", "visited"]),
        ],
        "every node whose set changed gets its whole new set"
    );
}

/// Two forced elements share ancestors: an ancestor keeps what the other one still needs.
#[test]
fn a_shared_ancestor_keeps_what_another_element_still_forces() {
    let mut registry = registry_with_session();
    registry.apply(ViewId(1), 10, vec![10, 5, 1], set(&[PseudoClass::Hover]));
    let changes = registry.apply(ViewId(1), 20, vec![20, 5, 1], set(&[PseudoClass::Hover]));
    assert_eq!(
        names(&changes),
        [(20, vec!["hover"])],
        "5 and 1 hover already"
    );
    let changes = registry.apply(ViewId(1), 10, vec![10, 5, 1], BTreeSet::new());
    assert_eq!(
        names(&changes),
        [(10, vec![])],
        "the first one goes; its ancestors still hover for the second"
    );
    assert_eq!(registry.classes_of(ViewId(1), 10), BTreeSet::new());
    assert_eq!(
        registry.classes_of(ViewId(1), 20),
        set(&[PseudoClass::Hover])
    );
}

#[test]
fn clearing_a_view_empties_every_node_it_forced() {
    let mut registry = registry_with_session();
    registry.apply(ViewId(1), 10, vec![10, 1], set(&[PseudoClass::Hover]));
    registry.apply(ViewId(1), 20, vec![20], set(&[PseudoClass::Focus]));
    assert_eq!(
        registry.forced_in(ViewId(1)),
        set(&[PseudoClass::Hover, PseudoClass::Focus])
    );
    let changes = registry.clear(ViewId(1));
    assert_eq!(names(&changes), [(1, vec![]), (10, vec![]), (20, vec![])]);
    assert!(registry.forced_in(ViewId(1)).is_empty());
    assert!(registry.forced_views().is_empty());
    assert!(registry.clear(ViewId(1)).is_empty(), "nothing left");
    assert!(registry.clear(ViewId(7)).is_empty(), "an unknown view");
}

/// A navigation (`DOM.documentUpdated` on rBrowser's session) takes the nodes with it in
/// Chromium: the view's anchors and document go without a CDP call; the session stays.
#[test]
fn a_new_document_drops_the_views_anchors_and_its_root() {
    let mut registry = registry_with_session();
    registry.apply(ViewId(1), 10, vec![10, 1], set(&[PseudoClass::Hover]));
    assert_eq!(registry.document_updated("F1"), Some((ViewId(1), true)));
    assert!(registry.forced_in(ViewId(1)).is_empty());
    assert_eq!(registry.session(ViewId(1)), Some(("F1".to_owned(), None)));
    assert_eq!(
        registry.document_updated("F1"),
        Some((ViewId(1), false)),
        "nothing was forced any more"
    );
    assert_eq!(
        registry.document_updated("OTHER"),
        None,
        "DevTools' own session"
    );
}

#[test]
fn a_detached_session_forgets_the_view() {
    let mut registry = registry_with_session();
    registry.apply(ViewId(1), 10, vec![10], set(&[PseudoClass::Hover]));
    assert_eq!(registry.detached("F1"), Some((ViewId(1), true)));
    assert_eq!(registry.session(ViewId(1)), None);
    assert_eq!(registry.detached("F1"), None);
}

#[test]
fn the_root_is_kept_until_the_document_changes() {
    let mut registry = Registry::default();
    assert_eq!(registry.session(ViewId(2)), None);
    registry.open(ViewId(2), "F9".to_owned(), 3);
    assert_eq!(
        registry.session(ViewId(2)),
        Some(("F9".to_owned(), Some(3)))
    );
    registry.document_updated("F9");
    registry.set_root(ViewId(2), 4);
    assert_eq!(
        registry.session(ViewId(2)),
        Some(("F9".to_owned(), Some(4)))
    );
    assert_eq!(registry.take(ViewId(2)), Some("F9".to_owned()));
    assert_eq!(registry.session(ViewId(2)), None);
}

#[test]
fn a_change_reads_as_a_set_an_addition_or_a_removal() {
    let had = set(&[PseudoClass::Hover, PseudoClass::Focus]);
    assert_eq!(
        Change::Set(set(&[PseudoClass::Visited])).applied_to(&had),
        set(&[PseudoClass::Visited])
    );
    assert_eq!(
        Change::Add(PseudoClass::Target).applied_to(&had),
        set(&[PseudoClass::Hover, PseudoClass::Focus, PseudoClass::Target])
    );
    assert_eq!(
        Change::Remove(PseudoClass::Hover).applied_to(&had),
        set(&[PseudoClass::Focus])
    );
    // A toggle decides by the element in the view the menu opened on.
    assert_eq!(
        Change::toggle(PseudoClass::Hover, &had),
        Change::Remove(PseudoClass::Hover)
    );
    assert_eq!(
        Change::toggle(PseudoClass::Active, &had),
        Change::Add(PseudoClass::Active)
    );
}

/// The menu's marks: this view's states, and the ones every view has.
#[test]
fn the_marks_are_this_views_states_and_the_ones_every_view_shares() {
    let mut registry = registry_with_session();
    registry.open(ViewId(2), "F2".to_owned(), 1);
    registry.apply(
        ViewId(1),
        10,
        vec![10],
        set(&[PseudoClass::Hover, PseudoClass::Focus]),
    );
    registry.apply(ViewId(2), 30, vec![30], set(&[PseudoClass::Hover]));
    let marks = registry.marks(ViewId(1), &[ViewId(1), ViewId(2)]);
    assert_eq!(
        marks.this_view,
        set(&[PseudoClass::Hover, PseudoClass::Focus])
    );
    assert_eq!(marks.all_views, set(&[PseudoClass::Hover]));
    let marks = registry.marks(ViewId(1), &[ViewId(1), ViewId(2), ViewId(3)]);
    assert!(marks.all_views.is_empty(), "view 3 forces nothing");
    assert!(registry.marks(ViewId(1), &[]).all_views.is_empty());
    assert!(marks.any_view);
    assert_eq!(registry.forced_views(), [ViewId(1), ViewId(2)]);
    registry.clear(ViewId(1));
    registry.clear(ViewId(2));
    assert!(!registry.marks(ViewId(1), &[ViewId(1)]).any_view);
    assert!(registry.forced_views().is_empty());
}

/// RB-094-16: the activity bar's entry names every view with a forced state and its states, in
/// the menu's order; a view whose states went is left out.
#[test]
fn the_activity_names_each_forced_view_and_its_states_in_menu_order() {
    let mut registry = registry_with_session();
    registry.open(ViewId(2), "F2".to_owned(), 1);
    registry.open(ViewId(3), "F3".to_owned(), 1);
    assert!(registry.activity().is_empty(), "nothing forced yet");
    registry.apply(
        ViewId(1),
        10,
        vec![10],
        set(&[PseudoClass::FocusWithin, PseudoClass::Hover]),
    );
    registry.apply(ViewId(1), 11, vec![11], set(&[PseudoClass::Hover]));
    registry.apply(ViewId(3), 30, vec![30], set(&[PseudoClass::Visited]));
    let words = |registry: &Registry| -> Vec<(ViewId, Vec<String>)> {
        registry
            .activity()
            .into_iter()
            .map(|entry| (entry.view, entry.states))
            .collect()
    };
    assert_eq!(
        words(&registry),
        [
            (
                ViewId(1),
                vec!["hover".to_owned(), "focus-within".to_owned()]
            ),
            (ViewId(3), vec!["visited".to_owned()]),
        ]
    );
    registry.clear(ViewId(1));
    assert_eq!(words(&registry), [(ViewId(3), vec!["visited".to_owned()])]);
}
