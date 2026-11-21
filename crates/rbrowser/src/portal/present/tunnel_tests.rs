use std::ffi::OsString;
#[cfg(unix)]
use std::time::Duration;

use super::*;

/// The box cloudflared draws around a Quick Tunnel's address (2026.11), and the line after it.
const BANNER: &[&str] = &[
    "2026-11-21T10:00:00Z INF Thank you for trying Cloudflare Tunnel. Doing so, without a \
     Cloudflare account, is a quick way to experiment and try it out.",
    "2026-11-21T10:00:00Z INF Requesting new quick Tunnel on trycloudflare.com...",
    "2026-11-21T10:00:01Z INF +--------------------------------------------------------------+",
    "2026-11-21T10:00:01Z INF |  Your quick Tunnel has been created! Visit it at (it may take |",
    "2026-11-21T10:00:01Z INF |  https://quick-brown-fox-jumps.trycloudflare.com              |",
    "2026-11-21T10:00:01Z INF +--------------------------------------------------------------+",
    "2026-11-21T10:00:02Z INF Registered tunnel connection connIndex=0 \
     connection=0a1b event=0 ip=198.41.200.13 location=fra08 protocol=quic",
];

#[test]
fn the_address_is_read_from_the_banner() {
    let found: Vec<String> = BANNER.iter().filter_map(|line| public_url(line)).collect();
    assert_eq!(found, ["https://quick-brown-fox-jumps.trycloudflare.com"]);
}

#[test]
fn only_a_quick_tunnel_name_is_an_address() {
    for line in [
        "ERR failed to request quick Tunnel: Post \"https://api.trycloudflare.com/tunnel\"",
        "see https://developers.cloudflare.com/cloudflare-one/",
        "https://evil.example.com/?x=.trycloudflare.com",
        "https://a.b.trycloudflare.com",
        "https://trycloudflare.com",
        "https://nodash.trycloudflare.com",
        "http://plain-http.trycloudflare.com",
        "https://-lead.trycloudflare.com",
    ] {
        assert_eq!(public_url(line), None, "{line}");
    }
    assert_eq!(
        public_url("x https://Mixed-Case.TryCloudflare.com/ y").as_deref(),
        Some("https://mixed-case.trycloudflare.com")
    );
}

#[test]
fn the_progress_names_the_address_then_the_connection_once_each() {
    let mut progress = Progress::default();
    let events: Vec<TunnelEvent> = BANNER
        .iter()
        .chain(BANNER.iter())
        .filter_map(|line| progress.feed(line))
        .collect();
    assert_eq!(
        events,
        [
            TunnelEvent::Url("https://quick-brown-fox-jumps.trycloudflare.com".to_owned()),
            TunnelEvent::Connected,
        ]
    );
}

#[test]
fn a_connection_before_the_address_is_none() {
    let mut progress = Progress::default();
    assert_eq!(progress.feed(BANNER[6]), None);
}

#[test]
fn the_arguments_ask_for_a_quick_tunnel_to_the_loopback_port_without_updates() {
    let config = std::path::Path::new("/tools/cloudflared/rbrowser-cloudflared.yml");
    assert_eq!(
        arguments(41234, config),
        [
            "tunnel",
            "--no-autoupdate",
            "--config",
            "/tools/cloudflared/rbrowser-cloudflared.yml",
            "--url",
            "http://127.0.0.1:41234"
        ]
    );
}

/// cloudflared reads `~/.cloudflared/config.yml` (ingress rules, a named tunnel, an origin
/// certificate) unless `--config` names another file: rBrowser's own, with nothing in it.
#[test]
fn the_blank_configuration_goes_to_the_first_folder_that_takes_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let blocked = dir.path().join("a-file");
    std::fs::write(&blocked, b"").expect("written");
    let tools = dir.path().join("tools").join("cloudflared");
    let config = blank_config([blocked.join("below"), tools.clone()]).expect("written");
    assert_eq!(config, tools.join(BLANK_CONFIG));
    let text = std::fs::read_to_string(&config).expect("reads");
    assert!(
        text.lines()
            .all(|line| line.starts_with('#') || line == "{}"),
        "{text}"
    );
    // Written again on every start: whatever was put there is replaced.
    std::fs::write(&config, b"ingress: []\n").expect("written");
    blank_config([tools.clone()]).expect("written");
    assert_eq!(std::fs::read_to_string(&config).expect("reads"), text);
    assert!(blank_config([blocked.join("below")]).is_err());
}

/// cloudflared reads `TUNNEL_*` variables as flags: a `TUNNEL_TOKEN` of the person's own would
/// run their named tunnel instead of a quick one.
#[test]
fn the_tunnel_variables_of_the_environment_are_removed() {
    let keys = [
        OsString::from("PATH"),
        OsString::from("TUNNEL_TOKEN"),
        OsString::from("TUNNEL_ORIGIN_CERT"),
        OsString::from("tunnel_lower"),
        OsString::from("NO_AUTOUPDATE"),
        OsString::from("TUNNELED"),
    ];
    let removed = scrubbed(keys);
    assert_eq!(
        removed,
        [
            OsString::from("TUNNEL_TOKEN"),
            OsString::from("TUNNEL_ORIGIN_CERT"),
            OsString::from("tunnel_lower"),
            OsString::from("NO_AUTOUPDATE"),
        ]
    );
}

#[cfg(unix)]
mod process {
    //! A fake cloudflared: a shell script that prints the banner and waits, or exits.

    use std::os::unix::fs::PermissionsExt as _;
    use std::path::{Path, PathBuf};

    use super::*;

    fn script(dir: &Path, body: &str) -> PathBuf {
        let path = dir.join("cloudflared");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("script");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).expect("mode");
        path
    }

    async fn next(events: &mut mpsc::UnboundedReceiver<TunnelEvent>) -> Option<TunnelEvent> {
        tokio::time::timeout(Duration::from_secs(10), events.recv())
            .await
            .ok()
            .flatten()
    }

    fn alive(pid: u32) -> bool {
        Path::new(&format!("/proc/{pid}")).exists()
    }

    #[tokio::test]
    async fn the_tunnel_reports_its_address_and_stop_leaves_no_process() {
        let dir = tempfile::tempdir().expect("tempdir");
        let program = script(
            dir.path(),
            "echo 'INF |  https://quick-brown-fox.trycloudflare.com  |' >&2\n\
             echo 'INF Registered tunnel connection connIndex=0' >&2\n\
             exec sleep 600",
        );
        let (tunnel, mut events) = Tunnel::spawn(&program, 4000).expect("starts");
        let pid = tunnel.pid().expect("a pid");
        assert_eq!(
            next(&mut events).await,
            Some(TunnelEvent::Url(
                "https://quick-brown-fox.trycloudflare.com".to_owned()
            ))
        );
        assert_eq!(next(&mut events).await, Some(TunnelEvent::Connected));
        assert!(alive(pid));
        tunnel.stop().await;
        assert!(!alive(pid), "the stop waited for the process to end");
        assert_eq!(
            next(&mut events).await,
            Some(TunnelEvent::Exited { stopped: true })
        );
    }

    #[tokio::test]
    async fn the_program_gets_the_quick_tunnel_arguments() {
        let dir = tempfile::tempdir().expect("tempdir");
        let seen = dir.path().join("args");
        let program = script(
            dir.path(),
            &format!("echo \"$@\" > '{}'\nexec sleep 600", seen.display()),
        );
        let (tunnel, _events) = Tunnel::spawn(&program, 4321).expect("starts");
        let mut args = String::new();
        for _ in 0..100 {
            args = std::fs::read_to_string(&seen).unwrap_or_default();
            if !args.is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        tunnel.stop().await;
        let config = dir.path().join(BLANK_CONFIG);
        assert_eq!(
            args.trim(),
            format!(
                "tunnel --no-autoupdate --config {} --url http://127.0.0.1:4321",
                config.display()
            )
        );
        assert!(
            config.is_file(),
            "the blank configuration beside the program"
        );
    }

    /// A crash ends the tunnel on its own: the event says it was not stopped.
    #[tokio::test]
    async fn a_crash_is_reported() {
        let dir = tempfile::tempdir().expect("tempdir");
        let program = script(
            dir.path(),
            "echo 'INF |  https://quick-brown-fox.trycloudflare.com  |' >&2\nexit 3",
        );
        let (_tunnel, mut events) = Tunnel::spawn(&program, 4000).expect("starts");
        let mut saw = Vec::new();
        while let Some(event) = next(&mut events).await {
            let exited = matches!(event, TunnelEvent::Exited { .. });
            saw.push(event);
            if exited {
                break;
            }
        }
        assert_eq!(saw.last(), Some(&TunnelEvent::Exited { stopped: false }));
    }

    /// Dropping the handle (rBrowser ends) kills the program too.
    #[tokio::test]
    async fn dropping_the_tunnel_ends_the_program() {
        let dir = tempfile::tempdir().expect("tempdir");
        let program = script(dir.path(), "exec sleep 600");
        let (tunnel, mut events) = Tunnel::spawn(&program, 4000).expect("starts");
        let pid = tunnel.pid().expect("a pid");
        drop(tunnel);
        assert_eq!(
            next(&mut events).await,
            Some(TunnelEvent::Exited { stopped: true })
        );
        assert!(!alive(pid));
    }
}
