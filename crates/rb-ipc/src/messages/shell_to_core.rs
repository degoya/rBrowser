//! The shell's requests ([`ShellToCore`]); what `setSync` switches is `sync_feature.rs`, the
//! Videos panel's payloads `video_requests.rs` (both re-exported from `messages`).

use rb_config::AnalyzerMode;
use rb_engine::{Target, ViewId};
use rb_settings::Scope;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::layout_modes::{DragPhase, LabelPiece, MapRect};
use crate::native_menu::NativeMenuRow;
use crate::payloads::{Notification, PathKind, TrustDecision};

pub use super::sync_feature::SyncFeature;

/// A request of the shell. Every request gets exactly one answer: a [`crate::CoreToShell`] (`ack`
/// when there is nothing to return) or a failure with an [`crate::IpcErrorCode`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ShellToCore {
    /// The current [`crate::StateSnapshot`]; answered with `state`.
    GetState,
    /// Load `url` in every view (navigation sync).
    Navigate {
        /// The URL as typed; the core completes a missing scheme.
        url: String,
    },
    /// History back in every view.
    Back,
    /// History forward in every view.
    Forward,
    /// Reload every view.
    Reload {
        /// Bypass the cache.
        #[serde(default)]
        ignore_cache: bool,
    },
    /// Switch sync globally or one kind of it.
    SetSync {
        /// What to switch.
        feature: SyncFeature,
        /// On or off.
        enabled: bool,
    },
    /// Switch to the layout preset `id` (RB-010-12).
    SelectLayoutPreset {
        /// The preset id.
        id: String,
    },
    /// Switch to the environment `id`, keeping path, query and hash (RB-010-12).
    SelectEnvironment {
        /// The environment id.
        id: String,
    },
    /// Make `view` the active view (click on a cell label).
    SetActiveView {
        /// The view.
        view: ViewId,
    },
    /// Set `view` to a breakpoint or a free size (quick pick, RB-010-16): either `breakpoint`
    /// or `width` and `height`.
    SetBreakpoint {
        /// The view.
        view: ViewId,
        /// A breakpoint id (config or a device preset).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        breakpoint: Option<String>,
        /// Free width in CSS px.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        width: Option<u32>,
        /// Free height in CSS px.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        height: Option<u32>,
        /// Free DPR (`500x800@2`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dpr: Option<f64>,
    },
    /// Run a command of the registry - what every toolbar button sends (RB-010-17).
    RunCommand {
        /// The command id (`view.zoom_fit_all`).
        id: String,
        /// The command's arguments; `null` when none.
        #[serde(default, skip_serializing_if = "Value::is_null")]
        args: Value,
    },
    /// The toolbar wraps into `rows` rows (RB-080-52): the core makes the shell view that high
    /// and the views move down. Answered with `ack`; refused when `rows` is not 1 or 2, or 2
    /// while `ui.toolbar.overflow` is not `wrap`.
    ToolbarRows {
        /// 1 or 2.
        rows: u8,
    },
    /// The commands with their effective bindings; answered with `commands`.
    ListCommands,
    /// Open the settings window, at `key` when given (deep link `rbrowser://settings/<key>`).
    OpenSettings {
        /// The setting key.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        key: Option<String>,
    },
    /// Every setting's value and origin; answered with `settings`.
    GetSettings {
        /// Also send the schema (the settings window needs it once).
        #[serde(default)]
        include_schema: bool,
    },
    /// Write a setting to a scope; `value: null` removes it from that scope (reset).
    SetSetting {
        /// The dotted key.
        key: String,
        /// The new value.
        value: Value,
        /// Where to write it.
        scope: Scope,
    },
    /// The user's answer to a `trustPrompt` (RB-010-09). Only the shell sends this, never MCP.
    TrustDecision {
        /// The prompt answered.
        prompt_id: u32,
        /// The decision.
        decision: TrustDecision,
    },
    /// Version and engine versions; answered with `about` (RB-010-18).
    GetAbout,
    /// Replace the bindings of command `id` with the user's (text form; an empty list unbinds
    /// it, the palette still reaches it); answered with `commands` (RB-010-17, the editor).
    SetBindings {
        /// The command id.
        id: String,
        /// The bindings in text form (`Ctrl+Shift+Space S S`).
        bindings: Vec<String>,
    },
    /// Restore the default bindings of `id`, or of every command when `id` is absent;
    /// answered with `commands`.
    ResetBinding {
        /// The command id; absent: all.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
    },
    /// May `binding` be bound to `id`? What the editor asks while recording; answered with
    /// `bindingCheck`.
    CheckBinding {
        /// The command id.
        id: String,
        /// The binding in text form.
        binding: String,
    },
    /// The user's bindings as a shortcut file; answered with `bindingsExport`.
    ExportBindings,
    /// Apply a shortcut file in place of the user's bindings; answered with `commands`, what
    /// was skipped comes as notifications.
    ImportBindings {
        /// The file's JSON text (`{ "version": 1, "bindings": { ... } }`).
        json: String,
    },
    /// The emulation `view` has now; answered with `emulation` (RB-010-05).
    GetEmulation {
        /// The view.
        view: ViewId,
    },
    /// The trust of the open project and the trusted projects; answered with `trust`
    /// (RB-010-09, Settings > Security > Workspace trust).
    GetTrust,
    /// Withdraw the trust of the project at `root`; its protected parts stop at once. Answered
    /// with `trust` (RB-010-09). Granting is `trustDecision` only.
    RevokeTrust {
        /// The project root as listed in `trust`.
        root: String,
    },
    /// Let the person choose a file or folder in the native dialog; answered with `pickedPath`
    /// (RB-010-08: the Node and Firefox paths).
    PickPath {
        /// File or folder.
        kind: PathKind,
        /// The i18n key of the dialog's title.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title_key: Option<String>,
    },
    /// Show the diagnostics ZIP saved last in the file manager (RB-080-73): no path - the core
    /// opens only the one it wrote. Answered with `ack`.
    RevealDiagnostics,
    /// The values of every `x-options` source the settings window shows (RB-080-72); answered
    /// with `settingSources`. Only the settings window asks: the check rules are enumerated in
    /// the plugins.
    GetSettingSources,
    /// The device presets in effect; answered with `devices` (RB-010-12).
    GetDevices,
    /// Analyse the site's CSS breakpoints (the wizard); answered with `ack`, the result follows
    /// as the event `breakpointProposal` (RB-010-13).
    AnalyzeBreakpoints {
        /// URLs besides `startUrl` and `analyzer.urls`.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        urls: Vec<String>,
        /// The mode; `analyzer.mode` when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mode: Option<AnalyzerMode>,
    },
    /// Write the chosen breakpoints of a proposal (and their layout) into `config.json`;
    /// answered with `ack` (RB-010-13). Nothing is written without this.
    ApplyBreakpoints {
        /// The proposal's `proposalId`.
        proposal_id: u32,
        /// The ids of the proposed breakpoints to write.
        selected: Vec<String>,
        /// Replace the config's breakpoints instead of appending.
        replace: bool,
    },
    /// Run every enabled check plugin over the views of `target`; answered with `ack`, the run
    /// follows as the event `findings` (RB-060-01).
    RunChecks {
        /// `"all"` or a view id.
        target: Target,
        /// Only these check plugins (ids).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        plugins: Option<Vec<String>>,
        /// The page to check instead of each view's own (`http(s)`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        url: Option<String>,
    },
    /// Mark a finding's element in `views` and scroll it into view; answered with `ack`
    /// (RB-060-01). The selector comes from a plugin: the core hands it to `DOM.querySelector`
    /// only.
    HighlightFinding {
        /// The finding's id.
        finding_id: String,
        /// Its element's CSS selector.
        selector: String,
        /// The views to mark it in.
        views: Vec<ViewId>,
    },
    /// Mark the element behind a view's Web Vital (the LCP element, the largest shift source,
    /// the interaction's target) and scroll it into view; answered with `ack` (RB-060-10). The
    /// core takes the selector from the view's own report, never from the shell.
    HighlightVital {
        /// The view.
        view: ViewId,
        /// The metric.
        metric: rb_sync::VitalMetric,
    },
    /// The palette's popup ran or was left: it hides and the main window takes the focus back;
    /// answered with `ack` (RB-060-11).
    DismissPalette,
    /// The last check run, for the findings window that opens after it: answered with
    /// `findings`, or `ack` before the first run (RB-060-01).
    GetFindings,
    /// The project's flows, for the palette; answered with `flows` (RB-060-11).
    ListFlows,
    /// The project's breakpoint shortcuts and their conflicts; answered with `projectShortcuts`.
    ListProjectShortcuts,
    /// A request of the plugin panels window (`{"op": "list" | "call" | "settings", ...}`, read by
    /// `rbrowser::plugins::panels::PanelRequest`); answered with `pluginPanel` (RB-060-12).
    PluginPanel {
        /// The request.
        request: Value,
    },
    /// The status bar's log, oldest first; answered with `statusMessages` (RB-080-15).
    GetStatusMessages,
    /// A message the shell raised itself (a refused request, an address it does not open) for
    /// the status bar; answered with `ack` (RB-080-15). The core checks its size.
    ReportStatus(Notification),
    /// The message history's popup was left with Escape: it hides and the main window takes the
    /// focus back; answered with `ack` (RB-080-15).
    DismissHistory,
    /// The portal's state and its open pairing; answered with `portal` (RB-080-05).
    GetPortal,
    /// The person's answer to an `extensionPrompt` (RB-080-23); answered with `ack`. Only the
    /// shell sends this, never MCP.
    ExtensionDecision {
        /// The prompt answered.
        prompt_id: u32,
        /// Install, update or remove it, or not.
        accepted: bool,
    },
    /// The extensions installed from the Chrome Web Store, for the settings' list; answered
    /// with `ack`, the list follows as the event `webStoreExtensions` (RB-080-30).
    ListWebStoreExtensions,
    /// The canvas board (RB-080-04): the label of `view` dragged `dx`/`dy` screen DIP from where
    /// the drag began. `move` shows the view there live, `end` snaps it and writes it into the
    /// preset, `cancel` puts it back; answered with `ack`.
    CanvasDrag {
        /// The view.
        view: ViewId,
        /// Right is positive.
        dx: i32,
        /// Down is positive.
        dy: i32,
        /// Where the drag is.
        phase: DragPhase,
    },
    /// The canvas board's empty background dragged `dx`/`dy` screen DIP: the canvas follows the
    /// pointer (`move` live, `end` kept, `cancel` back); answered with `ack`.
    CanvasPan {
        /// Right is positive.
        dx: i32,
        /// Down is positive.
        dy: i32,
        /// Where the drag is.
        phase: DragPhase,
    },
    /// An arrow key on the board's label of `view`: the view moves `dx`/`dy` (-1, 0 or 1) steps
    /// of the canvas's `snap` and is kept; answered with `ack`.
    CanvasStep {
        /// The view.
        view: ViewId,
        /// -1 left, 1 right.
        dx: i32,
        /// -1 up, 1 down.
        dy: i32,
    },
    /// A click on the board's label of `view` (RB-080-22): the view becomes active; was it
    /// active already, its size quick pick opens under the label, as a click on the active
    /// view's strip does in the other modes. Answered with `ack`.
    CanvasSizePick {
        /// The view.
        view: ViewId,
        /// The pick opens without making the view active (the view menu's "Size...",
        /// RB-080-41).
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        force: bool,
    },
    /// A click on the engine badge the board draws at the right end of the label of `view`
    /// (RB-080-33): the engine menu opens under the badge, as a click on a grid strip's badge
    /// opens it. Answered with `ack`.
    CanvasEnginePick {
        /// The view.
        view: ViewId,
    },
    /// A click on a piece of the board's label of `view` (RB-090-69): what a click on the grid
    /// strip's piece does, which the core looks up. Answered with `ack`.
    CanvasPiece {
        /// The view.
        view: ViewId,
        /// The piece.
        piece: LabelPiece,
    },
    /// A right-click on the canvas board (RB-080-46): on the label of `view` its menu opens,
    /// on the free area (no `view`) the add menu - native menus in front of the views, at
    /// `x`/`y` (DIP from the board's top-left corner). Answered with `ack`.
    CanvasMenu {
        /// The view whose label was clicked; absent on the free area.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        view: Option<ViewId>,
        /// The click, from the board's left edge.
        x: i32,
        /// The click, from the board's top edge.
        y: i32,
    },
    /// Ctrl + wheel on the canvas board (RB-080-50): the canvas zooms around `x`/`y` (DIP from
    /// the board's top-left corner, where the turning began) by `delta`, the wheel's whole turn
    /// since then in CSS px, negative to zoom in; the views are scaled with it. `move` shows it
    /// live, `end` (the wheel came to rest) keeps the camera, `cancel` goes back - as a drag.
    /// Nothing with `layoutModes.wheelZoom` off. Answered with `ack`.
    CanvasZoom {
        /// The pointer, from the board's left edge.
        x: i32,
        /// The pointer, from the board's top edge.
        y: i32,
        /// The wheel's turn since the gesture began, in CSS px (100 a notch).
        delta: i32,
        /// Where the gesture is.
        phase: DragPhase,
    },
    /// The canvas board painted the canvas `seq` it was sent (`canvasLive`, RB-090-55): the views
    /// go where it drew their frames. An older canvas than one placed moves nothing. Answered
    /// with `ack`.
    CanvasPainted {
        /// The number of the canvas painted.
        seq: u32,
    },
    /// The toolbar's "+" (RB-080-46): the add menu opens as a native menu with its top-left
    /// corner at `x`/`y` (DIP from the shell view's top-left corner). Answered with `ack`.
    AddViewMenu {
        /// From the shell view's left edge.
        x: i32,
        /// From the shell view's top edge.
        y: i32,
    },
    /// The progress of the check run in progress or the last one, for a findings panel that
    /// opens while it goes; answered with `checksProgress`, or `ack` before the first run
    /// (RB-080-89).
    GetChecksProgress,
    /// The status bar's zoom level button (RB-090-18): the presets (Fit, 25 to 200 %) open as
    /// a native menu with its bottom centre at `x`/`y` (DIP from the status bar view's top-left
    /// corner), above the bar - a `<select>` opened its list below it, off the screen with the
    /// bar at its bottom. Only on the canvas. Answered with `ack`.
    CanvasZoomMenu {
        /// From the status bar view's left edge.
        x: i32,
        /// From the status bar view's top edge.
        y: i32,
    },
    /// The text size the page is drawn at (RB-090-26); answered with `textScale`.
    GetTextScale,
    /// The open project's recordings for the Videos panel (RB-110-12); answered with `videos`.
    GetVideos,
    /// Delete a recording (every video file of it) or one file, after the person confirmed it in
    /// the Videos panel (RB-110-12); answered with `videos`. Shell only; the running one refused.
    DeleteVideo(super::video_requests::VideoFileRef),
    /// Show a recording, or one file, in the file manager (RB-110-12); answered with `ack`.
    RevealVideo(super::video_requests::VideoFileRef),
    /// Make a video into another format beside it (RB-090-52): a WebM exported as MP4, an old MP4
    /// converted to WebM; queued, one at a time. Answered with `videos`, which lists it.
    ConvertVideo(super::video_requests::ConvertVideoRequest),
    /// The Recorder panel (RB-090-60): the stored recordings; answered with `recordings`.
    GetRecordings,
    /// One stored recording as its file holds it; answered with `recording`.
    GetRecording {
        /// Its name.
        name: String,
    },
    /// Write `recording` (schema `recording.v1`) over file `name`; answers `recordings`; shell only.
    SaveRecording {
        /// The file's name; the recording keeps it.
        name: String,
        /// The recording, checked against the model before it is written.
        recording: Value,
    },
    /// Render `name` as `format` (`recorder-export`); `write` puts the flow into the flows folder
    /// and the test into a new export folder (RB-090-66). Answers `recordingExport`; shell only.
    ExportRecording {
        /// The recording.
        name: String,
        /// What to make.
        format: crate::RecordingExportFormat,
        /// The export's name; the recording's when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        export_name: Option<String>,
        /// Write the files; a preview without.
        write: bool,
        /// Replace a flow of the same name: the person said so in the dialog.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        overwrite: bool,
    },
    /// The Flows panel (RB-090-60): the run going on and the last result; answers `flowRun`.
    GetFlowRun,
    /// Push every step of the recording that runs again (a panel catching up, RB-090-89); `ack`.
    ResendRecordingSteps,
    /// A pulldown of the toolbar (RB-090-41): `rows` open as a native menu under `anchor`, the
    /// button's rectangle (DIP from the shell view's top-left corner), in front of the views
    /// and painted in rBrowser's theme. Answered with `ack`; once the menu closed the core
    /// pushes `nativeMenuClosed` with `menu` and the row chosen.
    NativeMenu {
        /// What the shell knows the menu by; comes back in `nativeMenuClosed`.
        menu: String,
        /// The button the menu hangs from.
        anchor: MapRect,
        /// The rows, in order.
        rows: Vec<NativeMenuRow>,
    },
    /// The activity bar (RB-090-67): the shell page asks for its room at `place` while it has
    /// entries to show, without `place` it gives the room back. The core makes the toolbar's or
    /// the status bar's view that much higher and the views smaller, and names the place in the
    /// state, where the page draws it. `ack`; refused for a place the settings do not name, or
    /// any while `ui.activityBar.mode` is `indicator`.
    ActivityBar {
        /// `top` or `bottom`; absent: no bar.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        place: Option<rb_config::ActivityBarPosition>,
    },
    /// Folds an entry of the activity bar into its toolbar indicator (`collapsed`) or opens it
    /// again (RB-090-67); the state names the folded ones for every page. Answered with `ack`.
    CollapseActivity {
        /// The entry.
        activity: crate::Activity,
        /// Folded or open.
        collapsed: bool,
    },
    /// The sitemap gallery page's run (RB-090-86); answered with `sitemapGallery`.
    GetSitemapGallery(crate::SitemapGalleryRequest),
    /// The setup wizard's requests (RB-091-05), from the pages window only.
    Setup {
        /// The request; answered with `setup` or `ack`.
        request: crate::SetupRequest,
    },
    /// The bug report's preview page and the status bar's "Copy Markdown" (RB-093-04); answered
    /// with `bugReport` or `ack`. Only the shell sends this, never MCP; a decision only the pages
    /// window.
    BugReport {
        /// The request.
        request: crate::BugReportRequest,
    },
    /// The MCP setup of the wizard and Settings > Automation > MCP (RB-091-09), from the pages
    /// window only; answered with `ack`, the `mcpSetup` event follows.
    McpSetup {
        /// The request.
        request: crate::McpSetupRequest,
    },
    /// Per environment whether a Basic auth credential is stored, and its user - never the
    /// password (RB-094-29); answered with `basicAuthStatus`.
    GetBasicAuthStatus,
}
