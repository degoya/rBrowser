use super::*;
use crate::fake::FakeHost;

fn font(family: &str, custom: bool, glyphs: u64) -> RenderedFont {
    RenderedFont {
        family: family.to_owned(),
        custom,
        glyphs,
    }
}

fn web(family: &str, status: &str) -> Face {
    Face {
        family: family.to_owned(),
        status: status.to_owned(),
        ..Face::default()
    }
}

#[test]
fn stacks_split_into_unquoted_families() {
    assert_eq!(
        families(r#""Inter ASCII", 'Open Sans',Arial , sans-serif"#),
        ["Inter ASCII", "Open Sans", "Arial", "sans-serif"]
    );
    assert_eq!(families(r#""a,b", c"#), ["a,b", "c"]);
    assert!(families("  ").is_empty());
    assert!(generic("Sans-Serif") && generic("system-ui") && !generic("Inter"));
}

#[test]
fn the_declared_font_drawing_everything_matches() {
    let faces = [web("Inter", "loaded")];
    assert_eq!(
        judge("Inter, Arial", &faces, &[font("Inter", true, 40)]),
        Verdict::Match
    );
    // A web font's file may carry another name than its @font-face alias.
    let brand = [web("Brand", "loaded")];
    assert_eq!(
        judge("Brand, Arial", &brand, &[font("Inter Display", true, 40)]),
        Verdict::Match
    );
    assert_eq!(
        judge("sans-serif", &[], &[font("Arial", false, 9)]),
        Verdict::Match
    );
}

#[test]
fn a_font_that_did_not_load_is_a_fallback() {
    let faces = [web("Inter", "error")];
    assert_eq!(
        judge(
            "Inter, Arial, sans-serif",
            &faces,
            &[font("Arial", false, 30)]
        ),
        Verdict::Fallback {
            rendered: "Arial".to_owned()
        }
    );
    // A second web font of the stack drawing it is no match of the first.
    let both = [web("Inter", "error"), web("Roboto", "loaded")];
    assert_eq!(
        judge("Inter, Roboto", &both, &[font("Roboto", true, 30)]),
        Verdict::Fallback {
            rendered: "Roboto".to_owned()
        }
    );
}

#[test]
fn glyphs_from_another_font_are_counted() {
    let faces = [web("Inter ASCII", "loaded")];
    let fonts = [
        font("Inter", true, 40),
        font("Arial", false, 5),
        font("Segoe UI Symbol", false, 1),
        font("Unused", false, 0),
    ];
    assert_eq!(
        judge(r#""Inter ASCII", Arial"#, &faces, &fonts),
        Verdict::Glyphs {
            fallback: "Arial".to_owned(),
            glyphs: 6
        }
    );
    assert_eq!(judge("Inter", &faces, &[]), Verdict::Unknown);
    assert_eq!(
        judge("", &faces, &[font("Arial", false, 1)]),
        Verdict::Unknown
    );
}

#[test]
fn the_document_is_pushed_not_requested_again() {
    let mut host = FakeHost::default();
    host.view(1, "Desktop", 1280, "http://x/");
    host.rendered("p.a", 11, &[("Inter", true, 20)]);
    let read = read(&mut host, 1, &["p.a", "p.missing"]).expect("read");
    assert_eq!(read[0], Some(vec![font("Inter", true, 20)]));
    assert_eq!(read[1], None);
    assert!(!host.methods().contains(&"DOM.getDocument"));
    assert!(!host.methods().iter().any(|m| m.ends_with(".enable")));
}

#[test]
fn the_document_is_requested_only_when_nobody_has() {
    let mut host = FakeHost::default();
    host.view(1, "Desktop", 1280, "http://x/");
    host.cdp.insert(
        "DOM.pushNodesByBackendIdsToFrontend".to_owned(),
        Err(Failure::coded(FailureKind::Other, codes::HOST_FAILED)),
    );
    assert_eq!(document(&mut host, 1).expect("root"), 1);
    assert!(host.methods().contains(&"DOM.getDocument"));
}

#[test]
fn a_stale_node_id_resolves_the_document_once_more() {
    let mut host = FakeHost::default();
    host.view(1, "Desktop", 1280, "http://x/");
    host.rendered("p.a", 11, &[("Inter", true, 20)]);
    host.fail_first.insert("DOM.querySelector".to_owned(), 1);
    let read = read(&mut host, 1, &["p.a"]).expect("read");
    assert_eq!(read[0], Some(vec![font("Inter", true, 20)]));
    let roots = host
        .methods()
        .iter()
        .filter(|m| **m == "Accessibility.getFullAXTree")
        .count();
    assert_eq!(roots, 2);
}

#[test]
fn a_view_without_cdp_is_unsupported() {
    let mut host = FakeHost::default();
    host.view(1, "Firefox", 1280, "http://x/");
    host.views[0].engine = "firefox".to_owned();
    let failure = read(&mut host, 1, &["p"]).expect_err("unsupported");
    assert_eq!(failure.code(), Some(codes::NOT_AVAILABLE));
}
