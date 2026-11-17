//! The protected parts of a project and their content hash (RB-010-09).
//!
//! The parts: every entry of `.rbrowser/{flows,snippets,plugins,mocks,scripts}/`, the request
//! headers of `config.json` and of `config.local.json` (`emulation.headers`,
//! `breakpoints[].headers`; a clone can commit the local file too), the `extensions` of both,
//! and the trust-gated network keys ([`crate::TRUST_GATED_KEYS`]) of `config.json` and
//! `config.local.json` - in `config.json` only those the local-only rule leaves
//! (`hostResolverRules`), since trust never lifts that rule.
//!
//! The hash covers exactly these parts: every file below a protected folder by its relative
//! path and the SHA-256 of its bytes, every config value in a canonical form (keys sorted)
//! named by what it is, a breakpoint's headers by the breakpoint's `id`. So reformatting
//! `config.json`, editing or reordering breakpoints, the layout or the environments leave the
//! hash alone; any change to a protected part changes it.
//!
//! A link (a symbolic link, on Windows also a junction) counts by the content it leads to when
//! that lies inside the project; one that leads out of it, nowhere or to neither a file nor a
//! folder is [`Protected::unpinned`]: no hash can pin what it loads, so such a project is never
//! trusted.

use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use rb_util::hex;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use super::{BlockedItem, ProtectedKind};
use crate::jsonio::io_error;
use crate::load::RawFiles;
use crate::{CONFIG_DIR, ConfigError, LOCAL_CONFIG_FILE, LOCAL_ONLY_KEYS, TRUST_GATED_KEYS};

/// The protected folders inside [`CONFIG_DIR`].
pub const PROTECTED_DIRS: [(ProtectedKind, &str); 5] = [
    (ProtectedKind::Flows, "flows"),
    (ProtectedKind::Snippets, "snippets"),
    (ProtectedKind::Plugins, "plugins"),
    (ProtectedKind::Mocks, "mocks"),
    (ProtectedKind::PageScripts, "scripts"),
];

/// What the hash starts with; a new hash scheme changes it, so old decisions ask again.
const HASH_DOMAIN: &[u8] = b"rbrowser-workspace-trust-v1\n";

/// The protected parts of one project as they are now.
#[derive(Debug, Default)]
pub struct Protected {
    /// Sorted by kind, then path.
    pub blocked: Vec<BlockedItem>,
    /// Lower-case hex SHA-256; empty without protected parts.
    pub hash: String,
    /// Links below a protected folder whose content the hash cannot cover (relative paths with
    /// `/`, sorted).
    pub unpinned: Vec<String>,
}

/// Collects the protected parts of the project at `root` whose files read as `raw`.
pub fn protected(root: &Path, raw: &RawFiles) -> Result<Protected, ConfigError> {
    let mut collector = Collector {
        root: fs::canonicalize(root).ok(),
        ..Collector::default()
    };
    for (kind, name) in PROTECTED_DIRS {
        collector.folder(root, kind, name)?;
    }
    collector.project(&raw.project);
    if let Some(local) = &raw.local {
        collector.headers(local, true);
        collector.gated(local, true);
        if let Some(extensions) = local.get("extensions").filter(|v| active(v)) {
            collector.value(
                ProtectedKind::Extensions,
                local_path("/extensions"),
                "config.local.json extensions",
                extensions,
            );
        }
    }
    Ok(collector.finish())
}

/// The kind of a trust-gated key; `None` for a key this build cannot offer for trust (it then
/// stays stripped).
fn gated_kind(key: &str) -> Option<ProtectedKind> {
    match key {
        "network.ignoreCertificateErrors" => Some(ProtectedKind::IgnoreCertificateErrors),
        "network.proxy" => Some(ProtectedKind::Proxy),
        "network.hostResolverRules" => Some(ProtectedKind::HostResolverRules),
        _ => None,
    }
}

/// A value that changes something: not `null`, `false`, `""`, `[]` or `{}`.
fn active(value: &Value) -> bool {
    match value {
        Value::Null | Value::Bool(false) => false,
        Value::String(text) => !text.is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::Object(map) => !map.is_empty(),
        _ => true,
    }
}

fn local_path(pointer: &str) -> String {
    format!("{CONFIG_DIR}/{LOCAL_CONFIG_FILE}#{pointer}")
}

#[derive(Default)]
struct Collector {
    blocked: Vec<BlockedItem>,
    /// (name, content) pairs the hash is built from.
    entries: Vec<(String, Vec<u8>)>,
    /// The project root, canonical; `None` when it does not resolve - then no link is followed.
    root: Option<PathBuf>,
    /// The canonical folders being walked, outermost first: a link back into one is a cycle.
    walking: Vec<PathBuf>,
    /// See [`Protected::unpinned`].
    unpinned: Vec<String>,
}

impl Collector {
    fn block(&mut self, kind: ProtectedKind, path: String) {
        self.blocked.push(BlockedItem { kind, path });
    }

    fn entry(&mut self, kind: ProtectedKind, name: &str, content: Vec<u8>) {
        self.entries.push((format!("{kind:?}\u{0}{name}"), content));
    }

    fn value(&mut self, kind: ProtectedKind, path: String, name: &str, value: &Value) {
        let canonical = serde_json::to_vec(&canonical(value)).unwrap_or_default();
        self.entry(kind, name, canonical);
        self.block(kind, path);
    }

    /// `config.json`: its headers, extensions and the gated keys it may set.
    fn project(&mut self, project: &Value) {
        self.headers(project, false);
        if let Some(extensions) = project.get("extensions").filter(|v| active(v)) {
            self.value(
                ProtectedKind::Extensions,
                "/extensions".to_owned(),
                "config.json extensions",
                extensions,
            );
        }
        self.gated(project, false);
    }

    /// The request headers of one file (`emulation.headers`, `breakpoints[].headers`); a
    /// committed `config.local.json` (`local`) can carry them as well as `config.json`.
    fn headers(&mut self, file: &Value, local: bool) {
        let file_name = if local {
            LOCAL_CONFIG_FILE
        } else {
            crate::CONFIG_FILE
        };
        let path = |pointer: &str| {
            if local {
                local_path(pointer)
            } else {
                pointer.to_owned()
            }
        };
        if let Some(headers) = file.pointer("/emulation/headers").filter(|v| active(v)) {
            self.value(
                ProtectedKind::Headers,
                path("/emulation/headers"),
                &format!("{file_name} emulation.headers"),
                headers,
            );
        }
        let breakpoints = file.get("breakpoints").and_then(Value::as_array);
        for (index, breakpoint) in breakpoints.into_iter().flatten().enumerate() {
            let Some(headers) = breakpoint.get("headers").filter(|v| active(v)) else {
                continue;
            };
            let id = breakpoint
                .get("id")
                .and_then(Value::as_str)
                .map_or_else(|| format!("#{index}"), str::to_owned);
            self.value(
                ProtectedKind::Headers,
                path(&format!("/breakpoints/{index}/headers")),
                &format!("{file_name} breakpoints[{id}].headers"),
                headers,
            );
        }
    }

    /// The trust-gated keys of one file; in `config.json` (`local` false) without the
    /// local-only ones, which trust does not lift.
    fn gated(&mut self, file: &Value, local: bool) {
        for key in TRUST_GATED_KEYS {
            if !local && LOCAL_ONLY_KEYS.contains(key) {
                continue;
            }
            let Some(kind) = gated_kind(key) else {
                continue;
            };
            let pointer = format!("/{}", key.replace('.', "/"));
            let Some(value) = file.pointer(&pointer).filter(|v| active(v)) else {
                continue;
            };
            let (path, file_name) = if local {
                (local_path(&pointer), LOCAL_CONFIG_FILE)
            } else {
                (pointer.clone(), crate::CONFIG_FILE)
            };
            self.value(kind, path, &format!("{file_name} {key}"), value);
        }
    }

    /// One protected folder: each top-level entry is listed, every file below is hashed.
    fn folder(&mut self, root: &Path, kind: ProtectedKind, name: &str) -> Result<(), ConfigError> {
        let path = root.join(CONFIG_DIR).join(name);
        let relative = format!("{CONFIG_DIR}/{name}");
        let Ok(meta) = fs::symlink_metadata(&path) else {
            return Ok(());
        };
        if !meta.is_dir() {
            self.block(kind, relative.clone());
            return self.walk(kind, &path, &relative);
        }
        self.walking.push(resolved(&path));
        for (entry_name, entry_path) in sorted_entries(&path)? {
            let entry_relative = format!("{relative}/{entry_name}");
            self.block(kind, entry_relative.clone());
            self.walk(kind, &entry_path, &entry_relative)?;
        }
        self.walking.pop();
        Ok(())
    }

    fn walk(
        &mut self,
        kind: ProtectedKind,
        path: &Path,
        relative: &str,
    ) -> Result<(), ConfigError> {
        let meta = fs::symlink_metadata(path).map_err(|source| io_error(path, source))?;
        // `is_symlink` is also true for a Windows junction (a name-surrogate reparse point).
        if meta.file_type().is_symlink() {
            self.link(kind, path, relative)
        } else if meta.is_dir() {
            self.dir(kind, path, relative)
        } else if meta.is_file() {
            self.file(kind, path, relative)
        } else {
            // A pipe or a device: never read (it could block or never end).
            self.entry(kind, relative, b"special".to_vec());
            Ok(())
        }
    }

    /// A link: followed when it leads inside the project, [`Protected::unpinned`] otherwise.
    fn link(
        &mut self,
        kind: ProtectedKind,
        path: &Path,
        relative: &str,
    ) -> Result<(), ConfigError> {
        let target = fs::canonicalize(path).ok().filter(|target| {
            self.root
                .as_ref()
                .is_some_and(|root| target.starts_with(root))
        });
        let meta = target.as_ref().and_then(|target| fs::metadata(target).ok());
        match (target, meta) {
            (Some(target), Some(meta)) if meta.is_dir() => {
                if self.walking.contains(&target) {
                    // Its content is hashed by the walk it leads back into.
                    self.entry(kind, &format!("{relative}/"), b"cycle".to_vec());
                    Ok(())
                } else {
                    self.dir(kind, &target, relative)
                }
            }
            (Some(target), Some(meta)) if meta.is_file() => self.file(kind, &target, relative),
            _ => {
                self.entry(kind, relative, b"unpinned".to_vec());
                self.unpinned.push(relative.to_owned());
                Ok(())
            }
        }
    }

    fn dir(&mut self, kind: ProtectedKind, path: &Path, relative: &str) -> Result<(), ConfigError> {
        self.entry(kind, &format!("{relative}/"), b"dir".to_vec());
        self.walking.push(resolved(path));
        for (name, child) in sorted_entries(path)? {
            self.walk(kind, &child, &format!("{relative}/{name}"))?;
        }
        // An error ends the whole collection, so the stack needs no unwinding then.
        self.walking.pop();
        Ok(())
    }

    fn file(
        &mut self,
        kind: ProtectedKind,
        path: &Path,
        relative: &str,
    ) -> Result<(), ConfigError> {
        let digest = file_digest(path).map_err(|source| io_error(path, source))?;
        let content = format!("file:{}", hex(&digest)).into_bytes();
        self.entry(kind, relative, content);
        Ok(())
    }

    fn finish(mut self) -> Protected {
        self.blocked
            .sort_by(|a, b| (a.kind, &a.path).cmp(&(b.kind, &b.path)));
        self.unpinned.sort();
        if self.entries.is_empty() {
            return Protected {
                blocked: self.blocked,
                hash: String::new(),
                unpinned: self.unpinned,
            };
        }
        self.entries.sort();
        let mut hasher = Sha256::new();
        hasher.update(HASH_DOMAIN);
        for (name, content) in &self.entries {
            for part in [name.as_bytes(), content.as_slice()] {
                hasher.update((part.len() as u64).to_le_bytes());
                hasher.update(part);
            }
        }
        Protected {
            blocked: self.blocked,
            hash: hex(&hasher.finalize()),
            unpinned: self.unpinned,
        }
    }
}

/// The SHA-256 of the file at `path`, read in pieces: a large plugin or mock never sits in
/// memory whole.
fn file_digest(path: &Path) -> std::io::Result<Vec<u8>> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        match file.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => hasher.update(&buffer[..read]),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(hasher.finalize().to_vec())
}

/// `path` resolved, or as given when it does not resolve.
fn resolved(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// The entries of `dir` by name, sorted.
fn sorted_entries(dir: &Path) -> Result<Vec<(String, PathBuf)>, ConfigError> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(dir).map_err(|source| io_error(dir, source))? {
        let entry = entry.map_err(|source| io_error(dir, source))?;
        entries.push((
            entry.file_name().to_string_lossy().into_owned(),
            entry.path(),
        ));
    }
    entries.sort();
    Ok(entries)
}

/// `value` with every object's keys sorted, so the file's key order does not count.
fn canonical(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut sorted = Map::new();
            for key in keys {
                sorted.insert(key.clone(), canonical(&map[key]));
            }
            Value::Object(sorted)
        }
        Value::Array(items) => Value::Array(items.iter().map(canonical).collect()),
        other => other.clone(),
    }
}

#[cfg(test)]
#[path = "hash_tests.rs"]
mod tests;
