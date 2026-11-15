use crate::{Failure, ViewInfo, codes};

fn view(id: u32, engine: &str) -> ViewInfo {
    ViewInfo {
        id,
        engine: engine.to_owned(),
        ..ViewInfo::default()
    }
}

#[test]
fn a_chromium_view_serves_the_page_services() {
    let views = [view(1, "chromium"), view(2, "webkit")];
    assert_eq!(views[0].require_chromium("evaluate"), Ok(()));
    let found = ViewInfo::chromium_of(&views, 1, "evaluate").expect("chromium");
    assert_eq!(found.id, 1);
}

#[test]
fn a_view_of_another_engine_answers_the_one_engine_failure() {
    let views = [view(1, "chromium"), view(2, "webkit")];
    let failure = ViewInfo::chromium_of(&views, 2, "cdp").expect_err("webkit");
    assert_eq!(failure, Failure::engine_unserved("cdp", 2, "webkit"));
    assert_eq!(failure.unserved_engine_in(2), Some("webkit"));
}

#[test]
fn a_view_that_is_gone_is_not_found() {
    let failure = ViewInfo::chromium_of(&[view(1, "chromium")], 9, "cdp").expect_err("gone");
    assert_eq!(failure.code(), Some(codes::NOT_FOUND));
}
