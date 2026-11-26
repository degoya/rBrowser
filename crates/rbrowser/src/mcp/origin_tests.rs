use std::panic::{AssertUnwindSafe, catch_unwind};

use super::*;

#[test]
fn a_call_is_a_persons_unless_mcp_runs_it() {
    assert!(!is_mcp());
    let inside = as_mcp(is_mcp);
    assert!(inside);
    assert!(!is_mcp(), "the mark ends with the call");
}

#[test]
fn a_nested_call_keeps_the_mark_until_the_outer_one_ends() {
    as_mcp(|| {
        as_mcp(|| assert!(is_mcp()));
        assert!(is_mcp(), "the inner call left the outer one marked");
    });
    assert!(!is_mcp());
}

#[test]
fn a_call_that_panics_takes_its_mark_along() {
    let unwound = catch_unwind(AssertUnwindSafe(|| as_mcp(|| panic!("the tool failed"))));
    assert!(unwound.is_err());
    assert!(!is_mcp(), "a thread of the pool is a person's again");
}

#[test]
fn another_thread_is_not_marked() {
    as_mcp(|| {
        let other = std::thread::spawn(is_mcp).join().expect("the thread ends");
        assert!(!other);
    });
}
