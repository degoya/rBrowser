//! Firefox in the app (RB-040-02): [`FirefoxBridge`] builds the engine from the settings - the
//! installed Firefox (`engines.firefox.path`, else found), rBrowser's profile
//! (`engines.firefox.profileDir`, else `firefox\profiles\<project>-<hash>` in rBrowser's local
//! folder - never in the project tree, see [`profile_dir`]), the extra prefs and the sync
//! agent - and plugs it into the external windows and the SyncHub's router. Without Firefox the
//! Firefox views say "Firefox was not found" with a link to the setting, and Chromium keeps
//! showing them; nothing else stops.
//!
//! | Part | Where |
//! | --- | --- |
//! | BiDi client, launcher, profile, `FirefoxEngine`, `FirefoxReplay` | `rb_bidi` |
//! | Settings, the engine's (re)build, the command `engine.firefox.toggle` | here |
//! | Firefox as a sync source, the grid stand-in following Firefox and back | [`follow`] |
//! | Each view's emulation (trust-gated headers) and the agents' configuration kept in step | [`live`] |
//! | The installed editions for the settings field, MCP `engines_list` and the command line (RB-080-32) | [`installed`] |

mod follow;
pub mod installed;
mod live;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};

use rb_bidi::firefox::{InstalledFirefox, Launch};
use rb_bidi::launcher::LocateError;
use rb_bidi::{FirefoxConfig, FirefoxEngine};
use rb_commands::CommandError;
use rb_config::EngineKind;
use rb_engine::external::{ExternalWindows, RestartError};
use rb_engine::unsupported::UnsupportedEngine;
use rb_engine::{Engine, ViewHost};
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use rb_sync::{AgentConfig, NoReplay};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::runtime::Handle;
use tokio::task::JoinHandle;

use super::HostEngines;
use crate::cdp::lock;
use crate::control::Core;
use crate::host::Host;

/// `Leader R F`: start or stop the Firefox windows (`{ "enabled"?: bool }`, a flip without).
/// MCP: covered by `engine_restart`.
pub const TOGGLE: &str = "engine.firefox.toggle";
/// The setting the "not found" notice links to.
pub const PATH_KEY: &str = "engines.firefox.path";

/// How Firefox is started: what the settings say, resolved. A change rebuilds the engine.
#[derive(Clone, Debug, PartialEq)]
pub struct Launching {
    /// The executable, or why there is none (for the user).
    pub executable: Result<PathBuf, String>,
    /// The profile folder.
    pub profile: Option<PathBuf>,
    /// `engines.firefox.extraPrefs`.
    pub extra_prefs: BTreeMap<String, Value>,
    /// No tabs or address bar: the views sit in their cells (`engines.external.embed`,
    /// RB-080-39). A change rebuilds the engine like any other, so the profile's pref follows.
    pub hide_chrome: bool,
    /// The factor every window renders CSS px to device px at, so the views fit their cells
    /// (`layout.css.devPixelsPerPx`, RB-080-40); `None` the display's own ratio. One per
    /// profile, read at start: a change rebuilds the engine.
    pub render_scale: Option<f64>,
}

/// Firefox for one window.
pub struct FirefoxBridge {
    me: Weak<FirefoxBridge>,
    core: Weak<Core>,
    host: Weak<Host>,
    runtime: Handle,
    windows: Mutex<Weak<ExternalWindows>>,
    state: Mutex<State>,
    notified: AtomicBool,
    /// One emulation update at a time ([`live`]).
    emulating: tokio::sync::Mutex<()>,
}

struct State {
    launching: Launching,
    engine: Arc<dyn Engine>,
    firefox: Option<Arc<FirefoxEngine>>,
    off: bool,
    sync_source: bool,
    tasks: Vec<JoinHandle<()>>,
    /// The engine switched off last, perhaps still quitting: the next one waits for it.
    retired: Option<Arc<FirefoxEngine>>,
}

fn registry() -> &'static Mutex<Vec<(Weak<Host>, Arc<FirefoxBridge>)>> {
    static REGISTRY: Mutex<Vec<(Weak<Host>, Arc<FirefoxBridge>)>> = Mutex::new(Vec::new());
    &REGISTRY
}

/// The Firefox of `host`'s window, once it is attached.
pub fn for_host(host: &Host) -> Option<Arc<FirefoxBridge>> {
    let mut entries = lock(registry());
    entries.retain(|(weak, _)| weak.strong_count() > 0);
    entries
        .iter()
        .find(|(weak, _)| std::ptr::eq(weak.as_ptr(), host))
        .map(|(_, bridge)| bridge.clone())
}

/// The Firefox of `core`'s window, before its external windows start ([`FirefoxBridge::engine`]
/// goes into them). Safe outside the runtime: nothing is spawned yet.
pub fn prepare(core: &Core) -> Arc<FirefoxBridge> {
    let sync_source = core.user_settings().engines.firefox.sync_source;
    let launching = launching(core);
    let agent = live::agent_config(core.host(), sync_source);
    let host = Arc::downgrade(core.host());
    let (engine, firefox) = engine_of(&launching, core.runtime(), agent, &host);
    Arc::new_cyclic(|me| FirefoxBridge {
        me: me.clone(),
        core: core.arc().as_ref().map(Arc::downgrade).unwrap_or_default(),
        host,
        runtime: core.runtime().clone(),
        windows: Mutex::new(Weak::new()),
        state: Mutex::new(State {
            launching,
            engine,
            firefox,
            off: false,
            sync_source,
            tasks: Vec::new(),
            retired: None,
        }),
        notified: AtomicBool::new(false),
        emulating: tokio::sync::Mutex::new(()),
    })
}

/// The settings changed: a new executable, profile or prefs rebuild the engine; the agents get
/// the sync configuration.
pub fn on_config_changed(core: &Core) {
    if let Some(bridge) = for_host(core.host()) {
        bridge.apply(core);
    }
}

/// The arguments of `engine.firefox.toggle`.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToggleArgs {
    /// On or off; a flip without.
    #[serde(default)]
    pub enabled: Option<bool>,
}

/// Runs `engine.firefox.toggle`; `None` for any other command.
pub fn run_command(core: &Core, id: &str, args: &Value) -> Option<Result<Value, CommandError>> {
    if id != TOGGLE {
        return None;
    }
    let args = if args.is_null() {
        Ok(ToggleArgs::default())
    } else {
        ToggleArgs::deserialize(args).map_err(|error| CommandError::InvalidArgs(error.to_string()))
    };
    Some(args.and_then(|args| {
        let bridge = for_host(core.host()).ok_or_else(|| {
            CommandError::Failed("the external windows are not started".to_owned())
        })?;
        let enabled = bridge.toggle(core, args.enabled);
        Ok(json!({ "enabled": enabled }))
    }))
}

impl FirefoxBridge {
    /// The engine the external windows start with.
    pub fn engine(&self) -> Arc<dyn Engine> {
        lock(&self.state).engine.clone()
    }

    /// The running Firefox engine, if Firefox was found and is not switched off.
    pub fn firefox(&self) -> Option<Arc<FirefoxEngine>> {
        lock(&self.state).firefox.clone()
    }

    /// Plugs in after the external windows started: the sync's router, the follow tasks, the
    /// "not found" notice.
    pub fn attach(self: &Arc<Self>, engines: &HostEngines) {
        *lock(&self.windows) = Arc::downgrade(engines.windows());
        if let Some(host) = self.host.upgrade() {
            lock(registry()).push((Arc::downgrade(&host), self.clone()));
            follow::watch_host(self, &host);
        }
        self.wire();
        self.notify_if_missing();
    }

    fn apply(&self, core: &Core) {
        let sync_source = core.user_settings().engines.firefox.sync_source;
        let launching = launching(core);
        let agent = live::agent_config(core.host(), sync_source);
        let (rebuild, firefox) = {
            let mut state = lock(&self.state);
            state.sync_source = sync_source;
            let rebuild = state.launching != launching && !state.off;
            if !rebuild {
                state.launching = launching.clone();
            }
            (rebuild, state.firefox.clone())
        };
        if rebuild {
            self.rebuild(launching, agent);
        } else if let Some(firefox) = firefox {
            firefox.set_agent_config(agent);
        }
    }

    /// The cells changed what Firefox renders at (RB-080-40): the settings applied again, which
    /// rebuilds the engine when the factor differs.
    pub(crate) fn refresh(&self) {
        if let Some(core) = self.core.upgrade() {
            self.apply(&core);
        }
    }

    /// Firefox on or off; answers whether it is on now. On while it runs changes nothing.
    fn toggle(&self, core: &Core, enabled: Option<bool>) -> bool {
        let off = lock(&self.state).off;
        let on = enabled.unwrap_or(off);
        if on && !off {
            return true;
        }
        if on {
            lock(&self.state).off = false;
            let sync_source = core.user_settings().engines.firefox.sync_source;
            self.rebuild(
                launching(core),
                live::agent_config(core.host(), sync_source),
            );
        } else {
            let reason = rb_i18n::t("engines.firefox.off");
            let old = {
                let mut state = lock(&self.state);
                state.off = true;
                state.engine = Arc::new(UnsupportedEngine::because(EngineKind::Firefox, reason));
                state.tasks.drain(..).for_each(|task| task.abort());
                let old = state.firefox.take();
                if old.is_some() {
                    state.retired.clone_from(&old);
                }
                old
            };
            self.wire();
            if let Some(old) = old {
                self.runtime.spawn(async move { old.shutdown().await });
            }
        }
        on
    }

    /// A new engine from `launching`; the views of the old one start again on it.
    /// The new Firefox starts on the profile only once the old one has quit.
    fn rebuild(&self, launching: Launching, agent: AgentConfig) {
        let (engine, firefox) = engine_of(&launching, &self.runtime, agent, &self.host);
        let old = {
            let mut state = lock(&self.state);
            state.tasks.drain(..).for_each(|task| task.abort());
            state.launching = launching;
            state.engine = engine;
            let old = std::mem::replace(&mut state.firefox, firefox.clone());
            old.or_else(|| state.retired.take())
        };
        if let (Some(new), Some(old)) = (&firefox, &old) {
            new.after(old.clone());
        }
        self.notified.store(false, Ordering::Release);
        self.wire();
        if let Some(windows) = lock(&self.windows).upgrade() {
            match windows.restart_after_rebuild(EngineKind::Firefox) {
                Ok(_) | Err(RestartError::NotRunning(_) | RestartError::Unavailable { .. }) => {}
                Err(error) => tracing::debug!(%error, "Firefox views not restarted"),
            }
        }
        if let Some(old) = old {
            self.runtime.spawn(async move { old.shutdown().await });
        }
        self.notify_if_missing();
    }

    /// The current engine into the external windows, its replay into the sync's router, its
    /// follow tasks started.
    fn wire(&self) {
        let (engine, firefox) = {
            let state = lock(&self.state);
            (state.engine.clone(), state.firefox.clone())
        };
        if let Some(windows) = lock(&self.windows).upgrade() {
            windows.set_engine(engine);
        }
        let Some(host) = self.host.upgrade() else {
            return;
        };
        if let Some(sync) = crate::sync_hub::for_host(&host) {
            match &firefox {
                Some(firefox) => sync
                    .router()
                    .set_engine(EngineKind::Firefox, firefox.replay()),
                None => sync
                    .router()
                    .set_engine(EngineKind::Firefox, Arc::new(NoReplay)),
            }
        }
        if let (Some(firefox), Some(me)) = (firefox, self.me.upgrade()) {
            let tasks = follow::start(&me, &firefox, &self.runtime);
            lock(&self.state).tasks.extend(tasks);
        }
    }

    /// Whether Firefox events lead the sync (`engines.firefox.syncSource`).
    pub fn sync_source(&self) -> bool {
        lock(&self.state).sync_source
    }

    /// Once per settings state: Firefox is missing while a Firefox breakpoint is in the layout.
    pub(crate) fn notify_if_missing(&self) {
        let missing = {
            let state = lock(&self.state);
            !state.off && state.firefox.is_none() && state.launching.executable.is_err()
        };
        let Some(host) = self.host.upgrade() else {
            return;
        };
        let wanted = host
            .views()
            .iter()
            .any(|view| view.engine == EngineKind::Firefox);
        if !missing || !wanted || self.notified.swap(true, Ordering::AcqRel) {
            return;
        }
        if let Some(core) = self.core.upgrade() {
            core.notify(CoreToShell::Notification(Notification {
                level: NotificationLevel::Warning,
                message_key: "engines.firefox.notFoundNotice".to_owned(),
                args: BTreeMap::new(),
                settings_key: Some(PATH_KEY.to_owned()),
            }));
        }
    }

    fn host(&self) -> Option<Arc<Host>> {
        self.host.upgrade()
    }
}

/// What the settings say about starting Firefox.
fn launching(core: &Core) -> Launching {
    let user = core.user_settings();
    let firefox = &user.engines.firefox;
    let root = core.host().project_root();
    Launching {
        executable: locate(user.engines.firefox_executable()).map_err(|error| reason_of(&error)),
        profile: profile_dir(
            firefox.profile_dir.as_deref(),
            root.as_deref(),
            Some(&local_base()),
        ),
        extra_prefs: firefox.extra_prefs.clone(),
        hide_chrome: user.engines.external.embed,
        render_scale: super::for_host(core.host())
            .and_then(|engines| engines.firefox_render_scale())
            .filter(|_| user.engines.external.embed),
    }
}

/// The profile folder: the setting (absolute, or relative to `local`, rBrowser's local folder),
/// else `firefox\profiles\<project>-<hash>` under `local`, keyed by the project like Chromium's
/// profile (`crate::project::profile_dir`). Never inside the project: a clone controls its files
/// and could ship a `prefs.js` (a proxy), a root certificate or an extension in the profile. A
/// relative setting that climbs out of `local` (`..`) gives none.
pub fn profile_dir(
    set: Option<&str>,
    root: Option<&Path>,
    local: Option<&Path>,
) -> Option<PathBuf> {
    match set.map(str::trim).filter(|set| !set.is_empty()) {
        Some(set) => {
            let path = PathBuf::from(set);
            if path.is_absolute() {
                return Some(path);
            }
            let climbs = path
                .components()
                .any(|part| !matches!(part, std::path::Component::Normal(_)));
            (!climbs)
                .then(|| local.map(|local| local.join(path)))
                .flatten()
        }
        None => local.map(|local| crate::project::profile_dir(&local.join("firefox"), root)),
    }
}

/// `%LOCALAPPDATA%\rBrowser` (beside the executable when portable), the temporary folder's
/// when the environment names none - the base of Chromium's profile too (`cef_app`).
fn local_base() -> PathBuf {
    rb_config::local_dir().unwrap_or_else(|| std::env::temp_dir().join(rb_config::LOCAL_APP_DIR))
}

/// Why Firefox cannot run, for the user.
pub fn reason_of(error: &LocateError) -> String {
    match error {
        LocateError::Configured(path) => rb_i18n::t_with(
            "engines.firefox.configuredMissing",
            &[("path", &path.display().to_string())],
        ),
        LocateError::NotFound => rb_i18n::t("engines.firefox.notFound"),
    }
}

/// The engine for `launching`: Firefox, or the stand-in that says why not.
fn engine_of(
    launching: &Launching,
    runtime: &Handle,
    agent_config: AgentConfig,
    host: &Weak<Host>,
) -> (Arc<dyn Engine>, Option<Arc<FirefoxEngine>>) {
    let unavailable = |reason: String| -> (Arc<dyn Engine>, Option<Arc<FirefoxEngine>>) {
        (
            Arc::new(UnsupportedEngine::because(EngineKind::Firefox, reason)),
            None,
        )
    };
    let launch: Arc<dyn Launch> = match (test_launch(), &launching.executable, &launching.profile) {
        (Some(fake), _, _) => fake,
        (None, Err(reason), _) => return unavailable(reason.clone()),
        (None, Ok(_), None) => return unavailable(rb_i18n::t("engines.firefox.noProfile")),
        (None, Ok(executable), Some(profile)) => Arc::new(InstalledFirefox {
            executable: executable.clone(),
            profile: profile.clone(),
            extra_prefs: launching.extra_prefs.clone(),
            hide_chrome: launching.hide_chrome,
            render_scale: launching.render_scale,
        }),
    };
    let firefox = FirefoxEngine::new(
        FirefoxConfig {
            launch,
            agent_source: Arc::from(crate::sync_agent::AGENT_SOURCE),
            agent_config,
        },
        runtime.clone(),
    );
    firefox.set_emulation_filter(live::filter_for(host.clone()));
    (firefox.clone(), Some(firefox))
}

#[cfg(not(test))]
fn locate(configured: Option<&str>) -> Result<PathBuf, LocateError> {
    rb_bidi::launcher::find_firefox(configured.map(Path::new))
}

#[cfg(not(test))]
fn test_launch() -> Option<Arc<dyn Launch>> {
    None
}

/// Unit tests never start the machine's Firefox: it is "not found" unless a test hands in a
/// fake (`rb_bidi::testing::FakeFirefox`).
#[cfg(test)]
fn locate(_configured: Option<&str>) -> Result<PathBuf, LocateError> {
    Err(LocateError::NotFound)
}

#[cfg(test)]
pub(crate) static TEST_LAUNCH: Mutex<Option<Arc<dyn Launch>>> = Mutex::new(None);

#[cfg(test)]
fn test_launch() -> Option<Arc<dyn Launch>> {
    lock(&TEST_LAUNCH).clone()
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
