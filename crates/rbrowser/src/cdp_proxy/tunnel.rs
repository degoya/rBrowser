//! The way in from WSL (RB-030-06, RB-030-04). In `mirrored` mode `127.0.0.1` inside the
//! distribution is Windows' own and the proxy's URL works as it is. In NAT mode (the reference
//! machine's) it is not: the app starts `rbrowser-wsl cdp-proxy` in the distribution through
//! `wsl.exe` - once per distribution, when `cdp_endpoint` first asks - and the helper's port
//! inside WSL becomes the URL from there. `rbrowser cdp-tunnel` does the same in the
//! foreground, for a proxy on a fixed port. The tunnel only carries bytes to the proxy, which
//! checks the token as for every client (threat model T13).

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use rb_config::UserSettings;
use rb_wsl::cdp_proxy::app::{Tunnel, start};
use rb_wsl::path::Translator;
use rb_wsl::settings::WslSettings;

use super::endpoint::WslRoute;
use crate::cdp::lock;

/// How long each `wsl.exe` query may take.
const QUERY_LIMIT: Duration = Duration::from_secs(5);
/// How long the helper may take to answer (a stopped distribution boots within it).
const START_LIMIT: Duration = Duration::from_secs(30);

/// The running tunnels, by distribution.
static TUNNELS: LazyLock<Mutex<Vec<(String, Tunnel)>>> = LazyLock::new(|| Mutex::new(Vec::new()));

/// Stops every tunnel (the proxy stopped, the app quits): dropping one kills its helper.
pub fn stop_all() {
    lock(&TUNNELS).clear();
}

/// The `wsl.*` settings of `user`.
fn wsl_settings(user: &UserSettings) -> WslSettings {
    WslSettings::from_value(serde_json::to_value(&user.wsl).ok().as_ref())
}

/// How a client in WSL reaches the proxy on `proxy_port`, starting the tunnel when the
/// distribution runs in NAT mode and none runs yet.
pub async fn route(user: &UserSettings, proxy_port: u16) -> WslRoute {
    let settings = wsl_settings(user);
    let exe = rb_wsl::distro::wsl_exe();
    let distros = match rb_wsl::query::list_distros(&exe, QUERY_LIMIT).await {
        Ok(distros) => distros,
        Err(error) => return WslRoute::Unavailable(error.to_string()),
    };
    let distro = match rb_wsl::distro::choose(&distros, settings.distro()) {
        Ok(distro) => distro.name.clone(),
        Err(error) => return WslRoute::Unavailable(error.to_string()),
    };
    let profile = std::env::var("USERPROFILE").ok();
    let wslconfig = rb_wsl::netmode::wslconfig_path(profile.as_deref());
    let detection = rb_wsl::query::detect_mode(
        &exe,
        &distro,
        settings.networking_mode_override.mode(),
        wslconfig.as_deref(),
        &crate::mcp::wsl::MODES,
        QUERY_LIMIT,
    )
    .await;
    let mode = detection.mode;
    if mode.reaches_windows_localhost() {
        return WslRoute::Direct { distro, mode };
    }
    if let Some(port) = running(&distro) {
        return WslRoute::Tunnel { distro, mode, port };
    }
    match open(&exe, &distro, settings.helper_path(), proxy_port).await {
        Ok(tunnel) => {
            let port = tunnel.port();
            let mut tunnels = lock(&TUNNELS);
            // Two callers may have raced; the first tunnel stays, the second one ends here.
            match tunnels
                .iter()
                .find(|(name, tunnel)| *name == distro && tunnel.is_running())
            {
                Some((_, kept)) => WslRoute::Tunnel {
                    port: kept.port(),
                    distro,
                    mode,
                },
                None => {
                    tunnels.retain(|(name, _)| *name != distro);
                    tunnels.push((distro.clone(), tunnel));
                    WslRoute::Tunnel { distro, mode, port }
                }
            }
        }
        Err(error) => WslRoute::Failed {
            distro,
            mode,
            error,
        },
    }
}

/// The port of the running tunnel into `distro`.
fn running(distro: &str) -> Option<u16> {
    lock(&TUNNELS)
        .iter()
        .find(|(name, tunnel)| name == distro && tunnel.is_running())
        .map(|(_, tunnel)| tunnel.port())
}

/// Starts `rbrowser-wsl cdp-proxy --port <proxy_port>` in `distro` through `wsl_exe` (the
/// packaged helper beside the executable, or `helper_path`) and tunnels to the proxy.
///
/// # Errors
///
/// Why the helper did not start or answer, in English.
pub async fn open(
    wsl_exe: &str,
    distro: &str,
    helper_path: Option<&str>,
    proxy_port: u16,
) -> Result<Tunnel, String> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.to_string_lossy().into_owned()))
        .unwrap_or_default();
    let translator = Translator::new(distro).map_err(|error| error.to_string())?;
    let helper = rb_wsl::helper::helper_linux_path(&translator, &exe_dir, helper_path)
        .map_err(|error| error.to_string())?;
    let subcommand = [
        "cdp-proxy".to_owned(),
        "--port".to_owned(),
        proxy_port.to_string(),
    ];
    let args = rb_wsl::helper::helper_args(distro, &helper, &subcommand)
        .map_err(|error| error.to_string())?;
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let command = rb_wsl::query::wsl_command(wsl_exe, &refs);
    let proxy = SocketAddr::from((Ipv4Addr::LOCALHOST, proxy_port));
    start(command, proxy, START_LIMIT)
        .await
        .map_err(|error| error.to_string())
}

/// `rbrowser cdp-tunnel`: the tunnel into `distro` (else `wsl.distro`, else WSL's default) for
/// the proxy on `port` (else `cdp.port`), in the foreground until the helper ends; the exit
/// code.
pub fn cli(port: Option<u16>, distro: Option<&str>, user: &UserSettings) -> i32 {
    use crate::cli::exit;
    let Some(proxy_port) = port.or(user.cdp.port) else {
        eprintln!(
            "rbrowser cdp-tunnel: pass --port <port> (the port cdp_endpoint names) or set a fixed cdp.port"
        );
        return exit::USAGE;
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("rbrowser cdp-tunnel: {error}");
            return exit::FAILED;
        }
    };
    let settings = wsl_settings(user);
    let code = runtime.block_on(async {
        let exe = rb_wsl::distro::wsl_exe();
        let wanted = distro.or(settings.distro());
        let name = match rb_wsl::query::list_distros(&exe, QUERY_LIMIT).await {
            Ok(distros) => match rb_wsl::distro::choose(&distros, wanted) {
                Ok(found) => found.name.clone(),
                Err(error) => {
                    eprintln!("rbrowser cdp-tunnel: {error}");
                    return exit::USAGE;
                }
            },
            Err(error) => {
                eprintln!("rbrowser cdp-tunnel: {error}");
                return exit::FAILED;
            }
        };
        match open(&exe, &name, settings.helper_path(), proxy_port).await {
            Ok(tunnel) => {
                println!(
                    "rbrowser cdp-tunnel: 127.0.0.1:{} in {name} leads to rBrowser's CDP proxy on 127.0.0.1:{proxy_port}",
                    tunnel.port()
                );
                match tunnel.wait().await {
                    Ok(_) => exit::OK,
                    Err(error) => {
                        eprintln!("rbrowser cdp-tunnel: {error}");
                        exit::FAILED
                    }
                }
            }
            Err(error) => {
                eprintln!("rbrowser cdp-tunnel: {error}");
                exit::FAILED
            }
        }
    });
    runtime.shutdown_background();
    code
}
