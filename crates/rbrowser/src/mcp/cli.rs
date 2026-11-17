//! `rbrowser mcp`: the stdio front end (RB-030-02, K7). It finds the project as `rbrowser` does
//! (`--project DIR`, a Linux path from WSL translated to the share; else upwards from the
//! working folder, which interop hands over as the share path) - but never the last project:
//! without a project the bridge takes the one running instance. It forwards stdin and stdout to
//! that project's running instance (`rb_mcp::stdio`), starting it first when
//! `mcp.stdioAutoStart` asks for it ([`super::autostart`]). Without an instance the client still
//! connects and every call says what to do; when stdin ends without one ever reached, the exit
//! code is 3 (none) or 2 (several, no project).

use rb_config::UserSettings;
use rb_mcp::stdio::{Locator, StdioError};

use super::autostart;
use crate::cli::plan::{Env, plan};
use crate::cli::{Cli, exit};

/// Runs `rbrowser mcp` in `env` with the user settings `user`; returns the exit code.
pub fn run(cli: &Cli, env: &Env, user: &UserSettings) -> i32 {
    let env = bridge_env(env);
    let root = match plan(cli, &env) {
        Ok(plan) => plan.root,
        Err((code, message)) => {
            eprintln!("rbrowser mcp: {message}");
            return code;
        }
    };
    let Some(local) = rb_config::local_dir() else {
        eprintln!("rbrowser mcp: no local folder (LOCALAPPDATA is not set)");
        return exit::FAILED;
    };
    let key = root.as_deref().map(crate::instance::file::key);
    let shown = root.as_ref().map(|root| root.display().to_string());
    let locator = Locator::new(&local, key.as_deref(), shown.as_deref());
    if let Some(root) = root.as_deref()
        && autostart::wanted(&user.mcp, Some(root))
        && matches!(locator.find(), Err(StdioError::NoInstance(_)))
    {
        autostart::start_and_wait(root, &locator);
    }
    match rb_mcp::stdio::run(&locator) {
        Ok(()) => exit::OK,
        Err(error) => {
            tracing::warn!(%error, "rbrowser mcp ended");
            eprintln!("rbrowser mcp: {error}");
            exit_code(&error)
        }
    }
}

/// `env` for the bridge: the last project is no guess for it.
pub fn bridge_env(env: &Env) -> Env {
    Env {
        open_last_project: false,
        ..env.clone()
    }
}

/// The exit code of a failed `rbrowser mcp`.
pub fn exit_code(error: &StdioError) -> i32 {
    match error {
        StdioError::NoInstance(_) => exit::NO_INSTANCE,
        StdioError::Several(_) => exit::USAGE,
        StdioError::Io(_) => exit::FAILED,
    }
}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod tests;
