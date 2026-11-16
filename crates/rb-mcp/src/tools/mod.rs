//! The core tools, by area. Every tool that acts runs a command of the one registry
//! (`Backend::run_command`); every view tool takes `target`: `"all"`, a view id or a breakpoint
//! id ([`crate::target`]).

pub mod app;
pub mod bookmarks;
pub mod capture;
pub mod cdp;
pub mod config;
pub mod cookies;
pub mod devtools;
pub mod engines;
pub mod env_compare;
pub mod extensions;
pub mod flows;
pub mod force_state;
pub mod forward;
pub mod layout;
pub mod logs;
pub mod metrics;
pub mod network_debug;
pub mod page;
pub mod page_zoom;
pub mod panels;
pub mod plugins;
pub mod portal;
pub mod project;
pub mod recorder;
pub mod sitemap;
pub mod status;
pub mod sync;
pub mod video;
pub mod views;
pub mod views_edit;
pub mod visual;

use serde_json::{Map, Value};

/// `fields` as a JSON object without the `None`s.
pub(crate) fn object(fields: impl IntoIterator<Item = (&'static str, Option<Value>)>) -> Value {
    Value::Object(
        fields
            .into_iter()
            .filter_map(|(key, value)| value.map(|value| (key.to_owned(), value)))
            .collect::<Map<String, Value>>(),
    )
}
