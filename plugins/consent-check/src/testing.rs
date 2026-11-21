//! The tests' fixtures: the acceptance page `https://shop.example.test/`, which loads a script
//! of a second site (`cdn.tracker.test`) and sets a cookie kept for a year before any consent;
//! after "Reject all" it still loads a pixel of a third site and sets one more cookie. The
//! host's probe capture (`$consent`), the console feed of a view (`$console`), the collector's
//! answer and a `MemoryHost` with the plugin's grants.

use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, ViewId, ViewInfo};
use serde_json::{Value, json};

use crate::capture::PROBE_KEY;

pub const PAGE: &str = "https://shop.example.test/";
/// A cookie value no answer, finding or export may carry.
pub const SECRET: &str = "secret-value-123";
/// The capture's time, seconds since the epoch.
pub const NOW: f64 = 1_790_000_000.0;
/// A year in seconds.
pub const YEAR: f64 = 365.0 * 86_400.0;

/// The acceptance capture of the probe session, with the phase after "Reject all" when
/// `rejected`.
pub fn probe_args(rejected: bool) -> Value {
    let mut consent = json!({
        "url": PAGE,
        "capturedAt": NOW,
        "session": { "fresh": true, "discarded": true },
        "banner": { "cmp": "onetrust", "visible": true, "rejected": rejected },
        "before": {
            "requests": [
                { "url": PAGE, "resourceType": "Document" },
                { "url": "https://static.example.test/site.css", "resourceType": "Stylesheet" },
                { "url": "https://cdn.tracker.test/t.js", "resourceType": "Script" },
            ],
            "cookies": [
                { "name": "_track", "value": SECRET, "domain": ".shop.example.test",
                  "expires": NOW + YEAR, "session": false },
            ],
            "storage": [],
        },
    });
    if rejected {
        consent["afterReject"] = json!({
            "requests": [
                { "url": "https://shop.example.test/after.js", "resourceType": "Script" },
                { "url": "https://pixel.ads.test/p.gif?uid=42", "resourceType": "Image" },
            ],
            "cookies": [
                { "name": "_track", "value": SECRET, "domain": ".shop.example.test",
                  "expires": NOW + YEAR, "session": false },
                { "name": "_ads", "value": SECRET, "domain": ".ads.test", "expires": -1,
                  "session": true },
                { "name": "OptanonConsent", "value": SECRET, "domain": ".shop.example.test",
                  "expires": NOW + YEAR, "session": false },
            ],
            "storage": ["consent-state"],
        });
    }
    json!({ PROBE_KEY: consent })
}

fn entry(url: &str, kind: &str, time: f64) -> Value {
    json!({ "id": format!("r{time}"), "url": url, "method": "GET", "resourceType": kind,
            "time": time, "fromCache": false })
}

/// The console feed of view `id`: one request of the document before, then the acceptance
/// page's.
pub fn feed_args(id: ViewId) -> Value {
    json!({ "$console": { "views": [{
        "view": id,
        "navigatedAt": 1_000.0,
        "network": [
            entry("https://old-page.test/x.js", "Script", 500.0),
            entry(PAGE, "Document", 1_001.0),
            entry("https://static.example.test/site.css", "Stylesheet", 1_002.0),
            entry("https://cdn.tracker.test/t.js", "Script", 1_003.0),
        ],
    }] } })
}

/// The collector's answer for a page at `url` with the storage `keys` and the banner of `cmp`.
pub fn collected(url: &str, keys: &[&str], cmp: Option<&str>) -> String {
    json!({
        "url": url, "storage": keys, "storageCut": false,
        "banner": { "cmp": cmp, "visible": cmp.is_some(), "rejectable": cmp.is_some() },
        "clicked": false,
    })
    .to_string()
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

/// A host granted what the manifest grants, with view 1 on the acceptance page, its cookie and
/// the collector's answer.
pub fn host() -> MemoryHost {
    let mut host = MemoryHost::new()
        .grant(Capability::Views)
        .grant(Capability::Evaluate)
        .grant(Capability::Cdp)
        .grant(Capability::Findings)
        .grant(Capability::OutWrite);
    host.cdp_domains = vec!["Network".to_owned()];
    host.now = NOW as u64;
    view(&mut host, 1, PAGE);
    host.cdp_answer = json!({ "cookies": [
        { "name": "_track", "value": SECRET, "domain": ".shop.example.test", "path": "/",
          "expires": NOW + YEAR, "session": false },
    ] })
    .to_string();
    host.evaluate_answer = collected(PAGE, &[], Some("onetrust"));
    host
}
