//! The core commands of the feature jobs of wave 0.1-w1b (contract 0.1-w1b,
//! `docs/contracts-0.1-w1b.md`): ids, groups, default leader chords and MCP mapping, defined here
//! once so the lint holds them against the reserved list and each other before any job lands.
//! The handlers live with the job in `crates/rbrowser` (`crate::features` routes to them); until
//! a job lands, its commands answer `CommandError::NotImplemented`.

use crate::{Binding, Chord, CommandDef, CommandId, Group, Key, McpCoverage};

/// Why the trust commands have no MCP tool: a trust decision is a human's.
pub const TRUST_OMITTED: &str = "protected: trust only via UI";

/// Why the external window commands have no MCP tool (RB-040-01).
pub const EXTERNAL_WINDOWS_OMITTED: &str = "desktop window layout, no use to an agent";

/// Why `visual.lfs_setup` has no MCP tool: it changes the Git configuration (RB-050-04).
pub const LFS_SETUP_OMITTED: &str = "protected: changes the Git configuration; only the user";

/// Why `recorder.check` has no MCP tool (RB-090-60).
pub const RECORDER_CHECK_OMITTED: &str =
    "ui-only: the element under the person's pointer; an agent writes its checks into the flow";

/// Why `panels.recorder` has no MCP tool (RB-090-60).
pub const RECORDER_PANEL_OMITTED: &str =
    "ui-only: the Recorder panel; recorder_export and the recording files serve agents";

/// Why `recorder.delete` has no MCP tool (RB-090-91).
pub const RECORDER_DELETE_OMITTED: &str =
    "protected: deleting project files is the person's; an agent never deletes a recording";

/// Why `recorder.pick_element` has no MCP tool (RB-092-01).
pub const RECORDER_PICK_OMITTED: &str = "ui-only: the person clicks the element; \
     recorder_step_add takes a point (x, y) and names the element there";

/// Why `layout.zoom_to_fit` has no MCP tool of its own (RB-090-63): on the canvas
/// `canvas_zoom` `"fit"` runs it; in the other modes it only undoes a hand zoom of the window.
pub const ZOOM_TO_FIT_OMITTED: &str = "ui-only: fits every view into the window; on the canvas \
     MCP canvas_zoom \"fit\" runs it, the other modes have no hand zoom an agent sets";

/// One row: id, group, the keys after the leader, MCP tool (`Ok`) or why there is none (`Err`).
type Row = (
    &'static str,
    Group,
    [char; 2],
    Result<&'static str, &'static str>,
);

const ROWS: &[Row] = &[
    // RB-010-05: emulation per view.
    (
        "emulation.set",
        Group::View,
        ['V', 'E'],
        Ok("emulation_set"),
    ),
    (
        "emulation.toggle_color_scheme",
        Group::View,
        ['V', 'C'],
        Ok("emulation_set"),
    ),
    (
        "emulation.toggle_reduced_motion",
        Group::View,
        ['V', 'M'],
        Ok("emulation_set"),
    ),
    (
        "emulation.reset_view",
        Group::View,
        ['V', 'R'],
        Ok("emulation_set"),
    ),
    // RB-010-06: DevTools.
    (
        "devtools.toggle",
        Group::Devtools,
        ['D', 'D'],
        Ok("devtools_open"),
    ),
    (
        "devtools.open",
        Group::Devtools,
        ['D', 'V'],
        Ok("devtools_open"),
    ),
    (
        "devtools.dock_cycle",
        Group::Devtools,
        ['D', 'W'],
        Err("ui-only: dock layout"),
    ),
    // RB-010-09: workspace trust. `trust.grant` opens the dialog; only its button trusts.
    (
        "trust.show",
        Group::Settings,
        [',', 'T'],
        Err(TRUST_OMITTED),
    ),
    (
        "trust.grant",
        Group::Settings,
        [',', 'G'],
        Err(TRUST_OMITTED),
    ),
    (
        "trust.revoke",
        Group::Settings,
        [',', 'V'],
        Err(TRUST_OMITTED),
    ),
    // RB-010-11: the CLI's counterpart in the app.
    (
        "project.open_recent",
        Group::Settings,
        [',', 'P'],
        Ok("project_open"),
    ),
    // RB-010-12: layout presets, focus mode, zoom and pan.
    (
        "layout.focus_mode",
        Group::Layout,
        ['L', 'F'],
        Ok("layout_set"),
    ),
    (
        "layout.focus_next",
        Group::Layout,
        ['L', ']'],
        Ok("layout_set"),
    ),
    (
        "layout.focus_prev",
        Group::Layout,
        ['L', '['],
        Ok("layout_set"),
    ),
    (
        "layout.zoom_to_fit",
        Group::Layout,
        ['L', 'Z'],
        Err(ZOOM_TO_FIT_OMITTED),
    ),
    // RB-010-13: the breakpoint analyser (`breakpoints.analyze` is in `builtin`).
    (
        "breakpoints.append_detected",
        Group::Breakpoints,
        ['B', 'N'],
        Ok("breakpoints_apply"),
    ),
    // RB-010-18: the About page; MCP answers its facts as `app_info`.
    ("app.about", Group::Settings, [',', 'A'], Ok("app_info")),
    // RB-080-51: Settings > Plugins (`X` as the plugins' leader group); MCP reads the same
    // list as `plugins_list`.
    (
        "settings.open_plugins",
        Group::Settings,
        [',', 'X'],
        Ok("plugins_list"),
    ),
    // RB-030-04: the WSL helper; MCP reports its state as `wsl_info`.
    (
        "wsl.restart_helper",
        Group::Reload,
        ['R', 'W'],
        Ok("wsl_info"),
    ),
    // RB-030-05: hot reload; `reload.all` is in `builtin`.
    ("reload.css_only", Group::Reload, ['R', 'C'], Ok("reload")),
    (
        "reload.toggle_pause",
        Group::Reload,
        ['R', 'P'],
        Err("session toggle: MCP sets reload.mode with settings_set"),
    ),
    // RB-060-01: the check runner and the findings panel.
    (
        "checks.run_all",
        Group::Checks,
        ['K', 'K'],
        Ok("checks_run"),
    ),
    (
        "panels.findings",
        Group::Panels,
        ['P', 'F'],
        Err("ui-only: findings panel; checks_run answers the findings"),
    ),
    // RB-050-03: screenshots; every mode is the MCP tool `screenshot`.
    (
        "capture.views",
        Group::Capture,
        ['C', 'S'],
        Ok("screenshot"),
    ),
    (
        "capture.full_page",
        Group::Capture,
        ['C', 'F'],
        Ok("screenshot"),
    ),
    (
        "capture.all_viewport",
        Group::Capture,
        ['C', 'A'],
        Ok("screenshot"),
    ),
    (
        "capture.element",
        Group::Capture,
        ['C', 'L'],
        Ok("screenshot"),
    ),
    (
        "capture.overview",
        Group::Capture,
        ['C', 'O'],
        Ok("screenshot"),
    ),
    (
        "capture.delay_cycle",
        Group::Capture,
        ['C', 'D'],
        Err("ui-only: UI preset; per MCP as delayMs"),
    ),
    // RB-020-02: the SyncHub (`sync.toggle` is in `builtin`).
    ("sync.skip_next", Group::Sync, ['S', 'N'], Ok("sync_set")),
    ("sync.resync", Group::Sync, ['S', 'R'], Ok("sync_set")),
    ("sync.toggle_view", Group::Sync, ['S', 'X'], Ok("sync_set")),
    // RB-020-06: navigation sync.
    (
        "sync.toggle_view_navigation",
        Group::Sync,
        ['S', 'U'],
        Ok("sync_set"),
    ),
    // RB-020-04: scroll sync on or off (`sync_set { feature: "scroll" }`).
    (
        "sync.toggle_scroll",
        Group::Sync,
        ['S', 'C'],
        Ok("sync_set"),
    ),
    // RB-020-05: focus sync (`sync.focus`).
    ("sync.toggle_focus", Group::Sync, ['S', 'F'], Ok("sync_set")),
    // RB-040-01: the external windows of Firefox and WebKit; arrange is `V D` ("distribute":
    // `V A` is the address bar's).
    (
        "view.external.arrange",
        Group::View,
        ['V', 'D'],
        Err(EXTERNAL_WINDOWS_OMITTED),
    ),
    (
        "view.external.raise",
        Group::View,
        ['V', 'W'],
        Err(EXTERNAL_WINDOWS_OMITTED),
    ),
    (
        "engine.restart",
        Group::Reload,
        ['R', 'E'],
        Ok("engine_restart"),
    ),
    // RB-040-02: the Firefox windows on or off, beside `R E`'s restart (the job's `V F` is
    // view.forward, `V O` view.zoom_out); the MCP side is `engine_restart { engine: "firefox" }`.
    (
        "engine.firefox.toggle",
        Group::Reload,
        ['R', 'F'],
        Ok("engine_restart"),
    ),
    // RB-040-04: WebKit windows on or off for the session (`engines.webkit.enabled` is the
    // default); MCP reaches WebKit views through every core tool and `engine_restart`.
    (
        "engine.webkit.toggle",
        Group::View,
        ['V', 'K'],
        Ok("engine_restart"),
    ),
    // RB-020-09: throttling of hidden views on or off (`performance.throttleHidden`); the job's
    // `V P` is the command palette's.
    (
        "performance.toggle_throttle",
        Group::View,
        ['V', 'Q'],
        Ok("settings_set"),
    ),
    // RB-050-01: the recorder; `recorder.pause` is `recorder_stop` with `pause: true`.
    (
        "recorder.start",
        Group::Flows,
        ['F', 'S'],
        Ok("recorder_start"),
    ),
    (
        "recorder.stop",
        Group::Flows,
        ['F', 'T'],
        Ok("recorder_stop"),
    ),
    (
        "recorder.pause",
        Group::Flows,
        ['F', 'P'],
        Ok("recorder_stop"),
    ),
    // RB-050-02: the recorder's export through the `recorder-export` plugin.
    (
        "recorder.export_flow",
        Group::Flows,
        ['F', 'W'],
        Ok("recorder_export"),
    ),
    (
        "recorder.export_spec",
        Group::Flows,
        ['F', 'E'],
        Ok("recorder_export"),
    ),
    // RB-090-66: a stored recording played through the flows runner, without an export; `Y`
    // for plaY (`F P` pauses the recorder, `F R` runs a flow).
    (
        "recorder.play",
        Group::Flows,
        ['F', 'Y'],
        Ok("recorder_play"),
    ),
    // RB-090-60: a check step from the page menu (the shortcut picks the element) and the
    // Recorder panel - in the Flows group: `P R` is the network panel's, `P W` the PWA panel's.
    (
        "recorder.check",
        Group::Flows,
        ['F', 'K'],
        Err(RECORDER_CHECK_OMITTED),
    ),
    (
        "panels.recorder",
        Group::Flows,
        ['F', 'V'],
        Err(RECORDER_PANEL_OMITTED),
    ),
    // RB-090-91: a stored recording deleted, the person's (`F D` deletes a flow); reMove.
    (
        "recorder.delete",
        Group::Flows,
        ['F', 'M'],
        Err(RECORDER_DELETE_OMITTED),
    ),
    // RB-092-01: the recording editor - New step, Just this one, from Here, continUe, Bypass, pIck.
    (
        "recorder.step_add",
        Group::Flows,
        ['F', 'N'],
        Ok("recorder_step_add"),
    ),
    (
        "recorder.step_try",
        Group::Flows,
        ['F', 'J'],
        Ok("recorder_step_try"),
    ),
    (
        "recorder.play_from",
        Group::Flows,
        ['F', 'H'],
        Ok("recorder_play"),
    ),
    (
        "recorder.continue_from",
        Group::Flows,
        ['F', 'U'],
        Ok("recorder_start"),
    ),
    (
        "recorder.step_toggle",
        Group::Flows,
        ['F', 'B'],
        Ok("recorder_step_toggle"),
    ),
    (
        "recorder.pick_element",
        Group::Flows,
        ['F', 'I'],
        Err(RECORDER_PICK_OMITTED),
    ),
    // RB-050-04: visual regression; setting up Git LFS changes the Git configuration.
    (
        "visual.compare",
        Group::Capture,
        ['C', 'R'],
        Ok("visual_compare"),
    ),
    (
        "visual.baseline_update",
        Group::Capture,
        ['C', 'B'],
        Ok("visual_baseline_update"),
    ),
    (
        "visual.lfs_setup",
        Group::Capture,
        ['C', 'G'],
        Err(LFS_SETUP_OMITTED),
    ),
    // RB-080-43: the page's context menu. `devtools.inspect_at { view, x, y }` opens DevTools
    // for the view and selects the node at the point; without a point it waits for a click in
    // the view (the inspect mode) - `D S`, "select an element".
    (
        "devtools.inspect_at",
        Group::Devtools,
        ['D', 'S'],
        Ok("devtools_inspect_at"),
    ),
];

/// The numbered families: id prefix, group, group key; `<prefix>.1` ... `<prefix>.9` on
/// `Leader <group key> 1` ... `9`, all covered by the MCP tool.
const NUMBERED: &[(&str, Group, char, &str)] = &[
    // RB-010-12: the layout presets and environments in config order.
    ("layout.preset", Group::Layout, 'L', "layout_set"),
    (
        "environments.switch",
        Group::Environments,
        'E',
        "environment_set",
    ),
];

/// The definitions: the rows in table order, then each numbered family 1 to 9. The title key
/// is `commands.<id>`.
pub fn definitions() -> Vec<CommandDef> {
    let rows = ROWS.iter().map(|(id, group, keys, mcp)| {
        def(
            id,
            *group,
            *keys,
            match mcp {
                Ok(tool) => McpCoverage::Covered((*tool).to_owned()),
                Err(reason) => McpCoverage::Omitted((*reason).to_owned()),
            },
        )
    });
    let numbered = NUMBERED.iter().flat_map(|(prefix, group, key, tool)| {
        ('1'..='9').map(move |digit| {
            def(
                &format!("{prefix}.{digit}"),
                *group,
                [*key, digit],
                McpCoverage::Covered((*tool).to_owned()),
            )
        })
    });
    rows.chain(numbered).collect()
}

fn def(id: &str, group: Group, keys: [char; 2], mcp: McpCoverage) -> CommandDef {
    CommandDef {
        id: CommandId::new(id).expect("the table's ids are checked by builtin_features_tests"),
        title_key: format!("commands.{id}"),
        group,
        default_bindings: vec![Binding::Leader(Chord {
            keys: keys.iter().map(|key| Key::Char(*key)).collect(),
        })],
        mcp,
    }
}

#[cfg(test)]
#[path = "builtin_features_tests.rs"]
mod tests;
