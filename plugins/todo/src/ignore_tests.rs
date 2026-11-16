use std::rc::Rc;

use super::{Ignore, ignored};

fn rules(base: &str, text: &str) -> Vec<Rc<Ignore>> {
    Ignore::parse(base, text).map(Rc::new).into_iter().collect()
}

#[test]
fn a_name_leaves_out_a_folder_of_that_name_at_any_depth_below() {
    let root = rules("", "# build output\n/node_modules\nlogs/\ncache\n\n");
    assert!(ignored(&root, "logs"));
    assert!(ignored(&root, "app/logs"));
    assert!(ignored(&root, "app/deep/cache"));
    assert!(ignored(&root, "node_modules"));
    assert!(!ignored(&root, "app/node_modules"), "anchored at the root");
    assert!(!ignored(&root, "docs"));
    assert!(!ignored(&root, "logs-old"));
}

#[test]
fn a_path_is_anchored_at_its_file_and_takes_wildcards() {
    let root = rules("", "public/build\n/storage/framework/*\n**/tmp/\n");
    assert!(ignored(&root, "public/build"));
    assert!(!ignored(&root, "build"));
    assert!(ignored(&root, "storage/framework/views"));
    assert!(!ignored(&root, "storage/framework"));
    assert!(ignored(&root, "tmp"));
    assert!(ignored(&root, "a/b/tmp"));
}

#[test]
fn the_last_rule_wins_and_a_negation_keeps_a_folder() {
    // Laravel's storage/framework/cache/.gitignore.
    let nested = rules("storage/framework/cache", "*\n!data/\n!.gitignore\n");
    assert!(ignored(&nested, "storage/framework/cache/other"));
    assert!(!ignored(&nested, "storage/framework/cache/data"));
    assert!(
        !ignored(&nested, "storage/framework/cache"),
        "only below its own folder"
    );
    assert!(!ignored(&nested, "storage/other"));
}

#[test]
fn a_deeper_file_overrules_the_root() {
    let mut all = rules("", "generated/\n");
    all.extend(rules("docs", "!generated/\n"));
    assert!(!ignored(&all, "docs/generated"));
    assert!(ignored(&all, "src/generated"));
}

#[test]
fn what_it_cannot_read_never_leaves_out_more_than_git() {
    // An escape or a class is not understood: as an exclusion it leaves out nothing, as a
    // negation it keeps every folder it might mean.
    let root = rules("", "[Bb]in/\n\\#hash\n");
    assert!(!ignored(&root, "bin"));
    assert!(!ignored(&root, "#hash"));
    let root = rules("", "out/\n![Oo]ut/\n");
    assert!(!ignored(&root, "out"));
    // Comments, blank lines and a lone slash are no rules.
    assert!(Ignore::parse("", "# only a comment\n\n/\n!\n").is_none());
}

#[test]
fn a_huge_file_is_cut_to_its_first_rules() {
    let mut text = "keep-out/\n".to_owned();
    for index in 0..super::MAX_RULES + 10 {
        text.push_str(&format!("folder{index}/\n"));
    }
    let root = rules("", &text);
    assert!(ignored(&root, "keep-out"));
    assert!(!ignored(&root, &format!("folder{}", super::MAX_RULES + 5)));
}
