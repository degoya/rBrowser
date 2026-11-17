use std::path::Path;

use super::*;

fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a temporary directory");
    for (path, text) in files {
        let file = dir.path().join(path);
        std::fs::create_dir_all(file.parent().expect("a parent")).expect("folders");
        std::fs::write(file, text).expect("writes");
    }
    dir
}

#[test]
fn the_palette_lists_the_flows_by_name_with_their_description() {
    let dir = project(&[
        (".rbrowser/flows/login.ts", "// @description Log in\n"),
        (".rbrowser/flows/cart.ts", "await page.goto(env);\n"),
    ]);
    let list = flow_list(Some(dir.path()), ".rbrowser/flows").expect("lists");
    assert_eq!(
        list.items,
        [
            FlowSummary {
                name: "cart".to_owned(),
                description: None,
            },
            FlowSummary {
                name: "login".to_owned(),
                description: Some("Log in".to_owned()),
            },
        ]
    );
}

#[test]
fn without_a_project_the_palette_has_no_flows() {
    assert_eq!(flow_list(None, ".rbrowser/flows"), Ok(FlowList::default()));
}

#[test]
fn a_folder_outside_the_protected_one_is_refused() {
    assert_eq!(
        flow_list(Some(Path::new("/site")), "../elsewhere"),
        Err(FlowError::BadDir("../elsewhere".to_owned()))
    );
}

#[test]
fn only_list_flows_is_answered_here() {
    assert!(answers(&ShellToCore::ListFlows));
    assert!(!answers(&ShellToCore::GetState));
}
