use rb_plugin_types::codes;

use super::check;

#[test]
fn every_folder_rb_config_hashes_needs_trust() {
    for (_, dir) in rb_config::trust::PROTECTED_DIRS {
        assert!(
            check(&[".rbrowser", dir, "a"], false, "a").is_err(),
            "{dir}"
        );
    }
}

#[test]
fn a_protected_folder_and_everything_below_it_need_trust() {
    for path in [
        &[".rbrowser", "snippets"][..],
        &[".rbrowser", "snippets", "a.js"],
        &[".rbrowser", "flows", "deep", "x.ts"],
        &[".rbrowser", "plugins"],
        &[".rbrowser", "mocks", "m.json"],
        &[".rbrowser", "scripts", "s.js"],
    ] {
        let failure = check(path, false, &path.join("/")).expect_err("untrusted");
        assert_eq!(failure.code(), Some(codes::UNTRUSTED_WORKSPACE), "{path:?}");
        assert!(check(path, true, "x").is_ok(), "{path:?} trusted");
    }
}

#[test]
fn everything_else_is_left_to_the_other_rules() {
    for path in [
        &[".rbrowser"][..],
        &[".rbrowser", "config.json"],
        &[".rbrowser", "out", "snippets", "library.json"],
        &["snippets", "a.js"],
        &["src", "flows", "x.ts"],
    ] {
        assert!(check(path, false, "x").is_ok(), "{path:?}");
    }
}

#[test]
fn a_protected_folder_is_found_whatever_its_case() {
    for path in [
        &[".RBROWSER", "snippets", "a.js"][..],
        &[".rbrowser", "Snippets", "a.js"],
        &[".Rbrowser", "FLOWS"],
        &[".rbrowſer", "scripts", "s.js"],
    ] {
        let failure = check(path, false, "x").expect_err("untrusted");
        assert_eq!(failure.code(), Some(codes::UNTRUSTED_WORKSPACE), "{path:?}");
    }
}
