use super::*;

#[test]
fn a_selector_is_a_string_literal_never_code() {
    let script = has_selector("a\"); alert(1); (\"");
    assert!(script.contains(r#"document.querySelector("a\"); alert(1); (\"")"#));
    assert!(script.contains("catch (error)"));
}

#[test]
fn the_scroll_is_bounded() {
    assert!(AUTO_SCROLL.contains(&format!("step < {MAX_SCROLL_STEPS}")));
    assert!(AUTO_SCROLL.contains("window.scrollTo(0, 0)"));
}
