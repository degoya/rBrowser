//! The diagnostics ZIP (`rbrowser diagnostics [--out <zip>]`, the command `diagnostics.create`,
//! the MCP tool `diagnostics`): the logs, the newest crash dumps, the versions, the effective
//! config and the plugin list, with every secret masked. Nothing leaves the machine; the user
//! passes the file on by hand.
//!
//! Entries: `logs/<file>` (every file of the log folder, masked), `crashdumps/<file>.dmp` (the
//! newest `max_crash_dumps` of the Crashpad database), `versions.json`, `config.effective.json`
//! (masked; `null` without a project) and `plugins.json`. A value masked in the config (a header
//! value, the password of a proxy) is masked in the logs too.

use std::fs::File;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::Serialize;
use serde_json::Value;
use zip::CompressionMethod;
use zip::write::{SimpleFileOptions, ZipWriter};

use crate::cef::civil_from_days;
use crate::error::DiagError;
use crate::paths::DiagPaths;
use crate::redact::{Secrets, redact_json};

/// The entry of the versions.
pub const VERSIONS_ENTRY: &str = "versions.json";
/// The entry of the effective config.
pub const CONFIG_ENTRY: &str = "config.effective.json";
/// The entry of the plugin list.
pub const PLUGINS_ENTRY: &str = "plugins.json";
/// The folder of the logs inside the ZIP.
pub const LOGS_DIR: &str = "logs/";
/// The folder of the crash dumps inside the ZIP.
pub const DUMPS_DIR: &str = "crashdumps/";

/// The versions of everything rBrowser runs on; `None` where a part is not found.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Versions {
    /// rBrowser, with commit and build time.
    pub app: String,
    /// CEF.
    pub cef: Option<String>,
    /// The Chromium inside CEF.
    pub chromium: Option<String>,
    /// The installed Firefox.
    pub firefox: Option<String>,
    /// The Node of the sidecar.
    pub node: Option<String>,
    /// Playwright in the sidecar.
    pub playwright: Option<String>,
    /// The operating system.
    pub os: OsInfo,
}

/// The operating system.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OsInfo {
    /// `windows`, `linux`, `macos`.
    pub family: String,
    /// `x86_64`, `aarch64`.
    pub arch: String,
    /// The OS version (Windows build), when the caller knows it.
    pub version: Option<String>,
}

impl Default for OsInfo {
    fn default() -> Self {
        Self {
            family: std::env::consts::OS.to_owned(),
            arch: std::env::consts::ARCH.to_owned(),
            version: None,
        }
    }
}

/// What goes into the ZIP.
#[derive(Clone, Debug, PartialEq)]
pub struct BundleInput {
    /// Where the logs and the crash dumps are.
    pub paths: DiagPaths,
    /// The versions.
    pub versions: Versions,
    /// The effective project config (`config.json` <- `config.local.json`), unmasked; `None`
    /// without a project.
    pub config_effective: Option<Value>,
    /// The plugins: id, version, enabled.
    pub plugins: Value,
    /// The newest crash dumps to take (setting `logs.crashDumpsInBundle`).
    pub max_crash_dumps: usize,
}

/// What was written.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct BundleReport {
    /// The ZIP.
    pub path: PathBuf,
    /// Its entries, in order.
    pub entries: Vec<String>,
}

/// The file name of a ZIP made at `now`: `rbrowser-diagnostics-YYYYMMDD-HHMMSS.zip` (UTC).
pub fn bundle_file_name(now: SystemTime) -> String {
    let seconds = now
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let days = i64::try_from(seconds / 86_400).unwrap_or(0);
    let (year, month, day) = civil_from_days(days);
    let rest = seconds % 86_400;
    format!(
        "rbrowser-diagnostics-{year:04}{month:02}{day:02}-{:02}{:02}{:02}.zip",
        rest / 3_600,
        rest / 60 % 60,
        rest % 60
    )
}

/// Where a ZIP made at `now` goes without `--out`.
pub fn default_bundle_path(paths: &DiagPaths, now: SystemTime) -> PathBuf {
    paths.bundles.join(bundle_file_name(now))
}

/// [`default_bundle_path`], or `<name>-2.zip`, `-3`, ... when a ZIP of the same second exists.
pub fn free_bundle_path(paths: &DiagPaths, now: SystemTime) -> PathBuf {
    let first = default_bundle_path(paths, now);
    if !first.exists() {
        return first;
    }
    let name = bundle_file_name(now);
    let stem = name.trim_end_matches(".zip");
    (2u32..1000)
        .map(|n| paths.bundles.join(format!("{stem}-{n}.zip")))
        .find(|candidate| !candidate.exists())
        .unwrap_or(first)
}

/// Writes the ZIP to `out` (its folder is created). An existing file is never overwritten:
/// `out` must not exist yet.
pub fn write_bundle(input: &BundleInput, out: &Path) -> Result<BundleReport, DiagError> {
    let fail = |source: io::Error| DiagError::Bundle {
        path: out.to_path_buf(),
        source,
    };
    let secrets = Secrets::default();
    let config = match &input.config_effective {
        Some(config) => {
            let redacted = redact_json(config);
            redacted.removed.iter().for_each(|value| secrets.add(value));
            redacted.value
        }
        None => Value::Null,
    };
    let plugins = redact_json(&input.plugins);
    plugins.removed.iter().for_each(|value| secrets.add(value));
    let logs = list_files(&input.paths.logs).map_err(fail)?;
    let dumps = newest_dumps(&input.paths.crash_dumps, input.max_crash_dumps).map_err(fail)?;

    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(fail)?;
    }
    let file = File::options()
        .write(true)
        .create_new(true)
        .open(out)
        .map_err(fail)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut entries = Vec::new();
    let mut add = |zip: &mut ZipWriter<File>, name: String, bytes: &[u8]| -> io::Result<()> {
        zip.start_file(name.as_str(), options)
            .map_err(io::Error::other)?;
        zip.write_all(bytes)?;
        entries.push(name);
        Ok(())
    };
    let json = |value: &Value| serde_json::to_vec_pretty(value).map_err(io::Error::other);

    zip.add_directory(LOGS_DIR, options)
        .map_err(|error| fail(io::Error::other(error)))?;
    for log in &logs {
        let text = std::fs::read(log).map_err(fail)?;
        let masked = secrets.redact(&crate::redact::redact(&String::from_utf8_lossy(&text)));
        add(
            &mut zip,
            format!("{LOGS_DIR}{}", file_name(log)),
            masked.as_bytes(),
        )
        .map_err(fail)?;
    }
    for dump in &dumps {
        let bytes = std::fs::read(dump).map_err(fail)?;
        add(&mut zip, format!("{DUMPS_DIR}{}", file_name(dump)), &bytes).map_err(fail)?;
    }
    let versions =
        serde_json::to_value(&input.versions).map_err(|error| fail(io::Error::other(error)))?;
    add(
        &mut zip,
        VERSIONS_ENTRY.to_owned(),
        &json(&versions).map_err(fail)?,
    )
    .map_err(fail)?;
    add(
        &mut zip,
        CONFIG_ENTRY.to_owned(),
        &json(&config).map_err(fail)?,
    )
    .map_err(fail)?;
    add(
        &mut zip,
        PLUGINS_ENTRY.to_owned(),
        &json(&plugins.value).map_err(fail)?,
    )
    .map_err(fail)?;
    zip.finish()
        .map_err(|error| fail(io::Error::other(error)))?;

    let mut all = vec![LOGS_DIR.to_owned()];
    all.extend(entries);
    Ok(BundleReport {
        path: out.to_path_buf(),
        entries: all,
    })
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The files directly in `dir`, by name, without ZIPs; none when `dir` does not exist.
fn list_files(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let read = match std::fs::read_dir(dir) {
        Ok(read) => read,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut files = Vec::new();
    for entry in read {
        let entry = entry?;
        let path = entry.path();
        let is_zip = path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"));
        if entry.file_type()?.is_file() && !is_zip {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

/// The newest `limit` `.dmp` files anywhere under `dir` (Crashpad keeps them in `reports\`,
/// pending ones in `pending\`), newest first.
fn newest_dumps(dir: &Path, limit: usize) -> io::Result<Vec<PathBuf>> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let mut dumps = Vec::new();
    let mut stack = vec![(dir.to_path_buf(), 0_u8)];
    while let Some((current, depth)) = stack.pop() {
        let read = match std::fs::read_dir(&current) {
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        for entry in read {
            let entry = entry?;
            let kind = entry.file_type()?;
            let path = entry.path();
            if kind.is_dir() && depth < 3 {
                stack.push((path, depth + 1));
            } else if kind.is_file()
                && path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("dmp"))
            {
                let modified = entry
                    .metadata()?
                    .modified()
                    .unwrap_or(SystemTime::UNIX_EPOCH);
                dumps.push((modified, path));
            }
        }
    }
    dumps.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    Ok(dumps
        .into_iter()
        .take(limit)
        .map(|(_, path)| path)
        .collect())
}

#[cfg(test)]
#[path = "bundle_tests.rs"]
mod tests;
