//! The instance file: how `rbrowser mcp` finds the running rBrowser of a project.
//!
//! The HTTP server writes `<local dir>/mcp/<project key>.json` (`{version, pid, port, token,
//! project}`, mode 0600 in a 0700 folder on Unix; on Windows in a folder whose protected DACL
//! grants the current user alone, inherited by the file - `rb_process::create_private_dir`,
//! RB-090-21: a portable data folder may otherwise inherit Modify for other local accounts) and
//! removes it when it stops. A file left by a crash is
//! harmless: [`probe`] asks the port before anything is forwarded, and only a server that knows
//! the token answers. The probe never sends the token: it sends a fresh nonce, and the server
//! proves the token with an HMAC over its port and the nonce ([`McpToken::proof`]).

use std::fmt;
use std::io::{self, Read as _};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::McpToken;
use crate::secret::random_hex;

/// The folder of the instance files inside the local folder.
pub const INSTANCES_DIR: &str = "mcp";
/// The version of the file.
pub const FILE_VERSION: u32 = 1;
/// The app name the health answer carries.
pub const APP: &str = "rbrowser";
/// The most bytes read of a file; anything larger is not ours.
const MAX_FILE: u64 = 4 * 1024;
/// How long the probe waits for the answer.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(2);

/// What a running server tells `rbrowser mcp`.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Instance {
    /// [`FILE_VERSION`].
    pub version: u32,
    /// The process of the instance.
    pub pid: u32,
    /// The port on 127.0.0.1.
    pub port: u16,
    /// The bearer token (64 hex digits).
    pub token: String,
    /// The project root the instance has open; empty without a project.
    pub project: String,
}

impl fmt::Debug for Instance {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Instance")
            .field("version", &self.version)
            .field("pid", &self.pid)
            .field("port", &self.port)
            .field("token", &"[redacted]")
            .field("project", &self.project)
            .finish()
    }
}

impl Instance {
    /// The MCP endpoint.
    #[must_use]
    pub fn mcp_url(&self) -> String {
        format!("http://127.0.0.1:{}/mcp", self.port)
    }

    /// The token as an [`McpToken`].
    #[must_use]
    pub fn token(&self) -> McpToken {
        McpToken::from_text(self.token.clone())
    }
}

/// The folder of the instance files under `local` (`rb_config::local_dir`).
pub fn instances_dir(local: &Path) -> PathBuf {
    local.join(INSTANCES_DIR)
}

/// The file name of the project with `key` (the hash of its root; `rbrowser`'s
/// `instance::file::key`), or `default` without a project.
pub fn file_path(local: &Path, key: Option<&str>) -> PathBuf {
    let name = key.filter(|key| valid_key(key)).unwrap_or("default");
    instances_dir(local).join(format!("{name}.json"))
}

/// A key is a short run of ASCII letters, digits and dashes - nothing that leaves the folder.
fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 64
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

/// The instance file at `path` when it is one: at most [`MAX_FILE`] bytes, this version, a
/// port and a well-formed token.
pub fn read(path: &Path) -> Option<Instance> {
    let file = std::fs::File::open(path).ok()?;
    let mut bytes = Vec::new();
    file.take(MAX_FILE + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() as u64 > MAX_FILE {
        return None;
    }
    let instance: Instance = serde_json::from_slice(&bytes).ok()?;
    let token_ok =
        instance.token.len() == 64 && instance.token.bytes().all(|byte| byte.is_ascii_hexdigit());
    (instance.version == FILE_VERSION && instance.port != 0 && token_ok).then_some(instance)
}

/// Every instance file in `dir` that reads, with its path.
pub fn read_all(dir: &Path) -> Vec<(PathBuf, Instance)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<(PathBuf, Instance)> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .filter_map(|path| read(&path).map(|instance| (path, instance)))
        .collect();
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found
}

/// Writes `instance` to `path` through a temporary file renamed over it, readable by the user
/// only.
///
/// # Errors
///
/// The folder or the file cannot be written.
pub fn write(path: &Path, instance: &Instance) -> io::Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| io::Error::other("the instance file has no folder"))?;
    rb_process::create_private_dir(dir)?;
    let bytes = serde_json::to_vec_pretty(instance).map_err(io::Error::other)?;
    rb_util::fs::write_atomic_private(path, &bytes)
}

/// Removes the instance file at `path` when it is still the one of the server on `port` (a
/// second instance of the project may have written its own over it).
pub fn remove_own(path: &Path, port: u16) {
    if read(path).is_some_and(|instance| instance.port == port) {
        let _ = std::fs::remove_file(path);
    }
}

/// A blocking HTTP agent for 127.0.0.1: no proxy (one from the environment would see the
/// token), no redirects, status codes as answers, every call within `timeout`, and no idle
/// connection kept: one left over from an instance that restarted would fail a call that never
/// reached it, which the bridge could not tell from one that ran (RB-030-02).
pub(crate) fn local_agent(timeout: Option<Duration>) -> ureq::Agent {
    ureq::Agent::config_builder()
        .proxy(None)
        .max_redirects(0)
        .max_idle_connections(0)
        .max_idle_connections_per_host(0)
        .http_status_as_error(false)
        .timeout_global(timeout)
        .build()
        .new_agent()
}

/// Whether a server that knows the instance's token answers on its port: the answer to a fresh
/// nonce must be the token's proof for this port and nonce. The token itself is not sent.
/// Blocking, at most [`PROBE_TIMEOUT`].
pub fn probe(instance: &Instance) -> bool {
    let Ok(nonce) = random_hex(32) else {
        return false;
    };
    let url = format!("http://127.0.0.1:{}/health?nonce={nonce}", instance.port);
    let Ok(mut response) = local_agent(Some(PROBE_TIMEOUT)).get(&url).call() else {
        return false;
    };
    if !response.status().is_success() {
        return false;
    }
    let Ok(text) = response
        .body_mut()
        .with_config()
        .limit(MAX_FILE)
        .read_to_string()
    else {
        return false;
    };
    serde_json::from_str::<serde_json::Value>(&text).is_ok_and(|body| {
        body["app"] == APP
            && body["proof"]
                .as_str()
                .is_some_and(|proof| instance.token().proof_matches(instance.port, &nonce, proof))
    })
}

#[cfg(test)]
#[path = "instance_tests.rs"]
mod tests;
