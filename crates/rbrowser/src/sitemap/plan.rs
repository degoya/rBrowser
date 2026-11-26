//! What a capture call becomes before anything is fetched: the plugin's `source` and `rewrite`
//! (the site, a sitemap, a project file, or the bookmarks; the environment the URLs move to)
//! and the breakpoints with their render views' specs. Pure but for reading a project file, so
//! every refusal is tested here.
//!
//! - `url` absent: the project's `startUrl` as the site. An `http(s)` URL whose path is `/` (or
//!   empty) is a site whose sitemaps are found; any other path names the sitemap itself. Anything
//!   else is a project-relative `.xml`, `.xml.gz` or `.txt` file inside the project, read here
//!   (the plugin has no file access) and handed over as base64, at most `max_bytes`.
//! - `env`: the environment's origin is the rewrite's `to`; `from` is the site's (the plugin's
//!   default) or, for a file, the `startUrl`'s. Bookmarks are resolved in the environment
//!   directly.
//! - Breakpoints: every Chromium breakpoint of the config, those the grid shows at the start
//!   (`views: "visible"`, `rb_config::resolve_scope`), or those named (an unknown id is
//!   refused); Firefox and WebKit breakpoints are not captured in hidden Chromium views and are
//!   named as skipped. Each gets its resolved emulation - the project's extra headers only in a
//!   trusted workspace - and its session, or the call's.

use std::collections::BTreeMap;
use std::path::Path;

use base64::Engine as _;
use rb_config::{DevicePreset, EngineKind, ProjectConfig, ScopeError, ViewScope};
use rb_engine::chromium::hidden::RenderSpec;
use rb_visual::sitemap::{RunBreakpoint, file_name};
use serde_json::{Value, json};

use super::backend::DEFAULT_SESSION;
use super::tool::CaptureArgs;
use rb_util::url::origin_of;

/// The file endings a project-relative source may have.
pub const FILE_ENDINGS: [&str; 3] = [".xml", ".xml.gz", ".txt"];

/// Why a call cannot become a run; the code is an i18n key.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PlanError {
    #[error("sitemap.error.no_source: no url, no bookmarks and no startUrl")]
    NoSource,
    #[error("sitemap.error.bad_file: {0}")]
    BadFile(String),
    #[error("sitemap.error.unknown_env: {0}")]
    UnknownEnv(String),
    #[error("sitemap.error.unknown_breakpoint: {0}")]
    UnknownBreakpoint(String),
    #[error("sitemap.error.unknown_session: {0}")]
    UnknownSession(String),
    #[error("sitemap.error.no_breakpoints: no Chromium breakpoint to capture")]
    NoBreakpoints,
    /// `views: "visible"` and the grid shows no Chromium breakpoint (RB-099-08).
    #[error(
        "sitemap.error.no_breakpoints: no visible Chromium breakpoint to capture; show one in the \
         grid or capture all views"
    )]
    NoVisibleBreakpoints,
    #[error("sitemap.error.emulation: {0}")]
    Emulation(String),
}

impl PlanError {
    /// The stable code (an i18n key).
    pub fn code(&self) -> &'static str {
        match self {
            Self::NoSource => "sitemap.error.no_source",
            Self::BadFile(_) => "sitemap.error.bad_file",
            Self::UnknownEnv(_) => "sitemap.error.unknown_env",
            Self::UnknownBreakpoint(_) => "sitemap.error.unknown_breakpoint",
            Self::UnknownSession(_) => "sitemap.error.unknown_session",
            Self::NoBreakpoints | Self::NoVisibleBreakpoints => "sitemap.error.no_breakpoints",
            Self::Emulation(_) => "sitemap.error.emulation",
        }
    }
}

/// The plugin's `source`, the label `run.json` keeps, and the rewrite's `from` when it is not
/// the site's.
#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    pub source: Value,
    pub label: String,
    pub from: Option<String>,
}

/// Whether `url` is an `http(s)` URL naming a site (no path) rather than a sitemap.
fn site_or_sitemap(url: &str) -> Option<Value> {
    let origin = origin_of(url)?;
    let rest = &url[url.find("://").map_or(0, |at| at + 3)..];
    let path = rest.find(['/', '?', '#']).map_or("", |at| &rest[at..]);
    if path.is_empty() || path == "/" {
        Some(json!({ "site": origin }))
    } else {
        Some(json!({ "sitemap": url }))
    }
}

/// Reads the project file `relative` below `root`, at most `max_bytes`.
pub fn read_project_file(
    root: &Path,
    relative: &str,
    max_bytes: usize,
) -> Result<Vec<u8>, PlanError> {
    let bad = |reason: &str| PlanError::BadFile(format!("{relative}: {reason}"));
    let lower = relative.to_ascii_lowercase();
    if !FILE_ENDINGS.iter().any(|ending| lower.ends_with(ending)) {
        return Err(bad("not a .xml, .xml.gz or .txt file"));
    }
    let path = Path::new(relative);
    if path.is_absolute()
        || path
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return Err(bad("not a path inside the project"));
    }
    let root = root
        .canonicalize()
        .map_err(|error| bad(&error.to_string()))?;
    let file = root
        .join(path)
        .canonicalize()
        .map_err(|error| bad(&error.to_string()))?;
    if !file.starts_with(&root) || !file.is_file() {
        return Err(bad("not a file inside the project"));
    }
    let length = file
        .metadata()
        .map_err(|error| bad(&error.to_string()))?
        .len();
    if usize::try_from(length).map_or(true, |length| length > max_bytes) {
        return Err(bad("larger than allowed"));
    }
    std::fs::read(&file).map_err(|error| bad(&error.to_string()))
}

/// The environment `id`'s origin.
fn env_origin(config: &ProjectConfig, id: &str) -> Result<String, PlanError> {
    config
        .environments
        .iter()
        .find(|environment| environment.id == id)
        .and_then(|environment| origin_of(&environment.url))
        .ok_or_else(|| PlanError::UnknownEnv(id.to_owned()))
}

/// The source of `args` in `config`, the project at `root`.
pub fn source(
    args: &CaptureArgs,
    config: &ProjectConfig,
    root: &Path,
    max_bytes: usize,
) -> Result<Source, PlanError> {
    if args.bookmarks {
        let urls = rb_config::bookmarks::resolve(config, args.env.as_deref());
        if urls.is_empty() {
            return Err(PlanError::NoSource);
        }
        return Ok(Source {
            source: json!({ "urls": urls }),
            label: "bookmarks".to_owned(),
            from: None,
        });
    }
    let start = config.start_url.as_deref();
    match args.url.as_deref().or(start) {
        None => Err(PlanError::NoSource),
        Some(url) if url.contains("://") => {
            let source = site_or_sitemap(url)
                .ok_or_else(|| PlanError::BadFile(format!("{url}: not an http or https URL")))?;
            Ok(Source {
                source,
                label: url.to_owned(),
                from: None,
            })
        }
        Some(file) => {
            let bytes = read_project_file(root, file, max_bytes)?;
            Ok(Source {
                source: json!({ "file": base64::engine::general_purpose::STANDARD.encode(bytes) }),
                label: file.to_owned(),
                from: start.and_then(origin_of),
            })
        }
    }
}

/// The plugin's `rewrite` for `args.env`: `None` without one, and for the bookmarks (resolved in
/// the environment already).
pub fn rewrite(
    args: &CaptureArgs,
    config: &ProjectConfig,
    source: &Source,
) -> Result<Option<Value>, PlanError> {
    let Some(env) = args.env.as_deref() else {
        return Ok(None);
    };
    let to = env_origin(config, env)?;
    if args.bookmarks {
        return Ok(None);
    }
    Ok(Some(match &source.from {
        Some(from) => json!({ "from": from, "to": to }),
        None => json!({ "to": to }),
    }))
}

/// The breakpoints of a run with the spec of each one's render view (by file name), and the
/// ids left out because another engine shows them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Breakpoints {
    pub run: Vec<RunBreakpoint>,
    pub specs: BTreeMap<String, RenderSpec>,
    pub skipped: Vec<String>,
}

/// The breakpoints of `args`' scope in `config`: those named, else every one or the visible
/// ones (`views`, RB-099-08), resolved once at the start.
pub fn breakpoints(
    args: &CaptureArgs,
    config: &ProjectConfig,
    devices: &[DevicePreset],
    trusted: bool,
) -> Result<Breakpoints, PlanError> {
    let scope = args.scope();
    let none = || match scope {
        ViewScope::Visible => PlanError::NoVisibleBreakpoints,
        _ => PlanError::NoBreakpoints,
    };
    let chosen = rb_config::resolve_scope(config, &scope).map_err(|error| match error {
        ScopeError::UnknownBreakpoint(id) => PlanError::UnknownBreakpoint(id),
        ScopeError::Empty => none(),
    })?;
    let sessions = crate::sessions::session_infos(config);
    let known = |id: &str| id == DEFAULT_SESSION || sessions.iter().any(|info| info.id == id);
    if let Some(session) = args.session.as_deref()
        && !known(session)
    {
        return Err(PlanError::UnknownSession(session.to_owned()));
    }
    let mut plan = Breakpoints::default();
    for breakpoint in &config.breakpoints {
        if !chosen.contains(&breakpoint.id) {
            continue;
        }
        if breakpoint.engine != EngineKind::Chromium {
            plan.skipped.push(breakpoint.id.clone());
            continue;
        }
        let mut emulation = rb_engine::resolve_emulation(breakpoint, &config.emulation, devices)
            .map_err(|error| PlanError::Emulation(error.to_string()))?;
        if !trusted {
            emulation.headers.clear();
        }
        let name = file_name(&breakpoint.id);
        if plan.specs.contains_key(&name) {
            continue;
        }
        let session = args
            .session
            .clone()
            .or_else(|| breakpoint.session.clone())
            .unwrap_or_else(|| DEFAULT_SESSION.to_owned());
        plan.run.push(RunBreakpoint {
            name: name.clone(),
            label: breakpoint
                .label
                .clone()
                .unwrap_or_else(|| breakpoint.id.clone()),
            width: emulation.width,
            height: emulation.height,
            dpr: emulation.dpr,
        });
        plan.specs.insert(
            name,
            RenderSpec {
                breakpoint: breakpoint.id.clone(),
                emulation,
                session,
            },
        );
    }
    if plan.run.is_empty() {
        return Err(none());
    }
    Ok(plan)
}

#[cfg(test)]
#[path = "plan_tests.rs"]
mod tests;
