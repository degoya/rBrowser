use super::{Locator, same_root};
use crate::instance::{FILE_VERSION, Instance, file_path, write};
use crate::stdio::StdioError;

fn dead(project: &str) -> Instance {
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .expect("a free port")
        .port();
    Instance {
        version: FILE_VERSION,
        pid: 1,
        port,
        token: "0".repeat(64),
        project: project.to_owned(),
    }
}

#[test]
fn the_two_share_names_and_both_separators_name_one_root() {
    assert!(same_root(
        r"\\wsl.localhost\Ubuntu\home\me\app",
        r"\\wsl$\Ubuntu\home\me\app\"
    ));
    assert!(same_root(
        r"\\?\UNC\wsl.localhost\Ubuntu\home\me\app",
        "//wsl.localhost/ubuntu/home/me/app"
    ));
    assert!(same_root(r"C:\Sites\App", "c:/sites/app/"));
    assert!(!same_root(r"C:\Sites\App", r"C:\Sites\App2"));
    assert!(!same_root("", r"C:\Sites\App"));
}

#[test]
fn without_a_live_instance_the_message_names_the_project_and_what_to_do() {
    let dir = tempfile::tempdir().expect("temp dir");
    let locator = Locator::new(dir.path(), Some("abc"), Some("/home/me/site"));
    let error = locator.find().expect_err("none");
    assert!(matches!(error, StdioError::NoInstance(_)));
    let text = error.to_string();
    assert!(text.contains("no running rBrowser"), "{text}");
    assert!(text.contains("/home/me/site"), "{text}");
    assert!(text.contains("rbrowser open"), "{text}");
    assert!(
        text.contains("Settings > Automation > MCP server"),
        "{text}"
    );
}

#[test]
fn dead_instance_files_are_ignored_with_and_without_a_project() {
    let dir = tempfile::tempdir().expect("temp dir");
    write(
        &file_path(dir.path(), Some("abc")),
        &dead(r"\\wsl.localhost\Ubuntu\home\me\site"),
    )
    .expect("written");
    write(&file_path(dir.path(), Some("def")), &dead(r"C:\other")).expect("written");
    for locator in [
        Locator::new(dir.path(), Some("abc"), None),
        Locator::new(dir.path(), Some("zzz"), Some(r"\\wsl$\Ubuntu\home\me\site")),
        Locator::new(dir.path(), None, None),
    ] {
        assert!(
            matches!(locator.find(), Err(StdioError::NoInstance(_))),
            "{locator:?}"
        );
    }
}

#[test]
fn the_description_names_the_project_or_any() {
    let dir = tempfile::tempdir().expect("temp dir");
    assert_eq!(
        Locator::new(dir.path(), None, None).describe(),
        "any project"
    );
    assert_eq!(
        Locator::new(dir.path(), Some("k"), Some("/a")).describe(),
        "`/a`"
    );
}
