//! The plugin runtime in the app (RB-060-12, `docs/plugins.md`, "Page scripts and panels"): the
//! loaded feature plugins' page scripts in every view ([`pages`]), their components' answers and
//! posts ([`runtime`]), their panels in the panels window ([`panels`], [`ipc`]), their commands
//! in the one registry ([`commands`], a command's `$open` in [`open`]) and their MCP tools
//! ([`mcp`]); the host services behind their imports ([`services`], Charge 6); the `$console`
//! feed ([`console`]) with what the page scripts log ([`page_log`], RB-080-66), the `$consent`
//! probe of the plugins granted `consent-probe` ([`consent`], RB-094-09) and the network
//! rules of the plugins granted `network-rules` ([`network`], RB-070-02); the list of the
//! installed plugins for Settings > Plugins and MCP ([`list`], RB-080-51); their notices in the
//! status bar ([`status`], RB-080-64); the shell pages told when the panels offered change
//! ([`panel_list`], RB-080-97); the signed plugin catalogue of Settings > Plugins > Catalog
//! ([`catalog`], RB-091-01); the page scripts told when their plugin's settings change
//! ([`settings_post`], RB-090-58); the bundled first-party plugins the person chose, the first-run
//! wizard's install and the notice of plugins a project names that are not installed
//! ([`bundle`], [`bundle_install`], [`bundle_offer`], [`first_party`], RB-091-04).
//!
//! The plugins are the ones `crate::analyzer::plugins` loads (on a thread of its own at start:
//! bundled, user-installed, the project's under workspace trust); the runtime of a window starts
//! once they are there and binds the services to the window. A project plugin runs only under
//! workspace trust - page scripts, commands, MCP tools, panels, checks and the services they
//! reach stop when it is revoked or goes stale. `plugins.pageScriptsDisabled` and
//! `plugins.panelsHidden` switch a plugin's page scripts and panels off without a restart. An
//! install, update or removal and a project trusted after the start take effect at once: the
//! plugins load again and the window follows ([`live`], RB-092-06).

pub mod bundle;
pub mod bundle_install;
pub mod bundle_offer;
pub mod catalog;
pub mod commands;
pub mod component;
pub mod consent;
pub mod console;
pub mod first_party;
pub mod host_actions;
pub mod ipc;
pub mod list;
pub mod live;
pub mod mcp;
pub mod network;
pub mod open;
pub mod page_log;
pub mod page_wait;
pub mod pages;
pub mod panel_errors;
pub mod panel_limit;
pub mod panel_list;
pub mod panels;
pub mod resume;
pub mod runtime;
pub mod services;
pub mod settings_post;
pub mod status;

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};

use rb_engine::{ViewEvent, ViewHost as _, ViewId};
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use rb_plugin_host::PluginSource;
use rb_plugin_types::codes;
use tokio::sync::broadcast::error::RecvError;

use crate::analyzer::plugins::LoadedPlugins;
use crate::cdp::lock;
use crate::control::Core;
use crate::host::Host;
use pages::PluginPages;
use runtime::{HostSink, NoticeSink, PanelSink, PluginRuntime, Sinks};

/// A window's plugin runtime.
struct Entry {
    host: Weak<Host>,
    runtime: Arc<PluginRuntime>,
    /// What its panels offered depended on when the shell pages were told last (RB-080-97).
    panels: Option<panel_list::Inputs>,
    /// The plugins it serves: the snapshot it started with or last followed (RB-092-06).
    followed: Arc<LoadedPlugins>,
}

fn registry() -> &'static Mutex<Vec<Entry>> {
    static REGISTRY: Mutex<Vec<Entry>> = Mutex::new(Vec::new());
    &REGISTRY
}

/// The runtime of `host`'s window, once it started.
pub fn for_host(host: &Host) -> Option<Arc<PluginRuntime>> {
    let mut entries = lock(registry());
    entries.retain(|entry| entry.host.strong_count() > 0);
    entries
        .iter()
        .find(|entry| std::ptr::eq(entry.host.as_ptr(), host))
        .map(|entry| entry.runtime.clone())
}

/// Starts the runtime of `core`'s window off the UI thread, once the bundled plugins are loaded
/// (at the very first start: once the first-run wizard's plugin choice is answered, RB-091-08).
pub fn on_start(core: &Core) {
    let Some(core) = core.arc() else {
        return;
    };
    if for_host(core.host()).is_some() {
        return;
    }
    // The catalogue's update check, when `plugins.catalog.checkOnStart` is on (RB-091-01).
    catalog::on_start(&core);
    let runtime = core.runtime().clone();
    runtime.spawn_blocking(move || start(&core));
}

/// The notice that a project trusted after the start loads its plugins at the next start - only
/// when they cannot load live (RB-092-06).
pub const RESTART_NOTICE_KEY: &str = "plugins.notice.restart_for_project_plugins";

fn start(core: &Arc<Core>) {
    // At the very first start the load waits for the wizard's plugin choice; then everything
    // below runs as at any start, live (RB-091-08).
    drop(crate::analyzer::plugins::bundled_when_answered());
    // Never beside a live change (RB-092-06): the snapshot taken under the lock is the one the
    // window follows from.
    let _following = lock(&live::FOLLOWING);
    let loaded = crate::analyzer::plugins::bundled();
    services::shared().bind(core);
    let plugins = runtime::plugins_of(&loaded.report.loaded);
    if plugins.iter().any(|plugin| plugin.console) {
        console::start(core);
    }
    consent::follow(core, &plugins);
    crate::api_client::follow(core, &plugins);
    crate::load_test::follow(core, &plugins);
    let weak = Arc::downgrade(core);
    let panels: PanelSink = Arc::new(move |plugin, event| {
        if let Some(core) = weak.upgrade() {
            core.send(&CoreToShell::PluginPanel {
                message: panels::event_message(plugin, event),
            });
        }
    });
    let status = notice_sink(core);
    let host = core.host();
    let weak = Arc::downgrade(core);
    let host_action: HostSink = Arc::new(move |plugin, message| {
        if let Some(core) = weak.upgrade() {
            host_actions::run(&core, plugin, message);
        }
    });
    let sinks = Sinks {
        panels,
        status,
        host: host_action,
    };
    let runtime = PluginRuntime::new(plugins, host.cdp().clone(), core.runtime().clone(), sinks);
    {
        let mut entries = lock(registry());
        if entries
            .iter()
            .any(|entry| std::ptr::eq(entry.host.as_ptr(), Arc::as_ptr(host)))
        {
            return;
        }
        entries.push(Entry {
            host: Arc::downgrade(host),
            runtime: runtime.clone(),
            panels: None,
            followed: Arc::clone(&loaded),
        });
    }
    let panels = apply_switches(core, &runtime);
    commands::register(core, &runtime, &loaded.report.loaded);
    // RB-080-97: a panels page that asked before is told the list is there now.
    announce_panels(core, panels);
    // RB-091-04: a plugin the project names that is not installed is said, not missing silently.
    bundle::notice_missing(core);
    // RB-094-13: the bundled plugins new in this version are named once.
    bundle::notice_new(core);
    follow_views(core, runtime.pages());
    follow_network(core, runtime);
}

/// The plugins granted `network-rules` get the window's interception rules: started with the
/// first such plugin - at the start or with a live change (RB-092-06) - and asked again after.
fn follow_network(core: &Core, runtime: Arc<PluginRuntime>) {
    if !runtime.plugins().iter().any(|plugin| plugin.network) {
        return;
    }
    if runtime.has_network_rules() {
        resync_network(core, runtime);
        return;
    }
    let rules = crate::network_rules::start(core);
    core.runtime()
        .spawn(async move { runtime.set_network_rules(rules).await });
}

/// The runtime of `core`'s window and the plugins it followed so far, which `now` replaces;
/// `None` before its runtime started.
fn take_followed(
    core: &Core,
    now: &Arc<LoadedPlugins>,
) -> Option<(Arc<PluginRuntime>, Arc<LoadedPlugins>)> {
    let mut entries = lock(registry());
    let entry = entries
        .iter_mut()
        .find(|entry| std::ptr::eq(entry.host.as_ptr(), Arc::as_ptr(core.host())))?;
    let before = std::mem::replace(&mut entry.followed, Arc::clone(now));
    Some((entry.runtime.clone(), before))
}

/// Shows a plugin's checked notice in the status bar: at most one per plugin a second, the
/// newest of those within it after the second (review S1 of RB-080-64).
fn notice_sink(core: &Arc<Core>) -> NoticeSink {
    let weak = Arc::downgrade(core);
    let throttle = Arc::new(status::Throttle::default());
    Arc::new(move |plugin, notice| {
        let Some(core) = weak.upgrade() else {
            return;
        };
        match throttle.offer(plugin, notice, std::time::Instant::now()) {
            status::Offered::Now(notice) => core.notify(CoreToShell::Notification(notice)),
            status::Offered::Later(wait) => {
                let (throttle, weak, plugin) = (throttle.clone(), weak.clone(), plugin.to_owned());
                core.runtime().spawn(async move {
                    tokio::time::sleep(wait).await;
                    let notice = throttle.take(&plugin, std::time::Instant::now());
                    if let (Some(core), Some(notice)) = (weak.upgrade(), notice) {
                        core.notify(CoreToShell::Notification(notice));
                    }
                });
            }
            status::Offered::Replaced => {}
        }
    })
}

/// Asks the plugins granted `network-rules` for their rules again, off the calling thread.
fn resync_network(core: &Core, runtime: Arc<PluginRuntime>) {
    if runtime.plugins().iter().any(|plugin| plugin.network) {
        core.runtime()
            .spawn(async move { runtime.sync_all_network().await });
    }
}

/// The user settings changed: `plugins.pageScriptsDisabled` takes effect, the plugins'
/// settings values are resolved again at their next use, and the page scripts of a plugin whose
/// settings changed are told ([`settings_post`], RB-090-58).
pub fn on_user_settings_changed(core: &Core) {
    services::shared().settings_changed();
    if let Some(runtime) = for_host(core.host()) {
        let panels = apply_switches(core, &runtime);
        announce_panels(core, panels);
        settings_post::post_changes(core, runtime.clone());
        resync_network(core, runtime);
    }
}

/// The project or its trust changed: the project plugins follow the trust, a trusted project's
/// origins are the local ones a plugin may fetch, project network rules (mocks) follow the trust
/// at once, and project plugins refused for want of trust load now (RB-092-06).
pub fn on_config_changed(core: &Core) {
    services::shared().project_changed(core);
    if let Some(rules) = crate::network_rules::current() {
        rules.set_trusted(core.host().shell_state().trusted);
    }
    on_user_settings_changed(core);
    if core.host().shell_state().trusted && refused_for_trust() {
        live::reload_for_trust(core);
    }
    bundle::notice_missing(core);
}

/// The workspace trust changed - decided, revoked, or stale after a change of a protected part:
/// as [`on_config_changed`], so the project plugins run or stop at once ([`runtime::may_run`])
/// and a trusted project's own origins open or close for `http` (review findings M2 and L2 of
/// Charge 6).
pub fn on_trust_changed(core: &Core) {
    on_config_changed(core);
}

/// Whether the load refused a project plugin because the workspace was not trusted then.
fn refused_for_trust() -> bool {
    // Only once the load is done: never a load (or its wait) on the calling thread.
    crate::analyzer::plugins::loaded().is_some_and(|loaded| {
        loaded.report.refused.iter().any(|refused| {
            refused.source == PluginSource::Project
                && refused.failure.code() == Some(codes::UNTRUSTED_WORKSPACE)
        })
    })
}

fn notify_restart_once(core: &Core) {
    static SHOWN: AtomicBool = AtomicBool::new(false);
    if SHOWN.swap(true, Ordering::AcqRel) {
        return;
    }
    core.notify(CoreToShell::Notification(Notification {
        level: NotificationLevel::Info,
        message_key: RESTART_NOTICE_KEY.to_owned(),
        args: Default::default(),
        settings_key: None,
    }));
}

/// The page script switches and the project plugins' trust in effect. Workspace trust is the
/// shell's state, decided by the trust module (no file is read or hashed here, so a settings
/// change on the UI thread stays cheap - review finding F6 of Charge 5), and asked only when a
/// project plugin is served. A plugin disabled (`plugins.disabled`, it stops loading at the next
/// start) or a project plugin without trust leaves no lasting emulation on the views
/// ([`services::resets`]). What the panels offered depend on now comes back.
fn apply_switches(core: &Core, runtime: &PluginRuntime) -> panel_list::Inputs {
    let user = core.user_settings();
    let off: BTreeSet<String> = user
        .plugins
        .page_scripts_disabled
        .iter()
        .map(|id| id.0.clone())
        .collect();
    let trusted = runtime.has_project_plugins() && core.host().shell_state().trusted;
    runtime.set_enabled(&off, trusted);
    let plugins = runtime.plugins();
    let stopped = plugins.iter().filter(|plugin| {
        user.plugins.disabled.iter().any(|id| id.0 == plugin.id)
            || !runtime::may_run(plugin.source, trusted)
    });
    services::shared().reset_cdp(stopped.map(|plugin| plugin.id.as_str()));
    // The plugins' generation too: a live change is news for the panels (RB-092-06).
    panel_list::Inputs::of(&user, trusted).at(crate::analyzer::plugins::generation())
}

/// Tells the shell pages that the panels offered changed when `now` is news for `core`'s window
/// (RB-080-97): the first time, then a panel hidden or shown, or the project plugins' trust.
fn announce_panels(core: &Core, now: panel_list::Inputs) {
    let news = lock(registry())
        .iter_mut()
        .find(|entry| std::ptr::eq(entry.host.as_ptr(), Arc::as_ptr(core.host())))
        .is_some_and(|entry| panel_list::take(&mut entry.panels, now));
    if news {
        core.send(&CoreToShell::PluginPanel {
            message: panel_list::changed_message(),
        });
    }
}

/// Injects the page scripts into every view's page session - now, and after every attach.
fn follow_views(core: &Core, pages: &Arc<PluginPages>) {
    let host = Arc::downgrade(core.host());
    let mut events = core.host().subscribe();
    let pages = Arc::downgrade(pages);
    core.runtime().spawn(async move {
        sync_all(&host, &pages).await;
        loop {
            let event = match events.recv().await {
                Ok(event) => event,
                Err(RecvError::Lagged(_)) => {
                    sync_all(&host, &pages).await;
                    continue;
                }
                Err(RecvError::Closed) => return,
            };
            match event {
                ViewEvent::Opened(view) | ViewEvent::Updated(view) => {
                    sync_view(&host, &pages, view).await;
                }
                ViewEvent::Closed(view) => {
                    page_log::forget_view(view);
                    if let Some(pages) = pages.upgrade() {
                        pages.forget(view);
                    }
                }
                _ => {}
            }
        }
    });
}

async fn sync_all(host: &Weak<Host>, pages: &Weak<PluginPages>) {
    let sessions = host
        .upgrade()
        .map(|host| host.cdp_sessions())
        .unwrap_or_default();
    for (view, _) in sessions {
        sync_view(host, pages, view).await;
    }
}

/// Attaches `view`'s page session when the page scripts are not in it yet.
async fn sync_view(host: &Weak<Host>, pages: &Weak<PluginPages>, view: ViewId) {
    let (Some(host), Some(pages)) = (host.upgrade(), pages.upgrade()) else {
        return;
    };
    let session = host
        .cdp_sessions()
        .into_iter()
        .find(|(id, _)| *id == view)
        .map(|(_, session)| session);
    if let Some(session) = session
        && pages.session_of(view).as_deref() != Some(session.as_str())
    {
        pages.attach(view, &session).await;
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
