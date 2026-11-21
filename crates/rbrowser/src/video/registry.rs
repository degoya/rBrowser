//! The recordings rBrowser itself wrote (RB-110-12, security review): the Videos panel lists,
//! plays and deletes only what is in here, so a video a repository checked in under
//! `.rbrowser/out/video/` is never parsed by the shell's own renderer.
//!
//! One JSON file per project in the user's local data folder (`<local>/video-registry/`, never in
//! the project), named by the SHA-256 of the project root and holding the root again; per
//! recording the names of its files with their size and modification time when the recorder saved
//! them ([`FileStamp`]). A file whose size or time no longer matches is not served. Written only
//! by the core: after a recording is saved ([`Registry::record`]) and after a deletion
//! ([`Registry::forget`]); read again for every list and every request. A missing, damaged or
//! foreign file knows nothing.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::Metadata;
use std::io::{self, Read as _};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::cdp::lock;

/// The registry's folder below the local data folder.
pub const DIR_NAME: &str = "video-registry";
/// The largest registry file read.
const MAX_BYTES: u64 = 4 * 1024 * 1024;

/// Read-modify-write of a registry file, one at a time in the process.
static WRITE: Mutex<()> = Mutex::new(());

/// A registry file's length and time when it was parsed.
type FileKey = (u64, Option<SystemTime>);

/// The parsed registries of the video route, by file, with the length and time they were read
/// at ([`Registry::load_cached`]); a write through [`Registry`] drops its entry.
static CACHE: Mutex<Option<HashMap<PathBuf, (FileKey, Recorded)>>> = Mutex::new(None);

/// The registry folder in the user's local data folder; `None` when the environment names none
/// (then nothing is listed or served).
#[must_use]
pub fn default_dir() -> Option<PathBuf> {
    rb_config::local_dir().map(|dir| dir.join(DIR_NAME))
}

/// A file as the recorder left it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileStamp {
    pub bytes: u64,
    /// Its modification time, ms since the epoch.
    pub modified_ms: u64,
}

impl FileStamp {
    /// The stamp of a file's metadata; `None` without a modification time.
    #[must_use]
    pub fn of(metadata: &Metadata) -> Option<Self> {
        let modified = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
        Some(Self {
            bytes: metadata.len(),
            modified_ms: u64::try_from(modified.as_millis()).ok()?,
        })
    }
}

/// What the registry holds for one project.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recorded {
    /// The project root it belongs to.
    pub root: String,
    /// Per recording id its files by name.
    pub recordings: BTreeMap<String, BTreeMap<String, FileStamp>>,
    /// Recordings deleted whose folder stayed (on Windows a player still held a file): their
    /// folders are removed once empty ([`super::retention::settle_pending`]).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub pending: BTreeSet<String>,
}

impl Recorded {
    /// Whether rBrowser wrote recording `id`.
    #[must_use]
    pub fn knows(&self, id: &str) -> bool {
        self.recordings.contains_key(id)
    }

    /// The stamp of file `name` of recording `id`.
    #[must_use]
    pub fn stamp(&self, id: &str, name: &str) -> Option<FileStamp> {
        self.recordings.get(id)?.get(name).copied()
    }

    /// Whether another video of recording `id` than `video` shares its poster - a WebM and its
    /// MP4 export of the same breakpoint (RB-090-52): the poster stays while one of them does.
    #[must_use]
    pub fn poster_shared(&self, id: &str, video: &str) -> bool {
        let poster = super::poster::poster_name(video);
        poster.is_some()
            && self.recordings.get(id).is_some_and(|files| {
                files.keys().any(|name| {
                    name != video
                        && super::library::is_video_name(name)
                        && super::poster::poster_name(name) == poster
                })
            })
    }

    /// Whether file `name` of recording `id` is the one rBrowser wrote: registered, and of the
    /// size and time `metadata` says.
    #[must_use]
    pub fn matches(&self, id: &str, name: &str, metadata: &Metadata) -> bool {
        self.stamp(id, name)
            .is_some_and(|stamp| FileStamp::of(metadata) == Some(stamp))
    }
}

/// The registry file of one project.
#[derive(Clone, Debug)]
pub struct Registry {
    file: PathBuf,
    root: String,
}

/// The root as the key compares it: Windows compares paths without case.
fn root_text(root: &Path) -> String {
    let text = root.to_string_lossy();
    if cfg!(windows) {
        text.to_lowercase()
    } else {
        text.into_owned()
    }
}

impl Registry {
    /// The registry of the project at `root` in the folder `dir`.
    #[must_use]
    pub fn for_project(dir: &Path, root: &Path) -> Self {
        let root = root_text(root);
        let name = rb_util::hex(&Sha256::digest(root.as_bytes()));
        Self {
            file: dir.join(format!("{name}.json")),
            root,
        }
    }

    /// What the registry holds; empty when the file is missing, too large, damaged or another
    /// project's.
    #[must_use]
    pub fn load(&self) -> Recorded {
        let empty = || Recorded {
            root: self.root.clone(),
            ..Recorded::default()
        };
        let Ok(file) = std::fs::File::open(&self.file) else {
            return empty();
        };
        let mut text = Vec::new();
        if file.take(MAX_BYTES + 1).read_to_end(&mut text).is_err()
            || !u64::try_from(text.len()).is_ok_and(|read| read <= MAX_BYTES)
        {
            return empty();
        }
        match serde_json::from_slice::<Recorded>(&text) {
            Ok(recorded) if recorded.root == self.root => recorded,
            Ok(_) => empty(),
            Err(error) => {
                tracing::warn!(%error, "the video registry is damaged; it is read as empty");
                empty()
            }
        }
    }

    /// [`Registry::load`], parsed again only when the file's length or time changed since the
    /// last read - for the video route, which asks on every range request.
    #[must_use]
    pub fn load_cached(&self) -> Recorded {
        let key = std::fs::metadata(&self.file)
            .ok()
            .map(|metadata| (metadata.len(), metadata.modified().ok()));
        let Some(key) = key else {
            return self.load();
        };
        if let Some((seen, recorded)) = lock(&CACHE)
            .as_ref()
            .and_then(|cache| cache.get(&self.file))
            && *seen == key
        {
            return recorded.clone();
        }
        let recorded = self.load();
        lock(&CACHE)
            .get_or_insert_with(HashMap::new)
            .insert(self.file.clone(), (key, recorded.clone()));
        recorded
    }

    /// Records the files of recording `id` (added to what it holds).
    ///
    /// # Errors
    ///
    /// The file cannot be written.
    pub fn record(&self, id: &str, files: &[(String, FileStamp)]) -> io::Result<()> {
        self.update(|recorded| {
            recorded
                .recordings
                .entry(id.to_owned())
                .or_default()
                .extend(files.iter().cloned());
        })
    }

    /// Forgets file `file` of recording `id`, or the whole recording; a recording left without
    /// files goes too.
    ///
    /// # Errors
    ///
    /// The file cannot be written.
    pub fn forget(&self, id: &str, file: Option<&str>) -> io::Result<()> {
        self.forget_deleted(id, file, true)
    }

    /// Forgets what a deletion removed - file `file` of recording `id` with its poster, or all of
    /// it - and,
    /// when the recording has no file left but its folder stayed (`folder_removed` false),
    /// keeps it as pending until the folder is gone.
    ///
    /// # Errors
    ///
    /// The file cannot be written.
    pub fn forget_deleted(
        &self,
        id: &str,
        file: Option<&str>,
        folder_removed: bool,
    ) -> io::Result<()> {
        self.update(|recorded| {
            match file {
                Some(name) => {
                    let shared = recorded.poster_shared(id, name);
                    if let Some(files) = recorded.recordings.get_mut(id) {
                        files.remove(name);
                        // Its poster went with it (`super::library::delete`), unless another
                        // video of the breakpoint keeps it.
                        if let Some(poster) = super::poster::poster_name(name)
                            && !shared
                        {
                            files.remove(&poster);
                        }
                        if files.is_empty() {
                            recorded.recordings.remove(id);
                        }
                    }
                }
                None => {
                    recorded.recordings.remove(id);
                }
            }
            if !recorded.recordings.contains_key(id) && !folder_removed {
                recorded.pending.insert(id.to_owned());
            }
        })
    }

    /// Drops `ids` from the pending recordings: their folders are gone.
    ///
    /// # Errors
    ///
    /// The file cannot be written.
    pub fn clear_pending(&self, ids: &[String]) -> io::Result<()> {
        self.update(|recorded| {
            for id in ids {
                recorded.pending.remove(id);
            }
        })
    }

    /// Reads, changes and writes the file: beside it first, then renamed over it.
    fn update(&self, change: impl FnOnce(&mut Recorded)) -> io::Result<()> {
        let _guard = lock(&WRITE);
        if let Some(cache) = lock(&CACHE).as_mut() {
            cache.remove(&self.file);
        }
        let mut recorded = self.load();
        change(&mut recorded);
        if let Some(dir) = self.file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_vec_pretty(&recorded).map_err(io::Error::other)?;
        rb_util::fs::write_atomic(&self.file, &text)
    }
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
