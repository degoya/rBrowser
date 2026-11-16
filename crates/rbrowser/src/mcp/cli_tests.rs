use std::path::{Path, PathBuf};

use clap::Parser as _;

use super::{bridge_env, exit_code};
use crate::cli::plan::{Env, plan};
use crate::cli::{Cli, exit};
use rb_mcp::stdio::StdioError;

fn project(dir: &Path, name: &str) -> PathBuf {
    let root = dir.join(name);
    std::fs::create_dir_all(root.join(rb_config::CONFIG_DIR)).expect("dir");
    std::fs::write(
        root.join(rb_config::CONFIG_DIR)
            .join(rb_config::CONFIG_FILE),
        "{\"version\":1}",
    )
    .expect("config");
    root
}

/// Without a project the bridge takes the one running instance, never the last project.
#[test]
fn the_bridge_never_guesses_the_last_project() {
    let dir = tempfile::tempdir().expect("temp dir");
    let last = project(dir.path(), "last");
    let outside = dir.path().join("elsewhere");
    std::fs::create_dir_all(&outside).expect("dir");
    let env = Env {
        cwd: outside,
        windows: false,
        distro: None,
        recent: vec![last.to_string_lossy().into_owned()],
        open_last_project: true,
        focus_on_open: true,
    };
    let cli = Cli::try_parse_from(["rbrowser", "mcp"]).expect("parses");
    assert_eq!(plan(&cli, &env).expect("plan").root, Some(last));
    assert_eq!(plan(&cli, &bridge_env(&env)).expect("plan").root, None);
}

/// The working folder inside a project names it, as interop hands it over.
#[test]
fn the_working_folder_names_the_project() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = project(dir.path(), "app");
    let inside = root.join("src");
    std::fs::create_dir_all(&inside).expect("dir");
    let env = Env {
        cwd: inside,
        windows: false,
        distro: None,
        recent: Vec::new(),
        open_last_project: false,
        focus_on_open: true,
    };
    let cli = Cli::try_parse_from(["rbrowser", "mcp"]).expect("parses");
    assert_eq!(
        plan(&cli, &bridge_env(&env)).expect("plan").root,
        Some(root)
    );
}

#[test]
fn no_instance_is_3_several_is_2() {
    assert_eq!(
        exit_code(&StdioError::NoInstance("x".to_owned())),
        exit::NO_INSTANCE
    );
    assert_eq!(
        exit_code(&StdioError::Several("a, b".to_owned())),
        exit::USAGE
    );
    assert_eq!(
        exit_code(&StdioError::Io(std::io::Error::other("closed"))),
        exit::FAILED
    );
}
