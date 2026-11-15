use super::*;
use clap::CommandFactory;

#[test]
fn the_command_line_is_well_formed() {
    Cli::command().debug_assert();
}

#[test]
fn the_version_starts_with_the_workspace_version() {
    assert!(VERSION.starts_with(env!("CARGO_PKG_VERSION")));
}

#[test]
fn open_and_project_name_the_project() {
    let cli = Cli::try_parse_from(["rbrowser", "open", "C:\\site"]).expect("parses");
    assert_eq!(cli.project_dir(), Some(PathBuf::from("C:\\site")));
    let cli =
        Cli::try_parse_from(["rbrowser", "--project", "D:\\x", "--no-sandbox"]).expect("parses");
    assert_eq!(cli.project_dir(), Some(PathBuf::from("D:\\x")));
    assert!(cli.no_sandbox);
    let cli = Cli::try_parse_from(["rbrowser"]).expect("parses");
    assert_eq!(cli.project_dir(), None);
}

#[test]
fn diagnostics_takes_an_output_path() {
    let cli = Cli::try_parse_from(["rbrowser", "diagnostics", "--out", "d.zip"]).expect("parses");
    match cli.command {
        Some(Command::Diagnostics(args)) => {
            assert_eq!(args.out.as_deref(), Some(std::path::Path::new("d.zip")));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn open_takes_a_project_and_urls_and_the_flags_after_it() {
    let cli = Cli::try_parse_from([
        "rbrowser",
        "open",
        "--project",
        "C:\\fixture",
        "--background",
        "http://localhost:5173/a",
    ])
    .expect("parses");
    assert_eq!(cli.project_dir(), Some(PathBuf::from("C:\\fixture")));
    assert_eq!(cli.urls(), ["http://localhost:5173/a"]);
    assert_eq!(cli.start_url().as_deref(), Some("http://localhost:5173/a"));
    assert!(cli.background);
    assert!(!cli.reload);
}

#[test]
fn urls_and_a_folder_work_without_a_command() {
    let cli = Cli::try_parse_from(["rbrowser", ".", "localhost:3000", "--reload"]).expect("parses");
    assert_eq!(cli.project_dir(), Some(PathBuf::from(".")));
    assert_eq!(cli.urls(), ["localhost:3000"]);
    assert_eq!(cli.start_url().as_deref(), Some("http://localhost:3000"));
    assert!(cli.reload);
}

#[test]
fn the_url_option_comes_first() {
    let cli = Cli::try_parse_from(["rbrowser", "--url", "http://a.test", "http://b.test"])
        .expect("parses");
    assert_eq!(cli.urls(), ["http://a.test", "http://b.test"]);
    let cli = Cli::try_parse_from(["rbrowser", "javascript:alert(1)"]).expect("parses");
    assert_eq!(cli.start_url(), None);
}

#[test]
fn init_takes_a_url_and_yes() {
    let cli = Cli::try_parse_from([
        "rbrowser",
        "init",
        "--url",
        "http://localhost:5173",
        "--yes",
    ])
    .expect("parses");
    match cli.command {
        Some(Command::Init(args)) => {
            assert_eq!(args.start_url.as_deref(), Some("http://localhost:5173"));
            assert!(args.yes);
        }
        other => panic!("{other:?}"),
    }
    let cli =
        Cli::try_parse_from(["rbrowser", "init", "-y", "--project", "D:\\x"]).expect("parses");
    assert!(matches!(
        cli.command,
        Some(Command::Init(InitArgs { yes: true, .. }))
    ));
    assert_eq!(cli.project, Some(PathBuf::from("D:\\x")));
}

#[test]
fn screenshot_is_a_command_with_its_flags() {
    let cli = Cli::try_parse_from([
        "rbrowser",
        "screenshot",
        "--views",
        "mobile,desktop",
        "--overview",
        "--json",
        "--project",
        "D:\\site",
    ])
    .expect("parses");
    match cli.command {
        Some(Command::Screenshot(args)) => {
            assert_eq!(args.views, vec!["mobile", "desktop"]);
            assert!(args.overview && args.json);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(cli.project, Some(PathBuf::from("D:\\site")));
}

#[test]
fn the_help_lists_the_exit_codes() {
    let help = Cli::command().render_long_help().to_string();
    assert!(help.contains("Exit codes:"));
    assert!(help.contains("--reload"));
    assert!(help.contains("--background"));
    assert!(help.contains("screenshot"));
}
