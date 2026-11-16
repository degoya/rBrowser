//! The manifest itself: linked, reachable from the page's origin, served as JSON, parsed; its
//! required members (`name` or `short_name`, `start_url`, an app `display`), the recommended
//! ones (a maskable icon, `theme_color`, `background_color`, `id`, `scope`), and its start URL
//! inside its scope and answering 200.

use rb_plugin_api::PluginHost;
use serde::Serialize;

use crate::collect::PageState;
use crate::fetch::{self, ACCEPT_HTML, ACCEPT_JSON, Budget, Fetched};
use crate::manifest::{Manifest, ParseError};
use crate::rules::{
    FETCH_OTHER_ORIGIN, MANIFEST_CONTENT_TYPE, MANIFEST_DISPLAY, MANIFEST_INVALID,
    MANIFEST_MISSING, MANIFEST_NAME, MANIFEST_RECOMMENDED, MANIFEST_START_URL,
    MANIFEST_UNREACHABLE, Place, START_URL_OUT_OF_SCOPE, START_URL_STATUS, Sink,
};
use crate::url::Url;

/// The content types a manifest may be served as.
pub const MANIFEST_TYPES: &[&str] = &["application/manifest+json", "application/json"];

/// The manifest as the report shows it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestReport {
    pub url: String,
    pub status: Option<u16>,
    pub content_type: Option<String>,
    /// Why the text is no manifest (the JSON parser's message, or `object`).
    pub error: Option<String>,
    /// `(member, value)`, sorted by member.
    pub table: Vec<(String, String)>,
    pub start_url: Option<String>,
    pub scope: Option<String>,
    /// The status the start URL answered with (after its redirects), or the failure.
    pub start_url_status: Option<String>,
}

fn param(key: &'static str, value: impl ToString) -> (&'static str, String) {
    (key, value.to_string())
}

/// The manifest's answer: `None` (and its finding) when it is not to be had.
fn load(
    host: &mut dyn PluginHost,
    budget: &Budget,
    page: &Url,
    url: &Url,
    report: &mut ManifestReport,
    sink: &mut Sink<'_>,
) -> Option<String> {
    let response = match fetch::get(host, budget, page, url, ACCEPT_JSON) {
        Fetched::Ok(response) => response,
        Fetched::OtherOrigin(other) => {
            let params = [param("what", "manifest"), param("url", other)];
            sink.emit(FETCH_OTHER_ORIGIN, Place::Manifest, &params);
            return None;
        }
        Fetched::Failed(code) => {
            let params = [param("url", url), param("status", code)];
            sink.emit(MANIFEST_UNREACHABLE, Place::Manifest, &params);
            return None;
        }
    };
    report.status = Some(response.status);
    report.content_type = response.header("content-type").map(str::to_owned);
    if response.status != 200 {
        let params = [param("url", url), param("status", response.status)];
        sink.emit(MANIFEST_UNREACHABLE, Place::Manifest, &params);
        return None;
    }
    let kind = report.content_type.clone().unwrap_or_default();
    let essence = kind
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if !MANIFEST_TYPES.contains(&essence.as_str()) {
        let shown = if kind.is_empty() {
            "-".to_owned()
        } else {
            kind
        };
        sink.emit(
            MANIFEST_CONTENT_TYPE,
            Place::Manifest,
            &[param("type", shown)],
        );
    }
    Some(String::from_utf8_lossy(&response.body).into_owned())
}

/// The members' rules.
fn members(manifest: &Manifest, sink: &mut Sink<'_>) {
    if manifest.name.is_none() && manifest.short_name.is_none() {
        sink.emit(MANIFEST_NAME, Place::Field("name"), &[]);
    }
    if !manifest.start_url_declared || manifest.start_url_ignored {
        let reason = if manifest.start_url_ignored {
            "ignored"
        } else {
            "missing"
        };
        sink.emit(
            MANIFEST_START_URL,
            Place::Field("start_url"),
            &[param("reason", reason)],
        );
    }
    if !manifest.installable_display() {
        let shown = manifest.display.clone().unwrap_or_else(|| "-".to_owned());
        sink.emit(
            MANIFEST_DISPLAY,
            Place::Field("display"),
            &[param("display", shown)],
        );
    }
    let recommended: [(&'static str, bool); 5] = [
        (
            "maskable",
            manifest.icons.iter().any(|icon| icon.maskable()),
        ),
        ("theme_color", manifest.theme_color.is_some()),
        ("background_color", manifest.background_color.is_some()),
        ("id", manifest.id.is_some()),
        ("scope", manifest.scope_declared),
    ];
    for (field, present) in recommended {
        if !present {
            sink.emit(
                MANIFEST_RECOMMENDED,
                Place::Field(field),
                &[param("field", field)],
            );
        }
    }
}

/// The start URL: inside the scope, answering 200 (redirects within the origin followed).
fn start_url(
    host: &mut dyn PluginHost,
    budget: &Budget,
    page: &Url,
    manifest: &Manifest,
    report: &mut ManifestReport,
    sink: &mut Sink<'_>,
) {
    let start = &manifest.start_url;
    if !start.within(&manifest.scope) {
        let params = [param("url", start), param("scope", &manifest.scope)];
        sink.emit(START_URL_OUT_OF_SCOPE, Place::Field("start_url"), &params);
    }
    if !sink.wants(START_URL_STATUS) {
        return;
    }
    let status = match fetch::get(host, budget, page, start, ACCEPT_HTML) {
        Fetched::Ok(response) => response.status.to_string(),
        Fetched::Failed(code) if code == "time" => return,
        Fetched::Failed(code) => code,
        Fetched::OtherOrigin(_) => "other_origin".to_owned(),
    };
    if status != "200" {
        let params = [param("url", start), param("status", &status)];
        sink.emit(START_URL_STATUS, Place::Field("start_url"), &params);
    }
    report.start_url_status = Some(status);
}

/// The manifest as far as it was had: its report once a link was found, the parsed manifest
/// once it was read.
#[derive(Clone, Debug, Default)]
pub struct Loaded {
    pub report: Option<ManifestReport>,
    pub manifest: Option<Manifest>,
}

/// The manifest rules of the page `state` describes.
pub fn check(
    host: &mut dyn PluginHost,
    budget: &Budget,
    page: &Url,
    state: &PageState,
    sink: &mut Sink<'_>,
) -> Loaded {
    let Some(linked) = state.manifests.first() else {
        sink.emit(MANIFEST_MISSING, Place::Manifest, &[]);
        return Loaded::default();
    };
    let Some(url) = page.join(linked) else {
        let params = [param("url", linked), param("status", "url")];
        sink.emit(MANIFEST_UNREACHABLE, Place::Manifest, &params);
        return Loaded::default();
    };
    let mut report = ManifestReport {
        url: url.to_string(),
        ..ManifestReport::default()
    };
    let Some(text) = load(host, budget, page, &url, &mut report, sink) else {
        return Loaded {
            report: Some(report),
            manifest: None,
        };
    };
    let manifest = match Manifest::parse(&text, &url, page) {
        Ok(manifest) => manifest,
        Err(error) => {
            let shown = match error {
                ParseError::Json(message) => message,
                ParseError::NotObject => "object".to_owned(),
            };
            sink.emit(MANIFEST_INVALID, Place::Manifest, &[param("error", &shown)]);
            report.error = Some(shown);
            return Loaded {
                report: Some(report),
                manifest: None,
            };
        }
    };
    report.table.clone_from(&manifest.table);
    report.start_url = Some(manifest.start_url.to_string());
    report.scope = Some(manifest.scope.to_string());
    members(&manifest, sink);
    start_url(host, budget, page, &manifest, &mut report, sink);
    Loaded {
        report: Some(report),
        manifest: Some(manifest),
    }
}

#[cfg(test)]
#[path = "members_tests.rs"]
mod tests;
