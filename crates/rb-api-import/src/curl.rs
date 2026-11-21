//! A pasted `curl` command, as the browsers' "Copy as cURL" writes it for bash (`'...'`,
//! `$'...'`, `\` at a line's end), cmd (`^"...^"`, `^` at a line's end) and PowerShell
//! (`curl.exe`, `` ` `` at a line's end).
//!
//! Mapped: `-X`, the URL (`--url`), `-H`, `-d`/`--data*` (JSON, a form, other text, `@file`),
//! `--data-urlencode`, `--json`, `-F`/`--form` (`@file;type=...`), `-G`, `-I`, `-u` (with
//! `--digest`), `--oauth2-bearer`, `-b` (a cookie, not a file), `-A`, `-e`, `-k`, `-L`, `-m`.
//! Output and display options (`-s`, `-v`, `-o`, `--compressed`, ...) change no request and
//! are passed over; every other option is reported by its name, never its value.

use rb_api_model::request::MAX_TIMEOUT_MS;
use rb_api_model::{ApiRequest, Auth, Body, Entry, Part};

use crate::Source;
use crate::collect::Collector;
use crate::common;
pub(crate) use crate::curl_words::words;
use crate::detect::is_curl;

/// The short options that take a value.
const SHORT_VALUE: &str = "XHdFubAemowxTrcEKCUYyz";
/// The long options of no meaning for the request.
const PASSED_OVER: &[&str] = &[
    "--compressed",
    "--silent",
    "--show-error",
    "--verbose",
    "--include",
    "--fail",
    "--globoff",
    "--http1.1",
    "--http2",
    "--http2-prior-knowledge",
    "--no-buffer",
    "--location-trusted",
    "--basic",
    "--progress-bar",
];
/// The long options with a value of no meaning for the request.
const PASSED_OVER_VALUE: &[&str] = &[
    "--output",
    "--write-out",
    "--stderr",
    "--trace",
    "--trace-ascii",
];
/// The long options with a value rBrowser has no counterpart for, reported.
const REPORTED_VALUE: &[&str] = &[
    "--proxy",
    "--proxy-user",
    "--connect-timeout",
    "--retry",
    "--max-redirs",
    "--cacert",
    "--cert",
    "--key",
    "--cookie-jar",
    "--range",
    "--upload-file",
    "--resolve",
    "--interface",
    "--limit-rate",
    "--config",
];

/// What the options say, before the body is decided.
#[derive(Default)]
struct Parsed {
    method: Option<String>,
    url: Option<String>,
    headers: Vec<Entry>,
    data: Vec<String>,
    data_file: Option<String>,
    parts: Vec<Part>,
    get: bool,
    head: bool,
    user: Option<String>,
    digest: bool,
    bearer: Option<String>,
    insecure: bool,
    timeout_ms: Option<u64>,
}

/// Reads a pasted curl command.
///
/// # Errors
///
/// No curl command, a quote left open, no URL.
pub fn read(source: &Source, out: &mut Collector) -> Result<(), String> {
    let text = source.text.trim();
    if !is_curl(text) {
        return Err("no curl command".to_owned());
    }
    let words = words(text)?;
    let mut parsed = Parsed::default();
    let mut notes: Vec<(&'static str, String)> = Vec::new();
    let mut index = 1;
    while index < words.len() {
        let word = &words[index];
        index += 1;
        let mut value = || -> String {
            let value = words.get(index).cloned().unwrap_or_default();
            index += 1;
            value
        };
        if let Some(long) = word.strip_prefix("--") {
            let option = format!("--{long}");
            match long {
                "request" => parsed.method = Some(value()),
                "url" => parsed.url = Some(value()),
                "header" => header(&mut parsed, &value()),
                "data" | "data-ascii" | "data-binary" => data(&mut parsed, value(), true),
                "data-raw" => data(&mut parsed, value(), false),
                "data-urlencode" => parsed.data.push(value()),
                "json" => {
                    let json = value();
                    for (name, wanted) in [
                        ("Content-Type", "application/json"),
                        ("Accept", "application/json"),
                    ] {
                        if !parsed
                            .headers
                            .iter()
                            .any(|h| h.name.eq_ignore_ascii_case(name))
                        {
                            parsed.headers.push(Entry::new(name, wanted));
                        }
                    }
                    data(&mut parsed, json, true);
                }
                "form" => form(&mut parsed, &value(), true),
                "form-string" => form(&mut parsed, &value(), false),
                "user" => parsed.user = Some(value()),
                "oauth2-bearer" => parsed.bearer = Some(value()),
                "cookie" => cookie(&mut parsed, &value(), &mut notes),
                "user-agent" => parsed.headers.push(Entry::new("User-Agent", value())),
                "referer" => parsed.headers.push(Entry::new("Referer", value())),
                "max-time" => parsed.timeout_ms = seconds(&value()),
                "get" => parsed.get = true,
                "head" => parsed.head = true,
                "insecure" => parsed.insecure = true,
                "digest" => parsed.digest = true,
                "location" => {}
                "ntlm" | "negotiate" | "anyauth" | "aws-sigv4" => notes.push(("auth", option)),
                _ if PASSED_OVER.contains(&option.as_str()) => {}
                _ if PASSED_OVER_VALUE.contains(&option.as_str()) => {
                    value();
                }
                _ if REPORTED_VALUE.contains(&option.as_str()) => {
                    value();
                    notes.push(("option", option));
                }
                _ => notes.push(("option", option)),
            }
        } else if let Some(short) = word.strip_prefix('-').filter(|s| !s.is_empty()) {
            let mut flags = short.chars();
            while let Some(flag) = flags.next() {
                if SHORT_VALUE.contains(flag) {
                    let rest: String = flags.by_ref().collect();
                    let given = if rest.is_empty() { value() } else { rest };
                    short_value(&mut parsed, flag, given, &mut notes);
                    break;
                }
                match flag {
                    'G' => parsed.get = true,
                    'I' => parsed.head = true,
                    'k' => parsed.insecure = true,
                    's' | 'S' | 'L' | 'v' | 'i' | 'f' | 'g' | 'N' | '#' | 'q' => {}
                    other => notes.push(("option", format!("-{other}"))),
                }
            }
        } else if parsed.url.is_none() {
            parsed.url = Some(word.clone());
        } else {
            notes.push(("option", "a second URL".to_owned()));
        }
    }
    let request = build(parsed, out)?;
    let at = crate::collect::title_of(&request);
    for (code, detail) in notes {
        out.reported(&at, code, &detail);
    }
    out.request(&[], request);
    Ok(())
}

fn short_value(
    parsed: &mut Parsed,
    flag: char,
    value: String,
    notes: &mut Vec<(&'static str, String)>,
) {
    match flag {
        'X' => parsed.method = Some(value),
        'H' => header(parsed, &value),
        'd' => data(parsed, value, true),
        'F' => form(parsed, &value, true),
        'u' => parsed.user = Some(value),
        'b' => cookie(parsed, &value, notes),
        'A' => parsed.headers.push(Entry::new("User-Agent", value)),
        'e' => parsed.headers.push(Entry::new("Referer", value)),
        'm' => parsed.timeout_ms = seconds(&value),
        'o' | 'w' => {}
        other => notes.push(("option", format!("-{other}"))),
    }
}

fn seconds(value: &str) -> Option<u64> {
    let seconds: f64 = value.trim().parse().ok()?;
    if !seconds.is_finite() || seconds <= 0.0 {
        return None;
    }
    let ms = (seconds * 1000.0).round().clamp(1.0, MAX_TIMEOUT_MS as f64) as u64;
    Some(ms)
}

fn header(parsed: &mut Parsed, line: &str) {
    if let Some((name, value)) = line.split_once(':') {
        parsed.headers.push(Entry::new(name.trim(), value.trim()));
    }
}

fn cookie(parsed: &mut Parsed, value: &str, notes: &mut Vec<(&'static str, String)>) {
    if value.contains('=') {
        parsed.headers.push(Entry::new("Cookie", value));
    } else {
        notes.push(("option", "--cookie from a file".to_owned()));
    }
}

fn data(parsed: &mut Parsed, value: String, files: bool) {
    match value.strip_prefix('@').filter(|_| files) {
        Some(file) => parsed.data_file = Some(file.to_owned()),
        None => parsed.data.push(value),
    }
}

fn form(parsed: &mut Parsed, value: &str, files: bool) {
    let (name, rest) = value.split_once('=').unwrap_or((value, ""));
    let mut part = Part {
        name: name.to_owned(),
        value: String::new(),
        file: None,
        content_type: None,
    };
    let mut pieces = rest.split(';');
    let first = pieces.next().unwrap_or("");
    match first.strip_prefix('@').or_else(|| first.strip_prefix('<')) {
        Some(file) if files => part.file = Some(file.to_owned()),
        _ => {
            part.value = if files {
                first.to_owned()
            } else {
                rest.to_owned()
            }
        }
    }
    if files {
        for piece in pieces {
            if let Some(kind) = piece.trim().strip_prefix("type=") {
                part.content_type = Some(kind.to_owned());
            }
        }
    }
    parsed.parts.push(part);
}

fn build(parsed: Parsed, out: &mut Collector) -> Result<ApiRequest, String> {
    let url = parsed
        .url
        .clone()
        .filter(|url| !url.is_empty())
        .ok_or("no URL")?;
    let joined = parsed.data.join("&");
    let has_body =
        !parsed.data.is_empty() || parsed.data_file.is_some() || !parsed.parts.is_empty();
    let url = if parsed.get && !joined.is_empty() {
        let glue = if url.contains('?') { '&' } else { '?' };
        format!("{url}{glue}{joined}")
    } else {
        url
    };
    let method = parsed.method.clone().unwrap_or_else(|| {
        if parsed.head {
            "HEAD".to_owned()
        } else if has_body && !parsed.get {
            "POST".to_owned()
        } else {
            "GET".to_owned()
        }
    });
    let mut request = ApiRequest::new(&method, &url);
    request.headers = parsed.headers;
    let at = crate::collect::title_of(&request);
    request.body = if !parsed.parts.is_empty() {
        let parts = parsed
            .parts
            .into_iter()
            .map(|mut part| {
                if let Some(file) = part.file.take() {
                    part.file = Some(common::local_file(&file, &at, out));
                }
                part
            })
            .collect();
        Body::Multipart { parts }
    } else if let Some(file) = &parsed.data_file {
        Body::Binary {
            path: common::local_file(file, &at, out),
        }
    } else if parsed.get || joined.is_empty() {
        Body::None
    } else {
        data_body(joined, &mut request.headers)
    };
    request.auth = match (parsed.bearer, parsed.user) {
        (Some(token), _) => Auth::Bearer { token },
        (None, Some(user)) => {
            let (username, password) = user.split_once(':').unwrap_or((user.as_str(), ""));
            let (username, password) = (username.to_owned(), password.to_owned());
            if parsed.digest {
                Auth::Digest { username, password }
            } else {
                Auth::Basic { username, password }
            }
        }
        (None, None) => Auth::None,
    };
    request.options.verify_tls = !parsed.insecure;
    if let Some(ms) = parsed.timeout_ms {
        request.options.timeout_ms = ms;
    }
    Ok(request)
}

/// A `-d` body: JSON, a form, or the text as it is, by the `Content-Type` and the text.
fn data_body(text: String, headers: &mut Vec<Entry>) -> Body {
    let named = common::content_type(headers);
    if named.contains("json") {
        return Body::Json { text };
    }
    let pairs = || {
        text.split('&').all(|pair| {
            pair.split_once('=')
                .is_some_and(|(name, _)| !name.is_empty() && !name.contains([' ', '{', '"']))
        })
    };
    if named.starts_with("application/x-www-form-urlencoded") || (named.is_empty() && pairs()) {
        let fields = text
            .split('&')
            .map(|pair| {
                let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
                Entry::new(
                    rb_api_model::http_parse::form_unescape(name),
                    rb_api_model::http_parse::form_unescape(value),
                )
            })
            .collect();
        return Body::Form { fields };
    }
    if named.is_empty()
        && serde_json::from_str::<serde_json::Value>(&text)
            .is_ok_and(|v| v.is_object() || v.is_array())
    {
        return Body::Json { text };
    }
    if named.is_empty() {
        headers.push(Entry::new(
            "Content-Type",
            "application/x-www-form-urlencoded",
        ));
    }
    Body::Raw { text }
}

#[cfg(test)]
#[path = "curl_tests.rs"]
mod tests;
