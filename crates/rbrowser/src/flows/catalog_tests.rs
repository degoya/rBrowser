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
fn the_folder_stays_inside_the_protected_one() {
    let root = Path::new("/site");
    assert_eq!(
        flows_dir(root, ".rbrowser/flows").expect("default"),
        root.join(".rbrowser/flows")
    );
    assert!(flows_dir(root, "./.rbrowser/flows/e2e").is_ok());
    assert!(flows_dir(root, ".rbrowser\\flows\\e2e").is_ok());
    for bad in [
        "flows",
        ".rbrowser",
        ".rbrowser/flowsX",
        ".rbrowser/flows/../plugins",
        "/etc",
        "C:\\flows",
        "../site/.rbrowser/flows",
    ] {
        assert_eq!(
            flows_dir(root, bad),
            Err(FlowError::BadDir(bad.to_owned())),
            "{bad}"
        );
    }
}

#[test]
fn names_are_plain() {
    for good in ["login", "fill-cart_2", "a.b"] {
        assert!(valid_name(good), "{good}");
    }
    for bad in ["", ".hidden", "a/b", "..", "a\\b", "x.", &"a".repeat(65)] {
        assert!(!valid_name(bad), "{bad}");
    }
}

#[test]
fn the_head_comments_give_description_views_and_timeout() {
    let head = parse_head(
        "// @description Log in and fill the cart\n//   @views mobile, desktop ,\n// @timeout 45000\nawait page.goto(env);\n",
    );
    assert_eq!(
        head.description.as_deref(),
        Some("Log in and fill the cart")
    );
    assert_eq!(head.views, ["mobile", "desktop"]);
    assert_eq!(head.timeout_ms, Some(45_000));
    assert_eq!(parse_head("// @timeout soon\n").timeout_ms, None);
    assert_eq!(parse_head("await page.goto(env);\n"), Head::default());
}

#[test]
fn the_storage_state_comment_names_a_login_state_file() {
    let head = parse_head("// @storageState state/admin.json\nawait page.goto(env);\n");
    assert_eq!(head.storage_state.as_deref(), Some("state/admin.json"));
    assert_eq!(parse_head("// @storageState\n").storage_state, None);
}

#[test]
fn listing_takes_the_ts_files_by_name_and_an_absent_folder_has_none() {
    let dir = project(&[
        (".rbrowser/flows/login.ts", "// @description Log in\n"),
        (".rbrowser/flows/cart.ts", "await page.goto(env);\n"),
        (".rbrowser/flows/notes.md", "no flow"),
        (".rbrowser/flows/bad name.ts", "skipped"),
    ]);
    let flows = list(dir.path(), ".rbrowser/flows").expect("lists");
    let names: Vec<&str> = flows.iter().map(|flow| flow.name.as_str()).collect();
    assert_eq!(names, ["cart", "login"]);
    assert_eq!(flows[1].path, ".rbrowser/flows/login.ts");
    assert_eq!(flows[1].description.as_deref(), Some("Log in"));
    let empty = project(&[]);
    assert_eq!(list(empty.path(), ".rbrowser/flows"), Ok(Vec::new()));
}

#[test]
fn reading_names_the_known_flows_when_one_is_missing() {
    let dir = project(&[(".rbrowser/flows/login.ts", "await page.goto(env);\n")]);
    let (path, source) = read(dir.path(), ".rbrowser/flows", "login").expect("reads");
    assert!(path.ends_with("login.ts"));
    assert_eq!(source, "await page.goto(env);\n");
    assert_eq!(
        read(dir.path(), ".rbrowser/flows", "cart"),
        Err(FlowError::NotFound {
            name: "cart".to_owned(),
            known: vec!["login".to_owned()],
        })
    );
    assert_eq!(
        read(dir.path(), ".rbrowser/flows", "../config"),
        Err(FlowError::BadName("../config".to_owned()))
    );
}

#[test]
fn a_flow_too_long_is_not_read() {
    let long = "x".repeat(usize::try_from(MAX_SOURCE_BYTES).expect("fits") + 1);
    let dir = project(&[(".rbrowser/flows/big.ts", long.as_str())]);
    assert!(matches!(
        read(dir.path(), ".rbrowser/flows", "big"),
        Err(FlowError::Io(_))
    ));
}
