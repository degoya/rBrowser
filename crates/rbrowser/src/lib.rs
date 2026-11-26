//! rBrowser: every breakpoint of a site at once, in one window with a configurable grid
//! (RB-010-04). The library is the whole app: `rbrowser.dll` for CEF's `bootstrap.exe` (the
//! sandboxed layout of ADR 0001) and the code behind `rbrowser-nosandbox.exe`.
//!
//! Everything but [`cef_app`] runs on every platform and is tested on Linux; the CEF code is
//! Windows in phase 1, plus the macOS test build (RB-090-15) and Linux (RB-097-05).

pub mod about;
pub mod activity_bar;
pub mod analyzer;
pub mod api_client;
pub mod app_theme;
pub mod basic_auth;
pub mod bookmarks;
pub mod breakpoint_keys;
pub mod bug_report;
pub mod bundle;
pub mod capture;
pub mod cdp;
pub mod cdp_proxy;
#[cfg(any(windows, target_os = "macos", target_os = "linux"))]
mod cef_app;
pub mod cell_info;
pub mod checks;
pub mod cli;
pub mod commands;
pub mod compare;
pub mod context_menu;
pub mod control;
pub mod cookies;
pub mod desktop_linux;
pub mod devtools;
pub mod emulation;
pub mod engine_compare;
pub mod engines;
pub mod extensions;
pub mod features;
pub mod first_run;
pub mod fit;
pub mod flows;
pub mod force_state;
pub mod form_sync;
pub mod host;
pub mod instance;
pub mod ipc;
pub mod launch;
pub mod layout;
pub mod layout_plan;
pub mod load_test;
pub mod local_time;
pub mod mcp;
pub mod media_codec;
pub mod nav_sync;
pub mod network_rules;
pub mod open_link;
pub mod page_window;
pub mod page_zoom;
pub mod panel_dock;
pub mod perf;
pub mod plugins;
pub mod pop_out;
pub mod portal;
pub mod presets;
pub mod project;
pub mod project_reload;
pub mod recorder;
pub mod reload;
pub mod replay;
pub mod report_view;
pub mod restart;
pub mod restore_last;
pub mod run_scope;
pub mod sandbox_linux;
pub mod scroll_sync;
pub mod secret;
pub mod session_state;
pub mod sessions;
pub mod settings_window;
pub mod shell;
pub mod sidecar;
pub mod sitemap;
pub mod start_failure;
pub mod status;
pub mod sync_agent;
pub mod sync_hub;
pub mod text_scale;
pub mod time_control;
pub mod toolbar_rows;
pub mod tools;
pub mod trust;
pub mod updates;
pub mod versioned_dir;
pub mod video;
pub mod view_footer;
pub mod visual;
pub mod vitals;
pub mod watch;
pub mod window_icon;

#[cfg(windows)]
pub use cef_app::RunWinMain;

/// Runs the app without the sandbox; returns the process exit code. On macOS the app of the
/// bundle (RB-090-15); on Linux the app itself, with the sandbox (RB-097-05).
#[cfg(any(windows, target_os = "macos", target_os = "linux"))]
pub fn run_plain() -> i32 {
    cef_app::run_plain()
}

/// Runs one of CEF's subprocesses in a helper app of the macOS bundle (`rbrowser Helper
/// (GPU).app` and the others, RB-090-15); returns its exit code.
#[cfg(target_os = "macos")]
pub fn run_helper() -> i32 {
    cef_app::run_helper()
}

/// The helper apps are macOS only: elsewhere CEF starts its subprocesses from the app itself.
#[cfg(not(target_os = "macos"))]
pub fn run_helper() -> i32 {
    eprintln!("rbrowser-helper runs inside the macOS app bundle only");
    1
}

/// Runs the app without the sandbox; returns the process exit code. The browser runs on
/// Windows, macOS and Linux: elsewhere only `--help` and `--version` work.
#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn run_plain() -> i32 {
    use clap::Parser;
    match cli::Cli::try_parse() {
        Ok(cli::Cli {
            command: Some(cli::Command::Diagnostics(args)),
            project,
            ..
        }) => control::diagnostics_cli(&args, project.as_deref()),
        // `init` and the hand-over need no browser (RB-010-11).
        Ok(cli) => match cli::before_ui(&cli, &control::settings::load_user().settings) {
            Err(code) => code,
            Ok(_) => {
                eprintln!(
                    "rBrowser {}: the browser runs on Windows, macOS and Linux only",
                    cli::VERSION
                );
                1
            }
        },
        Err(error) => {
            let _ = error.print();
            error.exit_code()
        }
    }
}
