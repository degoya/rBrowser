//! What a second call says to the running instance of its project: one JSON line
//! ([`Envelope`]) with the nonce of the instance file and one of the known [`Request`]s, answered
//! by one JSON line ([`Reply`]). Everything else is refused: unknown kinds and fields, another
//! version, a wrong nonce, a URL outside `http`, `https` and `file`, more than [`MAX_URLS`], a
//! screenshot out of bounds ([`check_screenshot`]).

use std::fmt;
use std::path::{Component, Path, PathBuf, Prefix};
use std::time::Duration;

use rb_visual::delay::MAX_DELAY_MS;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::capture::ScreenshotArgs;
use crate::capture::cdp::MAX_SELECTOR_LEN;

/// The version of this protocol.
pub const PROTOCOL_VERSION: u32 = 1;
/// The longest line either side reads; a longer one is refused unread.
pub const MAX_LINE: usize = 64 * 1024;
/// The most URLs one request carries.
pub const MAX_URLS: usize = 16;
/// The longest URL taken.
pub const MAX_URL_LEN: usize = 8 * 1024;
/// The longest reply either side writes or reads (a screenshot answer names a file per view).
pub const MAX_REPLY: usize = 256 * 1024;
/// The most view names one screenshot request carries.
pub const MAX_VIEW_NAMES: usize = 64;
/// The longest view name (a breakpoint id or a view id).
pub const MAX_VIEW_NAME_LEN: usize = 128;
/// The longest output folder.
pub const MAX_OUT_LEN: usize = 1024;
/// The longest reason a failed request names.
pub const MAX_MESSAGE_LEN: usize = 512;
/// How long a screenshot may take beyond its delay (every view, full pages included).
pub const SCREENSHOT_MARGIN: Duration = Duration::from_secs(120);
/// The random bytes of a nonce (hex-encoded: twice as many digits).
const NONCE_BYTES: usize = 32;
/// The most views `flow run --views` names.
pub const MAX_FLOW_VIEWS: usize = 32;
/// The longest view name of `flow run --views`.
const MAX_VIEW_NAME: usize = 64;
/// The schemes a URL from the command line may have.
const OPEN_SCHEMES: [&str; 3] = ["http", "https", "file"];

/// One request with its credentials.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Envelope {
    /// [`PROTOCOL_VERSION`].
    pub version: u32,
    /// The nonce of the instance file.
    pub nonce: String,
    /// What to do.
    pub request: Request,
}

impl Envelope {
    /// `request` with `nonce`, in this version.
    pub fn new(nonce: &str, request: Request) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            nonce: nonce.to_owned(),
            request,
        }
    }
}

impl fmt::Debug for Envelope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Envelope")
            .field("version", &self.version)
            .field("nonce", &"[redacted]")
            .field("request", &self.request)
            .finish()
    }
}

/// The requests a running instance takes - nothing else is ever run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Request {
    /// `rbrowser [url...]` for a running project: load the first URL in every view (none: keep
    /// the page) and bring the window to the front unless `background`.
    Open {
        /// The URLs; only the first is loaded.
        #[serde(default)]
        urls: Vec<String>,
        /// Leave the focus where it is.
        #[serde(default)]
        background: bool,
    },
    /// `rbrowser --reload`: reload every view.
    Reload {
        /// Bypass the cache.
        #[serde(default)]
        ignore_cache: bool,
    },
    /// `rbrowser screenshot`: capture the views and answer the `ScreenshotAnswer`
    /// (RB-050-03). Always to files, never base64 over the channel.
    Screenshot {
        /// What to capture, as the MCP tool `screenshot` takes it.
        args: ScreenshotArgs,
        /// The views as the command line names them (breakpoint id or view id); every view when
        /// empty. The instance resolves them against its grid.
        #[serde(default)]
        views: Vec<String>,
        /// An absolute folder instead of the project's screenshot folder.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        out: Option<String>,
    },
    /// `rbrowser flow run <name>` (RB-040-07): start the flow; the CLI waits for the run's
    /// `result.json` under `run_id`.
    FlowRun {
        /// The flow's name.
        name: String,
        /// View ids, breakpoint ids or labels; empty: the flow's own views.
        #[serde(default)]
        views: Vec<String>,
        /// The run's id, the CLI's: also the folder it waits on.
        run_id: String,
    },
    /// `rbrowser capture sitemap` (RB-050-05): start or resume a capture; the answer is the
    /// `CaptureStarted`, and the CLI follows the run's `run.json`.
    SitemapCapture {
        /// As the MCP tool `sitemap_capture` takes them.
        args: crate::sitemap::tool::CaptureArgs,
    },
}

/// Why a request was refused; the only detail a client learns.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Refusal {
    /// Not a request of this protocol.
    BadRequest,
    /// Another protocol version.
    BadVersion,
    /// The nonce is not the instance's.
    BadNonce,
    /// A URL is not allowed.
    BadUrl,
    /// The request was valid but could not be carried out.
    Failed,
}

/// The answer to a request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Reply {
    /// Whether the request was carried out.
    pub ok: bool,
    /// Why not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<Refusal>,
    /// What a carried-out request answers (`screenshot`: the `ScreenshotAnswer`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer: Option<Value>,
    /// Why a valid request could not be carried out ([`Refusal::Failed`] only): plain text of at
    /// most [`MAX_MESSAGE_LEN`] bytes. Never sent before the nonce was checked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl Reply {
    /// Done.
    pub fn ok() -> Self {
        Self {
            ok: true,
            error: None,
            answer: None,
            message: None,
        }
    }

    /// Done, answering `answer`.
    pub fn answered(answer: Value) -> Self {
        Self {
            answer: Some(answer),
            ..Self::ok()
        }
    }

    /// Refused for `refusal`.
    pub fn refused(refusal: Refusal) -> Self {
        Self {
            ok: false,
            error: Some(refusal),
            answer: None,
            message: None,
        }
    }

    /// A valid request that failed for `message`: control characters dropped, cut to
    /// [`MAX_MESSAGE_LEN`] bytes.
    pub fn failed(message: &str) -> Self {
        let mut text = String::new();
        for c in message.chars().filter(|c| !c.is_control()) {
            if text.len() + c.len_utf8() > MAX_MESSAGE_LEN {
                break;
            }
            text.push(c);
        }
        Self {
            message: Some(text),
            ..Self::refused(Refusal::Failed)
        }
    }
}

/// A new nonce: [`NONCE_BYTES`] random bytes from the OS's source as hex digits
/// ([`crate::secret`]).
///
/// # Errors
///
/// The OS gives no randomness.
pub fn new_nonce() -> std::io::Result<String> {
    crate::secret::random_hex(NONCE_BYTES)
}

/// Whether `offered` is `ours`, compared in constant time over the length of `ours`.
pub fn nonce_matches(ours: &str, offered: &str) -> bool {
    !ours.is_empty() && crate::secret::constant_time_eq(ours.as_bytes(), offered.as_bytes())
}

/// A URL from the command line, completed as the address bar does (`localhost:5173` ->
/// `http://localhost:5173`) and held to `http`, `https` and `file`.
pub fn open_url(text: &str) -> Result<String, String> {
    if text.len() > MAX_URL_LEN {
        return Err(format!("the URL is longer than {MAX_URL_LEN} characters"));
    }
    let url = crate::host::nav::normalize_url(text).map_err(|error| error.to_string())?;
    let scheme = url
        .split_once(':')
        .map(|(scheme, _)| scheme.to_ascii_lowercase())
        .unwrap_or_default();
    if OPEN_SCHEMES.contains(&scheme.as_str()) {
        Ok(url)
    } else {
        Err(format!(
            "{text}: only http, https and file URLs can be opened from the command line"
        ))
    }
}

/// The request in `line` when it is valid and carries `nonce`, its URLs completed.
pub fn check(line: &[u8], nonce: &str) -> Result<Request, Refusal> {
    let envelope: Envelope = serde_json::from_slice(line).map_err(|_| Refusal::BadRequest)?;
    if !nonce_matches(nonce, &envelope.nonce) {
        return Err(Refusal::BadNonce);
    }
    if envelope.version != PROTOCOL_VERSION {
        return Err(Refusal::BadVersion);
    }
    match envelope.request {
        Request::Open { urls, background } => {
            if urls.len() > MAX_URLS {
                return Err(Refusal::BadRequest);
            }
            let urls = urls
                .iter()
                .map(|url| open_url(url).map_err(|_| Refusal::BadUrl))
                .collect::<Result<_, _>>()?;
            Ok(Request::Open { urls, background })
        }
        reload @ Request::Reload { .. } => Ok(reload),
        Request::Screenshot { args, views, out } => {
            check_screenshot(&args, &views, out.as_deref()).map_err(|_| Refusal::BadRequest)?;
            Ok(Request::Screenshot { args, views, out })
        }
        Request::FlowRun {
            name,
            views,
            run_id,
        } => {
            let plain = crate::flows::catalog::valid_name(&name)
                && crate::flows::result::valid_run_id(&run_id)
                && views.len() <= MAX_FLOW_VIEWS
                && views
                    .iter()
                    .all(|view| (1..=MAX_VIEW_NAME).contains(&view.len()));
            if plain {
                Ok(Request::FlowRun {
                    name,
                    views,
                    run_id,
                })
            } else {
                Err(Refusal::BadRequest)
            }
        }
        Request::SitemapCapture { args } => match args.check() {
            Ok(()) => Ok(Request::SitemapCapture { args }),
            Err(_) => Err(Refusal::BadRequest),
        },
    }
}

/// Whether a screenshot request stays in bounds: saved to files, a delay of at most
/// [`MAX_DELAY_MS`], a selector of at most [`MAX_SELECTOR_LEN`], at most [`MAX_VIEW_NAMES`]
/// plain view names, an output folder [`check_out_dir`] takes. The command line checks the
/// same before it sends; the reason is for its user.
pub fn check_screenshot(
    args: &ScreenshotArgs,
    views: &[String],
    out: Option<&str>,
) -> Result<(), String> {
    if args.save == Some(false) {
        return Err("a screenshot from the command line is saved to files".to_owned());
    }
    if args.delay_ms.is_some_and(|delay| delay > MAX_DELAY_MS) {
        return Err(format!("the delay is at most {} s", MAX_DELAY_MS / 1000));
    }
    if let Some(selector) = &args.selector
        && (selector.trim().is_empty() || selector.len() > MAX_SELECTOR_LEN)
    {
        return Err(format!(
            "the selector must have 1 to {MAX_SELECTOR_LEN} characters"
        ));
    }
    if views.len() > MAX_VIEW_NAMES {
        return Err(format!("at most {MAX_VIEW_NAMES} views can be named"));
    }
    if let Some(view) = views.iter().find(|view| {
        view.trim().is_empty()
            || view.len() > MAX_VIEW_NAME_LEN
            || view.chars().any(char::is_control)
    }) {
        return Err(format!(
            "{:?} is no view name (1 to {MAX_VIEW_NAME_LEN} plain characters)",
            view.chars().take(MAX_VIEW_NAME_LEN).collect::<String>()
        ));
    }
    if let Some(out) = out {
        check_out_dir(out)?;
    }
    Ok(())
}

/// The output folder `text` when it is absolute and plain: at most [`MAX_OUT_LEN`] bytes, no
/// control character, no `.` or `..` part (the folder is created as written, never above what it
/// names), no device or verbatim prefix (`\\.\`, `\\?\`).
pub fn check_out_dir(text: &str) -> Result<PathBuf, String> {
    if text.is_empty() || text.len() > MAX_OUT_LEN || text.chars().any(char::is_control) {
        return Err(format!(
            "the output folder must have 1 to {MAX_OUT_LEN} plain characters"
        ));
    }
    let path = Path::new(text);
    if !path.is_absolute() {
        return Err(format!(
            "{text}: the output folder must be an absolute path"
        ));
    }
    let dotted = text
        .split(['/', '\\'])
        .any(|part| part == "." || part == "..");
    let odd = path.components().any(|component| match component {
        Component::CurDir | Component::ParentDir => true,
        Component::Prefix(prefix) => !matches!(prefix.kind(), Prefix::Disk(_) | Prefix::UNC(_, _)),
        Component::RootDir | Component::Normal(_) => false,
    });
    if dotted || odd {
        return Err(format!(
            "{text}: the output folder must be a plain absolute path, without . or .."
        ));
    }
    Ok(path.to_path_buf())
}

/// How long the instance lets a screenshot of `args` take: its delay and
/// [`SCREENSHOT_MARGIN`]. The client waits a little longer for the answer.
pub fn screenshot_budget(args: &ScreenshotArgs) -> Duration {
    Duration::from_millis(args.delay_ms.unwrap_or(0).min(MAX_DELAY_MS)) + SCREENSHOT_MARGIN
}

/// `envelope` as one line.
pub fn encode(envelope: &Envelope) -> Vec<u8> {
    line_of(envelope)
}

/// `reply` as one line.
pub fn encode_reply(reply: &Reply) -> Vec<u8> {
    line_of(reply)
}

/// The reply in `line`; `None` when it is none.
pub fn decode_reply(line: &[u8]) -> Option<Reply> {
    serde_json::from_slice(line).ok()
}

fn line_of(value: &impl Serialize) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(value).expect("a request or reply always serializes");
    bytes.push(b'\n');
    bytes
}

#[cfg(test)]
#[path = "protocol_tests.rs"]
mod tests;
