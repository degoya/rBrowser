use super::*;

#[test]
fn the_error_is_the_first_line_of_gits_output_cut() {
    assert_eq!(
        first_line("\n  fatal: not a git repository\nhint: x\n"),
        "fatal: not a git repository"
    );
    assert_eq!(first_line(""), "git lfs install failed");
    assert_eq!(
        first_line(&"x".repeat(500)).chars().count(),
        MAX_ERROR_CHARS
    );
}
