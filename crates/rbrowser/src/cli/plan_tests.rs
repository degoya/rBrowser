use std::sync::{Arc, Mutex};

use clap::Parser as _;

use super::*;
use crate::instance::protocol::Request;
use crate::instance::server::{Handler, start};

fn cli(args: &[&str]) -> Cli {
    Cli::try_parse_from(std::iter::once("rbrowser").chain(args.iter().copied())).expect("parses")
}

fn env(cwd: &Path) -> Env {
    Env {
        cwd: cwd.to_path_buf(),
        windows: false,
        distro: None,
        recent: Vec::new(),
        open_last_project: false,
        focus_on_open: true,
    }
}

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

#[test]
fn open_with_a_project_and_a_url() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = project(dir.path(), "site");
    let root_text = root.to_string_lossy().into_owned();
    let planned = plan(
        &cli(&["open", "--project", &root_text, "http://localhost:5173/a"]),
        &env(dir.path()),
    )
    .expect("plans");
    assert_eq!(
        planned,
        Plan {
            root: Some(root),
            urls: vec!["http://localhost:5173/a".to_owned()],
            reload: false,
            background: false,
        }
    );
}

#[test]
fn a_relative_folder_argument_names_the_project() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = project(dir.path(), "site");
    let planned = plan(&cli(&["./site", "localhost:3000"]), &env(dir.path())).expect("plans");
    assert_eq!(planned.root, Some(root));
    assert_eq!(planned.urls, ["http://localhost:3000"]);
}

#[test]
fn the_project_is_found_upwards_from_the_working_folder() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = project(dir.path(), "site");
    let inner = root.join("src").join("pages");
    std::fs::create_dir_all(&inner).expect("dir");
    let planned = plan(&cli(&[]), &env(&inner)).expect("plans");
    assert_eq!(planned.root, Some(root));
}

#[test]
fn the_last_project_opens_when_the_setting_asks_for_it() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = project(dir.path(), "site");
    let mut env = env(dir.path());
    env.recent = vec![
        dir.path().join("gone").to_string_lossy().into_owned(),
        root.to_string_lossy().into_owned(),
    ];
    assert_eq!(plan(&cli(&[]), &env).expect("plans").root, None);
    env.open_last_project = true;
    assert_eq!(plan(&cli(&[]), &env).expect("plans").root, Some(root));
}

#[test]
fn a_missing_project_folder_is_refused() {
    let dir = tempfile::tempdir().expect("temp dir");
    let missing = dir.path().join("missing").to_string_lossy().into_owned();
    let (code, _) = plan(&cli(&["--project", &missing]), &env(dir.path())).expect_err("refused");
    assert_eq!(code, exit::NO_PROJECT);
}

#[test]
fn two_project_folders_or_a_bad_url_are_usage_errors() {
    let dir = tempfile::tempdir().expect("temp dir");
    project(dir.path(), "a");
    project(dir.path(), "b");
    let (code, _) = plan(&cli(&["./a", "./b"]), &env(dir.path())).expect_err("refused");
    assert_eq!(code, exit::USAGE);
    let (code, _) =
        plan(&cli(&["--project", "./a", "./b"]), &env(dir.path())).expect_err("refused");
    assert_eq!(code, exit::USAGE);
    let (code, message) =
        plan(&cli(&["./a", "javascript:alert(1)"]), &env(dir.path())).expect_err("refused");
    assert_eq!(code, exit::USAGE);
    assert!(message.contains("javascript"));
}

#[test]
fn reload_needs_a_project_and_takes_no_url() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (code, _) = plan(&cli(&["--reload"]), &env(dir.path())).expect_err("refused");
    assert_eq!(code, exit::NO_PROJECT);
    project(dir.path(), "site");
    let (code, _) = plan(
        &cli(&["--reload", "./site", "http://x.test"]),
        &env(dir.path()),
    )
    .expect_err("refused");
    assert_eq!(code, exit::USAGE);
    let planned =
        plan(&cli(&["--reload", "--project", "./site"]), &env(dir.path())).expect("plans");
    assert!(planned.reload);
}

#[test]
fn background_comes_from_the_flag_or_the_setting() {
    let dir = tempfile::tempdir().expect("temp dir");
    assert!(
        plan(&cli(&["--background"]), &env(dir.path()))
            .expect("plans")
            .background
    );
    let mut env = env(dir.path());
    env.focus_on_open = false;
    assert!(plan(&cli(&[]), &env).expect("plans").background);
}

#[test]
fn a_linux_path_is_translated_on_windows_and_needs_a_distribution() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut env = env(dir.path());
    env.windows = true;
    let (code, message) = plan(&cli(&["--project", "/home/u/app"]), &env).expect_err("refused");
    assert_eq!(code, exit::USAGE);
    assert!(message.contains("WSL"));
}

fn recording() -> (Handler, Arc<Mutex<Vec<Request>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    let handler: Handler = Arc::new(move |request| {
        log.lock().expect("lock").push(request);
        Ok(None)
    });
    (handler, seen)
}

#[test]
fn a_second_call_hands_over_and_ends_with_zero() {
    let local = tempfile::tempdir().expect("temp dir");
    let root = project(local.path(), "site");
    let (handler, seen) = recording();
    start(local.path(), &root, handler).expect("listens");
    let open = Plan {
        root: Some(root.clone()),
        urls: vec!["http://localhost:5173/a".to_owned()],
        reload: false,
        background: false,
    };
    assert_eq!(forward(&open, Some(local.path())), Some(exit::OK));
    let reload = Plan {
        reload: true,
        urls: Vec::new(),
        ..open
    };
    assert_eq!(forward(&reload, Some(local.path())), Some(exit::OK));
    assert_eq!(seen.lock().expect("lock").len(), 2);
}

#[test]
fn without_a_running_instance_open_starts_and_reload_fails() {
    let local = tempfile::tempdir().expect("temp dir");
    let root = project(local.path(), "site");
    let open = Plan {
        root: Some(root),
        urls: Vec::new(),
        reload: false,
        background: false,
    };
    assert_eq!(forward(&open, Some(local.path())), None);
    let reload = Plan {
        reload: true,
        ..open
    };
    assert_eq!(
        forward(&reload, Some(local.path())),
        Some(exit::NO_INSTANCE)
    );
    assert_eq!(forward(&reload, None), Some(exit::NO_INSTANCE));
}

#[test]
fn another_project_starts_its_own_instance() {
    let local = tempfile::tempdir().expect("temp dir");
    let (handler, seen) = recording();
    start(local.path(), &project(local.path(), "site"), handler).expect("listens");
    let other = Plan {
        root: Some(project(local.path(), "other")),
        urls: Vec::new(),
        reload: false,
        background: false,
    };
    assert_eq!(forward(&other, Some(local.path())), None);
    assert!(seen.lock().expect("lock").is_empty());
}

#[test]
fn without_a_project_nothing_is_handed_over() {
    let local = tempfile::tempdir().expect("temp dir");
    let plan = Plan {
        root: None,
        urls: Vec::new(),
        reload: false,
        background: false,
    };
    assert_eq!(forward(&plan, Some(local.path())), None);
}

#[test]
fn the_environment_takes_the_settings_it_is_given() {
    let user = rb_config::UserSettings {
        recent_projects: vec!["C:\\site".to_owned()],
        startup: rb_config::StartupSettings {
            open_last_project: true,
            focus_on_open: false,
        },
        ..rb_config::UserSettings::default()
    };
    let env = Env::from_user(Path::new("/work"), &user);
    assert_eq!(env.cwd, Path::new("/work"));
    assert_eq!(env.recent, user.recent_projects);
    assert!(env.open_last_project);
    assert!(!env.focus_on_open);
}

#[test]
fn before_ui_answers_the_plan_the_launch_goes_on_with() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = project(dir.path(), "site");
    let root_text = root.to_string_lossy().into_owned();
    let user = rb_config::UserSettings {
        startup: rb_config::StartupSettings {
            focus_on_open: false,
            ..rb_config::StartupSettings::default()
        },
        ..rb_config::UserSettings::default()
    };
    let planned = before_ui(
        &cli(&["--project", &root_text, "http://localhost:5173/a"]),
        &user,
    )
    .expect("goes on")
    .expect("a plan");
    assert_eq!(planned.root, Some(root));
    assert_eq!(planned.urls, vec!["http://localhost:5173/a".to_owned()]);
    assert!(planned.background, "from the settings given");
    let refused = before_ui(
        &cli(&["--project", &root_text, "--reload", "http://a.test/"]),
        &user,
    );
    assert_eq!(refused, Err(exit::USAGE));
}
