use std::path::{Path, PathBuf};

use clap::Parser as _;
use rb_bidi::launcher::{Installed, Kind};
use serde_json::{Value, json};

use super::*;
use crate::cli::{Cli, Command};

fn firefox_args(args: &[&str]) -> FirefoxCliArgs {
    let cli =
        Cli::try_parse_from(["rbrowser", "engines", "firefox"].iter().chain(args)).expect("parses");
    match cli.command {
        Some(Command::Engines(EnginesCliArgs {
            command: EnginesCommand::Firefox(args),
        })) => args,
        other => panic!("{other:?}"),
    }
}

fn two() -> Vec<Installed> {
    vec![
        Installed {
            path: PathBuf::from("C:/Program Files/Mozilla Firefox/firefox.exe"),
            kind: Kind::Release,
            version: Some("134.0".to_owned()),
        },
        Installed {
            path: PathBuf::from("C:/Program Files/Firefox Nightly/firefox.exe"),
            kind: Kind::Nightly,
            version: None,
        },
    ]
}

#[test]
fn engines_firefox_needs_list_and_takes_json() {
    assert!(!firefox_args(&["--list"]).json);
    assert!(firefox_args(&["--list", "--json"]).json);
    assert!(Cli::try_parse_from(["rbrowser", "engines", "firefox"]).is_err());
    assert!(Cli::try_parse_from(["rbrowser", "engines"]).is_err());
}

#[test]
fn the_list_prints_one_line_per_installation_and_exits_ok() {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = render(
        &FirefoxCliArgs {
            list: true,
            json: false,
        },
        &two(),
        Some("D:/gone/firefox.exe"),
        |_| false,
        &mut out,
        &mut err,
    );
    assert_eq!(code, exit::OK);
    assert_eq!(
        String::from_utf8(out).expect("utf-8"),
        "configured\tmissing\tD:/gone/firefox.exe\n\
         release\t134.0\tC:/Program Files/Mozilla Firefox/firefox.exe\n\
         nightly\t-\tC:/Program Files/Firefox Nightly/firefox.exe\n"
    );
    assert!(err.is_empty());
}

#[test]
fn no_firefox_says_so_on_stderr_and_still_exits_ok() {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = render(
        &FirefoxCliArgs {
            list: true,
            json: false,
        },
        &[],
        None,
        |_| false,
        &mut out,
        &mut err,
    );
    assert_eq!(code, exit::OK);
    assert!(out.is_empty());
    assert!(
        String::from_utf8(err)
            .expect("utf-8")
            .contains("no Firefox"),
        "the notice goes to stderr"
    );
}

#[test]
fn json_is_the_answer_of_engines_list() {
    let mut out = Vec::new();
    let code = render(
        &FirefoxCliArgs {
            list: true,
            json: true,
        },
        &two(),
        Some("C:/Program Files/Mozilla Firefox/firefox.exe"),
        |path: &Path| path == Path::new("C:/Program Files/Mozilla Firefox/firefox.exe"),
        &mut out,
        &mut Vec::new(),
    );
    assert_eq!(code, exit::OK);
    let value: Value = serde_json::from_slice(&out).expect("json");
    assert_eq!(
        value["firefox"]["configured"],
        json!("C:/Program Files/Mozilla Firefox/firefox.exe")
    );
    assert_eq!(value["firefox"]["installed"][1]["kind"], json!("nightly"));
    assert!(value["firefox"].get("missing").is_none());
}
