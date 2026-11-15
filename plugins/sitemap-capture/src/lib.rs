//! Sitemap capture (RB-050-05): the plugin half - finding, reading and selecting a site's pages,
//! and the gallery of a run.
//!
//! **Where the work happens.** The plugin reads what comes from outside: `robots.txt`, the
//! sitemaps and their indexes (gzip included) over the host's `http` capability - rate-limited,
//! `http(s)` only, the answer capped, a redirect answered and never followed by the host (this
//! plugin follows it within its origin), no fetch and no page on this machine or the local
//! network unless the site is local, the pages held to the site's origin - and
//! picks the pages (patterns, languages, `lastmod`, sampling, the environment rewrite). Parsing
//! untrusted XML inside WebAssembly keeps a parser bug away from the app. The **core** keeps
//! what needs a browser or the disk: the hidden render views with each breakpoint's emulation
//! (CEF, `rb_engine::chromium::hidden`), the capture, `run.json` and the PNGs below
//! `.rbrowser/out/sitemap-capture/<run>/` (`rb_visual::sitemap_store`, no link followed), the
//! baseline comparison and the run's progress, resume and cancel. A run lives longer than one
//! plugin call (a fresh store each, a time limit), so it cannot live here. The gallery is made
//! here from `run.json` and written by the core into the run's folder, so the plugin needs no
//! write capability.
//!
//! The tools (`sitemapcapture_discover`, `sitemapcapture_gallery`; the core calls them too):
//!
//! - `discover`: `{ "source": { "site" | "sitemap": URL } | { "file": base64 } | { "urls": [..] },
//!   "limits": Limits, "selection": Selection, "rewrite": { "from"?, "to" } }` answers
//!   `{ "site", "pages": [Page], "counts": Counts, "sitemaps": [SitemapReport], "skipped",
//!   "foreign", "truncated" }`.
//! - `gallery`: `{ "run": run.json, "labels": { name: text } }` answers `{ "html" }`.
//!
//! The logic is tested natively against `rb_plugin_api::test_support::MemoryHost`; `guest.rs`
//! translates it into the WIT vocabulary and exists on `wasm32` alone.

#[cfg(target_arch = "wasm32")]
mod guest;

pub mod discover;
pub mod gallery;
pub mod gzip;
pub mod robots;
pub mod select;
pub mod sitemap;
pub mod url;

use std::collections::BTreeMap;

use base64::Engine as _;
use rb_plugin_api::feature::Feature;
use rb_plugin_api::{Failure, FailureKind, PluginHost, codes};
use serde::Deserialize;
use serde_json::{Value, json};

use discover::{Limits, Source, discover};
use select::{Rewrite, Selection, select};
use url::Url;

/// The plugin's id, as in `manifest.toml`.
pub const PLUGIN_ID: &str = "sitemap-capture";
/// The tool that finds and selects the pages.
pub const DISCOVER: &str = "discover";
/// The tool that renders the gallery.
pub const GALLERY: &str = "gallery";

/// `plugin.sitemap-capture.errors.<name>`.
#[must_use]
pub fn error_key(name: &str) -> String {
    format!("plugin.{PLUGIN_ID}.errors.{name}")
}

fn bad_args(reason: &str) -> Failure {
    Failure::coded(FailureKind::BadData, &error_key("bad_args"))
        .with_param("reason", reason.chars().take(200).collect::<String>())
}

/// Where the pages come from: exactly one field.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SourceArgs {
    pub site: Option<String>,
    pub sitemap: Option<String>,
    pub file: Option<String>,
    pub urls: Option<Vec<String>>,
}

/// The arguments of `discover`.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DiscoverArgs {
    pub source: SourceArgs,
    pub limits: Limits,
    pub selection: Selection,
    pub rewrite: Option<Rewrite>,
}

/// The arguments of `gallery`.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct GalleryArgs {
    pub run: gallery::Run,
    pub labels: BTreeMap<String, String>,
}

fn url_arg(text: &str) -> Result<Url, Failure> {
    Url::parse(text).ok_or_else(|| {
        Failure::coded(FailureKind::BadData, &error_key("bad_url"))
            .with_param("url", text.chars().take(200).collect::<String>())
    })
}

impl SourceArgs {
    fn source(&self) -> Result<Source, Failure> {
        match (&self.site, &self.sitemap, &self.file, &self.urls) {
            (Some(site), None, None, None) => Ok(Source::Auto(url_arg(site)?)),
            (None, Some(sitemap), None, None) => Ok(Source::Sitemap(url_arg(sitemap)?)),
            (None, None, Some(file), None) => base64::engine::general_purpose::STANDARD
                .decode(file.as_bytes())
                .map(Source::File)
                .map_err(|_| bad_args("source.file is no base64")),
            (None, None, None, Some(urls)) => Ok(Source::List(urls.clone())),
            _ => Err(bad_args(
                "source needs exactly one of site, sitemap, file, urls",
            )),
        }
    }
}

fn parse<T: for<'de> Deserialize<'de> + Default>(args: &str) -> Result<T, Failure> {
    if args.trim().is_empty() {
        return Ok(T::default());
    }
    serde_json::from_str(args).map_err(|error| bad_args(&error.to_string()))
}

/// `discover`: the pages of the source as the selection keeps them.
///
/// # Errors
///
/// Unreadable arguments, a bad URL or a pattern, date or rewrite that cannot be used.
pub fn run_discover(host: &mut dyn PluginHost, args: &str) -> Result<Value, Failure> {
    let args: DiscoverArgs = parse(args)?;
    let source = args.source.source()?;
    let robots = args.selection.respect_robots;
    let found = discover(host, &source, args.limits, robots);
    host.log(
        "info",
        &format!(
            "{} pages in {} sitemaps",
            found.entries.len(),
            found.sitemaps.len()
        ),
    );
    let (pages, counts) =
        select(&found, &args.selection, args.rewrite.as_ref()).map_err(|error| {
            Failure::coded(FailureKind::BadData, &error_key(error.code))
                .with_param("pattern", error.pattern)
        })?;
    Ok(json!({
        "site": found.site.map(|site| site.origin()),
        "pages": pages,
        "counts": counts,
        "sitemaps": found.sitemaps,
        "skipped": found.skipped,
        "foreign": found.foreign,
        "truncated": found.truncated,
    }))
}

/// `gallery`: the HTML of a run.
///
/// # Errors
///
/// Unreadable arguments.
pub fn run_gallery(args: &str) -> Result<Value, Failure> {
    let args: GalleryArgs = parse(args)?;
    Ok(json!({ "html": gallery::render(&args.run, &args.labels) }))
}

/// The plugin.
pub struct SitemapCapture;

fn unknown(name: &str) -> Failure {
    Failure::coded(FailureKind::NotFound, codes::UNKNOWN_CONTRIBUTION).with_param("name", name)
}

fn answer(value: Result<Value, Failure>) -> Result<String, Failure> {
    value.map(|value| value.to_string())
}

impl Feature for SitemapCapture {
    fn run_command(
        &mut self,
        _host: &mut dyn PluginHost,
        command: &str,
        _args: &str,
    ) -> Result<String, Failure> {
        Err(unknown(command))
    }

    fn call_tool(
        &mut self,
        host: &mut dyn PluginHost,
        tool: &str,
        args: &str,
    ) -> Result<String, Failure> {
        match tool {
            DISCOVER => answer(run_discover(host, args)),
            GALLERY => answer(run_gallery(args)),
            other => Err(unknown(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
