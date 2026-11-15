use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;

use super::{Fetch, HttpFetch, collect};

struct Pages(BTreeMap<&'static str, &'static str>);

impl Fetch for Pages {
    fn text(&self, url: &str, _limit: u64) -> Result<String, String> {
        self.0
            .get(url)
            .map(|text| (*text).to_owned())
            .ok_or_else(|| format!("{url}: 404"))
    }
}

#[test]
fn links_inline_sheets_and_imports_are_read_once() {
    let pages = Pages(BTreeMap::from([
        (
            "http://a.test/",
            "<link rel=stylesheet href=main.css><link rel=stylesheet href=/main.css>\
             <style>@import 'extra.css'; @media (min-width: 600px) {}</style>",
        ),
        (
            "http://a.test/main.css",
            "@import url(deep.css); @media (min-width: 700px) {}",
        ),
        (
            "http://a.test/deep.css",
            "@import url(main.css); @media (min-width: 800px) {}",
        ),
        ("http://a.test/extra.css", "@media (min-width: 900px) {}"),
    ]));
    let collected = collect(&pages, &["http://a.test/".to_owned()]).expect("collected");
    let urls: Vec<&str> = collected
        .sheets
        .iter()
        .map(|sheet| sheet.url.as_str())
        .collect();
    assert_eq!(
        urls,
        vec![
            "http://a.test/deep.css",
            "http://a.test/main.css",
            "http://a.test/extra.css",
            "http://a.test/#inline-1"
        ]
    );
}

#[test]
fn an_http_page_never_reads_a_local_file() {
    let pages = Pages(BTreeMap::from([(
        "http://a.test/",
        "<link rel=stylesheet href=file:///etc/passwd><style>@import 'file:///etc/hosts';</style>",
    )]));
    let collected = collect(&pages, &["http://a.test/".to_owned()]).expect("collected");
    assert_eq!(collected.sheets.len(), 1);
    assert_eq!(collected.sheets[0].url, "http://a.test/#inline-1");
}

#[test]
fn no_readable_page_is_an_error() {
    let pages = Pages(BTreeMap::new());
    let error = collect(&pages, &["http://a.test/".to_owned()]).expect_err("nothing read");
    assert!(error.contains("404"), "{error}");
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/analyzer")
}

/// A one-folder HTTP server on 127.0.0.1; `{{SECOND_ORIGIN}}` in a page becomes `second`.
fn serve(dir: PathBuf, second: Option<String>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let origin = format!("http://{}", listener.local_addr().expect("address"));
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            if reader.read_line(&mut line).is_err() {
                continue;
            }
            loop {
                let mut header = String::new();
                if reader.read_line(&mut header).map_or(true, |n| n == 0) || header == "\r\n" {
                    break;
                }
            }
            let path = line
                .split_whitespace()
                .nth(1)
                .unwrap_or("/")
                .trim_start_matches('/');
            let path = if path.is_empty() { "index.html" } else { path };
            let (status, body) = match std::fs::read_to_string(dir.join(path)) {
                Ok(body) => (
                    "200 OK",
                    body.replace("{{SECOND_ORIGIN}}", second.as_deref().unwrap_or("")),
                ),
                Err(_) => ("404 Not Found", String::new()),
            };
            let mut stream = reader.into_inner();
            let _ = write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    origin
}

#[test]
fn a_cross_origin_stylesheet_is_read() {
    let remote = serve(fixtures().join("cross-origin/remote"), None);
    let page = serve(fixtures().join("cross-origin"), Some(remote.clone()));
    let collected = collect(&HttpFetch::default(), &[format!("{page}/")]).expect("collected");
    let urls: Vec<String> = collected
        .sheets
        .iter()
        .map(|sheet| sheet.url.clone())
        .collect();
    assert_eq!(
        urls,
        vec![
            format!("{page}/#inline-1"),
            format!("{remote}/wide.css"),
            format!("{remote}/remote.css"),
        ]
    );
    assert!(collected.sheets[2].text.contains("min-width: 900px"));
}

#[test]
fn a_page_on_disk_reads_its_sheets_from_disk() {
    let index = fixtures().join("bootstrap/index.html");
    let url = format!("file://{}", index.display());
    let collected = collect(&HttpFetch::default(), &[url]).expect("collected");
    assert_eq!(collected.sheets.len(), 1);
    assert!(
        collected.sheets[0]
            .url
            .ends_with("bootstrap/css/bootstrap.min.css")
    );
    assert!(collected.sheets[0].text.contains("max-width:575.98px"));
}

#[test]
fn a_file_is_read_only_up_to_the_limit_and_only_when_it_is_a_regular_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let sheet = dir.path().join("a.css");
    std::fs::write(&sheet, "a{}b{}").expect("write");
    let url = |path: &std::path::Path| format!("file://{}", path.display());
    let fetch = HttpFetch::default();
    assert_eq!(fetch.text(&url(&sheet), 6).expect("fits"), "a{}b{}");
    let error = fetch.text(&url(&sheet), 5).expect_err("too large");
    assert!(error.contains("larger than 5 bytes"), "{error}");
    let error = fetch.text(&url(dir.path()), 1024).expect_err("a folder");
    assert!(error.contains("not a regular file"), "{error}");
}

#[test]
fn a_file_url_is_decoded_into_its_path() {
    use super::file_path;
    let path = |url: &str| file_path(url).map(|path| path.to_string_lossy().into_owned());
    assert_eq!(
        path("file:///C:/My%20Site/a%2Bb.css?v=1#top").as_deref(),
        Some("C:/My Site/a+b.css")
    );
    assert_eq!(
        path("file:///home/me/caf%C3%A9.css").as_deref(),
        Some("/home/me/caf\u{e9}.css")
    );
    assert_eq!(
        path("file://localhost/srv/a.css").as_deref(),
        Some("/srv/a.css")
    );
    assert_eq!(
        path("file://server/share/a.css").as_deref(),
        Some("//server/share/a.css")
    );
    assert_eq!(path("file:///a%zz.css"), None, "a broken escape");
    assert_eq!(path("file:///a%+f.css"), None, "a sign is no hex digit");
    assert_eq!(path("file:///a%FF.css"), None, "no UTF-8");
    assert_eq!(path("http://site.test/a.css"), None);
}
