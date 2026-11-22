//! The views of the fake.

use rb_engine::{EngineKind, ViewId, ViewInfo};

/// A view for a test: id, breakpoint, URL.
#[must_use]
pub fn view(id: u32, breakpoint: &str, url: &str) -> ViewInfo {
    ViewInfo {
        id: ViewId(id),
        label: breakpoint.to_owned(),
        breakpoint: Some(breakpoint.to_owned()),
        cell_breakpoint: Some(breakpoint.to_owned()),
        engine: EngineKind::Chromium,
        width: 375,
        height: 812,
        dpr: 3.0,
        scale: 1.0,
        url: url.to_owned(),
        title: String::new(),
        loading: false,
        session: "default".to_owned(),
        cell: None,
    }
}
