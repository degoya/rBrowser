//! The page's requests grouped by endpoint (RB-096-06): method, origin and the path with its ids
//! as placeholders ([`crate::template`]). Nothing is asked of the network - the host's capture
//! is all there is. Pure: the summary of a group is [`crate::endpoint_summary`], the check rules
//! [`crate::endpoint_rules`].

use std::collections::{BTreeMap, BTreeSet};

use rb_plugin_api::ViewId;

use crate::calls::Call;
use crate::template::{Template, split, templated};

/// Which calls count.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    /// Resource types (`Fetch`, `XHR`); every type when empty.
    pub types: BTreeSet<String>,
    /// The person's path patterns.
    pub templates: Vec<Template>,
    /// One view only.
    pub view: Option<ViewId>,
    /// Only calls of the current page of each view.
    pub current_only: bool,
    /// Lower-case text the endpoint's key contains.
    pub search: Option<String>,
}

/// One endpoint and its calls, oldest first.
#[derive(Clone, Debug, PartialEq)]
pub struct Endpoint<'a> {
    /// `GET https://a.test/api/users/{id}`: what tells two endpoints apart.
    pub key: String,
    pub method: String,
    pub origin: String,
    pub path: String,
    pub calls: Vec<&'a Call>,
}

/// The method (upper case), origin and templated path of a call to `url`; `None` for a URL
/// that is no `http(s)` one.
#[must_use]
pub fn key_of(method: &str, url: &str, own: &[Template]) -> Option<(String, String, String)> {
    let (origin, path) = split(url)?;
    Some((method.to_ascii_uppercase(), origin, templated(&path, own)))
}

/// An endpoint's key.
#[must_use]
pub fn endpoint_key(method: &str, origin: &str, path: &str) -> String {
    format!("{method} {origin}{path}")
}

impl Options {
    fn passes(&self, call: &Call) -> bool {
        (self.types.is_empty() || self.types.contains(&call.resource_type))
            && self.view.is_none_or(|view| view == call.view)
            && (!self.current_only || call.current)
    }
}

/// The endpoints of `calls` that `options` passes, in the order they were first called.
#[must_use]
pub fn group<'a>(calls: &'a [Call], options: &Options) -> Vec<Endpoint<'a>> {
    let mut out: Vec<Endpoint<'a>> = Vec::new();
    let mut index: BTreeMap<String, usize> = BTreeMap::new();
    for call in calls.iter().filter(|call| options.passes(call)) {
        let Some((method, origin, path)) = key_of(&call.method, &call.url, &options.templates)
        else {
            continue;
        };
        let key = endpoint_key(&method, &origin, &path);
        if let Some(search) = &options.search
            && !key.to_lowercase().contains(search.as_str())
        {
            continue;
        }
        let at = *index.entry(key.clone()).or_insert_with(|| {
            out.push(Endpoint {
                key,
                method,
                origin,
                path,
                calls: Vec::new(),
            });
            out.len() - 1
        });
        out[at].calls.push(call);
    }
    out
}

/// The calls of `endpoint` by view.
#[must_use]
pub fn by_view<'a>(endpoint: &Endpoint<'a>) -> BTreeMap<ViewId, Vec<&'a Call>> {
    let mut out: BTreeMap<ViewId, Vec<&'a Call>> = BTreeMap::new();
    for call in endpoint.calls.iter().copied() {
        out.entry(call.view).or_default().push(call);
    }
    out
}

#[cfg(test)]
#[path = "endpoints_tests.rs"]
pub(crate) mod tests;
