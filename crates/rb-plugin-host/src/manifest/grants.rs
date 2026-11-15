//! `[capabilities]`: which capabilities a manifest grants, and their settings.
//!
//! New in rBrowser (`docs/plugins.md`, "Capabilities and rights"). A capability is `true`, or a
//! table for the three that take settings: `cdp = { domains = ["CSS", "DOM"] }` (required;
//! `window_image = true` lets the plugin take a view's window image without a clip, RB-080-64),
//! `http = { rate_limit_per_second = 4 }`, `project-read = { globs = ["src/**/*.css"] }`,
//! `sidecar = { methods = ["lighthouse.run"] }` (required, Charge 6). `false`
//! or a missing key grants nothing; an unknown key refuses the manifest.

use std::collections::BTreeSet;

use anyhow::{Context, Result, bail};
use globset::{Glob, GlobSet, GlobSetBuilder};
use rb_plugin_types::{Capability, paths};

/// Requests per second when `http` names none.
pub const DEFAULT_HTTP_RATE_PER_SECOND: u32 = 4;
/// The most requests per second a manifest may ask for.
pub const MAX_HTTP_RATE_PER_SECOND: u32 = 50;
/// The most globs and CDP domains a manifest may list.
const MAX_LIST: usize = 64;
/// The sidecar namespaces a plugin may be granted: the modules made for plugins (Lighthouse,
/// RB-060-07). Everything else is the core's - the sidecar itself, the Playwright install,
/// WebKit views, flows - and so is any namespace a later module adds until it is listed here
/// (review finding L4 of Charge 6).
pub const PLUGIN_SIDECAR_NAMESPACES: &[&str] = &["lighthouse"];

/// The parsed `[capabilities]`.
#[derive(Clone, Debug)]
pub struct Grants {
    pub granted: BTreeSet<Capability>,
    /// `cdp.domains`, exactly as listed.
    pub cdp_domains: Vec<String>,
    /// `cdp.window_image`: `Page.captureScreenshot` of the view's window image without a clip
    /// (RB-080-64; review S2) - the colour picker only.
    pub cdp_window_image: bool,
    /// `http.rate_limit_per_second`.
    pub http_rate_per_second: u32,
    /// `project-read.globs`, as written.
    pub project_globs: Vec<String>,
    /// `sidecar.methods`, exactly as listed.
    pub sidecar_methods: Vec<String>,
    globs: GlobSet,
}

impl Default for Grants {
    fn default() -> Self {
        Self {
            granted: BTreeSet::new(),
            cdp_domains: Vec::new(),
            cdp_window_image: false,
            http_rate_per_second: DEFAULT_HTTP_RATE_PER_SECOND,
            project_globs: Vec::new(),
            sidecar_methods: Vec::new(),
            globs: GlobSet::empty(),
        }
    }
}

impl PartialEq for Grants {
    fn eq(&self, other: &Self) -> bool {
        self.granted == other.granted
            && self.cdp_domains == other.cdp_domains
            && self.cdp_window_image == other.cdp_window_image
            && self.http_rate_per_second == other.http_rate_per_second
            && self.project_globs == other.project_globs
            && self.sidecar_methods == other.sidecar_methods
    }
}

impl Grants {
    /// Reads `[capabilities]`.
    ///
    /// # Errors
    ///
    /// An unknown capability, a value of the wrong shape, an empty CDP domain list or one naming a
    /// domain off the allowlist, a sidecar method outside the plugins' namespaces,
    /// a rate out of range, a glob that climbs out or does not compile.
    pub fn parse(table: &toml::Table) -> Result<Self> {
        let mut grants = Self::default();
        for (key, value) in table {
            let capability: Capability =
                key.parse().with_context(|| format!("capabilities.{key}"))?;
            let settings = match value {
                toml::Value::Boolean(false) => continue,
                toml::Value::Boolean(true) => None,
                toml::Value::Table(settings) => Some(settings),
                _ => bail!("capabilities.{key} must be true, false or a table"),
            };
            match capability {
                Capability::Cdp => {
                    let mut settings = settings.cloned();
                    grants.cdp_window_image = window_image(settings.as_mut())?;
                    grants.cdp_domains = cdp_domains(settings.as_ref())?;
                    if grants.cdp_window_image && !grants.cdp_domains.iter().any(|d| d == "Page") {
                        bail!("capabilities.cdp.window_image needs the domain `Page`");
                    }
                }
                Capability::Http => grants.http_rate_per_second = http_rate(settings)?,
                Capability::Sidecar => grants.sidecar_methods = sidecar_methods(settings)?,
                Capability::ProjectRead => {
                    grants.project_globs = string_list(settings, "project-read", "globs")?;
                    grants.globs = compile_globs(&grants.project_globs)?;
                }
                _ => {
                    if settings.is_some_and(|table| !table.is_empty()) {
                        bail!("capabilities.{key} takes no settings");
                    }
                }
            }
            grants.granted.insert(capability);
        }
        Ok(grants)
    }

    /// Whether `capability` is granted.
    #[must_use]
    pub fn has(&self, capability: Capability) -> bool {
        self.granted.contains(&capability)
    }

    /// Whether the manifest lists sidecar method `method` (and grants `sidecar`).
    #[must_use]
    pub fn sidecar_allows(&self, method: &str) -> bool {
        self.has(Capability::Sidecar) && self.sidecar_methods.iter().any(|listed| listed == method)
    }

    /// Whether a project-relative path (`/`-joined segments) matches a declared glob.
    #[must_use]
    pub fn glob_matches(&self, relative: &str) -> bool {
        self.globs.is_match(relative)
    }
}

fn string_list(settings: Option<&toml::Table>, capability: &str, key: &str) -> Result<Vec<String>> {
    let Some(value) = settings.and_then(|table| table.get(key)) else {
        return Ok(Vec::new());
    };
    if let Some(other) = settings.and_then(|table| table.keys().find(|name| *name != key)) {
        bail!("capabilities.{capability}.{other} is unknown");
    }
    let list = value
        .as_array()
        .with_context(|| format!("capabilities.{capability}.{key} must be a list"))?;
    if list.len() > MAX_LIST {
        bail!("capabilities.{capability}.{key} lists more than {MAX_LIST} entries");
    }
    list.iter()
        .map(|item| {
            item.as_str()
                .map(str::to_owned)
                .with_context(|| format!("capabilities.{capability}.{key} holds a non-string"))
        })
        .collect()
}

/// Takes `window_image` out of the `cdp` table: `true`, `false` or absent.
fn window_image(settings: Option<&mut toml::Table>) -> Result<bool> {
    match settings.and_then(|table| table.remove("window_image")) {
        None | Some(toml::Value::Boolean(false)) => Ok(false),
        Some(toml::Value::Boolean(true)) => Ok(true),
        Some(_) => bail!("capabilities.cdp.window_image must be true or false"),
    }
}

fn cdp_domains(settings: Option<&toml::Table>) -> Result<Vec<String>> {
    let domains = string_list(settings, "cdp", "domains")?;
    if domains.is_empty() {
        bail!("capabilities.cdp needs `domains`, the CDP domains the plugin uses");
    }
    for domain in &domains {
        if domain.is_empty() || !domain.chars().all(|c| c.is_ascii_alphanumeric()) {
            bail!("capabilities.cdp.domains: `{domain}` is no CDP domain");
        }
        // The domains of the method allowlist only (review finding H3 of Charge 6).
        if !crate::cdp_policy::domain_allowed(domain) {
            bail!("capabilities.cdp.domains: `{domain}` is not a domain plugins may use");
        }
    }
    Ok(domains)
}

fn sidecar_methods(settings: Option<&toml::Table>) -> Result<Vec<String>> {
    let methods = string_list(settings, "sidecar", "methods")?;
    if methods.is_empty() {
        bail!("capabilities.sidecar needs `methods`, the sidecar methods the plugin calls");
    }
    for method in &methods {
        let parts: Vec<&str> = method.split('.').collect();
        let well_formed = parts.len() >= 2
            && method.len() <= 64
            && parts.iter().all(|part| {
                !part.is_empty() && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            });
        if !well_formed {
            bail!("capabilities.sidecar.methods: `{method}` is no `namespace.method`");
        }
        if !PLUGIN_SIDECAR_NAMESPACES.contains(&parts[0]) {
            bail!("capabilities.sidecar.methods: `{method}` is not in a namespace for plugins");
        }
    }
    Ok(methods)
}

fn http_rate(settings: Option<&toml::Table>) -> Result<u32> {
    let Some(settings) = settings else {
        return Ok(DEFAULT_HTTP_RATE_PER_SECOND);
    };
    if let Some(other) = settings
        .keys()
        .find(|name| *name != "rate_limit_per_second")
    {
        bail!("capabilities.http.{other} is unknown");
    }
    let Some(value) = settings.get("rate_limit_per_second") else {
        return Ok(DEFAULT_HTTP_RATE_PER_SECOND);
    };
    let rate = value
        .as_integer()
        .and_then(|rate| u32::try_from(rate).ok())
        .context("capabilities.http.rate_limit_per_second must be a whole number")?;
    if rate == 0 || rate > MAX_HTTP_RATE_PER_SECOND {
        bail!("capabilities.http.rate_limit_per_second must be 1 to {MAX_HTTP_RATE_PER_SECOND}");
    }
    Ok(rate)
}

fn compile_globs(globs: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for glob in globs {
        // A glob is a path pattern: it obeys the path rule, so it can never name `..` or a
        // drive, and it may not reach into `.rbrowser/` beyond what the capability gives anyway.
        paths::relative_segments(glob).map_err(|refusal| {
            anyhow::anyhow!("capabilities.project-read.globs: `{glob}`: {refusal}")
        })?;
        if glob.contains('\\') {
            bail!("capabilities.project-read.globs: `{glob}` must use `/`");
        }
        let compiled = Glob::new(glob)
            .with_context(|| format!("capabilities.project-read.globs: `{glob}`"))?;
        builder.add(compiled);
    }
    builder
        .build()
        .context("capabilities.project-read.globs do not compile")
}

#[cfg(test)]
#[path = "grants_tests.rs"]
mod tests;
