//! What the trust dialog shows of a project plugin before the person trusts it: the capabilities
//! its manifest declares and their details - CDP domains, `project-read` globs, sidecar methods,
//! the HTTP rate, `evaluate` - with the risky ones named for highlighting (review finding M1 of
//! Charge 6; `network-rules` and `console-logs` since L3 of Charge 7). Read from the package without verifying or trusting it; shown, never granted.

use std::collections::BTreeMap;
use std::path::Path;

use rb_config::trust::{BlockedItem, ProtectedKind};
use rb_ipc::PluginGrantDetails;
use rb_plugin_host::Grants;
use rb_plugin_types::Capability;

/// CDP domains that control the page or read its traffic: highlighted in the dialog. With any of
/// them - or with `evaluate` - a plugin has the page in its hands, its network included.
pub const RISKY_CDP_DOMAINS: &[&str] = &[
    "DOM", "Debugger", "Fetch", "Input", "Network", "Page", "Runtime",
];

/// Capabilities highlighted in the dialog, by id (review finding L3 of Charge 7): `network-rules`
/// mocks, blocks or rewrites every request of the views, `console-logs` reads what the pages log,
/// tokens and personal data included; `sidecar` runs Lighthouse and Playwright in Node, outside
/// the sandbox (security review RB-090-06, finding 4).
pub const RISKY_CAPABILITIES: &[Capability] = &[
    Capability::NetworkRules,
    Capability::ConsoleLogs,
    Capability::Sidecar,
];

/// The grants each project plugin package among `blocked` declares, by its item's path; `None`
/// for a package that does not read. Only real package paths below the project are opened.
pub fn declared(root: &Path, blocked: &[BlockedItem]) -> BTreeMap<String, Option<Grants>> {
    blocked
        .iter()
        .filter(|item| item.kind == ProtectedKind::Plugins)
        .filter(|item| item.path.ends_with(".rbplugin") && !item.path.contains(".."))
        .map(|item| {
            let grants = rb_plugin_host::declared_grants(&root.join(&item.path));
            (item.path.clone(), grants)
        })
        .collect()
}

/// The capability ids of each package; an unreadable one declares nothing.
pub fn capabilities(declared: &BTreeMap<String, Option<Grants>>) -> BTreeMap<String, Vec<String>> {
    declared
        .iter()
        .map(|(path, grants)| {
            let ids = grants
                .iter()
                .flat_map(|grants| grants.granted.iter())
                .map(|capability| capability.id().to_owned())
                .collect();
            (path.clone(), ids)
        })
        .collect()
}

/// The details of each package that reads.
pub fn details_of(
    declared: &BTreeMap<String, Option<Grants>>,
) -> BTreeMap<String, PluginGrantDetails> {
    declared
        .iter()
        .filter_map(|(path, grants)| Some((path.clone(), details(grants.as_ref()?))))
        .collect()
}

/// The details of `grants`, the risky parts named.
#[must_use]
pub fn details(grants: &Grants) -> PluginGrantDetails {
    let evaluate = grants.has(Capability::Evaluate);
    let cdp_domains = if grants.has(Capability::Cdp) {
        grants.cdp_domains.clone()
    } else {
        Vec::new()
    };
    let mut risky = Vec::new();
    if evaluate {
        risky.push("evaluate".to_owned());
    }
    risky.extend(
        cdp_domains
            .iter()
            .filter(|domain| RISKY_CDP_DOMAINS.contains(&domain.as_str()))
            .map(|domain| format!("cdp:{domain}")),
    );
    risky.extend(
        RISKY_CAPABILITIES
            .iter()
            .filter(|capability| grants.has(**capability))
            .map(|capability| capability.id().to_owned()),
    );
    PluginGrantDetails {
        evaluate,
        project_globs: if grants.has(Capability::ProjectRead) {
            grants.project_globs.clone()
        } else {
            Vec::new()
        },
        sidecar_methods: if grants.has(Capability::Sidecar) {
            grants.sidecar_methods.clone()
        } else {
            Vec::new()
        },
        http_rate_per_second: grants
            .has(Capability::Http)
            .then_some(grants.http_rate_per_second),
        cdp_domains,
        risky,
    }
}

#[cfg(test)]
#[path = "grants_tests.rs"]
mod tests;
