//! Every command of the app with its handler: the core commands of `rb_commands::builtin`, the
//! grid's ([`crate::commands`]), `diagnostics.create` (RB-010-10), the plugin system's
//! `plugins.*` (RB-010-03), the sidecar's ([`crate::sidecar`], RB-040-03) and the MCP server's
//! ([`crate::mcp`], RB-030-01). A command whose feature has not landed yet answers
//! `CommandError::NotImplemented`; one that lives in the shell is sent there as `uiCommand`.

use std::sync::{Arc, Weak};

use rb_commands::{CommandDef, CommandError, CommandRegistry, Handler, builtin};
use rb_diag::{BundleInput, DiagPaths, Versions};
use rb_engine::{Target, ViewHost};
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use serde_json::{Value, json};

use super::Core;
use crate::commands;
use crate::page_window::PageWindow;

/// The i18n key of the notice after the diagnostics ZIP was written; placeholder `{path}`.
pub const DIAGNOSTICS_CREATED_KEY: &str = "diagnostics.created";

/// The commands that live in the shell: the core resolves them and sends `uiCommand`.
const SHELL_COMMANDS: &[&str] = &[
    "view.focus_address_bar",
    "settings.open",
    builtin::SHORTCUTS_EDIT_COMMAND,
];

/// Registers every command with `registry`; returns what the registry refused (a test holds
/// it empty).
pub fn register_all(
    registry: &mut CommandRegistry,
    core: &Arc<Core>,
    diag: Option<DiagPaths>,
) -> Vec<CommandError> {
    let mut refused = Vec::new();
    for def in builtin::definitions() {
        let handler = builtin_handler(def.id.as_str(), Arc::downgrade(core));
        refused.extend(registry.register(def, handler).err());
    }
    refused.extend(commands::register(registry, core.host()));
    refused.extend(crate::breakpoint_keys::register(registry, core));
    refused.extend(crate::engines::set_engine::register(registry, core));
    refused.extend(crate::engines::android::commands::register(registry, core));
    // RB-095-04: the iOS simulators and devices of the Mac.
    refused.extend(crate::engines::ios::commands::register(registry, core));
    refused.extend(crate::emulation::extras::register(registry, core));
    // The page's text direction and pseudo-localisation (RB-092-02).
    refused.extend(crate::emulation::page_locale::register(registry, core));
    // The page clock and the animation rates (RB-094-07).
    refused.extend(crate::time_control::register(registry, core));
    refused.extend(crate::sidecar::register(registry, core));
    refused.extend(crate::sessions::register(registry, core));
    refused.extend(crate::basic_auth::register(registry, core));
    refused.extend(crate::session_state::register(registry, core));
    refused.extend(crate::bookmarks::register(registry, core));
    refused.extend(crate::restore_last::register(registry, core));
    refused.extend(crate::cell_info::edit::register(registry, core));
    refused.extend(crate::layout::defs::register(registry, core));
    refused.extend(crate::layout::view_edit::register(registry, core));
    refused.extend(crate::pop_out::register(registry, core));
    refused.extend(crate::updates::register(registry, core));
    refused.extend(crate::first_run::register(registry, core));
    // RB-094-23: the settings window's "Restart now".
    refused.extend(crate::restart::register(registry, core));
    refused.extend(crate::sitemap::register(registry, core));
    refused.extend(crate::compare::register(registry, core));
    refused.extend(crate::engine_compare::register(registry, core));
    refused.extend(crate::video::register(registry, core));
    refused.extend(crate::capture::clean_defs::register(registry, core));
    refused.extend(crate::extensions::register(registry, core));
    refused.extend(crate::cookies::register(registry, core));
    refused.extend(crate::sync_agent::page_commands::register(registry, core));
    refused.extend(crate::mcp::register(registry, core));
    refused.extend(crate::cdp_proxy::register(registry, core));
    refused.extend(crate::portal::register(registry, core));
    refused.extend(crate::flows::register(registry, core));
    refused.extend(crate::vitals::register(registry, core));
    refused.extend(crate::status::register(registry, core));
    refused.extend(crate::view_footer::register(registry, core));
    refused.extend(crate::toolbar_rows::register(registry, core));
    refused.extend(crate::panel_dock::register(registry, core));
    // rBrowser's own theme (RB-080-95).
    refused.extend(crate::app_theme::register(registry, core));
    refused.extend(crate::checks::cancel::register(registry, core));
    // The findings export (RB-092-05).
    refused.extend(crate::checks::export::command::register(registry, core));
    // The bug report bundle (RB-093-04).
    refused.extend(crate::bug_report::command::register(registry, core));
    refused.extend(crate::open_link::register(registry, core));
    // The page zoom inside a view (RB-090-20).
    refused.extend(crate::page_zoom::register(registry, core));
    // Forced pseudo-class states (RB-090-53).
    refused.extend(crate::force_state::register(registry, core));
    // The timing counters (RB-090-32).
    refused.extend(crate::perf::counters::register(registry, core));
    match rb_diag::command_def() {
        Ok(def) => refused.extend(
            registry
                .register(def, diagnostics_handler(Arc::downgrade(core), diag))
                .err(),
        ),
        Err(error) => refused.push(error),
    }
    match rb_plugin_host::core_command_defs() {
        Ok(defs) => {
            for def in defs {
                let handler = plugins_handler(&def, Arc::downgrade(core));
                refused.extend(registry.register(def, handler).err());
            }
        }
        Err(error) => refused.push(error),
    }
    refused
}

/// The handler of a core command of `rb_commands::builtin`.
fn builtin_handler(id: &str, core: Weak<Core>) -> Handler {
    let id = id.to_owned();
    Arc::new(move |args: Value| {
        let core = core
            .upgrade()
            .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
        // The feature jobs of wave 0.1-w1b answer their own commands (`crate::features`).
        if let Some(result) = crate::features::run_command(&core, &id, &args) {
            return result;
        }
        let host = core.host();
        let failed = |error: rb_engine::EngineError| CommandError::Failed(error.to_string());
        match id.as_str() {
            "view.back" => host.go_back(target_of(&args)?).map_err(failed)?,
            "view.forward" => host.go_forward(target_of(&args)?).map_err(failed)?,
            "view.navigate" => match args.get("url").and_then(Value::as_str) {
                Some(url) => host.navigate(target_of(&args)?, url).map_err(failed)?,
                // From a shortcut: the address bar takes the URL.
                None => core.send(&ui_command("view.focus_address_bar")),
            },
            "reload.all" => {
                let ignore_cache = args
                    .get("ignoreCache")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                host.reload(target_of(&args)?, ignore_cache)
                    .map_err(failed)?;
            }
            shell if SHELL_COMMANDS.contains(&shell) => core.send(&ui_command(shell)),
            // The palette in its popup below the toolbar (RB-060-11), which takes the typing;
            // `uiCommand` starts it afresh when the popup is loaded already.
            rb_commands::chord::PALETTE_COMMAND => {
                let palette = PageWindow::Palette;
                core.ui().open_page_window(palette, palette.url());
                core.send(&ui_command(rb_commands::chord::PALETTE_COMMAND));
            }
            // The findings panel (RB-060-01) in the panels page, floating or docked (RB-080-69).
            "panels.findings" => {
                // A person's (RB-090-71), as a person's check is (RB-099-25): a folded dock opens.
                crate::panel_dock::core_panels::open_unfolded(
                    &core,
                    crate::panel_dock::core_panels::FINDINGS,
                );
            }
            // sync.toggle (RB-020-02); the feature commands whose job has not landed.
            _ => return Err(CommandError::NotImplemented),
        }
        Ok(Value::Null)
    })
}

/// The `target` of a navigation command (MCP): `"all"` or a view id; every view when absent.
fn target_of(args: &Value) -> Result<Target, CommandError> {
    match args.get("target") {
        None | Some(Value::Null) => Ok(Target::All),
        Some(target) => serde_json::from_value(target.clone())
            .map_err(|error| CommandError::InvalidArgs(error.to_string())),
    }
}

fn ui_command(id: &str) -> CoreToShell {
    CoreToShell::UiCommand {
        id: id.to_owned(),
        args: None,
    }
}

/// `diagnostics.create`: from the shell (no arguments) the Save dialog and the ZIP where the
/// person chose (`diag_save`, RB-080-73); with arguments (MCP) rb-diag's handler over the current
/// config into the diagnostics folder, then a notice with the path.
fn diagnostics_handler(core: Weak<Core>, diag: Option<DiagPaths>) -> Handler {
    let Some(paths) = diag else {
        return Arc::new(|_| {
            Err(CommandError::Failed(
                "diagnostics.error.noLogDir".to_owned(),
            ))
        });
    };
    let input_core = core.clone();
    let bundles = paths.bundles.clone();
    let input: Arc<dyn Fn() -> BundleInput + Send + Sync> = Arc::new(move || {
        let core = input_core.upgrade();
        let mut input = bundle_input(
            &paths,
            core.as_ref().map(|core| core.host().config()).as_ref(),
            core.map_or(rb_diag::settings::DEFAULT_CRASH_DUMPS, |core| {
                usize::try_from(core.user_settings().logs.crash_dumps_in_bundle).unwrap_or(0)
            }),
        );
        // The sidecar's Node and Playwright when it runs; the ZIP never starts it (RB-040-03).
        if let Some(answer) = crate::sidecar::versions() {
            crate::sidecar::apply_versions(&mut input.versions, &answer);
        }
        input
    });
    let inner = rb_diag::command::handler(input.clone());
    Arc::new(move |args: Value| {
        // The shell (button, palette, shortcut) asks where to save (RB-080-73); MCP names its
        // arguments and writes to the diagnostics folder.
        if args.is_null() {
            let core = core
                .upgrade()
                .ok_or_else(|| CommandError::Failed("the core is gone".to_owned()))?;
            let input = input.clone();
            let write: super::diag_save::WriteZip = Arc::new(move |path: &std::path::Path| {
                let args = rb_diag::DiagnosticsArgs {
                    out: Some(path.to_path_buf()),
                };
                rb_diag::command::run_cli(&args, &input())
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            });
            let folder = super::diag_save::desktop().unwrap_or_else(|| bundles.clone());
            super::diag_save::ask(&core, &folder, write);
            return Ok(json!({ "asked": true }));
        }
        let answer = inner(args)?;
        if let (Some(core), Some(path)) = (core.upgrade(), answer["path"].as_str()) {
            core.notify(CoreToShell::Notification(Notification {
                level: NotificationLevel::Info,
                message_key: DIAGNOSTICS_CREATED_KEY.to_owned(),
                args: [("path".to_owned(), path.to_owned())].into(),
                settings_key: None,
            }));
        }
        Ok(answer)
    })
}

/// What the diagnostics ZIP takes: the versions, the effective config (masked by rb-diag) and
/// the plugins (none load yet).
pub fn bundle_input(
    paths: &DiagPaths,
    config: Option<&rb_config::ProjectConfig>,
    max_crash_dumps: usize,
) -> BundleInput {
    BundleInput {
        paths: paths.clone(),
        versions: Versions {
            app: crate::cli::VERSION.to_owned(),
            ..Versions::default()
        },
        config_effective: config.and_then(|config| serde_json::to_value(config).ok()),
        plugins: json!([]),
        // The timing counters of this process (RB-090-32); empty for `rbrowser diagnostics`.
        perf: crate::perf::counters::report(),
        max_crash_dumps,
    }
}

/// `rbrowser diagnostics [--out <zip>]`: writes the ZIP for the project at `project` (or none)
/// and prints its path; returns the exit code.
pub fn diagnostics_cli(args: &rb_diag::DiagnosticsArgs, project: Option<&std::path::Path>) -> i32 {
    // Inside the local folder, so the portable mode moves it too (RB-010-15).
    let Some(paths) = rb_config::local_dir().map(|dir| DiagPaths::under(&dir)) else {
        eprintln!("rbrowser diagnostics: no folder for the logs (LOCALAPPDATA is not set)");
        return 1;
    };
    let user = super::settings::load_user().settings;
    let config = project.and_then(|root| crate::project::load(root).ok());
    let dumps = usize::try_from(user.logs.crash_dumps_in_bundle).unwrap_or(0);
    match rb_diag::command::run_cli(args, &bundle_input(&paths, config.as_ref(), dumps)) {
        Ok(report) => {
            println!("{}", report.path.display());
            0
        }
        Err(error) => {
            eprintln!("rbrowser diagnostics: {error}");
            1
        }
    }
}

/// The `plugins.*` commands: `plugins.list` answers the rows of Settings > Plugins
/// (`crate::plugins::list`, RB-080-51), `plugins.panels` opens the panels window (RB-060-12),
/// `plugins.resume` runs a paused plugin again - the one named, or every paused one
/// (`crate::plugins::resume`, RB-090-59); install, enable, disable, remove and trust are
/// Settings > Plugins' own (its switch and the catalogue) and have no handler here. Asked with
/// `{"show": true}` (their rows in ≡ › Plugins) `plugins.list` and those open Settings > Plugins
/// (RB-090-82); their rows did nothing before.
fn plugins_handler(def: &CommandDef, core: Weak<Core>) -> Handler {
    match def.id.as_str() {
        // The loaded plugins (RB-060-12); their load finishes off the UI thread first.
        "plugins.list" => Arc::new(move |args: Value| {
            if shows(&args) {
                return open_plugin_list(&core);
            }
            let loaded = crate::analyzer::plugins::bundled();
            let rows = crate::plugins::list::rows(&loaded, crate::plugins::list::trusted());
            // An agent gets no text of a package that did not verify.
            let mut answer =
                crate::plugins::list::answer(Some(crate::plugins::list::for_mcp(rows)));
            // RB-091-04: the plugins the project names that are not installed, and the bundled
            // ones not chosen - by id; MCP installs neither.
            if let Value::Object(fields) = &mut answer {
                let missing = core
                    .upgrade()
                    .map(|core| crate::plugins::bundle::missing_now(&core))
                    .unwrap_or_default();
                let available: Vec<&str> = loaded
                    .report
                    .available
                    .iter()
                    .map(|entry| entry.manifest.id.as_str())
                    .collect();
                fields.insert("missing".to_owned(), json!(missing));
                fields.insert("available".to_owned(), json!(available));
            }
            Ok(answer)
        }),
        // The plugin catalogue (RB-091-01). From a shortcut or the palette (no arguments) the
        // catalogue's tab of Settings > Plugins, or the update check with its notice; from MCP
        // (`plugins_catalog_list`, an object) the catalogue as data - it installs nothing.
        "plugins.catalog" | "plugins.check_updates" => {
            let refresh = def.id.as_str() == "plugins.check_updates";
            Arc::new(move |args: Value| {
                let core = core
                    .upgrade()
                    .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
                if !args.is_object() {
                    if refresh {
                        // A person's check names its start (RB-090-85).
                        core.notify(CoreToShell::Notification(
                            crate::plugins::catalog::checking_notice(),
                        ));
                        crate::plugins::catalog::check_in_background(&core);
                    } else {
                        crate::settings_window::open(
                            &core,
                            Some(crate::plugins::catalog::CATALOG_SETTING),
                        );
                    }
                    return Ok(Value::Null);
                }
                let refresh = refresh
                    || args
                        .get("refresh")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                Ok(crate::plugins::catalog::for_mcp(&core, refresh))
            })
        }
        // A paused plugin runs again (RB-090-59): the Resume button of its row names it; the
        // palette and the shortcut resume every paused plugin and say which.
        "plugins.resume" => Arc::new(move |args: Value| {
            let answer =
                crate::plugins::resume::resume(crate::analyzer::plugins::healths(), &args)?;
            if args.get("id").is_none()
                && let Some(core) = core.upgrade()
            {
                let resumed: Vec<String> = answer["resumed"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|id| id.as_str().map(str::to_owned))
                    .collect();
                core.notify(CoreToShell::Notification(crate::plugins::resume::notice(
                    &resumed,
                )));
            }
            Ok(answer)
        }),
        rb_plugin_host::PANELS_COMMAND => Arc::new(move |args: Value| {
            let core = core
                .upgrade()
                .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
            Ok(crate::plugins::commands::open_panels_unfolded(&core, &args))
        }),
        "plugins.install" | "plugins.enable" | "plugins.disable" | "plugins.remove"
        | "plugins.trust_key" => Arc::new(move |args: Value| {
            if shows(&args) {
                return open_plugin_list(&core);
            }
            // The notice of the bundled plugins new in this version (RB-094-13): its button
            // installs them, live. MCP never reaches `plugins.install` (protected).
            if let Some(ids) = args.get("bundled") {
                return crate::plugins::bundle_install::install_new(&core, ids);
            }
            Err(CommandError::NotImplemented)
        }),
        _ => Arc::new(|_| Err(CommandError::NotImplemented)),
    }
}

/// Whether a `plugins.*` command was asked to show Settings > Plugins (`{"show": true}`, the
/// menu rows, RB-090-82).
fn shows(args: &Value) -> bool {
    args.get("show").and_then(Value::as_bool) == Some(true)
}

/// Settings > Plugins, for a `plugins.*` command asked to show it.
fn open_plugin_list(core: &Weak<Core>) -> Result<Value, CommandError> {
    let core = core
        .upgrade()
        .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
    crate::settings_window::open(&core, Some(crate::settings_window::PLUGINS_CATEGORY));
    Ok(Value::Null)
}
