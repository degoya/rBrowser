//! The core's state the menu is built from, read at the moment the menu opens
//! ([`super::model::MenuState`]): short lock reads only - the UI thread calls this, so nothing
//! here waits for a plugin load or probes the disk.

use std::collections::BTreeSet;

use rb_engine::{ViewHost as _, ViewId};
use rb_plugin_types::PluginKind;

use super::emulation::EmulationState;
use super::model::{MenuState, PluginCommand, PluginPanel};
use crate::control::Core;

/// The commands optional entries wait for.
const OPTIONAL_COMMANDS: &[&str] = &[
    "view.remove",
    "breakpoints.analyze",
    crate::vitals::defs::VITALS,
    crate::sitemap::defs::PANEL,
    crate::view_footer::defs::TOGGLE,
    crate::force_state::SET,
];

/// The state for `view`'s menu, opened at `point` (DIP of the view's widget; `None` when CEF
/// gave none).
pub fn state_for(core: &Core, view: ViewId, point: Option<(i32, i32)>) -> MenuState {
    let host = core.host();
    let user = core.user_settings();
    let (checks, panels) = plugin_entries(core, &user);
    MenuState {
        view,
        point,
        menu_enabled: user.ui.page_context_menu,
        attached: host.capture_target(view).is_some(),
        detached: host.is_detached(view),
        devtools_open: crate::devtools::for_host(host)
            .and_then(|devtools| devtools.current())
            .is_some_and(|open| open.view == view),
        open_on_inspect: user.devtools.open_on_inspect,
        emulation: user.ui.page_context_menu_emulation.then(|| {
            let views: Vec<_> = host
                .views()
                .iter()
                .filter_map(|info| host.emulation(info.id))
                .collect();
            EmulationState::of(host.emulation(view).as_ref(), &views, &user.emulation)
        }),
        recording: crate::video::app::recording_views(),
        checks_busy: crate::checks::is_busy(),
        checks,
        panels,
        known: OPTIONAL_COMMANDS
            .iter()
            .filter(|command| core.has_command(command))
            .map(|command| (*command).to_owned())
            .collect::<BTreeSet<String>>(),
        footer_visible: user.ui.view_footer.visible,
        forced: crate::force_state::marks(
            view,
            &host.views().iter().map(|info| info.id).collect::<Vec<_>>(),
        ),
    }
}

/// The served plugins' check entries (`context_menu` of their manifests, each command with its
/// title in the UI language, one key lookup each) and their panels as the panels window offers
/// them. Nothing before the plugin runtime of the window started or the load is done.
fn plugin_entries(
    core: &Core,
    user: &rb_config::UserSettings,
) -> (Vec<PluginCommand>, Vec<PluginPanel>) {
    let Some(runtime) = crate::plugins::for_host(core.host()) else {
        return (Vec::new(), Vec::new());
    };
    let Some(loaded) = crate::analyzer::plugins::loaded() else {
        return (Vec::new(), Vec::new());
    };
    let mut checks = Vec::new();
    for entry in &loaded.report.loaded {
        let manifest = entry.plugin.manifest();
        if manifest.context_menu.is_empty()
            || !manifest.is(PluginKind::Feature)
            || !runtime.serves(&manifest.id)
        {
            continue;
        }
        for command in &manifest.context_menu {
            let Some(declared) = manifest.commands.iter().find(|entry| &entry.id == command) else {
                continue;
            };
            let title = rb_i18n::plugin_text(&manifest.id, &declared.title)
                .unwrap_or_else(|| declared.title.clone());
            checks.push(PluginCommand {
                command: manifest.command_id(command),
                title,
            });
        }
    }
    let panels = crate::plugins::ipc::panels_offered_for(user, &runtime)
        .into_iter()
        .map(|panel| PluginPanel {
            plugin: panel.plugin,
            panel: panel.panel,
            title: panel.title,
        })
        .collect();
    (checks, panels)
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
