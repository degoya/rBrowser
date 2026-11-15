//! The instance file: `<local dir>\instances\<hash>.json`, one per open project, written by the
//! instance once its channel listens. It names the process, the channel and the nonce a second
//! call must present. The folder is the user's own (`%LOCALAPPDATA%`; mode 0700/0600 on Unix).
//! A file left by a crash is harmless: its channel is gone, so the next call finds nobody and
//! starts its own instance, which writes the file again.

use std::fmt;
use std::io::{self, Read as _, Write as _};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::channel;

/// The folder of the instance files inside the local folder.
pub const INSTANCES_DIR: &str = "instances";
/// The version of the file.
pub const FILE_VERSION: u32 = 1;
/// The most bytes read of a file; anything larger is not ours.
const MAX_FILE: u64 = 4 * 1024;

/// What a running instance tells a second call for its project.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstanceFile {
    /// [`FILE_VERSION`].
    pub version: u32,
    /// The process of the instance (Windows lets it take the focus for a hand-over).
    pub pid: u32,
    /// The channel: a named pipe on Windows, a Unix socket elsewhere.
    pub endpoint: String,
    /// What a request must carry (64 hex digits).
    pub nonce: String,
    /// The project root the instance has open.
    pub project: String,
}

impl fmt::Debug for InstanceFile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InstanceFile")
            .field("version", &self.version)
            .field("pid", &self.pid)
            .field("endpoint", &self.endpoint)
            .field("nonce", &"[redacted]")
            .field("project", &self.project)
            .finish()
    }
}

/// The folder of the instance files under `local` (`rb_config::local_dir`).
pub fn instances_dir(local: &Path) -> PathBuf {
    local.join(INSTANCES_DIR)
}

/// The key of the project at `root`: the hash of its browser profile (`project::profile_dir`),
/// over the path in lower case with either separator.
pub fn key(root: &Path) -> String {
    let profile = crate::project::profile_dir(Path::new(""), Some(root));
    let name = profile
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    name.rsplit('-').next().unwrap_or_default().to_owned()
}

/// The instance file of the project at `root`.
pub fn file_path(local: &Path, root: &Path) -> PathBuf {
    instances_dir(local).join(format!("{}.json", key(root)))
}

/// Whether the project `written` in a file is the one at `root`.
pub fn same_project(written: &str, root: &Path) -> bool {
    key(Path::new(written)) == key(root)
}

/// The instance file at `path` when it is one: at most [`MAX_FILE`] bytes, this version, a
/// well-formed nonce and a channel of this user (`local`'s instance folder or our pipe prefix).
pub fn read(path: &Path, local: &Path) -> Option<InstanceFile> {
    let file = std::fs::File::open(path).ok()?;
    let mut bytes = Vec::new();
    file.take(MAX_FILE + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() as u64 > MAX_FILE {
        return None;
    }
    let file: InstanceFile = serde_json::from_slice(&bytes).ok()?;
    let nonce_ok = file.nonce.len() == 64 && file.nonce.chars().all(|c| c.is_ascii_hexdigit());
    (file.version == FILE_VERSION
        && nonce_ok
        && channel::valid_endpoint(&file.endpoint, &instances_dir(local)))
    .then_some(file)
}

/// Writes `file` to `path` through a temporary file renamed over it, readable by the user only.
pub fn write(path: &Path, file: &InstanceFile) -> io::Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| io::Error::other("the instance file has no folder"))?;
    create_private_dir(dir)?;
    let bytes = serde_json::to_vec_pretty(file).map_err(io::Error::other)?;
    let temp = path.with_extension(format!("json.tmp-{}", std::process::id()));
    let written = private_file(&temp).and_then(|mut out| {
        out.write_all(&bytes)?;
        out.sync_all()
    });
    if let Err(error) = written {
        let _ = std::fs::remove_file(&temp);
        return Err(error);
    }
    std::fs::rename(&temp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&temp);
    })
}

/// Creates `dir` (mode 0700 on Unix; on Windows the local folder's ACL is the user's).
pub fn create_private_dir(dir: &Path) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn private_file(path: &Path) -> io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    options.open(path)
}

#[cfg(test)]
#[path = "file_tests.rs"]
mod tests;
