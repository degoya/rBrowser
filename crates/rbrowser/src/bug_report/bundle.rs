//! The files of a bug report (RB-093-04): its folder `.rbrowser/out/reports/<time>/` (a free
//! name, `<time>-2` when taken; no link on the way, so a checked-in link cannot send a report
//! elsewhere) or `<time>/` in the folder the person chose (`report.folder`, an existing absolute
//! folder of the user settings, never a project's), `report.md`, the images below `screens/` and
//! `report.zip` with all of it. A report cancelled in the preview is discarded whole.

use std::fs::File;
use std::io::{self, Write as _};
use std::path::{Component, Path, PathBuf};

use zip::CompressionMethod;
use zip::write::{SimpleFileOptions, ZipWriter};

/// The reports folder below the project root.
pub const REPORTS: [&str; 3] = [".rbrowser", "out", "reports"];
/// The Markdown file.
pub const MARKDOWN: &str = "report.md";
/// The ZIP of the whole report.
pub const ZIP: &str = "report.zip";
/// The images' folder inside a report.
pub const SCREENS: &str = "screens";
/// How many `<time>-<n>` names are tried before giving up.
const MAX_TRIES: u32 = 100;

/// Creates a new report folder `<root>/.rbrowser/out/reports/<name>` (`<name>-2`, ... when
/// taken) and answers it.
///
/// # Errors
///
/// A link or a file on the way, no free name, or the disk.
pub fn new_folder(root: &Path, name: &str) -> io::Result<PathBuf> {
    let mut dir = root.to_path_buf();
    for part in REPORTS {
        dir.push(part);
        plain_dir(&dir)?;
    }
    free_folder(&dir, name)
}

/// Creates a new report folder: in `chosen` (`report.folder`, an existing absolute folder) when
/// the person chose one, else [`new_folder`] in the project.
///
/// # Errors
///
/// A chosen folder that is relative, missing or no folder (a missing one is not made); else as
/// [`new_folder`].
pub fn new_report_folder(root: &Path, chosen: Option<&str>, name: &str) -> io::Result<PathBuf> {
    let Some(chosen) = chosen else {
        return new_folder(root, name);
    };
    let base = Path::new(chosen);
    if !base.is_absolute() {
        return Err(io::Error::other(format!(
            "the report folder {chosen} is no absolute path"
        )));
    }
    if !std::fs::metadata(base)?.is_dir() {
        return Err(io::Error::other(format!(
            "the report folder {chosen} is no folder"
        )));
    }
    free_folder(base, name)
}

/// Removes the report `folder` this run made with everything in it (a report cancelled in the
/// preview); a failure is logged, the folder's parent stays.
pub fn discard(folder: &Path) {
    match std::fs::remove_dir_all(folder) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => tracing::warn!(%error, "a cancelled bug report's folder stayed"),
    }
}

/// Creates `<dir>/<name>` (`<name>-2`, ... when taken) and answers it.
fn free_folder(dir: &Path, name: &str) -> io::Result<PathBuf> {
    for n in 1..=MAX_TRIES {
        let candidate = if n == 1 {
            dir.join(name)
        } else {
            dir.join(format!("{name}-{n}"))
        };
        match std::fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::other("no free name for the report folder"))
}

/// `dir` as a directory of its own: made when missing, refused when it is a link or a file.
fn plain_dir(dir: &Path) -> io::Result<()> {
    match std::fs::symlink_metadata(dir) {
        Ok(meta) if meta.file_type().is_symlink() => Err(io::Error::other(format!(
            "{} is a link; a report is written into the project only",
            dir.display()
        ))),
        Ok(meta) if meta.is_dir() => Ok(()),
        Ok(_) => Err(io::Error::other(format!("{} is no folder", dir.display()))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => match std::fs::create_dir(dir) {
            Err(error) if error.kind() != io::ErrorKind::AlreadyExists => Err(error),
            _ => Ok(()),
        },
        Err(error) => Err(error),
    }
}

/// `file` relative to `folder`, `/`-separated; `None` when it lies elsewhere or goes up.
pub fn relative(folder: &Path, file: &Path) -> Option<String> {
    let rest = file.strip_prefix(folder).ok()?;
    let parts: Option<Vec<String>> = rest
        .components()
        .map(|part| match part {
            Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();
    let parts = parts?;
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// Writes `markdown` as `report.md` of `folder` and answers its path.
///
/// # Errors
///
/// The disk.
pub fn write_markdown(folder: &Path, markdown: &str) -> io::Result<PathBuf> {
    let path = folder.join(MARKDOWN);
    let mut file = File::options().write(true).create_new(true).open(&path)?;
    file.write_all(markdown.as_bytes())?;
    Ok(path)
}

/// Writes `report.zip` of every file of `folder` (links left out, the ZIP itself too), in name
/// order with `/`-separated entry names, and answers its path.
///
/// # Errors
///
/// The disk or the ZIP.
pub fn write_zip(folder: &Path) -> io::Result<PathBuf> {
    let path = folder.join(ZIP);
    let mut files = Vec::new();
    collect_files(folder, &mut files)?;
    files.retain(|file| file != &path);
    files.sort();
    let out = File::options().write(true).create_new(true).open(&path)?;
    let mut zip = ZipWriter::new(out);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for file in &files {
        let Some(name) = relative(folder, file) else {
            continue;
        };
        let bytes = std::fs::read(file)?;
        zip.start_file(name.as_str(), options)
            .map_err(io::Error::other)?;
        zip.write_all(&bytes)?;
    }
    zip.finish().map_err(io::Error::other)?;
    Ok(path)
}

fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            collect_files(&entry.path(), files)?;
        } else if kind.is_file() {
            files.push(entry.path());
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "bundle_tests.rs"]
mod tests;
