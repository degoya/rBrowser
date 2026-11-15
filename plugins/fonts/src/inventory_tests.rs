use super::*;
use crate::fake::{fixture, view_fonts};
use crate::page::Located;
use crate::rendered::RenderedFont;
use crate::run::SampleResult;

fn fallback(family: &str, rendered: &str) -> SampleResult {
    SampleResult {
        family: family.to_owned(),
        stack: format!("{family}, Arial, sans-serif"),
        weight: 400,
        style: "normal".to_owned(),
        at: Located::default(),
        text: String::new(),
        non_ascii: String::new(),
        fonts: vec![RenderedFont {
            family: rendered.to_owned(),
            custom: false,
            glyphs: 9,
        }],
        verdict: Verdict::Fallback {
            rendered: rendered.to_owned(),
        },
    }
}

fn collected() -> Collected {
    let mut ok = fallback("Inter", "Inter");
    ok.verdict = Verdict::Match;
    Collected {
        views: vec![
            view_fonts(
                1,
                "Desktop",
                1280,
                fixture("wide"),
                Rendered::Read(vec![ok]),
            ),
            view_fonts(
                2,
                "Mobile",
                375,
                fixture("narrow"),
                Rendered::Read(vec![fallback("Legacy Serif", "Georgia")]),
            ),
        ],
        failures: Vec::new(),
    }
}

fn family<'a>(families: &'a [Family], name: &str) -> &'a Family {
    families.iter().find(|f| f.family == name).expect(name)
}

#[test]
fn inter_from_the_own_server_in_two_woff2_weights_with_a_count_per_view() {
    let families = families(&collected());
    let inter = family(&families, "Inter");
    assert_eq!(
        (inter.provider.as_str(), inter.host.as_str()),
        ("own", "localhost")
    );
    assert!(inter.web_font);
    assert_eq!(inter.formats, ["woff2"]);
    let weights: Vec<&str> = inter
        .faces
        .iter()
        .map(|face| face.weight.as_str())
        .collect();
    assert_eq!(weights, ["400", "600"]);
    assert_eq!(inter.faces[0].status.len(), 2);
    assert!(inter.faces[0].preloaded && !inter.faces[1].preloaded);
    assert_eq!(inter.faces[0].kb, Some(16.8));
    assert_eq!(inter.file_kb, Some(34.5));
    let counts: Vec<(u32, u64)> = inter.usage.iter().map(|u| (u.view, u.elements)).collect();
    assert_eq!(counts, [(1, 8), (2, 8)]);
    assert_eq!(inter.usage[0].weights, [400, 600]);
}

#[test]
fn roboto_from_google_and_the_installed_families() {
    let families = families(&collected());
    let roboto = family(&families, "Roboto");
    assert_eq!(
        (roboto.provider.as_str(), roboto.host.as_str()),
        ("google", "fonts.gstatic.com")
    );
    assert_eq!(roboto.usage.len(), 2);
    let unused = family(&families, "Unused Display");
    assert!(unused.usage.is_empty());
    assert_eq!(unused.faces[0].status[0].state, "unloaded");
}

#[test]
fn what_chromium_drew_per_view() {
    let families = families(&collected());
    let inter = family(&families, "Inter");
    assert_eq!(inter.rendered[0].state, "ok");
    let legacy = family(&families, "Legacy Serif");
    assert_eq!(legacy.rendered.len(), 1);
    assert_eq!(legacy.rendered[0].view, 2);
    assert_eq!(legacy.rendered[0].state, "fallback");
    assert_eq!(legacy.rendered[0].instead, ["Georgia"]);
}

#[test]
fn a_view_without_cdp_is_unsupported_and_a_failed_one_named() {
    let mut collected = collected();
    collected.views[1].rendered = Rendered::Unsupported;
    let failed = collected.views[1].view.clone();
    collected.failures.push((
        rb_plugin_api::ViewInfo {
            id: 3,
            engine: "firefox".to_owned(),
            ..failed
        },
        "plugins.error.not_available".to_owned(),
    ));
    let families = families(&collected);
    let inter = family(&families, "Inter");
    assert_eq!(
        inter
            .rendered
            .iter()
            .find(|r| r.view == 2)
            .expect("view 2")
            .state,
        "unsupported"
    );
    let views = views(&collected);
    assert_eq!(views.len(), 3);
    assert_eq!(views[2]["error"], "plugins.error.not_available");
    assert_eq!(views[2]["rendered"], "unsupported");
    assert_eq!(views[0]["rendered"], "read");
}

#[test]
fn every_font_file_once() {
    let files = files(&collected());
    assert_eq!(files.len(), 3);
    assert_eq!(files[2]["provider"], "google");
    assert_eq!(files[0]["kb"], 16.8);
}
