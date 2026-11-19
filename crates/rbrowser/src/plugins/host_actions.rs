//! The actions the application offers a plugin through a `host` post (RB-080-74,
//! `rb_plugin_host::PostTarget::Host`): `{"target": "host", "message": {"action": "<name>",
//! ...}}`. Only the pairs of [`OFFERED`] run - a plugin can ask for nothing else, and no plugin
//! for another's action; anything else is logged at debug level and does nothing.
//!
//! **`image-overlay` / `add_design`**: the image overlay's *Add design...* (its panel button and
//! command). A sandboxed panel can open no dialog and the plugin cannot write into the project,
//! so the core does it: only with an open project in a trusted workspace (it writes into the
//! project), one native file dialog at a time ([`PickGuard`]) and none again within
//! [`COOLDOWN`] of a cancel; off the UI thread the file is checked and copied into
//! `.rbrowser/designs/` ([`add_design`]), then the overlay's own tool `set` chooses it for the
//! active view's breakpoint and shows it. Each outcome - a cancel included - is one notice in
//! the status bar and the same sentence in the panel (`hostNotice`); no path reaches a log.
//!
//! **`form-autofill` / `record_fill`** (RB-090-78): the fields a fill set in one view, for a
//! running recording ([`crate::recorder::autofill`]). It opens nothing and asks no one, so it runs
//! from the plugin's tool answers too ([`GESTURELESS`]) - an autofill from its panel or MCP is
//! recorded as one from its command. Never from a page request.

pub mod add_design;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use rb_engine::{ViewHost as _, ViewId};
use rb_ipc::{CoreToShell, Notification, NotificationLevel, PathKind};
use rb_plugin_types::{Failure, FailureKind, codes};
use serde_json::{Value, json};

use crate::control::Core;
use crate::host::ui::PathPick;
use add_design::{Added, Refusal};

/// The image overlay's plugin id.
pub const IMAGE_OVERLAY: &str = "image-overlay";
/// Its action that adds a design.
pub const ADD_DESIGN: &str = "add_design";
/// The overlay's tool that chooses and shows a design.
pub const SET_TOOL: &str = "set";

/// The form autofill's plugin id.
pub const FORM_AUTOFILL: &str = "form-autofill";

/// The actions offered, as (plugin id, action).
pub const OFFERED: &[(&str, &str)] = &[
    (IMAGE_OVERLAY, ADD_DESIGN),
    (FORM_AUTOFILL, crate::recorder::autofill::RECORD_FILL),
];
/// The offered actions that need no person's gesture: they run from a tool's answer as well as
/// from a command's.
pub const GESTURELESS: &[(&str, &str)] = &[(FORM_AUTOFILL, crate::recorder::autofill::RECORD_FILL)];

/// The file dialog's title.
pub const PICK_TITLE_KEY: &str = "plugins.add_design.pick_title";
/// No project is open.
pub const NO_PROJECT_KEY: &str = "plugins.add_design.no_project";
/// The workspace is not trusted.
pub const UNTRUSTED_KEY: &str = "plugins.add_design.untrusted";
/// A dialog is open already.
pub const BUSY_KEY: &str = "plugins.add_design.busy";
/// Added and shown for `{breakpoint}`.
pub const ADDED_KEY: &str = "plugins.add_design.added";
/// Added under another name.
pub const ADDED_AS_KEY: &str = "plugins.add_design.added_as";
/// Added; the active view has no breakpoint, so the nearest width chooses.
pub const ADDED_NEAREST_KEY: &str = "plugins.add_design.added_nearest";
/// Added, but the overlay did not take it.
pub const NOT_SHOWN_KEY: &str = "plugins.add_design.not_shown";
/// Refused: no PNG, JPEG or WebP.
pub const BAD_TYPE_KEY: &str = "plugins.add_design.bad_type";
/// Refused: over 8 MiB.
pub const TOO_LARGE_KEY: &str = "plugins.add_design.too_large";
/// Refused: too many pixels.
pub const TOO_MANY_PIXELS_KEY: &str = "plugins.add_design.too_many_pixels";
/// The picked file could not be read.
pub const UNREADABLE_KEY: &str = "plugins.add_design.unreadable";
/// The copy could not be written.
pub const NOT_WRITTEN_KEY: &str = "plugins.add_design.not_written";
/// The same bytes were in the project already; shown for `{breakpoint}`.
pub const ALREADY_KEY: &str = "plugins.add_design.already";
/// The same bytes were there already; the nearest width chooses.
pub const ALREADY_NEAREST_KEY: &str = "plugins.add_design.already_nearest";
/// The dialog was cancelled.
pub const CANCELLED_KEY: &str = "plugins.add_design.cancelled";
/// Asked again right after a cancel ([`COOLDOWN`]).
pub const WAIT_KEY: &str = "plugins.add_design.wait";

/// How long after a cancelled dialog `add_design` does not open it again: a plugin (or a key held
/// down) cannot reopen it over and over (review S3 of RB-080-64/74).
pub const COOLDOWN: Duration = Duration::from_secs(2);

/// The action `message` names, when plugin `plugin` is offered it.
#[must_use]
pub fn offered<'a>(plugin: &str, message: &'a Value) -> Option<&'a str> {
    let action = message.get("action")?.as_str()?;
    OFFERED
        .iter()
        .any(|(id, name)| *id == plugin && *name == action)
        .then_some(action)
}

/// Whether plugin `plugin`'s `host` post `message` names an action of [`GESTURELESS`].
#[must_use]
pub fn gestureless(plugin: &str, message: &Value) -> bool {
    let action = message.get("action").and_then(Value::as_str);
    GESTURELESS
        .iter()
        .any(|(id, name)| *id == plugin && action == Some(*name))
}

/// Runs plugin `plugin`'s `host` post `message`.
pub fn run(core: &Arc<Core>, plugin: &str, message: &Value) {
    match offered(plugin, message) {
        Some(ADD_DESIGN) => start_add_design(core),
        Some(crate::recorder::autofill::RECORD_FILL) => {
            crate::recorder::autofill::record(core, message);
        }
        _ => tracing::debug!(
            plugin_id = plugin,
            "a plugin asked for a host action it is not offered; ignored"
        ),
    }
}

/// The project root a design is written into: only an open project in a trusted workspace.
///
/// # Errors
///
/// The key of the notice that says why not.
pub fn admit(root: Option<PathBuf>, trusted: bool) -> Result<PathBuf, &'static str> {
    let root = root.ok_or(NO_PROJECT_KEY)?;
    if trusted {
        Ok(root)
    } else {
        Err(UNTRUSTED_KEY)
    }
}

static PICKING: AtomicBool = AtomicBool::new(false);

/// The one file dialog of `add_design`: held from the dialog's opening to its answer.
#[derive(Debug)]
pub struct PickGuard(());

impl PickGuard {
    /// The guard, unless a dialog is open.
    #[must_use]
    pub fn take() -> Option<Self> {
        (!PICKING.swap(true, Ordering::AcqRel)).then_some(Self(()))
    }
}

impl Drop for PickGuard {
    fn drop(&mut self) {
        PICKING.store(false, Ordering::Release);
    }
}

/// When the last dialog was cancelled.
#[derive(Clone, Copy, Debug, Default)]
pub struct Cooldown {
    cancelled: Option<Instant>,
}

impl Cooldown {
    /// The dialog was cancelled at `now`.
    pub fn cancelled(&mut self, now: Instant) {
        self.cancelled = Some(now);
    }

    /// Whether `now` is within [`COOLDOWN`] of the last cancel.
    #[must_use]
    pub fn cooling(&self, now: Instant) -> bool {
        self.cancelled
            .is_some_and(|at| now.saturating_duration_since(at) < COOLDOWN)
    }
}

static COOLING: Mutex<Cooldown> = Mutex::new(Cooldown { cancelled: None });

/// The breakpoint a design is chosen for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    /// The breakpoint id, as the overlay's `designs` take it.
    pub id: String,
    /// The view's label, for the notice.
    pub label: String,
}

/// Whether `id` is a breakpoint id the overlay takes: 1 to 64 of `[A-Za-z0-9_-]`.
fn breakpoint_ok(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// The breakpoint of the `active` view among `views` (id, breakpoint, label); none for a free
/// size, no active view or an id the overlay does not take.
#[must_use]
pub fn active_target(
    views: impl IntoIterator<Item = (ViewId, Option<String>, String)>,
    active: Option<ViewId>,
) -> Option<Target> {
    let active = active?;
    let (_, breakpoint, label) = views.into_iter().find(|(id, _, _)| *id == active)?;
    let id = breakpoint.filter(|id| breakpoint_ok(id))?;
    Some(Target { id, label })
}

/// The arguments of the overlay's `set`: shown, and the design `name` for `breakpoint`.
#[must_use]
pub fn set_args(breakpoint: Option<&str>, name: &str) -> Value {
    match breakpoint {
        Some(id) => json!({ "enabled": true, "designs": { id: format!("designs/{name}") } }),
        None => json!({ "enabled": true }),
    }
}

/// One outcome, for the status bar and the panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub level: NotificationLevel,
    pub key: &'static str,
    pub args: Vec<(&'static str, String)>,
}

impl Notice {
    fn new(level: NotificationLevel, key: &'static str, args: Vec<(&'static str, String)>) -> Self {
        Self { level, key, args }
    }

    /// Whether it reports a refusal or a failure.
    #[must_use]
    pub fn failed(&self) -> bool {
        matches!(
            self.level,
            NotificationLevel::Error | NotificationLevel::Warning
        )
    }

    /// The status bar's notification.
    #[must_use]
    pub fn notification(&self) -> Notification {
        Notification {
            level: self.level,
            message_key: self.key.to_owned(),
            args: self
                .args
                .iter()
                .map(|(name, value)| ((*name).to_owned(), value.clone()))
                .collect(),
            settings_key: None,
        }
    }

    /// The overlay panel's event carrying `text`, this notice translated.
    #[must_use]
    pub fn panel_event(&self, text: &str) -> Value {
        super::panels::event_message(
            IMAGE_OVERLAY,
            json!({ "message": { "hostNotice": { "text": text, "failed": self.failed() } } }),
        )
    }
}

/// The notice of a picked file `name` refused for `refusal`.
#[must_use]
pub fn refused_notice(name: &str, refusal: Refusal) -> Notice {
    let key = match refusal {
        Refusal::BadType => BAD_TYPE_KEY,
        Refusal::TooLarge => TOO_LARGE_KEY,
        Refusal::TooManyPixels => TOO_MANY_PIXELS_KEY,
        Refusal::Unreadable => UNREADABLE_KEY,
        Refusal::NotWritten => NOT_WRITTEN_KEY,
    };
    Notice::new(
        NotificationLevel::Error,
        key,
        vec![("name", name.to_owned())],
    )
}

/// The notice of a cancelled dialog: it also ends the panel's "Adding" line.
#[must_use]
pub fn cancelled_notice() -> Notice {
    Notice::new(NotificationLevel::Info, CANCELLED_KEY, Vec::new())
}

/// The notice of an `add_design` within [`COOLDOWN`] of a cancel.
#[must_use]
pub fn cooling_notice() -> Notice {
    Notice::new(NotificationLevel::Warning, WAIT_KEY, Vec::new())
}

/// The notice of `added`, shown for `target` (none: the nearest width chooses).
#[must_use]
pub fn added_notice(added: &Added, target: Option<&Target>) -> Notice {
    let name = ("name", added.name.clone());
    let Some(target) = target else {
        let key = if added.reused {
            ALREADY_NEAREST_KEY
        } else {
            ADDED_NEAREST_KEY
        };
        return Notice::new(NotificationLevel::Success, key, vec![name]);
    };
    let breakpoint = ("breakpoint", target.label.clone());
    if added.reused {
        Notice::new(
            NotificationLevel::Success,
            ALREADY_KEY,
            vec![name, breakpoint],
        )
    } else if added.renamed() {
        let original = ("original", added.original.clone());
        Notice::new(
            NotificationLevel::Success,
            ADDED_AS_KEY,
            vec![name, original, breakpoint],
        )
    } else {
        Notice::new(
            NotificationLevel::Success,
            ADDED_KEY,
            vec![name, breakpoint],
        )
    }
}

/// The notice of `added` that the overlay did not take, for `error`.
#[must_use]
pub fn not_shown_notice(added: &Added, error: String) -> Notice {
    Notice::new(
        NotificationLevel::Error,
        NOT_SHOWN_KEY,
        vec![("name", added.name.clone()), ("error", error)],
    )
}

/// Shows `notice` in the status bar and the overlay's panel.
fn tell(core: &Core, notice: &Notice) {
    let args: Vec<(&str, &str)> = notice
        .args
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .collect();
    let text = rb_i18n::t_with(notice.key, &args);
    core.send(&CoreToShell::PluginPanel {
        message: notice.panel_event(&text),
    });
    core.notify(CoreToShell::Notification(notice.notification()));
}

fn admitted(core: &Core) -> Result<PathBuf, &'static str> {
    let host = core.host();
    admit(host.project_root(), host.shell_state().trusted)
}

/// `add_design`: the native file dialog, when a design may be added.
fn start_add_design(core: &Arc<Core>) {
    if let Err(key) = admitted(core) {
        tell(
            core,
            &Notice::new(NotificationLevel::Warning, key, Vec::new()),
        );
        return;
    }
    if rb_util::lock(&COOLING).cooling(Instant::now()) {
        tell(core, &cooling_notice());
        return;
    }
    let Some(guard) = PickGuard::take() else {
        tell(
            core,
            &Notice::new(NotificationLevel::Warning, BUSY_KEY, Vec::new()),
        );
        return;
    };
    let me: Weak<Core> = Arc::downgrade(core);
    let pick = PathPick {
        kind: PathKind::File,
        title: rb_i18n::t(PICK_TITLE_KEY),
        save_as: None,
    };
    core.ui().pick_path(
        pick,
        Box::new(move |picked| {
            drop(guard);
            let Some(core) = me.upgrade() else {
                return;
            };
            let Some(picked) = picked else {
                rb_util::lock(&COOLING).cancelled(Instant::now());
                tell(&core, &cancelled_notice());
                return;
            };
            let worker = Arc::clone(&core);
            core.runtime()
                .spawn(async move { finish(&worker, picked).await });
        }),
    );
}

/// Checks and copies `picked` off the UI thread, then shows it through the overlay's `set`.
async fn finish(core: &Arc<Core>, picked: PathBuf) {
    // The project or its trust may have changed while the dialog was open.
    let root = match admitted(core) {
        Ok(root) => root,
        Err(key) => {
            tell(
                core,
                &Notice::new(NotificationLevel::Warning, key, Vec::new()),
            );
            return;
        }
    };
    let picked_name = picked
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let copied = tokio::task::spawn_blocking(move || add_design::add(&root, &picked)).await;
    let added = match copied {
        Ok(Ok(added)) => added,
        Ok(Err((name, refusal))) => {
            tracing::info!(?refusal, "a design was not added");
            tell(core, &refused_notice(&name, refusal));
            return;
        }
        Err(_) => {
            tell(core, &refused_notice(&picked_name, Refusal::NotWritten));
            return;
        }
    };
    tracing::info!(
        renamed = added.renamed(),
        "a design was added to the project"
    );
    let host = core.host();
    let views = host
        .views()
        .into_iter()
        .map(|view| (view.id, view.breakpoint, view.label));
    let target = active_target(views, host.active_view());
    let args = set_args(
        target.as_ref().map(|target| target.id.as_str()),
        &added.name,
    );
    let shown = match super::for_host(host) {
        Some(runtime) => runtime
            .call_tool(IMAGE_OVERLAY, SET_TOOL, &args)
            .await
            .map(|_| ()),
        None => Err(Failure::coded(FailureKind::NotFound, codes::NOT_AVAILABLE)
            .with_param("service", IMAGE_OVERLAY)),
    };
    let notice = match shown {
        Ok(()) => added_notice(&added, target.as_ref()),
        Err(failure) => not_shown_notice(
            &added,
            super::commands::failure_message(IMAGE_OVERLAY, &failure),
        ),
    };
    tell(core, &notice);
}

#[cfg(test)]
#[path = "host_actions_tests.rs"]
mod tests;
