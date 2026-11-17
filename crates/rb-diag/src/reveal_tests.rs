use std::io;

use super::{Explorer, explorer_target};

const SHARE_FILE: &str =
    r"\\wsl.localhost\debian\home\dev\site\.rbrowser\out\flows\run\result.json";
const SHARE_FOLDER: &str = r"\\wsl.localhost\debian\home\dev\site\.rbrowser\out\flows\run";

#[test]
fn an_existing_file_is_selected() {
    let chosen = explorer_target(r"C:\out\shot.png", |path| path == r"C:\out\shot.png");
    assert_eq!(
        chosen.ok(),
        Some(Explorer::Select(r"C:\out\shot.png".to_owned()))
    );
}

#[test]
fn a_missing_file_opens_its_folder() {
    let chosen = explorer_target(SHARE_FILE, |path| path == SHARE_FOLDER);
    assert_eq!(chosen.ok(), Some(Explorer::Open(SHARE_FOLDER.to_owned())));
}

#[test]
fn neither_file_nor_folder_is_an_error_not_the_desktop() {
    let chosen = explorer_target(SHARE_FILE, |_| false);
    assert_eq!(
        chosen.err().map(|error| error.kind()),
        Some(io::ErrorKind::NotFound)
    );
}

#[test]
fn the_long_unc_form_and_forward_slashes_become_what_explorer_reads() {
    let long = r"\\?\UNC\wsl.localhost\debian\home\dev\site\.rbrowser/out/flows\run\result.json";
    let chosen = explorer_target(long, |path| path == SHARE_FILE);
    assert_eq!(chosen.ok(), Some(Explorer::Select(SHARE_FILE.to_owned())));
}

#[test]
fn the_long_drive_form_loses_its_prefix() {
    let chosen = explorer_target(r"\\?\C:\out\gone.webm", |path| path == r"C:\out");
    assert_eq!(chosen.ok(), Some(Explorer::Open(r"C:\out".to_owned())));
}

#[test]
fn a_file_at_the_drive_root_opens_the_root() {
    let chosen = explorer_target(r"C:\gone.txt", |path| path == r"C:\");
    assert_eq!(chosen.ok(), Some(Explorer::Open(r"C:\".to_owned())));
}

#[test]
fn a_share_root_has_no_folder_above_it() {
    let chosen = explorer_target(r"\\server\share", |_| false);
    assert!(chosen.is_err());
}
