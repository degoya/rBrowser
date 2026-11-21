use super::*;
use crate::restore_last::model::{FORMAT_VERSION, LastPage};

fn pages() -> LastPages {
    LastPages {
        version: FORMAT_VERSION,
        views: vec![LastPage {
            breakpoint: "mobile".to_owned(),
            index: 0,
            path: Some("/shop?token=SECRET123".to_owned()),
            url: None,
            decoupled: true,
        }],
    }
}

#[test]
fn the_pages_are_written_into_the_profile_and_read_back() {
    let profile = tempfile::tempdir().expect("a temporary folder");
    write(profile.path(), &pages()).expect("written");
    assert_eq!(read(profile.path()), Some(pages()));
    let files: Vec<String> = std::fs::read_dir(profile.path())
        .expect("readable")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(files, [FILE_NAME], "no temporary file is left beside it");
    assert_eq!(file(profile.path()), profile.path().join(FILE_NAME));
}

#[test]
fn a_profile_without_the_file_or_with_a_broken_one_has_no_last_session() {
    let profile = tempfile::tempdir().expect("a temporary folder");
    assert_eq!(read(profile.path()), None);
    std::fs::write(profile.path().join(FILE_NAME), "{ broken").expect("written");
    assert_eq!(read(profile.path()), None);
}

#[test]
fn a_write_replaces_the_pages_before() {
    let profile = tempfile::tempdir().expect("a temporary folder");
    write(profile.path(), &pages()).expect("written");
    let empty = LastPages {
        version: FORMAT_VERSION,
        views: Vec::new(),
    };
    write(profile.path(), &empty).expect("written again");
    assert_eq!(read(profile.path()), Some(empty));
}

#[test]
fn an_error_names_no_url() {
    let profile = tempfile::tempdir().expect("a temporary folder");
    let missing = profile.path().join("gone");
    let error = write(&missing, &pages()).expect_err("the folder does not exist");
    assert!(!error.contains("SECRET123"), "{error}");
    assert!(!error.contains("/shop"), "{error}");
}
