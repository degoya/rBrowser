use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use super::{temporary_path, write_atomic, write_atomic_private};

/// A fresh folder under the system's temporary folder, removed on drop (`std` only, as the
/// crate is).
struct Folder(PathBuf);

impl Folder {
    fn new() -> Folder {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("rb-util-fs-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("temporary folder");
        Folder(path)
    }

    fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&self.0)
            .expect("listed")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_file_is_written_whole_and_replaced_whole() {
    let folder = Folder::new();
    let path = folder.0.join("state.json");
    write_atomic(&path, b"{\"a\":1}\n").expect("written");
    assert_eq!(std::fs::read(&path).expect("read"), b"{\"a\":1}\n");
    write_atomic(&path, b"{}").expect("replaced");
    assert_eq!(std::fs::read(&path).expect("read"), b"{}");
    assert_eq!(folder.names(), ["state.json"], "no temporary file is left");
}

#[test]
fn a_failed_write_leaves_the_old_file_and_no_temporary_one() {
    let folder = Folder::new();
    let path = folder.0.join("kept.json");
    write_atomic(&path, b"old").expect("written");
    // The target is a folder: the rename over it fails after the temporary file was written.
    let blocked = folder.0.join("blocked");
    std::fs::create_dir_all(blocked.join("inside")).expect("folder");
    assert!(write_atomic(&blocked, b"new").is_err());
    assert_eq!(std::fs::read(&path).expect("read"), b"old");
    assert_eq!(folder.names(), ["blocked", "kept.json"]);
    // No folder to write into.
    assert!(write_atomic(&folder.0.join("missing/x.json"), b"x").is_err());
    assert_eq!(folder.names(), ["blocked", "kept.json"]);
}

#[test]
fn a_path_without_a_file_name_is_refused() {
    assert!(write_atomic(Path::new("/"), b"x").is_err());
    assert!(write_atomic(Path::new(""), b"x").is_err());
}

#[test]
fn each_write_has_a_hidden_temporary_file_of_its_own() {
    let target = Path::new("dir").join("search.json");
    let first = temporary_path(&target).expect("a name");
    let second = temporary_path(&target).expect("a name");
    assert_ne!(first, second);
    let pid = std::process::id().to_string();
    for path in [&first, &second] {
        assert_eq!(path.parent(), target.parent(), "beside the target");
        let name = path.file_name().expect("name").to_string_lossy();
        assert!(name.starts_with(".search.json."), "{name}");
        assert!(name.ends_with(".tmp"), "{name}");
        assert!(name.contains(&pid), "{name}");
    }
}

#[test]
fn a_private_file_is_the_owners_only() {
    let folder = Folder::new();
    let path = folder.0.join("instance.json");
    write_atomic_private(&path, b"{}").expect("written");
    assert_eq!(std::fs::read(&path).expect("read"), b"{}");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(&path).expect("meta").permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    assert_eq!(folder.names(), ["instance.json"]);
}
