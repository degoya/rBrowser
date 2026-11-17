use rb_wsl::watch::batch::Batch;
use rb_wsl::watch::{Change, ChangeKind};

use super::*;

fn batch(paths: &[(&str, ChangeKind)]) -> Batch {
    Batch::Changes(
        paths
            .iter()
            .map(|(path, kind)| Change::new(*path, *kind))
            .collect(),
    )
}

fn modified(paths: &[&str]) -> Batch {
    batch(
        &paths
            .iter()
            .map(|path| (*path, ChangeKind::Modify))
            .collect::<Vec<_>>(),
    )
}

#[test]
fn files_are_css_images_or_other_by_extension() {
    assert_eq!(file_kind("src/app.css"), FileKind::Css);
    assert_eq!(file_kind("SRC/APP.CSS"), FileKind::Css);
    for image in [
        "a.png", "b.jpg", "c.jpeg", "d.svg", "e.webp", "f.avif", "g.gif",
    ] {
        assert_eq!(file_kind(image), FileKind::Image, "{image}");
    }
    for other in [
        "a.js",
        "b.vue",
        "c.html",
        "d.scss",
        "Makefile",
        "e.css.map",
        "css",
    ] {
        assert_eq!(file_kind(other), FileKind::Other, "{other}");
    }
}

#[test]
fn css_and_images_are_swapped_in_auto_mode() {
    assert_eq!(
        plan(&modified(&["src/a.css", "img/logo.png"]), HotMode::Auto),
        Plan::Swap {
            css: vec!["a.css".into()],
            images: vec!["logo.png".into()],
        }
    );
}

#[test]
fn anything_else_in_the_batch_reloads_fully() {
    assert_eq!(
        plan(&modified(&["src/a.css", "src/main.js"]), HotMode::Auto),
        Plan::Full
    );
}

#[test]
fn a_removed_or_renamed_stylesheet_reloads_fully() {
    assert_eq!(
        plan(&batch(&[("a.css", ChangeKind::Remove)]), HotMode::Auto),
        Plan::Full
    );
    assert_eq!(
        plan(&batch(&[("a.png", ChangeKind::Rename)]), HotMode::Auto),
        Plan::Full
    );
}

#[test]
fn full_mode_never_swaps_and_off_does_nothing() {
    assert_eq!(plan(&modified(&["a.css"]), HotMode::Full), Plan::Full);
    assert_eq!(plan(&modified(&["a.css"]), HotMode::Off), Plan::Nothing);
    assert_eq!(plan(&Batch::Rescan, HotMode::Off), Plan::Nothing);
}

#[test]
fn a_rescan_reloads_fully() {
    assert_eq!(plan(&Batch::Rescan, HotMode::Auto), Plan::Full);
}

#[test]
fn the_ignore_file_changes_the_rules_and_reloads_nothing() {
    let only_rules = modified(&[".rbrowser/ignore"]);
    assert!(rules_changed(&only_rules));
    assert_eq!(plan(&only_rules, HotMode::Auto), Plan::Nothing);
    let both = modified(&[".rbrowser/ignore", "a.css"]);
    assert!(rules_changed(&both));
    assert_eq!(
        plan(&both, HotMode::Auto),
        Plan::Swap {
            css: vec!["a.css".into()],
            images: vec![],
        }
    );
    assert!(!rules_changed(&Batch::Rescan));
}

#[test]
fn names_are_reported_once() {
    assert_eq!(
        plan(&modified(&["a/x.css", "b/x.css"]), HotMode::Auto),
        Plan::Swap {
            css: vec!["x.css".into()],
            images: vec![],
        }
    );
}
