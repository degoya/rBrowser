use std::path::{Path, PathBuf};

use rb_bidi::launcher::{Installed, Kind};

use super::*;
use crate::about::{BUILD, Dirs, facts_from};

fn found(path: &str, version: Option<&str>) -> Installed {
    Installed {
        path: PathBuf::from(path),
        kind: Kind::Release,
        version: version.map(str::to_owned),
    }
}

fn base() -> AboutInfo {
    facts_from(BUILD, &Dirs::default())
}

const RELEASE: &str = r"C:\Program Files\Mozilla Firefox\firefox.exe";
const DEV: &str = r"C:\Program Files\Firefox Developer Edition\firefox.exe";

#[test]
fn the_first_installation_is_named_with_its_version() {
    let info = with_firefox(
        base(),
        &[found(RELEASE, Some("146.0")), found(DEV, Some("147.0b3"))],
        None,
        |_| true,
    );
    assert_eq!(info.firefox.as_deref(), Some("146.0"));
    assert_eq!(info.firefox_path.as_deref(), Some(RELEASE));
}

#[test]
fn the_setting_wins_and_takes_the_version_of_its_installation() {
    let info = with_firefox(
        base(),
        &[found(RELEASE, Some("146.0")), found(DEV, Some("147.0b3"))],
        Some(DEV),
        |path| path == Path::new(DEV),
    );
    assert_eq!(info.firefox.as_deref(), Some("147.0b3"));
    assert_eq!(info.firefox_path.as_deref(), Some(DEV));
}

#[test]
fn a_set_path_outside_the_list_reads_unknown() {
    let own = r"D:\tools\firefox\firefox.exe";
    let info = with_firefox(base(), &[found(RELEASE, Some("146.0"))], Some(own), |_| {
        true
    });
    assert_eq!(info.firefox.as_deref(), Some(UNKNOWN));
    assert_eq!(info.firefox_path.as_deref(), Some(own));
}

#[test]
fn a_missing_set_path_is_not_found_even_with_an_installation() {
    // The start refuses a missing set path (`LocateError::Configured`) rather than falling back.
    let info = with_firefox(
        base(),
        &[found(RELEASE, Some("146.0"))],
        Some(r"D:\gone\firefox.exe"),
        |path| path == Path::new(RELEASE),
    );
    assert_eq!(info.firefox, None);
    assert_eq!(info.firefox_path, None);
}

#[test]
fn nothing_installed_is_not_found_and_a_blank_setting_is_unset() {
    let info = with_firefox(base(), &[], Some("  "), |_| true);
    assert_eq!(info.firefox, None);
    assert_eq!(info.firefox_path, None);
}

#[test]
fn an_unreadable_version_reads_unknown() {
    let info = with_firefox(base(), &[found(RELEASE, None)], None, |_| true);
    assert_eq!(info.firefox.as_deref(), Some(UNKNOWN));
    assert_eq!(info.firefox_path.as_deref(), Some(RELEASE));
}
