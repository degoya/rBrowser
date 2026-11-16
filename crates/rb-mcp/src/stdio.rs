//! `rbrowser mcp`: a stdio front end that forwards to the running rBrowser of a project
//! (RB-030-02).
//!
//! [`Locator`] finds the instance through its file and [`instance::probe`]; [`forward`] passes
//! every JSON-RPC line from stdin to the server's `/mcp` with the bearer token and writes the
//! answers back as lines on stdout - nothing else ever reaches stdout. From WSL this is how an
//! agent reaches the Windows app: stdio crosses the interop, where WSL2's NAT would hide
//! Windows' localhost.
//!
//! The bridge never gives up while stdin is open. Without a reachable instance it answers
//! itself ([`offline`]): `initialize` succeeds and every other request gets an error that says
//! what to do (K7). Before a message goes out the instance is probed again when the last probe
//! is older than [`ALIVE_EVERY`] (after a crash whoever took the port over never sees the
//! token); a lost one - the probe fails, the connection is refused, the token is no longer
//! taken - is looked for again and the message goes to the new one, so a restarted rBrowser
//! answers the next call without restarting the client. The server is stateless, so the new
//! instance needs no second `initialize`. A message that may have run (the answer broke off) is
//! never sent twice: it is answered with an error. An answer that cannot be read (too large,
//! cut off) answers its request with an error and the forward goes on.
//!
//! The server is stateless and cannot notify, so the bridge sends
//! `notifications/tools/list_changed` for it: after the first message that reaches rBrowser
//! once the client was answered offline or the instance was lost (the tools it knows may be
//! none or another instance's), and after an answer whose [`TOOLS_REVISION`] differs from the
//! one of the last `tools/list` (plugin tools came or went). Never after `initialize`, so it
//! never comes before the client said it is initialized.

pub mod locate;
pub mod offline;

use std::io::{BufRead, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

pub use self::locate::{Locator, same_root};
use crate::http::TOOLS_REVISION;
use crate::instance::{self, Instance};

/// How old the last probe may be before a message goes out.
pub const ALIVE_EVERY: Duration = Duration::from_secs(3);
/// How long one request may take (a navigation waits for its loads).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(600);
/// The most bytes read of one answer.
const MAX_ANSWER: u64 = 64 * 1024 * 1024;

/// Why `rbrowser mcp` cannot reach rBrowser.
#[derive(Debug, thiserror::Error)]
pub enum StdioError {
    /// No live instance for the project (or none at all).
    #[error(
        "no running rBrowser instance for {0}; start it with `rbrowser open --project <dir>` and \
         switch on Settings > Automation > MCP server (Ctrl+Shift+Space , M)"
    )]
    NoInstance(String),
    /// Several live instances and no project to choose.
    #[error("several rBrowser windows run ({0}); pass --project DIR or run inside the project")]
    Several(String),
    /// stdin or stdout failed.
    #[error("stdio: {0}")]
    Io(#[from] std::io::Error),
}

/// The live instance of the project with `key` (the hash of its root) at `project`, or -
/// without a project - the one live instance there is ([`Locator::find`]).
///
/// # Errors
///
/// [`StdioError::NoInstance`] or [`StdioError::Several`].
pub fn find(
    local: &Path,
    key: Option<&str>,
    project: Option<&str>,
) -> Result<Instance, StdioError> {
    Locator::new(local, key, project).find()
}

/// The notification the bridge sends when the client's tool list may be out of date.
const LIST_CHANGED: &str = r#"{"jsonrpc":"2.0","method":"notifications/tools/list_changed"}"#;

/// What sending one message came to.
enum Sent {
    /// The answers to write (none for a notification) and the tool revision they came with.
    Answers(Vec<Value>, Option<String>),
    /// The instance is gone and the message never ran there: look again and send it again.
    Lost(String),
}

/// The bridge's state between messages.
struct Bridge {
    agent: ureq::Agent,
    current: Option<Instance>,
    probed: Instant,
    protocol: Option<String>,
    /// Whether an instance was ever reached.
    reached: bool,
    /// Why the last search found none.
    missed: Option<StdioError>,
    /// Whether the client was answered offline since the last instance was reached.
    offline: bool,
    /// The tool revision of the last forwarded `tools/list` (or of the last announcement).
    tools: Option<String>,
    /// Whether `notifications/tools/list_changed` is due.
    announce: bool,
}

/// Forwards the JSON-RPC lines of `input` to the instance `locate` finds - asked again whenever
/// the instance is gone - and its answers to `output`, until `input` ends. Blocking.
///
/// # Errors
///
/// stdin or stdout fails; or `input` ended and no instance was ever reached (the last reason
/// none was found, so `rbrowser mcp` ends with its exit code).
pub fn forward<L>(
    mut locate: L,
    input: impl BufRead,
    mut output: impl Write,
) -> Result<(), StdioError>
where
    L: FnMut() -> Result<Instance, StdioError>,
{
    let mut bridge = Bridge {
        agent: instance::local_agent(Some(REQUEST_TIMEOUT)),
        current: None,
        probed: Instant::now(),
        protocol: None,
        reached: false,
        missed: None,
        offline: false,
        tools: None,
        announce: false,
    };
    for line in input.lines() {
        let line = line?;
        let message = line.trim();
        if message.is_empty() {
            continue;
        }
        let parsed = serde_json::from_str::<Value>(message).ok();
        let id = parsed
            .as_ref()
            .and_then(|value| value.get("id").cloned())
            .filter(|id| !id.is_null());
        let method = parsed
            .as_ref()
            .and_then(|value| value["method"].as_str())
            .unwrap_or_default()
            .to_owned();
        for answer in bridge.handle(&mut locate, message, id.as_ref(), &method) {
            if let Some(version) = answer["result"]["protocolVersion"].as_str() {
                bridge.protocol = Some(version.to_owned());
            }
            writeln!(output, "{answer}")?;
        }
        if bridge.announce && method != "initialize" {
            bridge.announce = false;
            writeln!(output, "{LIST_CHANGED}")?;
        }
        output.flush()?;
    }
    match bridge.missed {
        Some(error) if !bridge.reached => Err(error),
        _ => Ok(()),
    }
}

impl Bridge {
    /// The answers to `message` (a request of `method`): from the instance, or - without one -
    /// the bridge's own.
    fn handle<L>(
        &mut self,
        locate: &mut L,
        message: &str,
        id: Option<&Value>,
        method: &str,
    ) -> Vec<Value>
    where
        L: FnMut() -> Result<Instance, StdioError>,
    {
        let mut lost = None;
        for _ in 0..2 {
            let Some(instance) = self.instance(locate) else {
                let why = self
                    .missed
                    .as_ref()
                    .map_or_else(String::new, ToString::to_string);
                self.offline = true;
                return offline::answer(message, &why).into_iter().collect();
            };
            match self.send(&instance, message, id) {
                Sent::Answers(answers, revision) => {
                    self.tools_seen(method, revision);
                    return answers;
                }
                Sent::Lost(why) => {
                    tracing::info!(port = instance.port, %why, "the running rBrowser is gone; looking for it again");
                    self.current = None;
                    lost = Some(why);
                }
            }
        }
        let why = lost.unwrap_or_default();
        error_for(id, &format!("the running rBrowser {why}"))
    }

    /// Takes the tool `revision` an answer to `method` came with: a `tools/list` is the list
    /// the client knows now; any other answer with a different one makes the announcement due.
    fn tools_seen(&mut self, method: &str, revision: Option<String>) {
        if method == "tools/list" {
            self.announce = false;
            self.tools = revision;
            return;
        }
        if let Some(revision) = revision {
            if self.tools.as_ref().is_some_and(|known| *known != revision) {
                self.announce = true;
            }
            if self.tools.is_some() {
                self.tools = Some(revision);
            }
        }
    }

    /// The instance to send to: the current one while it answers, else a fresh search.
    fn instance<L>(&mut self, locate: &mut L) -> Option<Instance>
    where
        L: FnMut() -> Result<Instance, StdioError>,
    {
        if let Some(current) = &self.current {
            if self.probed.elapsed() < ALIVE_EVERY || instance::probe(current) {
                if self.probed.elapsed() >= ALIVE_EVERY {
                    self.probed = Instant::now();
                }
                return Some(current.clone());
            }
            tracing::info!(
                port = current.port,
                "the running rBrowser no longer answers"
            );
            self.current = None;
        }
        match locate() {
            Ok(found) => {
                tracing::info!(port = found.port, project = %found.project, "rbrowser mcp reached the running rBrowser");
                // The client knows no tools (answered offline) or another instance's.
                self.announce |= self.reached || self.offline;
                self.offline = false;
                self.tools = None;
                self.current = Some(found.clone());
                self.probed = Instant::now();
                self.reached = true;
                self.missed = None;
                Some(found)
            }
            Err(error) => {
                self.missed = Some(error);
                None
            }
        }
    }

    /// Sends `message` to `instance`.
    fn send(&mut self, instance: &Instance, message: &str, id: Option<&Value>) -> Sent {
        let bearer = format!("Bearer {}", instance.token);
        let mut request = self
            .agent
            .post(instance.mcp_url())
            .header("Authorization", &bearer)
            .header("Accept", "application/json, text/event-stream")
            .content_type("application/json");
        if let Some(version) = &self.protocol {
            request = request.header("MCP-Protocol-Version", version);
        }
        let mut response = match request.send(message) {
            Ok(response) => response,
            // Nothing reached a server: safe to send again.
            Err(ureq::Error::ConnectionFailed) => {
                return Sent::Lost("refused the connection".to_owned());
            }
            Err(ureq::Error::Io(error))
                if error.kind() == std::io::ErrorKind::ConnectionRefused =>
            {
                return Sent::Lost("refused the connection".to_owned());
            }
            // It may have run: answered, not sent again; the next message looks again.
            Err(error) => {
                self.current = None;
                return Sent::Answers(
                    error_for(
                        id,
                        &format!("the running rBrowser did not answer ({error})"),
                    ),
                    None,
                );
            }
        };
        let status = response.status().as_u16();
        if status == 401 {
            // Another instance took the port, or the token changed: it never ran.
            return Sent::Lost("no longer takes this token".to_owned());
        }
        let revision = response
            .headers()
            .get(TOOLS_REVISION)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        if status == 202 {
            return Sent::Answers(Vec::new(), revision);
        }
        let event_stream = response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.starts_with("text/event-stream"));
        let body = response
            .body_mut()
            .with_config()
            .limit(MAX_ANSWER)
            .read_to_string()
            .map_err(|error| error.to_string());
        if !(200..300).contains(&status) {
            return Sent::Answers(
                error_for(
                    id,
                    &format!("the running rBrowser refused the message (HTTP {status})"),
                ),
                None,
            );
        }
        Sent::Answers(answers_of(body, event_stream, id), revision)
    }
}

/// The messages to write for one answer `body` (a server-sent event stream or one JSON
/// message): every one that parses. When none does - the body could not be read (larger than
/// [`MAX_ANSWER`], cut off) or holds no JSON - the request `id`, when it is one, gets a
/// JSON-RPC error instead of silence; the bridge goes on with the next line.
fn answers_of(body: Result<String, String>, event_stream: bool, id: Option<&Value>) -> Vec<Value> {
    let parsed: Vec<Value> = match &body {
        Ok(body) if event_stream => sse_data(body)
            .iter()
            .filter_map(|answer| serde_json::from_str(answer).ok())
            .collect(),
        Ok(body) => serde_json::from_str(body).ok().into_iter().collect(),
        Err(_) => Vec::new(),
    };
    if !parsed.is_empty() {
        return parsed;
    }
    let why = match body {
        Err(error) => format!("the answer of the running rBrowser could not be read ({error})"),
        Ok(_) => {
            "the answer of the running rBrowser could not be read (no JSON-RPC message)".to_owned()
        }
    };
    error_for(id, &why)
}

/// A JSON-RPC error answering the request `id`; nothing for a notification.
fn error_for(id: Option<&Value>, why: &str) -> Vec<Value> {
    id.map(|id| {
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32603, "message": why },
        })
    })
    .into_iter()
    .collect()
}

/// The `data:` payloads of a server-sent event stream, one per event.
fn sse_data(body: &str) -> Vec<String> {
    let mut events = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    for line in body.lines().chain(std::iter::once("")) {
        if line.is_empty() {
            if !current.is_empty() {
                events.push(current.join("\n"));
                current.clear();
            }
        } else if let Some(data) = line.strip_prefix("data:") {
            current.push(data.strip_prefix(' ').unwrap_or(data));
        }
    }
    events
}

/// `rbrowser mcp`: forwards stdin and stdout to the instance `locator` finds.
///
/// # Errors
///
/// As [`forward`].
pub fn run(locator: &Locator) -> Result<(), StdioError> {
    let stdin = std::io::stdin();
    forward(|| locator.find(), stdin.lock(), std::io::stdout().lock())
}

#[cfg(test)]
#[path = "stdio_tests.rs"]
mod tests;
