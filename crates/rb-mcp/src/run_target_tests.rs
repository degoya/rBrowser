use rb_config::ViewScope;
use serde_json::json;

use super::{EMPTY_SELECTION, NO_BREAKPOINT, RunTarget, Split, TargetItem, split, with_scope};
use crate::target::{TargetArg, VIEW_NOT_FOUND};
use crate::testing::view;

fn views() -> Vec<rb_engine::ViewInfo> {
    let mut free = view(3, "mobile", "https://a.test/");
    free.breakpoint = None;
    free.cell_breakpoint = None;
    free.label = "500x800".to_owned();
    vec![
        view(1, "mobile", "https://a.test/"),
        view(2, "desktop", "https://a.test/"),
        free,
    ]
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|name| (*name).to_owned()).collect()
}

#[test]
fn the_target_reads_a_word_an_id_or_a_list() {
    for (text, target) in [
        (json!("all"), RunTarget::Name("all".to_owned())),
        (json!("visible"), RunTarget::Name("visible".to_owned())),
        (json!(2), RunTarget::Id(2)),
        (
            json!([1, "desktop"]),
            RunTarget::List(vec![
                TargetItem::Id(1),
                TargetItem::Name("desktop".to_owned()),
            ]),
        ),
    ] {
        let read: RunTarget = serde_json::from_value(text.clone()).expect("reads");
        assert_eq!(read, target, "{text}");
    }
    assert!(serde_json::from_value::<RunTarget>(json!({ "views": [1] })).is_err());
}

#[test]
fn all_an_id_and_one_breakpoint_keep_the_target_of_every_view_tool() {
    assert_eq!(split(None, &views()), Ok(Split::Target(None)));
    for (target, kept) in [
        (RunTarget::Id(2), TargetArg::Id(2)),
        (
            RunTarget::Name("all".to_owned()),
            TargetArg::Name("all".to_owned()),
        ),
        (
            RunTarget::Name("mobile".to_owned()),
            TargetArg::Name("mobile".to_owned()),
        ),
    ] {
        assert_eq!(
            split(Some(&target), &views()),
            Ok(Split::Target(Some(kept)))
        );
    }
}

#[test]
fn visible_is_the_scope_of_the_breakpoints_the_grid_shows() {
    for word in ["visible", "Visible"] {
        assert_eq!(
            split(Some(&RunTarget::Name(word.to_owned())), &views()),
            Ok(Split::Scope(ViewScope::Visible))
        );
    }
}

#[test]
fn a_list_becomes_the_breakpoints_of_its_views_and_ids_once_each() {
    let list = RunTarget::List(vec![
        TargetItem::Name("desktop".to_owned()),
        TargetItem::Id(1),
        TargetItem::Name("2".to_owned()),
        TargetItem::Name("tablet".to_owned()),
    ]);
    assert_eq!(
        split(Some(&list), &views()),
        Ok(Split::Scope(ViewScope::Breakpoints(names(&[
            "desktop", "mobile", "tablet"
        ]))))
    );
}

#[test]
fn an_empty_list_an_unknown_view_and_a_free_size_view_are_clear_errors() {
    let empty = split(Some(&RunTarget::List(Vec::new())), &views()).expect_err("empty");
    assert_eq!(empty.code, EMPTY_SELECTION);
    let unknown =
        split(Some(&RunTarget::List(vec![TargetItem::Id(9)])), &views()).expect_err("unknown");
    assert_eq!(unknown.code, VIEW_NOT_FOUND);
    assert!(
        unknown.message.contains("known views"),
        "{}",
        unknown.message
    );
    let free =
        split(Some(&RunTarget::List(vec![TargetItem::Id(3)])), &views()).expect_err("free size");
    assert_eq!(free.code, NO_BREAKPOINT);
    assert!(free.message.contains('3'), "{}", free.message);
}

#[test]
fn the_scope_goes_into_the_arguments() {
    let args = with_scope(&json!({ "url": "https://a.test/" }), &ViewScope::Visible);
    assert_eq!(
        args,
        json!({ "url": "https://a.test/", "scope": "visible" })
    );
    let args = with_scope(
        &serde_json::Value::Null,
        &ViewScope::Breakpoints(names(&["mobile"])),
    );
    assert_eq!(args, json!({ "scope": ["mobile"] }));
}
