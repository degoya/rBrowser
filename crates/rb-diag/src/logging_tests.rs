use std::path::Path;

use super::*;

fn config(dir: &Path, level: LogLevel, max_bytes: u64) -> LogConfig {
    LogConfig {
        dir: dir.to_path_buf(),
        level,
        rotation: RotationPolicy {
            max_bytes,
            max_files: 3,
        },
    }
}

fn log_of(dir: &Path) -> String {
    std::fs::read_to_string(dir.join(LOG_FILE_NAME)).expect("log file")
}

#[test]
fn building_the_logger_creates_rbrowser_log() {
    let dir = tempfile::tempdir().expect("temp dir");
    let logs = dir.path().join("logs");
    let (_dispatch, _guard) = build(&config(&logs, LogLevel::Info, 1 << 20), None).expect("build");
    assert!(logs.join("rbrowser.log").is_file());
}

#[test]
fn a_line_lands_in_the_file_with_its_target() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (dispatch, guard) =
        build(&config(dir.path(), LogLevel::Info, 1 << 20), None).expect("build");
    tracing::dispatcher::with_default(&dispatch, || {
        tracing::info!(target: target::CDP, "proxy listening on 9222");
        tracing::debug!(target: target::CORE, "hidden at info");
    });
    drop(guard);
    let log = log_of(dir.path());
    assert!(log.contains(" cdp: proxy listening on 9222"), "{log}");
    assert!(!log.contains("hidden at info"), "{log}");
}

#[test]
fn a_token_in_a_line_is_masked() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (dispatch, guard) =
        build(&config(dir.path(), LogLevel::Info, 1 << 20), None).expect("build");
    crate::redact::register_secret("cdp-proxy-secret-99");
    tracing::dispatcher::with_default(&dispatch, || {
        tracing::info!(target: target::MCP, token = "abc123secret", "mcp server up");
        tracing::info!(target: target::CDP, "endpoint ws://127.0.0.1:9222/cdp-proxy-secret-99");
        tracing::info!(target: target::CORE, "Authorization: Bearer zzz.yyy");
    });
    drop(guard);
    let log = log_of(dir.path());
    assert!(
        log.contains("token=\"***\"") || log.contains("token=***"),
        "{log}"
    );
    assert!(log.contains("ws://127.0.0.1:9222/***"), "{log}");
    for secret in ["abc123secret", "cdp-proxy-secret-99", "zzz.yyy"] {
        assert!(!log.contains(secret), "{secret} leaked: {log}");
    }
}

#[test]
fn the_log_rotates_past_the_size_limit() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (dispatch, guard) = build(&config(dir.path(), LogLevel::Info, 300), None).expect("build");
    tracing::dispatcher::with_default(&dispatch, || {
        for index in 0..20 {
            tracing::info!(target: target::SYNC, "scroll event number {index}");
        }
    });
    drop(guard);
    assert!(dir.path().join("rbrowser.1.log").is_file());
    assert!(dir.path().join("rbrowser.2.log").is_file());
    assert!(!dir.path().join("rbrowser.3.log").exists());
    assert!(log_of(dir.path()).contains("scroll event number 19"));
}

#[test]
fn the_level_changes_at_runtime() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (dispatch, guard) =
        build(&config(dir.path(), LogLevel::Warn, 1 << 20), None).expect("build");
    let handle = guard.handle();
    tracing::dispatcher::with_default(&dispatch, || {
        tracing::info!("before the change");
        handle.set_level(LogLevel::Debug).expect("set level");
        tracing::debug!("after the change");
    });
    drop(guard);
    let log = log_of(dir.path());
    assert!(!log.contains("before the change"), "{log}");
    assert!(log.contains("after the change"), "{log}");
}

#[test]
fn rust_log_wins_over_the_setting() {
    assert_eq!(
        filter_directives(LogLevel::Info, Some("cdp=trace")),
        "cdp=trace"
    );
    assert_eq!(
        filter_directives(LogLevel::Info, Some("rmcp=debug")),
        "rmcp=debug",
        "set explicitly, RUST_LOG is not capped"
    );
    assert_eq!(filter_directives(LogLevel::Warn, Some("  ")), "warn");
    assert_eq!(filter_directives(LogLevel::Debug, None), "debug,rmcp=warn");
    assert_eq!(filter_directives(LogLevel::Error, None), "error");
}

/// rmcp logs every tool call and answer whole at debug (batch-3 security review, finding 2):
/// page content, evaluate results, settings values. Without `RUST_LOG` it is capped at warn,
/// whatever `logs.level` says.
#[test]
fn a_debug_logger_writes_no_mcp_payload() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (dispatch, guard) =
        build(&config(dir.path(), LogLevel::Trace, 1 << 20), None).expect("build");
    let handle = guard.handle();
    tracing::dispatcher::with_default(&dispatch, || {
        tracing::debug!(target: "rmcp::service", request = "payload-at-start", "received request");
        handle.set_level(LogLevel::Debug).expect("set level");
        tracing::debug!(target: "rmcp::service", result = "payload-after-change", "response");
        tracing::warn!(target: "rmcp::service", "rmcp warning kept");
        tracing::debug!(target: target::MCP, "our own debug line");
    });
    drop(guard);
    let log = log_of(dir.path());
    assert!(!log.contains("payload-at-start"), "{log}");
    assert!(!log.contains("payload-after-change"), "{log}");
    assert!(log.contains("rmcp warning kept"), "{log}");
    assert!(log.contains("our own debug line"), "{log}");
}

#[test]
fn an_invalid_rust_log_falls_back_to_the_setting() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (dispatch, guard) =
        build(&config(dir.path(), LogLevel::Info, 1 << 20), Some("=[{bad")).expect("build");
    tracing::dispatcher::with_default(&dispatch, || tracing::info!("still logged"));
    drop(guard);
    assert!(log_of(dir.path()).contains("still logged"));
}

#[test]
fn a_panic_lands_in_the_log_with_its_backtrace() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (dispatch, guard) =
        build(&config(dir.path(), LogLevel::Info, 1 << 20), None).expect("build");
    tracing::dispatcher::with_default(&dispatch, || {
        crate::panic::install_hook();
        let caught = std::panic::catch_unwind(|| panic!("boom 4711"));
        assert!(caught.is_err());
    });
    drop(guard);
    let log = log_of(dir.path());
    assert!(log.contains(" panic: panicked at "), "{log}");
    assert!(log.contains("boom 4711"), "{log}");
    assert!(log.contains("backtrace:"), "{log}");
}

/// RB-094-28: `logs.maxFileSizeMb` and `logs.maxFiles` apply to the running logger.
#[test]
fn the_rotation_changes_at_runtime() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (dispatch, guard) =
        build(&config(dir.path(), LogLevel::Info, 1 << 20), None).expect("build");
    let handle = guard.handle();
    handle.set_rotation(RotationPolicy {
        max_bytes: 300,
        max_files: 2,
    });
    tracing::dispatcher::with_default(&dispatch, || {
        for index in 0..20 {
            tracing::info!(target: target::SYNC, "scroll event number {index}");
        }
    });
    drop(guard);
    assert!(dir.path().join("rbrowser.1.log").is_file());
    assert!(!dir.path().join("rbrowser.2.log").exists());
    assert!(log_of(dir.path()).contains("scroll event number 19"));
}
