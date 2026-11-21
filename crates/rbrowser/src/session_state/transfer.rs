//! Reading and writing a session's login state through its Chromium views: the cookies once,
//! through any view of the session (CEF's request context is the session's jar), and the
//! `localStorage` of each origin a view of the session shows, through that view. Import holds
//! the state to its bounds first ([`StorageState::check`]); an origin no view shows is counted,
//! not written. Errors are fixed text: a CDP answer never reaches a notice.

use async_trait::async_trait;
use rb_engine::ViewId;
use serde_json::Value;

use super::model::{NameValue, OriginState, StateError, StorageState};
use crate::cookies::model::ChromeCookie;

/// What import and export need of a session's views.
#[async_trait]
pub trait StateJar: Send + Sync {
    /// Every cookie of `view`'s session.
    async fn cookies(&self, view: ViewId) -> Result<Vec<ChromeCookie>, String>;
    /// The `localStorage` of `origin`, which `view` shows.
    async fn local_storage(&self, view: ViewId, origin: &str) -> Result<Vec<NameValue>, String>;
    /// `Network.setCookies` with `params` in `view`'s session.
    async fn set_cookies(&self, view: ViewId, params: Value) -> Result<(), String>;
    /// One `localStorage` item of `origin`, which `view` shows.
    async fn set_item(&self, view: ViewId, origin: &str, item: &NameValue) -> Result<(), String>;
}

/// A Chromium view of the session and the origin its page has (`None`: none, `about:blank`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shown {
    pub view: ViewId,
    pub origin: Option<String>,
}

/// Why a transfer failed. Never carries a value.
#[derive(Debug, thiserror::Error)]
pub enum TransferError {
    /// The session has no Chromium view to read or write through.
    #[error("the session has no Chromium view; open one of its breakpoints first")]
    NoView,
    /// The file is out of bounds.
    #[error("{0}")]
    State(#[from] StateError),
    /// CDP refused a step.
    #[error("the browser refused to {0}")]
    Cdp(&'static str),
}

/// What an import did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImportReport {
    /// Cookies handed to the session.
    pub cookies: usize,
    /// Origins whose `localStorage` was written.
    pub origins: usize,
    /// Origins no view of the session shows: not written.
    pub origins_skipped: usize,
}

/// The login state of the session whose Chromium views are `shown`.
///
/// # Errors
///
/// [`TransferError::NoView`]; [`TransferError::Cdp`] when the cookies cannot be read.
pub async fn export(jar: &dyn StateJar, shown: &[Shown]) -> Result<StorageState, TransferError> {
    let first = shown.first().ok_or(TransferError::NoView)?;
    let cookies = jar
        .cookies(first.view)
        .await
        .map_err(|_| TransferError::Cdp("read the cookies"))?;
    let mut origins: Vec<OriginState> = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    for item in shown {
        let Some(origin) = item.origin.as_deref() else {
            continue;
        };
        if seen.contains(&origin) {
            continue;
        }
        seen.push(origin);
        match jar.local_storage(item.view, origin).await {
            Ok(local_storage) => origins.push(OriginState {
                origin: origin.to_owned(),
                local_storage,
            }),
            Err(_) => tracing::debug!(view = %item.view, "an origin's storage was not read"),
        }
    }
    Ok(StorageState::from_chrome(&cookies, origins))
}

/// Writes `state` into the session whose Chromium views are `shown`.
///
/// # Errors
///
/// [`TransferError::NoView`], [`TransferError::State`] before anything is written,
/// [`TransferError::Cdp`] when a step is refused.
pub async fn import(
    jar: &dyn StateJar,
    shown: &[Shown],
    state: &StorageState,
) -> Result<ImportReport, TransferError> {
    let first = shown.first().ok_or(TransferError::NoView)?;
    state.check()?;
    let mut report = ImportReport::default();
    if !state.cookies.is_empty() {
        jar.set_cookies(first.view, state.set_cookies_params())
            .await
            .map_err(|_| TransferError::Cdp("set the cookies"))?;
        report.cookies = state.cookies.len();
    }
    for origin in &state.origins {
        let view = shown
            .iter()
            .find(|item| item.origin.as_deref() == Some(origin.origin.as_str()));
        let Some(view) = view else {
            report.origins_skipped += 1;
            continue;
        };
        for item in &origin.local_storage {
            jar.set_item(view.view, &origin.origin, item)
                .await
                .map_err(|_| TransferError::Cdp("set a localStorage item"))?;
        }
        report.origins += 1;
    }
    Ok(report)
}

#[cfg(test)]
#[path = "transfer_tests.rs"]
mod tests;
