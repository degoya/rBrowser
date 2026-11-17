//! The CEF application (Windows; the macOS test build, RB-090-15): the entry points, the launch
//! before CEF starts, and the window and views (ADR 0001). Two entries share [`run`] on
//! Windows: `RunWinMain`, which CEF's `bootstrap.exe` (shipped as `rbrowser.exe`) calls in
//! `rbrowser.dll` with the sandbox, and [`run_plain`] for `rbrowser-nosandbox.exe`. On macOS
//! [`run_plain`] is the bundle's `rbrowser` and [`run_helper`] its helper apps' executable.
//!
//! The Win32 modules have a macOS twin of the same name under `macos/` (the `path` attributes
//! below): what macOS does not have yet is a no-op there that says so, so the grid, the views
//! and the shell run on both.

#[cfg_attr(target_os = "macos", path = "macos/activate.rs")]
mod activate;
mod active_label;
mod app;
#[cfg(target_os = "macos")]
#[path = "macos/application.rs"]
mod application;
mod backend;
#[cfg_attr(target_os = "macos", path = "macos/board_pan.rs")]
mod board_pan;
mod canvas_board;
mod cell_footers;
mod cell_info;
mod cell_labels;
mod console;
mod context_menu;
mod devtools;
mod dialogs;
mod embed;
#[cfg_attr(target_os = "macos", path = "macos/embed_windows.rs")]
mod embed_windows;
mod engine_badge;
mod frame;
mod grid;
mod handle;
mod icon;
mod keys;
mod label_strip;
#[cfg_attr(target_os = "macos", path = "macos/message_box.rs")]
mod message_box;
mod native_menu;
mod page_windows;
mod page_zoom;
mod panel_dock;
#[cfg_attr(target_os = "macos", path = "macos/pipe.rs")]
mod pipe;
mod placed;
mod pop_out;
mod popup;
mod queries;
mod render;
mod report_scheme;
mod report_window;
mod scheme;
mod settings_window;
mod splitter;
mod start;
mod status_bar;
#[cfg_attr(target_os = "macos", path = "macos/strip_clicks.rs")]
mod strip_clicks;
mod take_focus;
mod task;
mod teardown;
mod theme;
#[cfg_attr(target_os = "macos", path = "macos/theme_dwm.rs")]
mod theme_dwm;
mod ui_watchdog;
mod video_scheme;
mod view_focus;
mod view_menus;
mod views;
mod window;
mod zoom;
mod zoom_menu;

use std::path::{Path, PathBuf};
use std::time::Duration;

use cef::*;
use clap::Parser;

use crate::cli::{Cli, Command, Plan};
use crate::control::{CoreSetup, settings};
use crate::host::state::default_config;
use crate::{launch, project, shell};
use app::{RbApp, SWITCHES};
use start::{LAUNCH, Launch};

/// The exit code when the app is started from the WSL share.
const EXIT_WSL_SHARE: i32 = 2;

/// Entry of the plain executable (no sandbox).
#[cfg(windows)]
pub fn run_plain() -> i32 {
    // Must be the very first CEF call, or libcef aborts with exit code 3 (ADR 0001, Decision 1).
    let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);
    let args = args::Args::new();
    let Some(command_line) = args.as_cmd_line() else {
        return 1;
    };
    run(
        args.as_main_args(),
        &command_line,
        std::ptr::null_mut(),
        false,
    )
}

/// Entry of the macOS app, `rBrowser.app/Contents/MacOS/rbrowser` (RB-090-15). The CEF
/// framework is loaded from the bundle first - every CEF call before it would fail -, then the
/// CDP pipe takes its descriptors before anything else opens a file, and the shared
/// application becomes one Chromium's message pump can drive. No sandbox in the test build.
#[cfg(target_os = "macos")]
pub fn run_plain() -> i32 {
    let Some(loader) = load_framework(false) else {
        return 1;
    };
    let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);
    pipe::reserve();
    let Some(main_thread) = objc2::MainThreadMarker::new() else {
        eprintln!("rbrowser: the app must start on the main thread");
        return 1;
    };
    application::install(main_thread);
    let args = args::Args::new();
    let Some(command_line) = args.as_cmd_line() else {
        return 1;
    };
    let code = run(
        args.as_main_args(),
        &command_line,
        std::ptr::null_mut(),
        false,
    );
    drop(loader);
    code
}

/// Entry of the helper apps of the macOS bundle (RB-090-15): the framework three folders up,
/// then the subprocess CEF started the helper for, with the app's own application object.
#[cfg(target_os = "macos")]
pub fn run_helper() -> i32 {
    let Some(_loader) = load_framework(true) else {
        return 1;
    };
    let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);
    let args = args::Args::new();
    let mut app = RbApp::new();
    execute_process(
        Some(args.as_main_args()),
        Some(&mut app),
        std::ptr::null_mut(),
    )
}

/// Loads the CEF framework of the bundle the running executable belongs to (`helper`: from a
/// helper app). `None`, said on stderr, when the executable lies in no bundle.
#[cfg(target_os = "macos")]
fn load_framework(helper: bool) -> Option<library_loader::LibraryLoader> {
    let exe = std::env::current_exe().ok()?;
    // The loader panics on a missing framework: checked here first.
    if !crate::bundle::framework(&exe, helper).is_some_and(|path| path.exists()) {
        eprintln!(
            "rbrowser: no CEF framework beside {}; start rBrowser.app, not the bare executable",
            exe.display()
        );
        return None;
    }
    let loader = library_loader::LibraryLoader::new(&exe, helper);
    if !loader.load() {
        eprintln!("rbrowser: the CEF framework could not be loaded");
        return None;
    }
    Some(loader)
}

/// Entry of the sandboxed layout: CEF's `bootstrap.exe`, renamed `rbrowser.exe`, loads
/// `rbrowser.dll` and calls this with the sandbox information.
///
/// # Safety
///
/// Called by `bootstrap.exe` only, with the arguments of its `wWinMain` and a valid
/// `sandbox_info` from the CEF sandbox.
#[cfg(windows)]
#[allow(
    unsafe_code,
    reason = "the exported entry point bootstrap.exe calls by name"
)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn RunWinMain(
    instance: sys::HINSTANCE,
    _command_line: *const u8,
    _command_show: i32,
    sandbox_info: *mut u8,
) -> i32 {
    let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);
    let args = args::Args::from(MainArgs { instance });
    let Some(command_line) = args.as_cmd_line() else {
        return 1;
    };
    run(args.as_main_args(), &command_line, sandbox_info, true)
}

/// Every process starts here; CEF's child processes (`--type=...`) leave at once.
fn run(
    main_args: &MainArgs,
    command_line: &CommandLine,
    sandbox_info: *mut u8,
    sandboxed: bool,
) -> i32 {
    let mut app = RbApp::new();
    let child = command_line.has_switch(Some(&CefString::from("type"))) != 0;
    if child {
        return execute_process(Some(main_args), Some(&mut app), sandbox_info);
    }
    // The terminal of rbrowser.com or the calling shell, until the window opens (RB-010-11).
    crate::cli::console::attach();
    // An older Finder adds `-psn_...` to an app it starts (macOS); no parser knows it.
    let cli = match Cli::try_parse_from(crate::bundle::without_process_serial(std::env::args_os()))
    {
        Ok(cli) => cli,
        Err(error) => {
            let _ = error.print();
            return error.exit_code();
        }
    };

    // The log first (RB-010-10): until then `tracing` events go nowhere.
    let user = settings::load_user();
    // Inside the local folder, so the portable mode moves the logs too (RB-010-15).
    let diag = rb_config::local_dir().map(|dir| rb_diag::DiagPaths::under(&dir));
    let log_settings = settings::log_settings(&user.settings);
    let _log = diag.as_ref().and_then(|paths| {
        rb_diag::init(&rb_diag::LogConfig::new(paths, &log_settings))
            .inspect_err(|error| eprintln!("rBrowser: the log cannot be written: {error}"))
            .ok()
    });
    // `logs.level` changes at runtime from the settings window (RB-010-08).
    if let Some(guard) = &_log {
        crate::settings_window::set_log_handle(guard.handle());
    }
    user.log();
    if let Some(Command::Diagnostics(args)) = &cli.command {
        return crate::control::diagnostics_cli(args, cli.project.as_deref());
    }
    // `init`, `--reload` and the hand-over to a running instance (RB-010-11); the plan goes on
    // into the launch, over the user settings read above.
    let plan = match crate::cli::before_ui(&cli, &user.settings) {
        Ok(plan) => plan,
        Err(code) => return code,
    };

    let exe = std::env::current_exe().unwrap_or_default();
    if project::is_wsl_share(&exe) {
        tracing::error!(exe = %exe.display(), "rBrowser does not run from the WSL share; copy it to a local folder");
        eprintln!("rbrowser: rBrowser does not run from the WSL share; copy it to a local folder");
        return EXIT_WSL_SHARE;
    }
    // Read before CEF starts: the global context and every session's are made with it.
    crate::sessions::set_persist_session_cookies(user.settings.sessions.persist_session_cookies);
    // The app opens its window: the terminal is free again (rbrowser.com sees its pipes end).
    crate::cli::console::release();
    let setup = CoreSetup {
        user_load_failed: user.error.is_some(),
        user: user.settings,
        user_path: user.path,
        diag,
        cef_build: Some(crate::control::cef_age::embedded_build()),
    };
    let Some(launch) = prepare(&cli, plan.as_ref(), &exe, setup) else {
        return 1;
    };
    let setting = launch.0.setup.user.engines.chromium.sandbox;
    if sandboxed && !setting {
        tracing::warn!("the Chromium sandbox is off (setting engines.chromium.sandbox)");
    }
    let sandbox = sandboxed && !cli.no_sandbox && setting;
    tracing::info!(sandbox, profile = %launch.1.display(), "starting CEF");
    let cef_log = launch.0.setup.diag.as_ref().map_or_else(
        || launch.1.join(rb_diag::paths::CEF_LOG_FILE_NAME),
        rb_diag::DiagPaths::cef_log_file,
    );
    let mut switches = launch::browser_switches(
        launch.0.pipe.map(|ends| ends.switch_value()).as_deref(),
        &launch.0.config.network,
    );
    // The person's unpacked extensions and the installed Web Store ones, checked now and
    // loaded for the instance (RB-080-06, RB-080-23).
    let extensions =
        crate::extensions::start::current_plan(&launch.0.setup.user.engines.chromium.extensions);
    switches.extend(crate::extensions::start::switches(&extensions));
    crate::extensions::start::record(extensions);
    if SWITCHES.set(switches).is_err() {
        return 1;
    }
    let ret = execute_process(Some(main_args), Some(&mut app), sandbox_info);
    if ret >= 0 {
        return ret;
    }
    let (launch, profile, runtime) = (launch.0, launch.1, launch.2);
    if LAUNCH.set(launch).is_err() {
        return 1;
    }
    // Named sessions keep their data under the profile too (RB-020-07).
    crate::sessions::set_profile(profile.clone());
    let settings = Settings {
        no_sandbox: i32::from(!sandbox),
        root_cache_path: cef_path(&profile),
        cache_path: cef_path(&profile.join(crate::sessions::DEFAULT_CACHE_DIR)),
        // Session cookies end with the app unless the user keeps them (RB-020-07).
        persist_session_cookies: i32::from(crate::sessions::persist_session_cookies()),
        log_file: cef_path(&cef_log),
        log_severity: log_severity(log_settings.level),
        ..Default::default()
    };
    if initialize(
        Some(main_args),
        Some(&settings),
        Some(&mut app),
        sandbox_info,
    ) != 1
    {
        return start_failed(&profile);
    }
    run_message_loop();
    start::stop();
    crate::mcp::shutdown();
    crate::cdp_proxy::shutdown();
    crate::portal::app::shutdown();
    // A video export or conversion stops and removes its half-written file (RB-090-52).
    crate::video::convert_queue::shutdown();
    // The engines first: WebKit closes its windows over the sidecar, which must still run.
    // Both before `CefShutdown`, which comes last (RB-080-91, `teardown.rs`).
    crate::engines::shutdown(&runtime);
    crate::sidecar::shutdown(&runtime);
    shutdown();
    runtime.shutdown_timeout(Duration::from_secs(1));
    0
}

/// CEF did not start: the reason (its exit code) in the log and in a message box, so a start
/// without a window never ends silently (an elevated start, a profile still in use).
fn start_failed(profile: &Path) -> i32 {
    let failure = crate::start_failure::StartFailure::new(get_exit_code());
    let logs = LAUNCH
        .get()
        .and_then(|launch| launch.setup.diag.as_ref())
        .map(|diag| diag.logs.clone());
    let message = failure.message(profile, logs.as_deref());
    tracing::error!(
        code = failure.exit_code(),
        reason = failure.reason_key(),
        "CEF did not initialise: {message}"
    );
    message_box::error(&crate::start_failure::title(), &message);
    failure.exit_code()
}

/// Everything decided before CEF starts: the project and its config (from `plan`, which
/// `cli::before_ui` checked), the profile, the pipe, the runtime.
fn prepare(
    cli: &Cli,
    plan: Option<&Plan>,
    exe: &Path,
    setup: CoreSetup,
) -> Option<(Launch, PathBuf, tokio::runtime::Runtime)> {
    let root = plan.and_then(|plan| plan.root.clone());
    let (mut config, trusted) = match &root {
        Some(root) => project::load_trusted(root).unwrap_or_else(|error| {
            tracing::error!(%error, root = %root.display(), "the project config cannot be read; showing the default grid");
            (default_config(), false)
        }),
        None => (default_config(), false),
    };
    if let Some(url) = plan.and_then(|plan| plan.urls.first()) {
        config.start_url = Some(url.clone());
    }
    // `%LOCALAPPDATA%\rBrowser`, or beside the executable in the portable mode (RB-010-15).
    let base = rb_config::local_dir()
        .unwrap_or_else(|| std::env::temp_dir().join(rb_config::LOCAL_APP_DIR));
    let profile = project::profile_dir(&base, root.as_deref());
    if let Err(error) = std::fs::create_dir_all(&profile) {
        tracing::error!(%error, profile = %profile.display(), "the browser profile cannot be created");
        return None;
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("rb-core")
        .enable_all()
        .build()
        .inspect_err(|error| tracing::error!(%error, "the core runtime cannot start"))
        .ok()?;
    // Beside the executable; in the macOS bundle in `Contents/Resources` (RB-090-15).
    let ui_dir = crate::bundle::resource_dir(exe).map_or_else(
        || PathBuf::from(shell::UI_DIR),
        |dir| dir.join(shell::UI_DIR),
    );
    let launch = Launch {
        background: plan.map_or(cli.background, |plan| plan.background),
        title: launch::window_title(root.as_deref()),
        config,
        root,
        trusted,
        runtime: runtime.handle().clone(),
        pipe: pipe::create(),
        ui_dir,
        setup,
    };
    Some((launch, profile, runtime))
}

/// CEF's own log severity for our level: one step quieter (rb-diag).
fn log_severity(level: rb_diag::LogLevel) -> LogSeverity {
    match rb_diag::CefLogSeverity::from(level) {
        rb_diag::CefLogSeverity::Verbose => LogSeverity::VERBOSE,
        rb_diag::CefLogSeverity::Info => LogSeverity::INFO,
        rb_diag::CefLogSeverity::Warning => LogSeverity::WARNING,
        rb_diag::CefLogSeverity::Error => LogSeverity::ERROR,
    }
}

fn cef_path(path: &Path) -> CefString {
    CefString::from(path.to_string_lossy().as_ref())
}
