use super::*;
use crate::fake::{fixture, view_fonts};
use crate::page::Located;
use crate::rendered::RenderedFont;
use crate::run::Rendered;

/// `rule [params] at path` per finding.
fn lines(findings: &[Finding]) -> Vec<String> {
    findings
        .iter()
        .map(|f| {
            let params: Vec<String> = f.params.iter().map(|(k, v)| format!("{k}={v}")).collect();
            format!(
                "{} [{}] at {}",
                f.rule,
                params.join(", "),
                f.path.as_deref().unwrap_or("-")
            )
        })
        .collect()
}

fn rules_of(findings: &[Finding]) -> Vec<&str> {
    findings.iter().map(|f| f.rule.as_str()).collect()
}

#[test]
fn the_fixture_gives_the_privacy_performance_and_readability_findings() {
    let view = view_fonts(2, "Mobile", 375, fixture("narrow"), Rendered::Skipped);
    let found = lines(&view_findings(&view, &Settings::default()));
    assert_eq!(
        found,
        [
            "external-host [family=Roboto, host=fonts.gstatic.com, provider=Google Fonts] at @font-host fonts.gstatic.com Roboto",
            "font-display-missing [family=Inter, weight=600, style=normal] at @font-face Inter 600 normal",
            "preload-missing [family=Inter, weight=600, style=normal, url=http://localhost:8080/fonts/inter-600.woff2] at @font-face Inter 600 normal",
            "format-not-woff2 [family=Legacy Serif, weight=400, style=normal, format=truetype] at @font-face Legacy Serif 400 normal",
            "face-unused [family=Unused Display, weight=400, style=normal] at @font-face Unused Display 400 normal",
            "text-too-small [size=11, min=12, text=Fine print at 11 px.] at html > body > main > p:nth-of-type(6)",
            "input-too-small [size=14, min=16] at html > body > main > form > input",
            "line-height-tight [ratio=1, min=1.2, text=A long paragraph with a line height of one, so its lines nearly touch each other] at html > body > main > p:nth-of-type(5)",
        ]
    );
}

fn view_findings(view: &ViewFonts, settings: &Settings) -> Vec<Finding> {
    super::view(&[], view, settings)
}

#[test]
fn an_allowed_host_is_no_privacy_finding() {
    let view = view_fonts(2, "Mobile", 375, fixture("narrow"), Rendered::Skipped);
    let settings = Settings {
        allowed_hosts: vec!["fonts.gstatic.com".to_owned()],
        ..Settings::default()
    };
    assert!(!rules_of(&view_findings(&view, &settings)).contains(&rules::EXTERNAL_HOST));
}

#[test]
fn inputs_count_on_phones_and_tablets_only() {
    let view = view_fonts(1, "Desktop", 1280, fixture("wide"), Rendered::Skipped);
    assert!(
        !rules_of(&view_findings(&view, &Settings::default())).contains(&rules::INPUT_TOO_SMALL)
    );
}

#[test]
fn thresholds_come_from_the_settings() {
    let view = view_fonts(2, "Mobile", 375, fixture("narrow"), Rendered::Skipped);
    let settings = Settings {
        min_text_px: 10.0,
        min_input_px: 14.0,
        min_line_height: 1.0,
        max_weights: 1,
        max_file_kb: 16,
        ..Settings::default()
    };
    let found = lines(&view_findings(&view, &settings));
    let rules: Vec<&str> = found
        .iter()
        .map(|line| line.split(' ').next().unwrap_or(""))
        .collect();
    for gone in [
        rules::TEXT_TOO_SMALL,
        rules::INPUT_TOO_SMALL,
        rules::LINE_HEIGHT_TIGHT,
    ] {
        assert!(!rules.contains(&gone), "{gone}");
    }
    assert!(found.contains(
        &"too-many-weights [family=Inter, count=2, max=1] at @font-family Inter".to_owned()
    ));
    assert!(found.contains(
        &"large-file [url=http://localhost:8080/fonts/inter-600.woff2, kb=17.67, max=16] at @font-file http://localhost:8080/fonts/inter-600.woff2"
            .to_owned()
    ));
}

#[test]
fn a_face_of_an_unreadable_google_sheet_is_reported_by_the_sheet_host() {
    let mut fonts = fixture("narrow");
    fonts.faces = vec![Face {
        family: "Open Sans".to_owned(),
        weight: "400".to_owned(),
        style: "normal".to_owned(),
        display: "swap".to_owned(),
        status: "unloaded".to_owned(),
        sheet: "https://fonts.googleapis.com/css2?family=Open+Sans".to_owned(),
        ..Face::default()
    }];
    fonts.resources.clear();
    fonts.sheets = vec![serde_json::json!(
        "https://fonts.googleapis.com/css2?family=Open+Sans"
    )];
    let view = view_fonts(2, "Mobile", 375, fonts, Rendered::Skipped);
    let found = lines(&view_findings(&view, &Settings::default()));
    assert!(found.contains(
        &"external-host [family=Open Sans, host=fonts.googleapis.com, provider=Google Fonts] at @font-host fonts.googleapis.com Open Sans"
            .to_owned()
    ));
    // The sheet's host is reported once, with the face.
    assert_eq!(
        found
            .iter()
            .filter(|line| line.starts_with("external-host"))
            .count(),
        1
    );
}

#[test]
fn a_third_party_font_file_no_face_explains_is_reported_by_its_host() {
    let mut fonts = fixture("narrow");
    fonts.faces.clear();
    fonts.resources = vec![crate::page::Resource {
        url: "https://cdn.fonts.example/a.woff2".to_owned(),
        size: 0,
        decoded: 0,
    }];
    let view = view_fonts(2, "Mobile", 375, fonts, Rendered::Skipped);
    let found = lines(&view_findings(&view, &Settings::default()));
    assert!(found.contains(
        &"external-host [host=cdn.fonts.example, provider=cdn.fonts.example] at @font-host cdn.fonts.example".to_owned()
    ));
}

fn sample(family: &str, path: &str, chars: &str, verdict: Verdict) -> SampleResult {
    SampleResult {
        family: family.to_owned(),
        stack: format!("{family}, Arial"),
        weight: 400,
        style: "normal".to_owned(),
        at: Located {
            selector: path.to_owned(),
            path: path.to_owned(),
        },
        text: "text".to_owned(),
        non_ascii: chars.to_owned(),
        fonts: vec![RenderedFont {
            family: "Arial".to_owned(),
            custom: false,
            glyphs: 3,
        }],
        verdict,
    }
}

#[test]
fn rendered_fallbacks_and_foreign_glyphs_are_reported_at_their_element() {
    let results = vec![
        sample(
            "Inter",
            "p:a",
            "",
            Verdict::Fallback {
                rendered: "Arial".to_owned(),
            },
        ),
        sample(
            "Inter ASCII",
            "p:b",
            "\u{f6}\u{df}",
            Verdict::Glyphs {
                fallback: "Arial".to_owned(),
                glyphs: 5,
            },
        ),
        sample("Roboto", "p:c", "", Verdict::Match),
    ];
    let mut fonts = fixture("narrow");
    fonts.faces.clear();
    fonts.small_text.clear();
    fonts.small_inputs.clear();
    fonts.tight_lines.clear();
    fonts.resources.clear();
    let view = view_fonts(2, "Mobile", 375, fonts, Rendered::Read(results));
    assert_eq!(
        lines(&view_findings(&view, &Settings::default())),
        [
            "rendered-fallback [declared=Inter, rendered=Arial, text=text] at p:a",
            "glyph-fallback [family=Inter ASCII, fallback=Arial, glyphs=5, chars=\u{f6}\u{df}] at p:b",
        ]
    );
}

#[test]
fn face_weights_serve_their_text() {
    assert!(weight_serves("400", 400));
    assert!(!weight_serves("400", 700));
    assert!(weight_serves("100 900", 650));
    assert!(!weight_serves("bold", 700));
}
