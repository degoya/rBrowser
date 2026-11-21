//! One URL checked: its document asked for once ([`crate::fetch`]), the cookies the views hold
//! for it added (CDP `Network.getCookies` in a view showing the URL's origin - the host lets it
//! name only that origin), the answer audited ([`crate::audit`]). The URL's environment is the
//! project environment of its origin.

use rb_plugin_api::{Failure, Finding, PluginHost};
use rb_util::url::origin_of;
use serde_json::json;

use crate::audit::{self, Document};
use crate::cookies::{self, Cookie};
use crate::environments::{self, Environment};
use crate::fetch::{self, Fetched};
use crate::levels::RuleLevels;
use crate::rules::{self, Issue, Scope};
use crate::settings::Settings;

/// One URL, checked.
#[derive(Clone, Debug)]
pub struct Checked {
    /// The URL asked for.
    pub url: String,
    /// The project environment of its origin.
    pub environment: Option<String>,
    pub fetched: Fetched,
    /// The cookies held to the rules: the view's, then those the answer sets.
    pub cookies: Vec<Cookie>,
    pub issues: Vec<Issue>,
}

impl Checked {
    /// The findings of the rules `enabled` asks for (all when empty), at the person's levels.
    #[must_use]
    pub fn findings(&self, enabled: &[String], levels: &RuleLevels) -> Vec<Finding> {
        let scope = Scope {
            enabled,
            levels,
            url: &self.url,
            environment: self.environment.as_deref(),
        };
        rules::findings(&self.issues, &scope)
    }
}

/// The cookies a view showing `url`'s origin holds for its page; none without such a view or
/// when the host refuses.
fn view_cookies(host: &mut dyn PluginHost, url: &str) -> Vec<Cookie> {
    let Some(origin) = origin_of(url) else {
        return Vec::new();
    };
    let Ok(views) = host.views() else {
        return Vec::new();
    };
    let Some(view) = views
        .into_iter()
        .find(|view| origin_of(&view.url).as_deref() == Some(origin.as_str()))
    else {
        return Vec::new();
    };
    let params = json!({ "urls": [view.url] }).to_string();
    host.cdp_send(view.id, "Network.getCookies", &params)
        .map(|answer| cookies::from_cdp(&answer))
        .unwrap_or_default()
}

/// `url` checked; `with_view_cookies` adds the cookies a view holds for it (the environment
/// comparison leaves them out, so every environment is read the same way).
///
/// # Errors
///
/// The document could not be fetched ([`fetch::document`]).
pub fn check_url(
    host: &mut dyn PluginHost,
    url: &str,
    environments: &[Environment],
    settings: &Settings,
    with_view_cookies: bool,
) -> Result<Checked, Failure> {
    let fetched = fetch::document(host, url)?;
    let held = if with_view_cookies {
        view_cookies(host, url)
    } else {
        Vec::new()
    };
    let cookies = cookies::merge(held, fetched.cookies.clone());
    let document = Document {
        url: fetched.final_url.clone(),
        downgrade: fetched.downgrade.clone(),
        headers: fetched.headers.clone(),
        cookies: cookies.clone(),
    };
    let issues = audit::audit(&document, settings);
    Ok(Checked {
        url: url.to_owned(),
        environment: environments::of_url(environments, url).map(|found| found.id.clone()),
        fetched,
        cookies,
        issues,
    })
}

#[cfg(test)]
#[path = "page_tests.rs"]
mod tests;
