//! [`HostJar`]: the session's views of one window as a [`StateJar`] - the cookies through CDP's
//! `Network.getAllCookies` / `Network.setCookies` (they act on the view's CEF request context,
//! so on its session), `localStorage` through `DOMStorage` of the origin the view shows. No
//! script runs in the page.

use std::sync::Arc;

use async_trait::async_trait;
use rb_config::EngineKind;
use rb_engine::{ViewHost, ViewId};
use serde_json::{Value, json};

use super::model::NameValue;
use super::transfer::{Shown, StateJar};
use crate::cookies::model::{ChromeCookie, cookies_of};
use crate::host::Host;

/// The views of `host`'s window.
pub struct HostJar {
    /// The window.
    pub host: Arc<Host>,
}

/// The Chromium views of `session` in `host`'s window, grid order, with their pages' origins.
pub fn shown(host: &Host, session: &str) -> Vec<Shown> {
    host.views()
        .into_iter()
        .filter(|view| view.engine == EngineKind::Chromium && view.session == session)
        .map(|view| Shown {
            view: view.id,
            origin: rb_util::url::origin_of(&view.url),
        })
        .collect()
}

fn storage_id(origin: &str) -> Value {
    json!({ "securityOrigin": origin, "isLocalStorage": true })
}

#[async_trait]
impl StateJar for HostJar {
    async fn cookies(&self, view: ViewId) -> Result<Vec<ChromeCookie>, String> {
        self.host
            .cdp_send(view, "Network.getAllCookies", json!({}))
            .await
            .map(|answer| cookies_of(&answer))
            .map_err(|error| error.to_string())
    }

    async fn local_storage(&self, view: ViewId, origin: &str) -> Result<Vec<NameValue>, String> {
        let answer = self
            .host
            .cdp_send(
                view,
                "DOMStorage.getDOMStorageItems",
                json!({ "storageId": storage_id(origin) }),
            )
            .await
            .map_err(|error| error.to_string())?;
        let entries = answer
            .get("entries")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default();
        Ok(entries
            .iter()
            .filter_map(|entry| match entry.as_array().map(Vec::as_slice) {
                Some([Value::String(name), Value::String(value)]) => Some(NameValue {
                    name: name.clone(),
                    value: value.clone(),
                }),
                _ => None,
            })
            .collect())
    }

    async fn set_cookies(&self, view: ViewId, params: Value) -> Result<(), String> {
        self.host
            .cdp_send(view, "Network.setCookies", params)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    async fn set_item(&self, view: ViewId, origin: &str, item: &NameValue) -> Result<(), String> {
        let params =
            json!({ "storageId": storage_id(origin), "key": item.name, "value": item.value });
        self.host
            .cdp_send(view, "DOMStorage.setDOMStorageItem", params)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}
