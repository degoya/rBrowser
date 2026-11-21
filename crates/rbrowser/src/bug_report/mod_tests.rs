// The acceptance fixture of RB-093-04: three test secrets - a token in the query, an
// `Authorization` value and a cookie value - planted everywhere a page or a project can put them;
// a search over the whole written bundle, the ZIP's entries included, finds none of them.

use std::io::Read as _;
use std::path::Path;

use serde_json::json;

use super::*;

const QUERY_TOKEN: &str = "fixture-query-token-91";
const AUTH_VALUE: &str = "fixture-auth-value-92";
const COOKIE_VALUE: &str = "fixture-cookie-value-93";
const SECRETS: [&str; 3] = [QUERY_TOKEN, AUTH_VALUE, COOKIE_VALUE];

fn words(key: &str, args: &[(&str, &str)]) -> String {
    let args: String = args
        .iter()
        .map(|(name, value)| format!(" {name}={value}"))
        .collect();
    format!("{key}{args}")
}

fn view(id: u32, label: &str) -> RawView {
    RawView {
        id,
        label: label.to_owned(),
        breakpoint: Some(label.to_owned()),
        width: 390,
        height: 844,
        dpr: 3.0,
        engine: "chromium".to_owned(),
        url: format!("https://example.test/account?token={QUERY_TOKEN}#sid={COOKIE_VALUE}"),
        title: format!("Account of {COOKIE_VALUE}"),
        emulation: json!({
            "width": 390, "height": 844, "dpr": 3.0, "mobile": true,
            "userAgent": format!("Mozilla/5.0 auth={AUTH_VALUE}"),
            "headers": { "Authorization": format!("Bearer {AUTH_VALUE}") }
        }),
    }
}

fn gathered() -> Gathered {
    let feed = json!({ "views": [ {
        "view": 1,
        "console": [
            { "level": "error", "text": format!("GET https://example.test/api?token={QUERY_TOKEN} 401") },
            { "level": "error", "text": format!("Authorization: Basic {AUTH_VALUE}") },
            { "level": "warning", "text": format!("request {{\"authorization\":\"Token {AUTH_VALUE}\"}}") },
            { "level": "error", "text": format!("document.cookie: sid={COOKIE_VALUE}; theme=dark") },
            { "level": "error", "text": format!("Set-Cookie: sid={COOKIE_VALUE}; HttpOnly"),
              "url": format!("https://example.test/app.js?key={QUERY_TOKEN}"), "line": 4 }
        ],
        "errors": [ { "text": format!("Uncaught Error: login failed for {COOKIE_VALUE}") } ],
        "network": [
            { "url": format!("https://example.test/api?token={QUERY_TOKEN}"), "method": "GET", "status": 401,
              "requestHeaders": { "Authorization": format!("Bearer {AUTH_VALUE}"), "Cookie": format!("sid={COOKIE_VALUE}") },
              "responseHeaders": { "Set-Cookie": format!("sid={COOKIE_VALUE}") },
              "postData": format!("token={QUERY_TOKEN}") },
            { "url": format!("https://user:{AUTH_VALUE}@example.test/x"), "method": "POST",
              "failed": "net::ERR_FAILED" }
        ]
    } ] });
    Gathered {
        created: "2026-11-20_14-03-07".to_owned(),
        version: "0.9.3".to_owned(),
        environment: Some((
            "staging".to_owned(),
            format!("https://staging.example.test/?token={QUERY_TOKEN}"),
        )),
        note: Some(format!(
            "Cookie: sid={COOKIE_VALUE}\nAuthorization: Bearer {AUTH_VALUE}"
        )),
        views: vec![view(1, "mobile")],
        feed,
        images: vec![Image {
            view: Some(1),
            path: Some("screens/mobile.png".to_owned()),
            error: None,
        }],
        overview: Some(Image {
            view: None,
            path: None,
            error: Some(format!("failed at https://example.test/?t={QUERY_TOKEN}")),
        }),
        settings: BugReportConfig::default(),
    }
}

fn masker_of(gathered: &Gathered) -> Masker {
    Masker::new(secrets(vec![COOKIE_VALUE.to_owned()], &gathered.views))
}

/// Every file below `dir`, a ZIP's entries unpacked, as text.
fn every_text(dir: &Path, out: &mut Vec<(String, String)>) {
    for entry in std::fs::read_dir(dir).expect("read dir") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            every_text(&path, out);
            continue;
        }
        let bytes = std::fs::read(&path).expect("read");
        if path.extension().is_some_and(|ext| ext == "zip") {
            let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).expect("a zip");
            for index in 0..archive.len() {
                let mut member = archive.by_index(index).expect("member");
                let name = member.name().expect("a member name").into_owned();
                let mut inner = Vec::new();
                member.read_to_end(&mut inner).expect("unpack");
                out.push((
                    format!("{}!{name}", path.display()),
                    String::from_utf8_lossy(&inner).into_owned(),
                ));
            }
        }
        out.push((
            path.display().to_string(),
            String::from_utf8_lossy(&bytes).into_owned(),
        ));
    }
}

#[test]
fn no_test_secret_is_anywhere_in_the_bundle() {
    let gathered = gathered();
    let masker = masker_of(&gathered);
    let report = compose(&gathered, &masker);
    let text = markdown(&report, &masker, &words);
    let root = tempfile::tempdir().expect("temp dir");
    let folder = bundle::new_folder(root.path(), &gathered.created).expect("folder");
    std::fs::create_dir_all(folder.join(bundle::SCREENS)).expect("screens");
    std::fs::write(folder.join("screens/mobile.png"), b"\x89PNG pixels").expect("png");
    bundle::write_markdown(&folder, &text).expect("markdown");
    bundle::write_zip(&folder).expect("zip");
    let mut files = Vec::new();
    every_text(&folder, &mut files);
    assert!(
        files.len() >= 4,
        "report.md, the image, the zip and its entries"
    );
    let hits: Vec<String> = files
        .iter()
        .flat_map(|(name, body)| {
            SECRETS
                .iter()
                .filter(|secret| body.contains(*secret))
                .map(move |secret| format!("{secret} in {name}"))
        })
        .collect();
    assert!(hits.is_empty(), "{hits:?}\n{text}");
    // What the report is for is still there.
    for expected in [
        "![mobile](screens/mobile.png)",
        "https://example.test/api?token=***",
        "401 GET",
        "net::ERR_FAILED",
        "Uncaught Error: login failed for ***",
        "headers: Authorization",
    ] {
        assert!(text.contains(expected), "{expected:?} missing in\n{text}");
    }
}

#[test]
fn the_settings_pick_the_parts() {
    let mut gathered = gathered();
    gathered.settings.include = vec![ReportPart::Console];
    let masker = masker_of(&gathered);
    let report = compose(&gathered, &masker);
    assert!(report.console.is_some());
    assert!(report.requests.is_none());
    assert!(report.images.is_empty() && report.overview.is_none());
    assert!(report.version.is_none() && report.environment.is_none());
    assert!(!report.show_pages && !report.show_emulation && !report.show_engines);
    assert!(report.views.iter().all(|view| view.emulation.is_empty()));
    assert_eq!(report.views.len(), 1, "the views are always named");
}

#[test]
fn the_note_is_trimmed_bounded_and_masked() {
    let mut gathered = gathered();
    gathered.note = Some(format!("  {}  ", "n".repeat(MAX_NOTE_CHARS + 10)));
    let report = compose(&gathered, &masker_of(&gathered));
    assert_eq!(
        report.note.map(|note| note.chars().count()),
        Some(MAX_NOTE_CHARS)
    );
    gathered.note = Some("   ".to_owned());
    assert_eq!(compose(&gathered, &masker_of(&gathered)).note, None);
}

#[test]
fn the_secrets_are_the_cookies_and_the_header_values() {
    let gathered = gathered();
    assert_eq!(
        secrets(vec!["c".to_owned()], &gathered.views),
        ["c".to_owned(), format!("Bearer {AUTH_VALUE}")]
    );
}
