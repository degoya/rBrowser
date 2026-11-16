//! The project of this instance: where it is, its config, and the browser profile that belongs
//! to it. One project per instance; each project has its own profile (`root_cache_path`), so
//! two instances on different projects never share a cache.

use std::path::{Path, PathBuf};

use rb_config::{CONFIG_DIR, CONFIG_FILE, ConfigError, DevicePreset, ProjectConfig};

/// The project root: the folder given on the command line, else the first folder from `cwd`
/// upwards that holds `.rbrowser/config.json`.
pub fn find_root(given: Option<&Path>, cwd: &Path) -> Option<PathBuf> {
    if let Some(given) = given {
        return Some(given.to_path_buf());
    }
    rb_config::discover(cwd).or_else(|| {
        cwd.ancestors()
            .find(|dir| dir.join(CONFIG_DIR).join(CONFIG_FILE).is_file())
            .map(Path::to_path_buf)
    })
}

/// The effective config of the project at `root`: RB-010-01's loader (merge with
/// `config.local.json`, migration, validation) behind RB-010-09's trust gate, so the gated
/// network keys apply only in a trusted workspace; its warnings go to the log, and its header
/// values are masked there from now on.
pub fn load(root: &Path) -> Result<ProjectConfig, ConfigError> {
    load_trusted(root).map(|(config, _)| config)
}

/// [`load`], and whether the workspace was trusted for it (the start-up trust,
/// `crate::trust::start_trust`). A breakpoint that takes its size from a device preset the
/// presets in effect do not hold refuses the config ([`check_sizes`], RB-010-12).
pub fn load_trusted(root: &Path) -> Result<(ProjectConfig, bool), ConfigError> {
    let (loaded, trusted) = crate::trust::load_config(root)?;
    check_sizes(&loaded.config, &devices_in_effect())?;
    crate::control::settings::register_config_secrets(&loaded.config);
    for warning in &loaded.warnings {
        tracing::warn!(key = %warning.key, path = %warning.path, "{}", warning.message);
    }
    Ok((loaded.config, trusted))
}

/// Refuses `config` when a breakpoint leaves out a side and names a device preset `devices`
/// does not hold: it would have no size (`rb_config::check_viewports`).
pub fn check_sizes(config: &ProjectConfig, devices: &[DevicePreset]) -> Result<(), ConfigError> {
    let issues = rb_config::check_viewports(config, devices);
    if issues.is_empty() {
        Ok(())
    } else {
        Err(ConfigError::Invalid(issues))
    }
}

/// The device presets a project config is checked against: the user's `devices` from the
/// settings file over the built-in list. They go into effect for the views at once, so a
/// breakpoint sized by one of the user's presets has that size from the first grid on, before
/// the core reads the user settings itself.
fn devices_in_effect() -> Vec<DevicePreset> {
    crate::emulation::set_user_devices(&crate::control::settings::load_user().settings.devices);
    crate::emulation::device_list()
}

/// The browser profile of the project at `root` under `base` (`%LOCALAPPDATA%\rBrowser`):
/// `profiles\<folder>-<hash>`, the hash over the whole path so equal folder names stay apart;
/// `profiles\default` without a project.
pub fn profile_dir(base: &Path, root: Option<&Path>) -> PathBuf {
    let profiles = base.join("profiles");
    let Some(root) = root else {
        return profiles.join("default");
    };
    // Both separators on every platform: the path may come from Windows or from WSL.
    let written = root.to_string_lossy().replace('/', "\\");
    let written = written.trim_end_matches('\\');
    let name: String = written
        .rsplit('\\')
        .next()
        .unwrap_or_default()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(40)
        .collect();
    let name = if name.is_empty() {
        "project".to_owned()
    } else {
        name
    };
    profiles.join(format!(
        "{name}-{:016x}",
        fnv1a(written.to_lowercase().as_bytes())
    ))
}

/// FNV-1a, 64 bit: stable across builds and platforms, unlike the std hasher.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// Whether `path` lies on the WSL share (`\\wsl.localhost\...`, `\\wsl$\...`). rBrowser never
/// runs from there (ADR 0001, Decision 6): the sandbox cannot start child processes from it and
/// the GPU process crashes.
pub fn is_wsl_share(path: &Path) -> bool {
    let text = path
        .to_string_lossy()
        .replace('/', "\\")
        .to_ascii_lowercase();
    let text = text
        .strip_prefix("\\\\?\\unc\\")
        .map_or(text.clone(), |rest| format!("\\\\{rest}"));
    text.starts_with("\\\\wsl.localhost\\") || text.starts_with("\\\\wsl$\\")
}

#[cfg(test)]
#[path = "project_tests.rs"]
mod tests;
