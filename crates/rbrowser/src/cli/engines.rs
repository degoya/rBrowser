//! `rbrowser engines firefox --list [--json]` (RB-080-32): the installed Firefox editions, as
//! the settings field and MCP `engines_list` see them - the configured path first (`ok` or
//! `missing`), then `<kind>\t<version>\t<path>` per installation in search order; `--json` the
//! answer of `engines_list`. A local read: no running rBrowser is needed, nothing starts.
//! Exit code 0, also with nothing found (the notice goes to stderr); 2 for a wrong call.

use std::io::Write;
use std::path::Path;

use clap::{Args, Subcommand};
use rb_bidi::launcher::Installed;
use rb_config::UserSettings;

use super::exit;
use crate::engines::firefox::installed;

/// `rbrowser engines`.
#[derive(Debug, Args)]
pub struct EnginesCliArgs {
    /// The engine.
    #[command(subcommand)]
    pub command: EnginesCommand,
}

/// The subcommands of `rbrowser engines`.
#[derive(Debug, Subcommand)]
pub enum EnginesCommand {
    /// The installed Firefox editions rBrowser can drive (`--list`), in the order it searches
    /// them; the first is what an empty `engines.firefox.path` starts.
    Firefox(FirefoxCliArgs),
}

/// `rbrowser engines firefox`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Args)]
pub struct FirefoxCliArgs {
    /// List every installed Firefox: kind (release, esr, developer, nightly), version and
    /// path, one per line, after the configured path and whether it exists.
    #[arg(long, required = true)]
    pub list: bool,

    /// Print the list as JSON (the answer of MCP `engines_list`).
    #[arg(long)]
    pub json: bool,
}

/// Runs `rbrowser engines firefox --list` and answers the exit code.
pub fn run(args: &FirefoxCliArgs, user: &UserSettings) -> i32 {
    render(
        args,
        &installed::installed(),
        user.engines.firefox_executable(),
        Path::is_file,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
}

/// Prints the list for `installed` and the configured path to `out` (`err` for the notice
/// that nothing was found) and answers the exit code.
pub fn render(
    args: &FirefoxCliArgs,
    installed: &[Installed],
    configured: Option<&str>,
    exists: impl Fn(&Path) -> bool,
    out: &mut impl Write,
    err: &mut impl Write,
) -> i32 {
    let text = if args.json {
        let value = installed::list_json(installed, configured, exists);
        format!("{value:#}\n")
    } else {
        let lines = installed::text_lines(installed, configured, exists);
        if installed.is_empty() {
            let _ = writeln!(
                err,
                "rbrowser engines: no Firefox was found; install it or set engines.firefox.path"
            );
        }
        lines.iter().map(|line| format!("{line}\n")).collect()
    };
    if out
        .write_all(text.as_bytes())
        .and_then(|()| out.flush())
        .is_err()
    {
        return exit::FAILED;
    }
    exit::OK
}

#[cfg(test)]
#[path = "engines_tests.rs"]
mod tests;
