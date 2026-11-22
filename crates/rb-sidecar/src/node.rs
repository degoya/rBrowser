//! Which Node runtime runs the sidecar: `engines.nodePath` when set, else the one the package
//! ships (`<install>/node/node.exe`; `<install>/node/bin/node` in the macOS app bundle, whose
//! `<install>` is `Contents/Resources` - [`crate::layout`]). Its major version must be the one of
//! `.nvmrc`; there is no fallback to a `node` on `PATH`.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use rb_process::NoConsoleWindow;
use tokio::process::Command;

use crate::error::SidecarError;
use crate::settings::non_empty;

/// The repository's `.nvmrc`: the Node version the package ships and the sidecar is built for.
pub const NVMRC: &str = include_str!("../../../.nvmrc");

/// How long `node --version` may take (a first start on Windows waits for the virus scanner).
const VERSION_TIMEOUT: Duration = Duration::from_secs(15);

/// `major.minor.patch` from `v26.13.0` or `26.13.0`, blanks around ignored.
pub fn parse_version(text: &str) -> Option<(u32, u32, u32)> {
    let text = text.trim();
    let text = text.strip_prefix('v').unwrap_or(text);
    let mut parts = text.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    parts.next().is_none().then_some((major, minor, patch))
}

/// The major version of [`NVMRC`].
pub fn expected_major() -> u32 {
    parse_version(NVMRC)
        .map(|(major, _, _)| major)
        .expect("the repository's .nvmrc holds a full version (a test guards it)")
}

/// The runtime the package ships: `node\node.exe` on Windows, `node/bin/node` elsewhere.
pub fn bundled_node(install_dir: &Path) -> PathBuf {
    let node = install_dir.join("node");
    if cfg!(windows) {
        node.join("node.exe")
    } else {
        node.join("bin").join("node")
    }
}

/// The sidecar bundle the package ships: `sidecar/dist/main.js`.
pub fn sidecar_script(install_dir: &Path) -> PathBuf {
    install_dir.join("sidecar").join("dist").join("main.js")
}

/// The runtime to start: `configured` (`engines.nodePath`) when set - an absolute path to a
/// file - else the bundled one. `is_file` is `Path::is_file` outside tests.
pub fn resolve_node(
    configured: Option<&str>,
    install_dir: &Path,
    is_file: impl Fn(&Path) -> bool,
) -> Result<PathBuf, SidecarError> {
    if let Some(configured) = non_empty(configured) {
        let path = PathBuf::from(configured);
        return if path.is_absolute() && is_file(&path) {
            Ok(path)
        } else {
            Err(SidecarError::NodePathInvalid(path))
        };
    }
    let bundled = bundled_node(install_dir);
    if is_file(&bundled) {
        Ok(bundled)
    } else {
        Err(SidecarError::NodeNotFound(bundled))
    }
}

/// Checks the output of `node --version` against `expected`; the version found on success.
pub fn check_version(path: &Path, output: &str, expected: u32) -> Result<String, SidecarError> {
    let Some((major, minor, patch)) = parse_version(output) else {
        return Err(SidecarError::NodeVersionUnknown {
            path: path.to_path_buf(),
            reason: "no version in the output of --version".to_owned(),
        });
    };
    let found = format!("{major}.{minor}.{patch}");
    if major == expected {
        Ok(found)
    } else {
        Err(SidecarError::NodeVersionMismatch {
            expected,
            found,
            path: path.to_path_buf(),
        })
    }
}

/// Runs `<node> --version` and checks it against [`expected_major`].
pub async fn verify(node: &Path) -> Result<String, SidecarError> {
    let unknown = |reason: String| SidecarError::NodeVersionUnknown {
        path: node.to_path_buf(),
        reason,
    };
    let mut command = Command::new(node);
    command
        .no_console_window()
        .arg("--version")
        .env_remove("NODE_OPTIONS")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let output = tokio::time::timeout(VERSION_TIMEOUT, command.output())
        .await
        .map_err(|_| unknown(format!("no answer within {} s", VERSION_TIMEOUT.as_secs())))?
        .map_err(|error| unknown(error.to_string()))?;
    if !output.status.success() {
        return Err(unknown(format!("--version ended with {}", output.status)));
    }
    check_version(
        node,
        &String::from_utf8_lossy(&output.stdout),
        expected_major(),
    )
}

#[cfg(test)]
#[path = "node_tests.rs"]
mod tests;
