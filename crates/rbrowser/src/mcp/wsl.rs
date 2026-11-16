//! The answer of `wsl_info` (rb-wsl's [`rb_wsl::info::WslInfo`]): the distributions from
//! `wsl.exe`, the one in use (`wsl.distro`, else WSL's default), its networking mode and the
//! helper's state. The helper's supervisor is not wired into the app yet, so it reads `stopped`
//! until it is.

use std::sync::LazyLock;
use std::time::Duration;

use rb_config::UserSettings;
use rb_wsl::info::WslInfo;
use rb_wsl::netmode::ModeCache;
use rb_wsl::settings::WslSettings;
use rb_wsl::supervisor::HelperStatus;
use serde_json::Value;

/// How long each `wsl.exe` call may take.
const LIMIT: Duration = Duration::from_secs(5);

/// The detected networking modes, per distribution, for the app's life.
pub(crate) static MODES: LazyLock<ModeCache> = LazyLock::new(ModeCache::default);

/// The WSL state for `user`'s settings, as JSON.
pub async fn info(user: &UserSettings) -> Value {
    let settings = WslSettings::from_value(serde_json::to_value(&user.wsl).ok().as_ref());
    let exe = rb_wsl::distro::wsl_exe();
    let answer = match rb_wsl::query::list_distros(&exe, LIMIT).await {
        Err(error) => WslInfo::new(Vec::new(), None, None, HelperStatus::Stopped, Some(&error)),
        Ok(distros) => {
            match rb_wsl::distro::choose(&distros, settings.distro()).map(|d| d.name.clone()) {
                Err(error) => {
                    WslInfo::new(distros, None, None, HelperStatus::Stopped, Some(&error))
                }
                Ok(name) => {
                    let profile = std::env::var("USERPROFILE").ok();
                    let wslconfig = rb_wsl::netmode::wslconfig_path(profile.as_deref());
                    let detection = rb_wsl::query::detect_mode(
                        &exe,
                        &name,
                        settings.networking_mode_override.mode(),
                        wslconfig.as_deref(),
                        &MODES,
                        LIMIT,
                    )
                    .await;
                    WslInfo::new(
                        distros,
                        Some(name),
                        Some(detection),
                        HelperStatus::Stopped,
                        None,
                    )
                }
            }
        }
    };
    serde_json::to_value(answer).unwrap_or(Value::Null)
}
