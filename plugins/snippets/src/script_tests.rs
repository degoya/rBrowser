use super::{MAX_TEXT_CHARS, MAX_VALUE_BYTES, wrap};

#[test]
fn the_code_is_the_body_of_the_function_handed_to_the_helpers() {
    let script = wrap("return document.title", 5000);
    assert!(script.starts_with("// The wrapper of every snippet"));
    assert!(script.contains("\n((__rbSnippetRun) => {\n"));
    assert!(script.ends_with("})(async () => {\nreturn document.title\n})"));
}

#[test]
fn the_limits_are_filled_in() {
    let script = wrap("1", 1234);
    assert!(script.contains("const LIMIT = 1234;"));
    assert!(script.contains(&format!("const MAX_TEXT = {MAX_TEXT_CHARS};")));
    assert!(script.contains(&format!("const MAX_VALUE = {MAX_VALUE_BYTES};")));
    assert!(!script.contains("__LIMIT__"));
}

#[test]
fn the_snippet_runs_inside_the_evaluation_and_is_raced_against_its_timeout() {
    let script = wrap("while (true) {}", 100);
    // Called at once (its synchronous part is cut by the CDP timeout), never from a microtask.
    assert!(script.contains("running = Promise.resolve(__rbSnippetRun());"));
    assert!(script.contains("Promise.race([running, timeout])"));
}

#[test]
fn cycles_nodes_and_errors_are_described() {
    let script = wrap("1", 100);
    assert!(script.contains("'[Circular]'"));
    assert!(script.contains("describeNode(value)"));
    assert!(script.contains("value instanceof Error"));
}
