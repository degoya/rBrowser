use super::{RELOAD, SHEETS, wait_script};

#[test]
fn the_wait_tells_the_old_document_by_its_time_origin() {
    let script = wait_script(1_731_400_000_123.5, 500);
    assert!(script.contains("performance.timeOrigin === 1731400000123.5"));
    assert!(script.contains("< 500"));
    assert!(script.contains("state: 'ready'"));
}

#[test]
fn the_scripts_are_single_expressions() {
    for script in [SHEETS, RELOAD] {
        assert!(script.starts_with("(() => {"));
        assert!(script.trim_end().ends_with("})()"));
    }
    assert!(RELOAD.contains("location.reload()"));
}
