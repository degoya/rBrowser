//! A portable installation of version A, its data folder, and the ZIP of version B, for the
//! install tests: either the data in the profile (`%LOCALAPPDATA%\rBrowser`) or the portable mode
//! (a `portable` marker and `data\local` beside the program).

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use super::{DEFAULT_CONFIRM_TIMEOUT_SECS, Journal, Plan};
use crate::{install_kind::InstallKind, sign::hex_sha256};

pub(crate) const OLD: &str = "1.0.0";
pub(crate) const NEW: &str = "1.1.0";
pub(crate) const EXE: &str = "rbrowser.exe";

pub(crate) struct Fixture {
    pub _root: tempfile::TempDir,
    pub install: PathBuf,
    pub data: PathBuf,
    pub journal: Journal,
}

impl Fixture {
    /// The data folder in the profile, apart from the program.
    pub(crate) fn zip() -> Self {
        Self::new(false)
    }

    /// The portable mode: the data folder inside the program folder, never moved.
    pub(crate) fn portable() -> Self {
        Self::new(true)
    }

    fn new(portable: bool) -> Self {
        let root = tempfile::tempdir().expect("tempdir");
        let install = root.path().join("rbrowser-1.0.0-windows-x64");
        let data = if portable {
            write(&install.join("portable"), "");
            install.join("data").join("local")
        } else {
            root.path().join("appdata").join("rBrowser")
        };
        write(&install.join(EXE), "old program");
        write(&install.join("libcef.dll"), "old cef");
        write(&install.join("VERSION.txt"), &format!("rBrowser {OLD}\n"));
        write(&install.join("ui").join("index.html"), "old ui");
        write(&install.join("notes.txt"), "the person's own file");
        write(&data.join("settings.json"), "the person's settings");
        let artifact = root.path().join("rbrowser-1.1.0-windows-x64.zip");
        let folder = "rbrowser-1.1.0-windows-x64";
        let files: Vec<(String, &str)> = vec![
            (format!("{folder}/{EXE}"), "new program"),
            (format!("{folder}/libcef.dll"), "new cef"),
            (format!("{folder}/VERSION.txt"), "rBrowser 1.1.0\n"),
            (format!("{folder}/ui/index.html"), "new ui"),
            (format!("{folder}/LICENSE"), "licence"),
            // Never moved, whatever an archive carries.
            (
                format!("{folder}/data/local/settings.json"),
                "from the archive",
            ),
            (format!("{folder}/portable"), ""),
        ];
        zip_archive(&artifact, &files);
        let bytes = fs::read(&artifact).expect("artifact");
        let plan = Plan {
            kind: InstallKind::Portable,
            from_version: OLD.to_owned(),
            target_version: NEW.to_owned(),
            artifact,
            sha256: hex_sha256(&bytes),
            size: bytes.len() as u64,
            install_dir: install.clone(),
            executable: EXE.to_owned(),
            data_dir: data.clone(),
            args: Vec::new(),
            start_new: false,
            confirm_timeout_secs: DEFAULT_CONFIRM_TIMEOUT_SECS,
        };
        let mut journal = Journal::begin(plan);
        journal.write().expect("journal");
        Self {
            _root: root,
            install,
            data,
            journal,
        }
    }

    pub(crate) fn read(&self, relative: &str) -> Option<String> {
        fs::read_to_string(self.install.join(relative)).ok()
    }

    pub(crate) fn stored(&self) -> Journal {
        Journal::read(&self.data)
            .expect("journal")
            .expect("written")
    }

    /// Everything version A had, and nothing version B brought.
    pub(crate) fn assert_old(&self) {
        assert_eq!(self.read(EXE).as_deref(), Some("old program"));
        assert_eq!(self.read("libcef.dll").as_deref(), Some("old cef"));
        assert_eq!(
            self.read("VERSION.txt").as_deref(),
            Some(format!("rBrowser {OLD}\n").as_str())
        );
        assert_eq!(self.read("ui/index.html").as_deref(), Some("old ui"));
        assert!(self.read("LICENSE").is_none());
        assert!(!self.journal.previous_dir().exists());
        assert!(!self.journal.staged_dir().exists());
        self.assert_untouched();
    }

    /// Everything version B brought, version A aside.
    pub(crate) fn assert_new(&self) {
        assert_eq!(self.read(EXE).as_deref(), Some("new program"));
        assert_eq!(self.read("libcef.dll").as_deref(), Some("new cef"));
        assert_eq!(self.read("ui/index.html").as_deref(), Some("new ui"));
        assert_eq!(self.read("LICENSE").as_deref(), Some("licence"));
        assert_eq!(
            fs::read_to_string(self.journal.previous_dir().join(EXE))
                .ok()
                .as_deref(),
            Some("old program")
        );
        self.assert_untouched();
    }

    /// What is neither version's stays where it was: the person's files and data, and the
    /// portable mode's marker and data folder.
    pub(crate) fn assert_untouched(&self) {
        assert_eq!(
            self.read("notes.txt").as_deref(),
            Some("the person's own file")
        );
        assert_eq!(
            fs::read_to_string(self.data.join("settings.json"))
                .ok()
                .as_deref(),
            Some("the person's settings")
        );
        let portable = self.data.starts_with(&self.install);
        assert_eq!(self.install.join("portable").is_file(), portable);
        assert_eq!(self.install.join("data").is_dir(), portable);
    }
}

pub(crate) fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("folder");
    }
    fs::write(path, text).expect("write");
}

/// `previous/locked/rbrowser-mcp` in a folder that refuses removals; `None` (and the test is
/// moot) where permissions do not hold, as for root.
#[cfg(unix)]
pub(crate) fn locked_folder(previous: &Path) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt as _;
    let locked = previous.join("locked");
    write(&locked.join("rbrowser-mcp"), "a server a client started");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).expect("lock");
    if fs::write(locked.join("probe"), "").is_ok() {
        eprintln!("permissions do not hold for this user; nothing to prove");
        unlock(&locked);
        return None;
    }
    Some(locked)
}

#[cfg(unix)]
pub(crate) fn unlock(folder: &Path) {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(folder, fs::Permissions::from_mode(0o755)).expect("unlock");
}

pub(crate) fn zip_archive(path: &Path, files: &[(String, &str)]) {
    let file = fs::File::create(path).expect("archive");
    let mut writer = zip::ZipWriter::new(file);
    for (name, text) in files {
        writer
            .start_file(name.as_str(), zip::write::SimpleFileOptions::default())
            .expect("entry");
        writer.write_all(text.as_bytes()).expect("write");
    }
    writer.finish().expect("zip");
}
