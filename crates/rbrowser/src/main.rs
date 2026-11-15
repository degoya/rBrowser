//! `rbrowser-nosandbox.exe`: rBrowser without the Chromium sandbox, for debugging. The shipped
//! `rbrowser.exe` is CEF's `bootstrap.exe`, which runs the same app from `rbrowser.dll` with
//! the sandbox (ADR 0001, Decision 6). The logging init (RB-010-10) is in the shared entry,
//! `cef_app::run`, so both executables get it.

fn main() {
    std::process::exit(rbrowser::run_plain());
}
