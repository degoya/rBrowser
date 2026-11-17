//! A plugin's notices in the status bar (RB-080-64): an answer's post `{"target": "status",
//! "message": {"key", "params", "level"}}` (`rb_plugin_host::PostTarget::Status`) shows the
//! plugin's own text there, under its name - the colour picker says "picking", "picked #1e88e5"
//! and "stopped", however the pipette was started or ended.
//!
//! Review S1: only a plugin granted the capability `status`; only a key below `plugin.<id>.`
//! that the plugin's catalogue has, with at most [`MAX_PARAMS`] short parameters; `info` or
//! `success`, never a warning or an error a plugin could dress up as the app's; control and
//! format characters (line breaks, bidi overrides) taken out and the text cut to
//! [`MAX_TEXT_CHARS`]; at most one notice per plugin a second, the newest replacing one that
//! waits ([`Throttle`]). Anything else shows nothing and is logged at debug level. A command
//! whose answer posted a notice that was shown gets no generic "done".
//!
//! `host` posts (`rb_plugin_host::PostTarget::Host`) go on only from a command's answer of a
//! bundled plugin (review S3) - never from a tool, a page request or a plugin of the same id
//! installed by a user or a project.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rb_ipc::{Notification, NotificationLevel};
use rb_plugin_host::{PluginSource, PostTarget};
use serde_json::Value;

use super::runtime::{PanelSink, Plugin};
use crate::cdp::lock;

/// The core text a plugin's notice is shown in: `{plugin}: {text}`.
pub const PLUGIN_STATUS_KEY: &str = "plugins.notice.plugin_status";
/// The most parameters of one notice.
pub const MAX_PARAMS: usize = 8;
/// The longest parameter value, in characters.
pub const MAX_PARAM_CHARS: usize = 96;
/// The longest notice text, in characters.
pub const MAX_TEXT_CHARS: usize = 200;
/// The shortest time between two notices of one plugin.
pub const NOTICE_INTERVAL: Duration = Duration::from_secs(1);

/// What shows a plugin's checked notice in the status bar.
pub type NoticeSink = Arc<dyn Fn(&str, Notification) + Send + Sync>;
/// What runs a plugin's `host` post ([`super::host_actions`]).
pub type HostSink = Arc<dyn Fn(&str, &Value) + Send + Sync>;

/// Where the posts of the runtime's answers go besides the page scripts.
#[derive(Clone)]
pub struct Sinks {
    pub panels: PanelSink,
    pub status: NoticeSink,
    /// `host` posts: the actions the application offers a plugin ([`super::host_actions`]).
    pub host: HostSink,
}

/// Hands a `status` or `host` post of `served` on; whether a notice was shown. `command`: the
/// post is in the answer of one of the plugin's commands.
pub(super) fn side_post(
    served: Option<&Plugin>,
    sinks: &Sinks,
    target: PostTarget,
    message: &Value,
    command: bool,
) -> bool {
    let Some(plugin) = served else {
        return false;
    };
    match target {
        PostTarget::Status if plugin.status => match notification_of(&plugin.id, message) {
            Some(notice) => {
                (sinks.status)(&plugin.id, notice);
                true
            }
            None => false,
        },
        PostTarget::Host if command && plugin.source == PluginSource::Bundled => {
            (sinks.host)(&plugin.id, message);
            false
        }
        _ => {
            tracing::debug!(
                plugin_id = plugin.id.as_str(),
                "a plugin's status or host post it may not send; ignored"
            );
            false
        }
    }
}

/// The notification of plugin `plugin`'s status post `message`; `None` for one that does not
/// hold (logged at debug).
#[must_use]
pub fn notification_of(plugin: &str, message: &Value) -> Option<Notification> {
    let notice = read(plugin, message);
    if notice.is_none() {
        tracing::debug!(
            plugin_id = plugin,
            "a plugin's status notice does not hold; not shown"
        );
    }
    notice
}

/// `text` without control and format characters (line breaks, bidi overrides, zero widths).
#[must_use]
pub fn clean(text: &str) -> String {
    text.chars()
        .filter(|c| {
            !c.is_control()
                && !matches!(c, '\u{00ad}' | '\u{061c}' | '\u{180e}' | '\u{200b}'..='\u{200f}'
                    | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{feff}'
                    | '\u{fff9}'..='\u{fffb}')
        })
        .collect()
}

fn read(plugin: &str, message: &Value) -> Option<Notification> {
    let key = message.get("key")?.as_str()?;
    let own = key
        .strip_prefix("plugin.")
        .and_then(|rest| rest.strip_prefix(plugin))
        .is_some_and(|rest| rest.starts_with('.'));
    if !own || !rb_i18n::is_well_formed_key(key) {
        return None;
    }
    let text = rb_i18n::plugin_text(plugin, key)?;
    let level = match message.get("level").and_then(Value::as_str) {
        None | Some("info") => NotificationLevel::Info,
        Some("success") => NotificationLevel::Success,
        Some(_) => return None,
    };
    let params = match message.get("params") {
        None | Some(Value::Null) => serde_json::Map::new(),
        Some(Value::Object(params)) if params.len() <= MAX_PARAMS => params.clone(),
        Some(_) => return None,
    };
    let mut values = Vec::with_capacity(params.len());
    for (name, value) in &params {
        let value = match value {
            Value::String(text) => clean(text),
            Value::Number(number) => number.to_string(),
            _ => return None,
        };
        if name.chars().count() > 64 || value.chars().count() > MAX_PARAM_CHARS {
            return None;
        }
        values.push((name.as_str(), value));
    }
    let args: Vec<(&str, &str)> = values
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .collect();
    let text: String = clean(&rb_i18n::format_message(&text, &args))
        .chars()
        .take(MAX_TEXT_CHARS)
        .collect();
    let name = rb_i18n::plugin_text(plugin, &format!("plugin.{plugin}.name"))
        .map_or_else(|| plugin.to_owned(), |name| clean(&name));
    Some(Notification {
        level,
        message_key: PLUGIN_STATUS_KEY.to_owned(),
        args: BTreeMap::from([("plugin".to_owned(), name), ("text".to_owned(), text)]),
        settings_key: None,
    })
}

/// What [`Throttle::offer`] decided for a notice.
#[derive(Debug, PartialEq)]
pub enum Offered {
    /// Show it now.
    Now(Notification),
    /// It waits; show what [`Throttle::take`] gives after this long.
    Later(Duration),
    /// It replaced a notice already waiting.
    Replaced,
}

/// At most one notice per plugin per [`NOTICE_INTERVAL`]; within it the newest waits, replacing
/// an older one that waits.
#[derive(Debug, Default)]
pub struct Throttle {
    plugins: Mutex<HashMap<String, (Instant, Option<Notification>)>>,
}

impl Throttle {
    /// Plugin `plugin`'s `notice` at `now`.
    pub fn offer(&self, plugin: &str, notice: Notification, now: Instant) -> Offered {
        let mut plugins = lock(&self.plugins);
        match plugins.get_mut(plugin) {
            Some((shown, waiting)) if now.duration_since(*shown) < NOTICE_INTERVAL => {
                let replaced = waiting.replace(notice).is_some();
                if replaced {
                    Offered::Replaced
                } else {
                    Offered::Later(NOTICE_INTERVAL - now.duration_since(*shown))
                }
            }
            _ => {
                plugins.insert(plugin.to_owned(), (now, None));
                Offered::Now(notice)
            }
        }
    }

    /// The notice of `plugin` that waited, now shown at `now`.
    pub fn take(&self, plugin: &str, now: Instant) -> Option<Notification> {
        let mut plugins = lock(&self.plugins);
        let (shown, waiting) = plugins.get_mut(plugin)?;
        let notice = waiting.take()?;
        *shown = now;
        Some(notice)
    }
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;
