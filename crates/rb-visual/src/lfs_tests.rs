use std::cell::RefCell;

use super::*;

/// A Git that answers as told and records the installs.
#[derive(Default)]
struct FakeGit {
    installed: bool,
    install_fails: bool,
    installs: RefCell<u32>,
}

impl Git for FakeGit {
    fn lfs_installed(&self, _root: &Path) -> bool {
        self.installed
    }

    fn lfs_install_local(&self, _root: &Path) -> Result<(), String> {
        *self.installs.borrow_mut() += 1;
        if self.install_fails {
            Err("not a git repository".to_owned())
        } else {
            Ok(())
        }
    }
}

fn git(installed: bool) -> FakeGit {
    FakeGit {
        installed,
        ..FakeGit::default()
    }
}

#[test]
fn the_usual_lfs_lines_track_the_baselines() {
    for text in [
        LFS_LINE,
        "/.rbrowser/baselines/** filter=lfs diff=lfs merge=lfs -text",
        "*.png filter=lfs diff=lfs merge=lfs -text",
        "# images\n**/*.png filter=lfs diff=lfs merge=lfs -text\n",
    ] {
        assert!(tracks_baselines(text), "{text}");
    }
}

#[test]
fn other_patterns_and_unset_filters_do_not() {
    for text in [
        "",
        "*.jpg filter=lfs diff=lfs merge=lfs -text",
        ".rbrowser/baselines/** text",
        "# .rbrowser/baselines/** filter=lfs",
        "*.png filter=lfs\n.rbrowser/baselines/** -filter",
        "*.png filter=custom",
    ] {
        assert!(!tracks_baselines(text), "{text}");
    }
}

#[test]
fn the_line_is_appended_on_a_line_of_its_own() {
    assert_eq!(with_lfs_line(""), format!("{LFS_LINE}\n"));
    assert_eq!(
        with_lfs_line("*.sh text eol=lf"),
        format!("*.sh text eol=lf\n{LFS_LINE}\n")
    );
    assert_eq!(
        with_lfs_line("*.sh text eol=lf\n"),
        format!("*.sh text eol=lf\n{LFS_LINE}\n")
    );
}

#[test]
fn a_repository_without_the_line_or_lfs_is_named() {
    let repo = tempfile::tempdir().expect("temp repo");
    assert_eq!(
        check(repo.path(), &git(false)),
        LfsStatus {
            tracked: false,
            installed: false,
        }
    );
    std::fs::write(repo.path().join(GITATTRIBUTES), format!("{LFS_LINE}\n")).expect("attributes");
    let status = check(repo.path(), &git(true));
    assert!(status.ok());
}

#[test]
fn the_set_up_adds_the_line_once_and_runs_the_local_install() {
    let repo = tempfile::tempdir().expect("temp repo");
    let path = repo.path().join(GITATTRIBUTES);
    std::fs::write(&path, "*.sh text eol=lf\n").expect("attributes");
    let git = git(true);
    assert_eq!(
        set_up(repo.path(), &git).expect("set up"),
        SetUp { line_added: true }
    );
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        format!("*.sh text eol=lf\n{LFS_LINE}\n")
    );
    assert_eq!(
        set_up(repo.path(), &git).expect("again"),
        SetUp { line_added: false }
    );
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        format!("*.sh text eol=lf\n{LFS_LINE}\n"),
        "the line is not added twice"
    );
    assert_eq!(*git.installs.borrow(), 2);
}

#[test]
fn without_git_lfs_the_set_up_changes_nothing() {
    let repo = tempfile::tempdir().expect("temp repo");
    let git = git(false);
    assert!(matches!(
        set_up(repo.path(), &git),
        Err(VisualError::Lfs(_))
    ));
    assert!(!repo.path().join(GITATTRIBUTES).exists());
    assert_eq!(*git.installs.borrow(), 0);
}

#[test]
fn a_failed_install_is_an_error() {
    let repo = tempfile::tempdir().expect("temp repo");
    let git = FakeGit {
        installed: true,
        install_fails: true,
        ..FakeGit::default()
    };
    assert!(matches!(
        set_up(repo.path(), &git),
        Err(VisualError::Lfs(_))
    ));
}

#[cfg(unix)]
#[test]
fn a_linked_gitattributes_is_never_written() {
    let repo = tempfile::tempdir().expect("temp repo");
    let elsewhere = repo.path().join("elsewhere");
    std::fs::write(&elsewhere, "keep\n").expect("target");
    std::os::unix::fs::symlink(&elsewhere, repo.path().join(GITATTRIBUTES)).expect("link");
    assert!(!check(repo.path(), &git(true)).tracked);
    assert!(set_up(repo.path(), &git(true)).is_err());
    assert_eq!(std::fs::read_to_string(&elsewhere).expect("kept"), "keep\n");
}

#[test]
fn the_set_up_line_covers_the_pngs_not_the_meta_files() {
    assert!(LFS_LINE.starts_with(".rbrowser/baselines/**/*.png "));
    assert!(tracks_baselines(LFS_LINE));
}

#[test]
fn a_gitattributes_below_the_root_counts_and_the_deeper_one_wins() {
    let repo = tempfile::tempdir().expect("temp repo");
    let rbrowser = repo.path().join(".rbrowser");
    let baselines = rbrowser.join("baselines");
    std::fs::create_dir_all(&baselines).expect("folders");
    std::fs::write(
        rbrowser.join(GITATTRIBUTES),
        "baselines/**/*.png filter=lfs diff=lfs merge=lfs -text\n",
    )
    .expect("attributes");
    assert!(
        check(repo.path(), &git(true)).ok(),
        ".rbrowser/.gitattributes"
    );
    std::fs::remove_file(rbrowser.join(GITATTRIBUTES)).expect("removed");

    std::fs::write(
        baselines.join(GITATTRIBUTES),
        "*.png filter=lfs diff=lfs merge=lfs -text\n",
    )
    .expect("attributes");
    assert!(
        check(repo.path(), &git(true)).ok(),
        "baselines/.gitattributes"
    );
    let set_up_git = git(true);
    assert_eq!(
        set_up(repo.path(), &set_up_git).expect("set up"),
        SetUp { line_added: false },
        "no root line when a deeper file tracks them"
    );
    assert!(!repo.path().join(GITATTRIBUTES).exists());

    std::fs::write(repo.path().join(GITATTRIBUTES), format!("{LFS_LINE}\n")).expect("root");
    std::fs::write(baselines.join(GITATTRIBUTES), "*.png -filter\n").expect("unset");
    assert!(
        !check(repo.path(), &git(true)).tracked,
        "the deeper file unsets the root's filter"
    );
}
