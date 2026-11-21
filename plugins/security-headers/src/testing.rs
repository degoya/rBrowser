//! The tests' host: `MemoryHost` with the plugin's grants, Chromium views and the answers of a
//! fixture site whose headers pass every rule unless a test changes them.

use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, HttpResponse, ViewId, ViewInfo};
use serde_json::json;

use crate::environments::CONFIG_PATH;

pub const URL: &str = "https://example.test/";

/// The headers of a page that passes every rule.
pub fn good() -> Vec<(&'static str, &'static str)> {
    vec![
        ("date", "Fri, 20 Nov 2026 10:00:00 GMT"),
        (
            "content-security-policy",
            "default-src 'self'; script-src 'self' 'nonce-r4nd0m'; object-src 'none'; base-uri 'self'; frame-ancestors 'none'",
        ),
        ("strict-transport-security", "max-age=63072000"),
        ("x-frame-options", "DENY"),
        ("x-content-type-options", "nosniff"),
        ("referrer-policy", "strict-origin-when-cross-origin"),
        ("permissions-policy", "camera=()"),
    ]
}

/// An answer with `headers`.
pub fn response(status: u16, headers: &[(&str, &str)]) -> HttpResponse {
    HttpResponse {
        status,
        headers: headers
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect(),
        body: Vec::new(),
    }
}

/// A host granted what the manifest grants; its views hold no cookie.
pub fn host() -> MemoryHost {
    let mut host = MemoryHost::new()
        .grant(Capability::Views)
        .grant(Capability::Cdp)
        .grant(Capability::Http)
        .grant(Capability::ProjectRead)
        .grant(Capability::Findings);
    host.cdp_domains = vec!["Network".to_owned()];
    host.cdp_answer = json!({ "cookies": [] }).to_string();
    host
}

/// `url` answers 200 with `headers`.
pub fn serve(host: &mut MemoryHost, url: &str, headers: &[(&str, &str)]) {
    host.responses
        .insert(url.to_owned(), response(200, headers));
}

/// Adds a Chromium view `id` showing `url`; the first one is active.
pub fn view(host: &mut MemoryHost, id: ViewId, url: &str) {
    host.views.push(ViewInfo {
        id,
        label: format!("View {id}"),
        breakpoint: format!("bp{id}"),
        engine: "chromium".to_owned(),
        width: 1280,
        height: 800,
        dpr: 1.0,
        url: url.to_owned(),
        title: String::new(),
    });
    if host.active.is_none() {
        host.active = Some(id);
    }
}

/// The project config with `environments` as `(id, url)`.
pub fn with_environments(host: &mut MemoryHost, environments: &[(&str, &str)]) {
    let list: Vec<_> = environments
        .iter()
        .map(|(id, url)| json!({ "id": id, "url": url }))
        .collect();
    host.project.insert(
        CONFIG_PATH.to_owned(),
        json!({ "environments": list }).to_string().into_bytes(),
    );
}
