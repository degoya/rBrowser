//! [`PluginRuntime`]: a window's feature plugins at run time (RB-060-12) - their components'
//! answers to commands, tools and page scripts, and the posts those answers carry, delivered to
//! the page scripts ([`PluginPages`]), the panels (the shell) and the status bar (RB-080-64,
//! [`super::status`]). The plugins served are swapped whole by a live change (RB-092-06,
//! [`PluginRuntime::replace_plugins`]); a call takes them as they are when it starts.

use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};

use async_trait::async_trait;
use rb_engine::ViewId;
use rb_plugin_host::{
    BUSY, LoadedEntry, MAX_MESSAGE_BYTES, PageBridge, PageLog, PluginSource, Post, PostTarget,
    split_posts,
};
use rb_plugin_types::{Failure, FailureKind, PluginKind, codes};
use serde_json::{Value, json};
use tokio::runtime::Handle;

use super::pages::{Inbox, PluginPages};
use super::status::{Answer, side_post};
pub use super::status::{HostSink, NoticeSink, Sinks};
use crate::cdp::{CdpRouter, lock};
use crate::network_rules::state::Rules;

pub use super::component::Component;
use super::consent::{self, Call};
use crate::api_client::protocol::{self as api_protocol, Origin};
use crate::load_test::call::with_load;
use crate::load_test::protocol as load_protocol;

/// One feature plugin the runtime serves.
#[derive(Clone)]
pub struct Plugin {
    pub id: String,
    pub source: PluginSource,
    pub component: Arc<dyn Component>,
    /// Its page scripts; `None` without any.
    pub bridge: Option<Arc<PageBridge>>,
    /// Granted `console-logs`: every command and tool call gets the views' buffers
    /// ([`super::console`]).
    pub console: bool,
    /// Granted `network-rules`: every call gets the `$network` state, and the host asks for
    /// the plugin's rules after it ([`super::network`]).
    pub network: bool,
    /// Granted `status`: its `status` posts reach the status bar ([`super::status`]).
    pub status: bool,
}

#[path = "runtime_plugins.rs"]
mod runtime_plugins;
pub use runtime_plugins::{enabled_bridges, may_run, plugins_of, runnable};

#[path = "runtime_swap.rs"]
mod runtime_swap;
pub use runtime_swap::keep_bridges;

/// What hands a panel event to the shell: the plugin and the event's JSON.
pub type PanelSink = Arc<dyn Fn(&str, Value) + Send + Sync>;

/// The feature plugins of one window.
pub struct PluginRuntime {
    /// The plugins served now, swapped whole ([`PluginRuntime::replace_plugins`]).
    plugins: Mutex<Arc<Vec<Plugin>>>,
    pages: Arc<PluginPages>,
    sinks: Sinks,
    runtime: Handle,
    /// Page requests waiting for their component, by plugin.
    pending: Mutex<HashMap<String, usize>>,
    /// The window's interception rules, once started ([`PluginRuntime::set_network_rules`]).
    network: Mutex<Option<Arc<Rules>>>,
    /// The workspace trust in effect for the project plugins; none before the first
    /// [`PluginRuntime::set_enabled`].
    project_trusted: AtomicBool,
}

impl PluginRuntime {
    /// The runtime of `plugins` over `cdp`; its tasks run on `runtime`, panel events and
    /// status-bar notices go to `sinks`. No page script runs before
    /// [`PluginRuntime::set_enabled`].
    pub fn new(
        plugins: Vec<Plugin>,
        cdp: Arc<CdpRouter>,
        runtime: Handle,
        sinks: Sinks,
    ) -> Arc<Self> {
        Arc::new_cyclic(|me: &Weak<Self>| {
            let inbox: Weak<dyn Inbox> = me.clone();
            Self {
                plugins: Mutex::new(Arc::new(plugins)),
                pages: PluginPages::new(cdp, runtime.clone(), inbox),
                sinks,
                runtime,
                pending: Mutex::new(HashMap::new()),
                network: Mutex::new(None),
                project_trusted: AtomicBool::new(false),
            }
        })
    }

    /// The page scripts of every view.
    pub fn pages(&self) -> &Arc<PluginPages> {
        &self.pages
    }

    /// The plugins served now; a live change after this leaves the list taken as it is.
    pub fn plugins(&self) -> Arc<Vec<Plugin>> {
        Arc::clone(&lock(&self.plugins))
    }

    /// The page scripts that run from now on: those [`enabled_bridges`] gives; and whether the
    /// project plugins run at all ([`may_run`]).
    pub fn set_enabled(&self, off: &BTreeSet<String>, project_trusted: bool) {
        self.project_trusted
            .store(project_trusted, Ordering::Release);
        self.pages
            .set_plugins(enabled_bridges(&self.plugins(), off, project_trusted));
    }

    /// Runs command `command` of plugin `plugin`: its answer without its posts, which are
    /// delivered.
    ///
    /// # Errors
    ///
    /// The plugin is not served (`plugins.error.not_available`), or its call failed.
    pub async fn run_command(
        &self,
        plugin: &str,
        command: &str,
        args: &Value,
    ) -> Result<Value, Failure> {
        Ok(self.run_command_noted(plugin, command, args).await?.0)
    }

    /// [`PluginRuntime::run_command`], and whether the answer showed a status-bar notice of its
    /// own (the command's generic "done" is then not shown). Errors as that one's.
    pub async fn run_command_noted(
        &self,
        plugin: &str,
        command: &str,
        args: &Value,
    ) -> Result<(Value, bool), Failure> {
        let component = self.component(plugin)?;
        let args = self.fed(plugin, args, Origin::Command);
        let raw = with_load(&*component, Call::Command(command), &args).await;
        self.sync_network(plugin).await;
        self.answered_noted(plugin, raw, Answer::Command)
    }

    /// Calls tool `tool` of plugin `plugin`, as [`PluginRuntime::run_command`].
    ///
    /// # Errors
    ///
    /// As [`PluginRuntime::run_command`].
    pub async fn call_tool(
        &self,
        plugin: &str,
        tool: &str,
        args: &Value,
    ) -> Result<Value, Failure> {
        self.call_tool_from(plugin, tool, args, Origin::Tool).await
    }

    /// [`PluginRuntime::call_tool`] for a caller `origin`: what the API client lets it do
    /// (RB-096-01, `crate::api_client::protocol`).
    async fn call_tool_from(
        &self,
        plugin: &str,
        tool: &str,
        args: &Value,
        origin: Origin,
    ) -> Result<Value, Failure> {
        let component = self.component(plugin)?;
        let args = self.fed(plugin, args, origin);
        let raw = with_load(&*component, Call::Tool(tool), &args).await;
        self.sync_network(plugin).await;
        self.answered(plugin, raw, Answer::Tool)
    }

    /// A panel's call: as [`PluginRuntime::run_command`] or [`PluginRuntime::call_tool`], its
    /// answer held to `limit`, `plugins.panelMessageMaxBytes` (F7 of Charge 5, RB-090-44).
    ///
    /// # Errors
    ///
    /// As those, or the answer is over `limit` ([`super::panel_limit::too_large`]).
    pub async fn panel_run_command(
        &self,
        plugin: &str,
        command: &str,
        args: &Value,
        limit: usize,
    ) -> Result<Value, Failure> {
        self.say_waiting(plugin);
        super::panel_limit::bounded(self.run_command(plugin, command, args).await?, limit)
    }

    /// The tool form of [`PluginRuntime::panel_run_command`].
    ///
    /// # Errors
    ///
    /// As [`PluginRuntime::panel_run_command`].
    pub async fn panel_call_tool(
        &self,
        plugin: &str,
        tool: &str,
        args: &Value,
        limit: usize,
    ) -> Result<Value, Failure> {
        self.say_waiting(plugin);
        let answer = self
            .call_tool_from(plugin, tool, args, Origin::Panel)
            .await?;
        super::panel_limit::bounded(answer, limit)
    }

    /// RB-080-90: a panel's call that waits for a running one of the same plugin (a check run,
    /// another panel's call) tells the plugin's panels `{"host": "waiting"}` first; they say that
    /// they wait instead of seeming to hang or failing.
    fn say_waiting(&self, plugin: &str) {
        if self
            .component(plugin)
            .is_ok_and(|component| component.busy())
        {
            (self.sinks.panels)(plugin, json!({ "host": "waiting" }));
        }
    }

    /// `args` with the views' buffers for a plugin granted `console-logs`, the `$network`
    /// state for one granted `network-rules`, the probe offered to one granted `consent-probe`
    /// and the API work to one granted `api-client` (for `origin`), the load test to one granted
    /// `load-test` (RB-096-07); a caller's `$consent`, `$apiResult` and `$loadResult` dropped for
    /// every plugin ([`consent::offered`], [`api_protocol::offered`], [`load_protocol::offered`]).
    fn fed(&self, plugin: &str, args: &Value, origin: Origin) -> Value {
        let Some(served) = self.served(plugin) else {
            let args = api_protocol::offered(&consent::offered(args, false), false, origin);
            return load_protocol::offered(&args, false, origin);
        };
        let args = consent::offered(args, served.component.consent_probe());
        let args = api_protocol::offered(&args, served.component.api_client(), origin);
        let args = load_protocol::offered(&args, served.component.load_test(), origin);
        let mut args = if served.console {
            super::console::with_feed(&args, super::console::current())
        } else {
            args
        };
        if served.network
            && let Some(rules) = self.network_rules()
        {
            args = super::network::with_state(&args, rules.state(plugin));
        }
        args
    }

    /// The interception rules the plugins granted `network-rules` feed from now on; each of them
    /// is asked for its rules at once.
    pub async fn set_network_rules(&self, rules: Arc<Rules>) {
        *lock(&self.network) = Some(rules);
        self.sync_all_network().await;
    }

    fn network_rules(&self) -> Option<Arc<Rules>> {
        lock(&self.network).clone()
    }

    /// Asks every plugin granted `network-rules` for its rules again (trust or settings changed).
    pub async fn sync_all_network(&self) {
        let ids: Vec<String> = self
            .plugins()
            .iter()
            .filter(|plugin| plugin.network)
            .map(|plugin| plugin.id.clone())
            .collect();
        for id in ids {
            self.sync_network(&id).await;
        }
    }

    /// Asks `plugin` for its rules when it is granted `network-rules`, the rules run and it
    /// [`may_run`]; a plugin that may not run loses its rules (review finding M2 of Charge 7). A
    /// project plugin's rules are all project rules, whatever origin they declare.
    async fn sync_network(&self, plugin: &str) {
        let Some(served) = self.served(plugin).filter(|served| served.network) else {
            return;
        };
        let Some(rules) = self.network_rules() else {
            return;
        };
        if !may_run(served.source, self.project_trusted.load(Ordering::Acquire)) {
            rules.remove(plugin);
            return;
        }
        let project = served.source == PluginSource::Project;
        super::network::sync(&*served.component, plugin, &rules, project).await;
    }

    /// Whether a project plugin is among the plugins served.
    pub fn has_project_plugins(&self) -> bool {
        self.plugins()
            .iter()
            .any(|plugin| plugin.source == PluginSource::Project)
    }

    /// Whether plugin `plugin` is served and [`may_run`] now: what MCP lists and the panels
    /// window offers.
    pub fn serves(&self, plugin: &str) -> bool {
        self.component(plugin).is_ok()
    }

    /// The component of plugin `plugin` when it is served and [`may_run`].
    fn component(&self, plugin: &str) -> Result<Arc<dyn Component>, Failure> {
        let served = self.served(plugin).ok_or_else(|| not_available(plugin))?;
        if !may_run(served.source, self.project_trusted.load(Ordering::Acquire)) {
            return Err(
                Failure::coded(FailureKind::AccessDenied, codes::UNTRUSTED_WORKSPACE)
                    .with_param("plugin", plugin),
            );
        }
        Ok(Arc::clone(&served.component))
    }

    /// The JSON of a raw answer without its posts; the posts are delivered.
    fn answered(
        &self,
        plugin: &str,
        raw: Result<String, Failure>,
        from: Answer,
    ) -> Result<Value, Failure> {
        Ok(self.answered_noted(plugin, raw, from)?.0)
    }

    /// [`PluginRuntime::answered`], and whether a status-bar notice was shown; `from`: whose
    /// answer it is (which `host` posts run, [`Answer`]).
    fn answered_noted(
        &self,
        plugin: &str,
        raw: Result<String, Failure>,
        from: Answer,
    ) -> Result<(Value, bool), Failure> {
        // RB-092-06: the plugin was removed while the call ran; its answer and posts are dropped.
        if self.served(plugin).is_none() {
            return Err(not_available(plugin));
        }
        let answer: Value = serde_json::from_str(&raw?)
            .map_err(|_| Failure::coded(FailureKind::BadData, codes::BAD_OUTPUT))?;
        let (answer, posts, dropped) = split_posts(answer);
        if dropped > 0 {
            tracing::warn!(
                plugin_id = plugin,
                dropped,
                "posts of a plugin answer dropped"
            );
        }
        let noted = self.deliver(plugin, posts, from);
        Ok((answer, noted))
    }

    /// Hands `posts` on: to the page scripts on the runtime's tasks, to the panels, the status
    /// bar and the host at once; whether a notice was shown.
    fn deliver(&self, plugin: &str, posts: Vec<Post>, from: Answer) -> bool {
        let (mut to_pages, mut noted) = (Vec::new(), false);
        let served = self.served(plugin);
        for post in posts {
            match post.target {
                PostTarget::Panel => {
                    (self.sinks.panels)(plugin, json!({ "message": post.message }));
                }
                PostTarget::All => to_pages.push((None, post.message)),
                PostTarget::View(view) => to_pages.push((Some(ViewId(view)), post.message)),
                // `status` and `host`.
                side => {
                    noted |= side_post(served.as_ref(), &self.sinks, side, &post.message, from);
                }
            }
        }
        if to_pages.is_empty() {
            return noted;
        }
        let pages = Arc::clone(&self.pages);
        let plugin = plugin.to_owned();
        self.runtime.spawn(async move {
            for (view, message) in to_pages {
                pages.post(&plugin, view, &message).await;
            }
        });
        noted
    }
}

#[async_trait]
impl Inbox for PluginRuntime {
    async fn request(&self, plugin: &str, view: ViewId, message: Value) -> Result<Value, String> {
        let Some(_slot) = self.take_slot(plugin) else {
            super::page_wait::note_busy(plugin, view);
            return Err(BUSY.to_owned());
        };
        let component = self.component(plugin).map_err(code_of)?;
        let raw = super::page_wait::within_page_time(&*component, plugin, view, &message).await?;
        self.answered(plugin, raw, Answer::Page)
            .and_then(bounded)
            .map_err(code_of)
    }

    fn emit(&self, plugin: &str, view: ViewId, message: Value) {
        (self.sinks.panels)(plugin, json!({ "view": view.0, "message": message }));
    }

    fn log(&self, plugin: &str, view: ViewId, line: PageLog) {
        super::page_log::record(plugin, view, &line);
    }
}

/// `answer` when its JSON text fits a page script's message (64 KiB, UTF-8 bytes); a panel's
/// limit is its own (RB-090-44, [`super::panel_limit`]).
fn bounded(answer: Value) -> Result<Value, Failure> {
    if answer.to_string().len() > MAX_MESSAGE_BYTES {
        Err(Failure::coded(FailureKind::Limit, codes::TOO_LARGE)
            .with_param("limit", MAX_MESSAGE_BYTES.to_string()))
    } else {
        Ok(answer)
    }
}

/// Plugin `plugin` is not served (`plugins.error.not_available`).
fn not_available(plugin: &str) -> Failure {
    Failure::coded(FailureKind::NotFound, codes::NOT_AVAILABLE).with_param("service", plugin)
}

/// A failure's code for a page script: the plugin's own key or `plugins.error.*`.
fn code_of(failure: Failure) -> String {
    failure.code().unwrap_or(codes::HOST_FAILED).to_owned()
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "runtime_wait_tests.rs"]
mod wait_tests;

#[cfg(test)]
#[path = "runtime_health_tests.rs"]
mod health_tests;
