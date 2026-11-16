use rb_engine::{Target, ViewId};

use super::{Resolved, TargetArg, VIEW_NOT_FOUND, resolve, resolve_one};
use crate::testing::view;

fn views() -> Vec<rb_engine::ViewInfo> {
    vec![
        view(1, "mobile", "https://a.test/"),
        view(2, "tablet", "https://a.test/"),
        view(3, "mobile", "https://a.test/"),
    ]
}

#[test]
fn absent_and_all_are_every_view() {
    assert_eq!(resolve(None, &views()), Ok(Resolved::All));
    assert_eq!(
        resolve(Some(&TargetArg::all()), &views()),
        Ok(Resolved::All)
    );
    assert_eq!(
        resolve(Some(&TargetArg::Name("ALL".to_owned())), &views()),
        Ok(Resolved::All)
    );
    assert_eq!(Resolved::All.targets(), vec![Target::All]);
}

#[test]
fn a_view_id_as_number_or_text_is_that_view() {
    assert_eq!(
        resolve(Some(&TargetArg::Id(2)), &views()),
        Ok(Resolved::Views(vec![ViewId(2)]))
    );
    assert_eq!(
        resolve(Some(&TargetArg::Name("3".to_owned())), &views()),
        Ok(Resolved::Views(vec![ViewId(3)]))
    );
}

#[test]
fn a_breakpoint_id_is_every_view_showing_it() {
    let resolved = resolve(Some(&TargetArg::Name("mobile".to_owned())), &views());
    assert_eq!(resolved, Ok(Resolved::Views(vec![ViewId(1), ViewId(3)])));
    assert_eq!(
        Resolved::Views(vec![ViewId(1), ViewId(3)]).targets(),
        vec![Target::View(ViewId(1)), Target::View(ViewId(3))]
    );
}

#[test]
fn an_unknown_target_lists_the_known_views() {
    let error =
        resolve(Some(&TargetArg::Name("foo".to_owned())), &views()).expect_err("no view is foo");
    assert_eq!(error.code, VIEW_NOT_FOUND);
    assert_eq!(
        error.message,
        "view `foo` not found; known views: 1 (mobile), 2 (tablet), 3 (mobile)"
    );
    let error = resolve(Some(&TargetArg::Id(9)), &[]).expect_err("no views");
    assert_eq!(error.message, "view `9` not found; known views: none");
}

#[test]
fn resolve_one_wants_exactly_one_view() {
    assert_eq!(resolve_one(None, &views()), Ok(None));
    assert_eq!(
        resolve_one(Some(&TargetArg::Name("tablet".to_owned())), &views()),
        Ok(Some(ViewId(2)))
    );
    assert!(resolve_one(Some(&TargetArg::all()), &views()).is_err());
    assert!(resolve_one(Some(&TargetArg::Name("mobile".to_owned())), &views()).is_err());
}

#[test]
fn the_argument_reads_numbers_and_text() {
    let id: TargetArg = serde_json::from_str("7").expect("a number");
    assert_eq!(id, TargetArg::Id(7));
    let name: TargetArg = serde_json::from_str("\"all\"").expect("a string");
    assert_eq!(name, TargetArg::all());
}
