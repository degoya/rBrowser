//! What the native tests share: a host with the plugin's grants and three views.

use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, ViewInfo};
use serde_json::Value;

use crate::state::{STATE_FILE, STATE_PATH};

pub const CDP_METHOD: &str = "Emulation.setEmulatedVisionDeficiency";

fn view(id: u32, label: &str, engine: &str) -> ViewInfo {
    ViewInfo {
        id,
        label: label.to_owned(),
        breakpoint: label.to_lowercase(),
        engine: engine.to_owned(),
        width: 390,
        height: 844,
        dpr: 1.0,
        url: "http://localhost:3000/".to_owned(),
        title: "Fixture".to_owned(),
    }
}

/// The manifest's grants; views 1 and 2 are Chromium, view 3 Firefox; view 1 is active.
pub fn host() -> MemoryHost {
    let mut host = MemoryHost::new()
        .grant(Capability::Views)
        .grant(Capability::Cdp)
        .grant(Capability::ProjectRead)
        .grant(Capability::OutWrite);
    host.cdp_domains = vec!["Emulation".to_owned()];
    host.views = vec![
        view(1, "Mobile", "chromium"),
        view(2, "Desktop", "chromium"),
        view(3, "Firefox", "firefox"),
    ];
    host.active = Some(1);
    host
}

/// `host` with the state file written by an earlier call.
pub fn with_saved(host: MemoryHost, saved: &MemoryHost) -> MemoryHost {
    let bytes = saved.out.get(STATE_FILE).cloned().expect("written");
    host.with_file(STATE_PATH, &bytes)
}

/// The CDP `type`s sent, as `(view, type)`.
pub fn emulated(host: &MemoryHost) -> Vec<(u32, String)> {
    host.cdp_sent
        .iter()
        .filter(|(_, method, _)| method == CDP_METHOD)
        .map(|(view, _, params)| {
            let params: Value = serde_json::from_str(params).expect("JSON params");
            (
                *view,
                params["type"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect()
}

pub fn json(text: &str) -> Value {
    serde_json::from_str(text).expect("JSON answer")
}
