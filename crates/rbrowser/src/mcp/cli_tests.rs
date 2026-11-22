use std::path::{Path, PathBuf};

use clap::Parser as _;

use super::{McpCommand, ToolsFormat, bridge_env, exit_code, tools_json};
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

#[test]
fn mcp_tools_prints_the_core_tools_in_the_messages_api_format() {
    let cli = Cli::try_parse_from([
        "rbrowser",
        "mcp",
        "tools",
        "--format",
        "anthropic",
        "--allowed-caller",
        "code_execution_20260120",
    ])
    .expect("parses");
    let Some(crate::cli::Command::Mcp(args)) = &cli.command else {
        panic!("the mcp command");
    };
    let Some(McpCommand::Tools(tools)) = &args.command else {
        panic!("the tools subcommand");
    };
    assert_eq!(tools.format, ToolsFormat::Anthropic);
    let printed: serde_json::Value = serde_json::from_str(&tools_json(tools)).expect("JSON");
    let navigate = printed
        .as_array()
        .and_then(|tools| tools.iter().find(|tool| tool["name"] == "navigate"))
        .expect("navigate is listed");
    assert_eq!(navigate["input_schema"]["type"], "object");
    assert_eq!(
        navigate["allowed_callers"],
        serde_json::json!(["code_execution_20260120"])
    );
}

#[test]
fn bare_rbrowser_mcp_stays_the_stdio_bridge() {
    let cli = Cli::try_parse_from(["rbrowser", "mcp"]).expect("parses");
    let Some(crate::cli::Command::Mcp(args)) = &cli.command else {
        panic!("the mcp command");
    };
    assert!(args.command.is_none());
}
