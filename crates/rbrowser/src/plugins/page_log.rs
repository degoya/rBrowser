//! What the plugins' page scripts log (RB-080-66): `rbPlugin.log` and the `console` of their
//! isolated worlds reach the host as a [`PageLog`] (`rb_plugin_host`, the binding's `l:`), never
//! the page's console - rBrowser's messages are not the page's, and the page's console counted
//! them as its own ("[rulers-guides] plugins.error.busy", 39 times, "unknown source").
//!
//! A line becomes a sentence ([`sentence`]): a failure code read from the plugin's own catalogue
//! (`plugin.<id>.*`) or the core's (`plugins.error.*`), the plugin's own `[<id>]` prefix dropped
//! (the source names it). It goes to the rBrowser log, at most [`LOG_BURST`] lines per plugin
//! in [`LOG_WINDOW`] ([`LogGate`]), and to the shared console's feed with the source `rbrowser`
//! ([`PluginLines`], `console::FEED_KEY`), where the console plugin shows it filterable.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rb_diag::page_text::redact_page_text;
use rb_engine::ViewId;
use rb_plugin_host::{LogLevel, PageLog};
use serde_json::{Value, json};

use crate::cdp::lock;

/// The source of a plugin's line in the console feed.
pub const SOURCE: &str = "rbrowser";
/// The most lines of one plugin kept for the console feed, all views together; older ones drop
/// out. Each plugin has its own share, so a chatty one never evicts another's (review of
/// RB-080-66, 8).
pub const MAX_LINES_PER_PLUGIN: usize = 100;
/// The most lines of one plugin written to the rBrowser log per [`LOG_WINDOW`].
pub const LOG_BURST: u32 = 20;
/// The window of [`LOG_BURST`].
pub const LOG_WINDOW: Duration = Duration::from_secs(60);

/// `values` of plugin `plugin`'s line as one sentence: codes read, the `[<plugin>]` prefix
/// dropped, control characters as spaces, credentials masked.
#[must_use]
pub fn sentence(plugin: &str, values: &[String]) -> String {
    let prefix = format!("[{plugin}]");
    let mut values = values.iter().map(String::as_str).peekable();
    if values.peek() == Some(&prefix.as_str()) {
        values.next();
    }
    let text = values
        .map(|value| text_of(plugin, value).unwrap_or_else(|| value.to_owned()))
        .collect::<Vec<_>>()
        .join(" ");
    // Untrusted text: one line (no forged log lines), credentials masked as every page text.
    let line: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    redact_page_text(&line)
}

/// The text of `value` when it is a code with one: the plugin's own (`plugin.<plugin>.*`) from
/// its catalogue, a host code (`plugins.error.*`) from the core's. Another plugin's code stays
/// as it is, so no plugin speaks with another's voice.
fn text_of(plugin: &str, value: &str) -> Option<String> {
    if !is_key(value) {
        return None;
    }
    let own = value
        .strip_prefix("plugin.")
        .and_then(|rest| rest.strip_prefix(plugin))
        .is_some_and(|rest| rest.starts_with('.'));
    if own {
        return rb_i18n::plugin_text(plugin, value);
    }
    if value.starts_with("plugins.error.") {
        let text = rb_i18n::t(value);
        return (text != value).then_some(text);
    }
    None
}

/// Dotted lower-case words: what a code looks like.
fn is_key(value: &str) -> bool {
    (3..=200).contains(&value.len())
        && value.contains('.')
        && value
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'))
}

/// How many lines of one plugin went to the log in the current window.
#[derive(Clone, Copy, Debug)]
pub struct LogGate {
    since: Instant,
    written: u32,
}

/// Whether a line goes to the log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Admit {
    Write,
    /// The first line over the burst: it is not written, a note says the rest are not.
    FirstHeld,
    Held,
}

impl LogGate {
    #[must_use]
    pub const fn new(now: Instant) -> Self {
        Self {
            since: now,
            written: 0,
        }
    }

    /// Counts a line at `now`.
    pub fn admit(&mut self, now: Instant) -> Admit {
        if now.duration_since(self.since) >= LOG_WINDOW {
            *self = Self::new(now);
        }
        self.written = self.written.saturating_add(1);
        match self.written {
            n if n <= LOG_BURST => Admit::Write,
            n if n == LOG_BURST + 1 => Admit::FirstHeld,
            _ => Admit::Held,
        }
    }
}

/// The lines the console feed shows, per plugin, newest last, at most [`MAX_LINES_PER_PLUGIN`]
/// each.
#[derive(Debug, Default)]
pub struct PluginLines {
    lines: BTreeMap<String, VecDeque<(ViewId, Value)>>,
}

impl PluginLines {
    /// Keeps a line of `plugin` in `view` at `time` (milliseconds since the epoch); `stack` for a
    /// page script's failure.
    pub fn push(
        &mut self,
        view: ViewId,
        plugin: &str,
        level: LogLevel,
        text: &str,
        stack: Option<&str>,
        time: f64,
    ) {
        let own = self.lines.entry(plugin.to_owned()).or_default();
        if own.len() == MAX_LINES_PER_PLUGIN {
            own.pop_front();
        }
        let mut line = json!({
            "level": level.as_str(),
            "source": SOURCE,
            "plugin": plugin,
            "text": text,
            "url": format!("rbrowser://plugin/{plugin}"),
            "time": time,
        });
        if let Some(stack) = stack {
            line["stack"] = json!(stack);
        }
        own.push_back((view, line));
    }

    /// The lines of `view`, oldest first.
    #[must_use]
    pub fn of_view(&self, view: ViewId) -> Vec<Value> {
        let mut found: Vec<Value> = self
            .lines
            .values()
            .flatten()
            .filter(|(owner, _)| *owner == view)
            .map(|(_, line)| line.clone())
            .collect();
        found.sort_by(|a, b| {
            let time = |line: &Value| line["time"].as_f64().unwrap_or_default();
            time(a).total_cmp(&time(b))
        });
        found
    }

    /// The view is closed: its lines go.
    pub fn forget_view(&mut self, view: ViewId) {
        for own in self.lines.values_mut() {
            own.retain(|(owner, _)| *owner != view);
        }
        self.lines.retain(|_, own| !own.is_empty());
    }
}

fn lines() -> &'static Mutex<PluginLines> {
    static LINES: Mutex<PluginLines> = Mutex::new(PluginLines {
        lines: BTreeMap::new(),
    });
    &LINES
}

/// The lines of `view` for the console feed.
#[must_use]
pub fn lines_of(view: ViewId) -> Vec<Value> {
    lock(lines()).of_view(view)
}

/// View `view` closed: its lines leave the console feed.
pub fn forget_view(view: ViewId) {
    lock(lines()).forget_view(view);
}

/// Takes a line plugin `plugin`'s page script in `view` logged: to the rBrowser log (within the
/// gate) and to the console feed.
pub fn record(plugin: &str, view: ViewId, line: &PageLog) {
    let text = sentence(plugin, &line.values);
    // A page script's failure (a stack) has its own share of the log, so its chatter never hides
    // it (review of RB-080-66, 4).
    let stack = line
        .stack
        .as_deref()
        .map(|stack| redact_page_text(&stack.replace('\r', "")));
    let key = if stack.is_some() {
        format!("{plugin}#failure")
    } else {
        plugin.to_owned()
    };
    match admit(&key) {
        Admit::Write => write(plugin, view, line.level, &text, stack.as_deref()),
        Admit::FirstHeld => tracing::info!(
            plugin_id = plugin,
            burst = LOG_BURST,
            "a plugin's page script logs more; further lines this minute go to the console only"
        ),
        Admit::Held => {}
    }
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |since| since.as_secs_f64() * 1000.0);
    lock(lines()).push(view, plugin, line.level, &text, stack.as_deref(), time);
}

/// Counts a log line under `key` (a plugin's id, or its id and a topic) in its [`LogGate`].
pub fn admit(key: &str) -> Admit {
    static GATES: Mutex<Option<HashMap<String, LogGate>>> = Mutex::new(None);
    let now = Instant::now();
    let mut gates = lock(&GATES);
    gates
        .get_or_insert_with(HashMap::new)
        .entry(key.to_owned())
        .or_insert_with(|| LogGate::new(now))
        .admit(now)
}

fn write(plugin: &str, view: ViewId, level: LogLevel, text: &str, stack: Option<&str>) {
    let view = view.0;
    if let Some(stack) = stack {
        tracing::warn!(plugin_id = plugin, view, %stack, "plugin page script failed: {text}");
        return;
    }
    match level {
        LogLevel::Error | LogLevel::Warning => {
            tracing::warn!(
                plugin_id = plugin,
                view,
                level = level.as_str(),
                "plugin page script: {text}"
            );
        }
        LogLevel::Info | LogLevel::Log => {
            tracing::info!(plugin_id = plugin, view, "plugin page script: {text}");
        }
        LogLevel::Debug => tracing::debug!(plugin_id = plugin, view, "plugin page script: {text}"),
    }
}

#[cfg(test)]
#[path = "page_log_tests.rs"]
mod tests;
