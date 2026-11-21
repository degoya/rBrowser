use super::*;

#[test]
fn the_version_is_the_first_line_of_defaults_output() {
    assert_eq!(version_of("26.1\n"), Some("26.1".to_owned()));
    assert_eq!(version_of("  18.6  \n\n"), Some("18.6".to_owned()));
    assert_eq!(version_of(""), None);
    assert_eq!(
        version_of("The domain/default pair of (x, y) does not exist\n"),
        None
    );
    assert_eq!(version_of(&"9".repeat(100)), None);
}

#[test]
fn safari_is_only_on_macos() {
    assert_eq!(SUPPORTED, cfg!(target_os = "macos"));
}

#[test]
fn a_remembered_version_is_answered_without_asking() {
    remember_version("26.1");
    assert_eq!(cached_version().as_deref(), Some("26.1"));
    remember_version("not a version");
    assert_eq!(cached_version().as_deref(), Some("26.1"));
}

#[tokio::test]
async fn outside_macos_no_version_is_found() {
    if SUPPORTED {
        return;
    }
    assert_eq!(detect_version(crate::driver::Edition::Safari).await, None);
}
