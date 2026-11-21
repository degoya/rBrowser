//! A session's login state (RB-094-08): its cookies and the `localStorage` of the pages its
//! views show, written as Playwright's `storageState` and loaded into a session or a flow.
//!
//! | Part | Where |
//! | --- | --- |
//! | The file: Playwright's `storageState`, its bounds, the CDP parameters | [`model`] |
//! | Where it lives: only under `.rbrowser/out/`, the user's only | [`paths`] |
//! | Reading and writing through the session's Chromium views | [`transfer`], [`jar`] |
//! | `session.state_export` (`Leader E W`), `session.state_import` (`Leader E I`), `omitted(secrets)` | [`defs`] |
//! | A flow's `// @storageState <file>`, loaded into its views' sessions before it runs | [`load_for_flow`], `crate::flows::run` |
//!
//! The values are secrets: they travel from CDP into the file and from the file into CDP, and
//! nowhere else - the notices and answers carry counts and the file's name, every type hides
//! them in its `Debug`. The file holds session cookies, so the export warns, and only a person
//! exports or imports one: MCP is refused both commands.

pub mod defs;
pub mod jar;
pub mod model;
pub mod paths;
pub mod transfer;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use rb_commands::CommandError;
use rb_engine::{Target, ViewHost, ViewId};
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use serde::Deserialize;
use serde_json::{Value, json};

pub use defs::{EXPORT, IMPORT, definitions, refuse_mcp, register};

use crate::control::Core;
use crate::host::Host;
use model::StorageState;

/// The notice after an export; placeholders `{session}`, `{file}`, `{cookies}`.
pub const EXPORTED_KEY: &str = "sessions.state.exported";
/// The notice after an import; placeholders `{session}`, `{file}`, `{cookies}`, `{origins}`,
/// `{skipped}`.
pub const IMPORTED_KEY: &str = "sessions.state.imported";

/// The arguments of both commands; every one optional.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Args {
    /// The session; the active view's when absent.
    #[serde(default)]
    session: Option<String>,
    /// The file relative to `.rbrowser/out/`; `state/<session>.json` when absent.
    #[serde(default)]
    file: Option<String>,
}

/// Runs `session.state_export` or `session.state_import` (registered by [`register`]): waits on
/// a thread of the runtime; on another thread (CEF's UI thread, which CDP needs to answer) it
/// starts the transfer and answers `{ "started": true }`. A notice tells the person either way.
pub fn run(core: &Core, id: &str, args: &Value) -> Result<Value, CommandError> {
    let args: Args = if args.is_null() {
        Args::default()
    } else {
        Args::deserialize(args)
            .map_err(|_| CommandError::InvalidArgs("expected { session?, file? }".to_owned()))?
    };
    let host = core.host().clone();
    let session = match args.session {
        Some(session) => session,
        None => host
            .active_view()
            .and_then(|view| host.view_session(view).ok())
            .unwrap_or_else(|| rb_config::DEFAULT_SESSION.to_owned()),
    };
    crate::sessions::require_known(&host.config(), &session)
        .map_err(|error| CommandError::InvalidArgs(error.to_string()))?;
    let root = host
        .project_root()
        .ok_or_else(|| CommandError::Failed("open a project first".to_owned()))?;
    let name = args.file.unwrap_or_else(|| paths::default_name(&session));
    let path = paths::resolve(&root, &name)
        .map_err(|error| CommandError::InvalidArgs(error.to_string()))?;
    let export = match id {
        EXPORT => true,
        IMPORT => false,
        other => return Err(CommandError::UnknownCommand(other.to_owned())),
    };
    let job = Job {
        session,
        name,
        root,
        path,
    };
    if tokio::runtime::Handle::try_current().is_err() {
        let Some(core) = core.arc() else {
            return Err(CommandError::Failed("the window is closed".to_owned()));
        };
        core.runtime().clone().spawn(async move {
            if let Err(error) = job.run(&core, export).await {
                tracing::warn!(%error, "the login state was not transferred");
                core.notify(CoreToShell::Notification(Notification {
                    level: NotificationLevel::Error,
                    message_key: "sessions.state.failed".to_owned(),
                    args: BTreeMap::from([("error".to_owned(), error.to_string())]),
                    settings_key: None,
                }));
            }
        });
        return Ok(json!({ "started": true }));
    }
    let runtime = core.runtime().clone();
    tokio::task::block_in_place(|| runtime.block_on(job.run(core, export)))
}

/// One export or import.
struct Job {
    session: String,
    name: String,
    root: std::path::PathBuf,
    path: std::path::PathBuf,
}

impl Job {
    async fn run(self, core: &Core, export: bool) -> Result<Value, CommandError> {
        let host = core.host().clone();
        let shown = jar::shown(&host, &self.session);
        let state_jar = jar::HostJar { host: host.clone() };
        let failed = |error: &dyn std::fmt::Display| CommandError::Failed(error.to_string());
        if export {
            let mut state = transfer::export(&state_jar, &shown)
                .await
                .map_err(|e| failed(&e))?;
            // The project's sites only, unless the person chose every domain (w28 review).
            if !core.user_settings().sessions.state_export_all_domains {
                let origins: Vec<String> = shown
                    .iter()
                    .filter_map(|item| item.origin.clone())
                    .collect();
                state.keep_hosts(&model::scope_hosts(&host.config(), &origins));
            }
            let bytes = serde_json::to_vec_pretty(&state).map_err(|e| failed(&e))?;
            paths::write(&self.root, &self.path, &bytes).map_err(|e| failed(&e))?;
            let cookies = state.cookies.len();
            core.notify(self.notice(
                NotificationLevel::Warning,
                EXPORTED_KEY,
                &[("cookies", cookies)],
            ));
            return Ok(
                json!({ "session": self.session, "file": self.name, "cookies": cookies,
                "origins": state.origins.len() }),
            );
        }
        let state = read_state(&self.root, &self.path)?;
        let report = transfer::import(&state_jar, &shown, &state)
            .await
            .map_err(|e| failed(&e))?;
        for item in &shown {
            if let Err(error) = host.reload(Target::View(item.view), false) {
                tracing::debug!(%error, "a view was not reloaded after the import");
            }
        }
        core.notify(self.notice(
            NotificationLevel::Info,
            IMPORTED_KEY,
            &[
                ("cookies", report.cookies),
                ("origins", report.origins),
                ("skipped", report.origins_skipped),
            ],
        ));
        Ok(
            json!({ "session": self.session, "file": self.name, "cookies": report.cookies,
            "origins": report.origins, "originsSkipped": report.origins_skipped }),
        )
    }

    fn notice(&self, level: NotificationLevel, key: &str, counts: &[(&str, usize)]) -> CoreToShell {
        let mut args = BTreeMap::from([
            ("session".to_owned(), self.session.clone()),
            ("file".to_owned(), self.name.clone()),
        ]);
        for (name, count) in counts {
            args.insert((*name).to_owned(), count.to_string());
        }
        CoreToShell::Notification(Notification {
            level,
            message_key: key.to_owned(),
            args,
            settings_key: None,
        })
    }
}

/// The state in the file at `path`, held to its bounds. The parse error is fixed text: serde
/// would name the value it could not take.
fn read_state(
    root: &std::path::Path,
    path: &std::path::Path,
) -> Result<StorageState, CommandError> {
    let text = paths::read(root, path).map_err(|error| CommandError::Failed(error.to_string()))?;
    let state: StorageState = serde_json::from_str(&text).map_err(|_| {
        CommandError::InvalidArgs("the file is no Playwright storageState".to_owned())
    })?;
    state
        .check()
        .map_err(|error| CommandError::InvalidArgs(error.to_string()))?;
    Ok(state)
}

/// Loads the state file `name` (relative to `.rbrowser/out/`, a flow's `// @storageState`) into
/// the sessions of `views` before a flow runs in them; the flow navigates itself.
///
/// # Errors
///
/// The file's or a session's failure, as text without a value.
pub async fn load_for_flow(host: &Arc<Host>, name: &str, views: &[ViewId]) -> Result<(), String> {
    let root = host
        .project_root()
        .ok_or_else(|| "open a project first".to_owned())?;
    let path = paths::resolve(&root, name).map_err(|error| error.to_string())?;
    let state = read_state(&root, &path).map_err(|error| error.to_string())?;
    let sessions: BTreeSet<String> = views
        .iter()
        .filter_map(|view| host.view_session(*view).ok())
        .collect();
    let state_jar = jar::HostJar {
        host: Arc::clone(host),
    };
    for session in sessions {
        let shown = jar::shown(host, &session);
        transfer::import(&state_jar, &shown, &state)
            .await
            .map_err(|error| format!("session {session}: {error}"))?;
    }
    Ok(())
}
