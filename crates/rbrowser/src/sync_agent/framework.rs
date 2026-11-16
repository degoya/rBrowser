//! Framework DevTools on the host's side (RB-080-24): the main-world scripts of `agent/`
//! ([`Bundles`]) registered per view with `Page.addScriptToEvaluateOnNewDocument` (no world
//! name, before the page's own scripts) - the framework probe whenever a framework is not off,
//! and a framework's DevTools hook (React's `react-devtools-inline` backend, Vue's
//! `@vue/devtools-kit`) when its setting is `on`, or `auto` and the probe found the framework
//! in the view. In `auto` the hook reaches the view's next document: the shell says to reload
//! once. Nothing is registered for a framework that is off, and without Vue or React in the
//! page no hook at all. The tasks that feed this are `framework_tasks.rs`; the panel is the
//! `framework-devtools` plugin, which talks to the hooks over DOM events in the page.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

use rb_config::{FrameworkDevtoolsMode, FrameworkDevtoolsSettings};
use rb_engine::ViewId;
use rb_ipc::ViewFrameworks;
use rb_sync::{Framework, FrameworkMessage};
use serde_json::json;

use crate::cdp::{CdpRouter, lock};

/// The framework probe (`agent/dist/rb-framework-probe.js`), copied by `build.rs`; empty when
/// `agent/` was not built.
pub const PROBE_SOURCE: &str = include_str!(concat!(env!("OUT_DIR"), "/rb-framework-probe.js"));
/// The React DevTools backend (`agent/dist/rb-react-devtools.js`).
pub const REACT_SOURCE: &str = include_str!(concat!(env!("OUT_DIR"), "/rb-react-devtools.js"));
/// The Vue DevTools backend (`agent/dist/rb-vue-devtools.js`).
pub const VUE_SOURCE: &str = include_str!(concat!(env!("OUT_DIR"), "/rb-vue-devtools.js"));

/// A main-world script the host registers in a view.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Script {
    /// The framework probe.
    Probe,
    /// A framework's DevTools hook.
    Hook(Framework),
}

/// The three bundles.
#[derive(Clone, Copy, Debug)]
pub struct Bundles {
    pub probe: &'static str,
    pub react: &'static str,
    pub vue: &'static str,
}

impl Bundles {
    /// The bundles `build.rs` embedded.
    pub const EMBEDDED: Bundles = Bundles {
        probe: PROBE_SOURCE,
        react: REACT_SOURCE,
        vue: VUE_SOURCE,
    };

    /// The source of `script`; `None` when it was not built.
    pub fn source(&self, script: Script) -> Option<&'static str> {
        let text = match script {
            Script::Probe => self.probe,
            Script::Hook(Framework::React) => self.react,
            Script::Hook(Framework::Vue) => self.vue,
        };
        (!text.trim().is_empty()).then_some(text)
    }
}

/// The mode of `framework` in `modes`.
pub fn mode_of(modes: &FrameworkDevtoolsSettings, framework: Framework) -> FrameworkDevtoolsMode {
    match framework {
        Framework::Vue => modes.vue,
        Framework::React => modes.react,
    }
}

/// The scripts a view wants under `modes`, having `seen` these frameworks in its session: the
/// probe while a framework is not off; a hook when its mode is `on`, or `auto` and the
/// framework was seen.
pub fn wanted(modes: &FrameworkDevtoolsSettings, seen: &BTreeSet<Framework>) -> BTreeSet<Script> {
    let mut scripts = BTreeSet::new();
    if modes.probes() {
        scripts.insert(Script::Probe);
    }
    for framework in Framework::ALL {
        let hook = match mode_of(modes, framework) {
            FrameworkDevtoolsMode::On => true,
            FrameworkDevtoolsMode::Auto => seen.contains(&framework),
            FrameworkDevtoolsMode::Off => false,
        };
        if hook {
            scripts.insert(Script::Hook(framework));
        }
    }
    scripts
}

/// The hooks of one view.
#[derive(Default)]
struct ViewHooks {
    /// The view's page session.
    session: Option<String>,
    /// The scripts registered in `session`, by identifier.
    registered: BTreeMap<Script, String>,
    /// The frameworks the probe found in any document of this session: a hook, once wanted
    /// in `auto`, stays until the view closes or the mode changes.
    seen: BTreeSet<Framework>,
    /// The probe's last report of the current document.
    report: Option<FrameworkMessage>,
}

struct Inner {
    modes: FrameworkDevtoolsSettings,
    views: HashMap<ViewId, ViewHooks>,
}

/// The Framework DevTools scripts of every view.
pub struct FrameworkHooks {
    cdp: Arc<CdpRouter>,
    bundles: Bundles,
    inner: Mutex<Inner>,
    /// Serialises the registrations, so a later decision never loses against an earlier one.
    register: tokio::sync::Mutex<()>,
}

impl FrameworkHooks {
    /// The hooks over `cdp` with the embedded bundles; both frameworks `auto` until the user
    /// settings arrive (`set_modes`).
    pub fn new(cdp: Arc<CdpRouter>) -> Arc<Self> {
        Self::with_bundles(cdp, Bundles::EMBEDDED)
    }

    pub(crate) fn with_bundles(cdp: Arc<CdpRouter>, bundles: Bundles) -> Arc<Self> {
        Arc::new(Self {
            cdp,
            bundles,
            inner: Mutex::new(Inner {
                modes: FrameworkDevtoolsSettings::default(),
                views: HashMap::new(),
            }),
            register: tokio::sync::Mutex::new(()),
        })
    }

    /// `view` has a new page `session` (an attach): the scripts it wants go into it, before
    /// the first document loads.
    pub async fn attach(&self, view: ViewId, session: &str) {
        {
            let mut inner = lock(&self.inner);
            let hooks = inner.views.entry(view).or_default();
            hooks.session = Some(session.to_owned());
            // The old scripts went with the old session.
            hooks.registered.clear();
            hooks.report = None;
        }
        self.reconcile(&[view]).await;
    }

    /// The probe of `view`'s top document reported `message`: the shell learns it, and in
    /// `auto` a found framework's hook goes into the view's next document.
    pub async fn on_report(&self, view: ViewId, message: &FrameworkMessage) {
        {
            let mut inner = lock(&self.inner);
            let hooks = inner.views.entry(view).or_default();
            hooks.seen.extend(message.detected());
            hooks.report = Some(message.clone());
        }
        self.reconcile(&[view]).await;
    }

    /// `view`'s top frame started a new document: the old document's report is gone until the
    /// probe speaks again.
    pub fn on_navigated(&self, view: ViewId) {
        if let Some(hooks) = lock(&self.inner).views.get_mut(&view) {
            hooks.report = None;
        }
    }

    /// The settings changed: every view gets what it wants now and loses the rest.
    pub async fn set_modes(&self, modes: FrameworkDevtoolsSettings) {
        let views: Vec<ViewId> = {
            let mut inner = lock(&self.inner);
            if inner.modes == modes {
                return;
            }
            inner.modes = modes;
            inner.views.keys().copied().collect()
        };
        self.reconcile(&views).await;
    }

    /// The view is gone.
    pub fn forget(&self, view: ViewId) {
        lock(&self.inner).views.remove(&view);
    }

    /// What the shell offers: per view with a report, the frameworks found whose mode is not
    /// off, in view order.
    pub fn frameworks(&self) -> Vec<ViewFrameworks> {
        let inner = lock(&self.inner);
        let mut list: Vec<ViewFrameworks> = inner
            .views
            .iter()
            .filter_map(|(view, hooks)| {
                let report = hooks.report.as_ref()?;
                let offered = |framework: Framework| {
                    mode_of(&inner.modes, framework)
                        .is_on()
                        .then(|| report.info(framework).cloned())
                        .flatten()
                };
                let (vue, react) = (offered(Framework::Vue), offered(Framework::React));
                (vue.is_some() || react.is_some()).then_some(ViewFrameworks {
                    view: *view,
                    vue,
                    react,
                })
            })
            .collect();
        list.sort_by_key(|entry| entry.view);
        list
    }

    /// Whether `script` is registered in `view`'s session now.
    #[cfg(test)]
    pub(crate) fn is_registered(&self, view: ViewId, script: Script) -> bool {
        lock(&self.inner)
            .views
            .get(&view)
            .is_some_and(|hooks| hooks.registered.contains_key(&script))
    }

    /// Registers what each of `views` wants and is missing, removes what it has and no longer
    /// wants.
    async fn reconcile(&self, views: &[ViewId]) {
        let _serial = self.register.lock().await;
        for &view in views {
            let (session, add, remove) = {
                let inner = lock(&self.inner);
                let Some(hooks) = inner.views.get(&view) else {
                    continue;
                };
                let Some(session) = hooks.session.clone() else {
                    continue;
                };
                let wanted = wanted(&inner.modes, &hooks.seen);
                let add: Vec<Script> = wanted
                    .iter()
                    .copied()
                    .filter(|script| !hooks.registered.contains_key(script))
                    .collect();
                let remove: Vec<(Script, String)> = hooks
                    .registered
                    .iter()
                    .filter(|(script, _)| !wanted.contains(script))
                    .map(|(script, identifier)| (*script, identifier.clone()))
                    .collect();
                (session, add, remove)
            };
            for (script, identifier) in remove {
                self.remove_script(&session, &identifier).await;
                if let Some(hooks) = lock(&self.inner).views.get_mut(&view)
                    && hooks.session.as_deref() == Some(session.as_str())
                {
                    hooks.registered.remove(&script);
                }
            }
            for script in add {
                let Some(identifier) = self.add_script(&session, script).await else {
                    continue;
                };
                if let Some(hooks) = lock(&self.inner).views.get_mut(&view)
                    && hooks.session.as_deref() == Some(session.as_str())
                {
                    hooks.registered.insert(script, identifier);
                }
            }
        }
    }

    /// Registers `script` in `session`; its identifier.
    async fn add_script(&self, session: &str, script: Script) -> Option<String> {
        let Some(source) = self.bundles.source(script) else {
            tracing::warn!(
                ?script,
                "the Framework DevTools script is not built (agent/: npm run build)"
            );
            return None;
        };
        // No world name: the page's main world, before its scripts. At once in the running
        // document too: the probe reports it, and Vue replays apps mounted moments ago.
        let params = json!({ "source": source, "runImmediately": true });
        match self
            .cdp
            .call(
                "Page.addScriptToEvaluateOnNewDocument",
                params,
                Some(session),
            )
            .await
        {
            Ok(answer) => answer["identifier"].as_str().map(str::to_owned),
            Err(error) => {
                tracing::warn!(?script, %error, "the Framework DevTools script was not registered");
                None
            }
        }
    }

    async fn remove_script(&self, session: &str, identifier: &str) {
        let params = json!({ "identifier": identifier });
        if let Err(error) = self
            .cdp
            .call(
                "Page.removeScriptToEvaluateOnNewDocument",
                params,
                Some(session),
            )
            .await
        {
            tracing::debug!(%error, "the Framework DevTools script was not removed");
        }
    }
}

#[cfg(test)]
#[path = "framework_tests.rs"]
mod tests;
