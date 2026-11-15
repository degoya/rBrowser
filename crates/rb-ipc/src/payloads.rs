//! The payloads of the shell <-> core messages (`messages/`): what the answers and events
//! carry. Mirrored in `ui/src/ipc/protocol.ts` like the messages.

use std::collections::BTreeMap;

use rb_commands::{Group, McpCoverage};
use rb_config::trust::BlockedItem;
use serde::{Deserialize, Serialize};

/// Whether a binding may be recorded for a command.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BindingCheck {
    /// The binding fits.
    pub ok: bool,
    /// Why not: the i18n key (`shortcuts.reserved.*`, `shortcuts.rejected.*`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason_key: Option<String>,
    /// The command that holds the binding already.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflict: Option<String>,
}

/// A notification.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    /// Severity.
    pub level: NotificationLevel,
    /// The i18n key of the text.
    pub message_key: String,
    /// Values for the text's `{name}` placeholders.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub args: BTreeMap<String, String>,
    /// A setting to link to (opens `rbrowser://settings/<key>`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings_key: Option<String>,
}

/// The severity of a notification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NotificationLevel {
    /// Information.
    Info,
    /// Something to look at.
    Warning,
    /// Something failed.
    Error,
    /// Something finished well (the status bar, RB-080-15).
    Success,
}

/// A workspace-trust decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TrustDecision {
    /// Trust the project: headers, extensions, plugins and flows apply.
    Trust,
    /// Open it restricted: nothing trust-gated applies.
    Restricted,
}

/// What an extension prompt asks for (RB-080-23).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExtensionPromptKind {
    /// An MCP client asks to install an extension from the Web Store.
    Install,
    /// The Web Store has a newer version of an installed extension.
    Update,
    /// An MCP client asks to remove an extension installed from the Web Store (RB-080-30).
    Remove,
}

/// An extension installed from the Chrome Web Store (RB-080-30): the event
/// `webStoreExtensions` lists them for the settings' list with its Remove button.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebStoreExtension {
    /// The extension id.
    pub id: String,
    /// The manifest's name.
    pub name: String,
    /// The active version.
    pub version: String,
    /// Whether this start loaded it (a removal shows at the next start then).
    pub loaded: bool,
}

/// The question in the window before an extension from the Web Store is installed or updated
/// (RB-080-23) or removed (RB-080-30): the person's `extensionDecision` is the only way it goes
/// on. `name` and `version` are the downloaded, verified CRX's (a removal: the installed one's);
/// `currentVersion` the installed one's at an update.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionPrompt {
    /// Echoed in the `extensionDecision`.
    pub prompt_id: u32,
    /// Install, update or remove.
    pub kind: ExtensionPromptKind,
    /// The extension id.
    pub id: String,
    /// The manifest's name.
    pub name: String,
    /// The version that would be installed.
    pub version: String,
    /// The version installed now (an update).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_version: Option<String>,
}

/// The trust question for a project (RB-010-09): the banner lists `blocked`, the dialog
/// details them (with a project plugin's capabilities, looked up by its path).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustPrompt {
    /// Echoed in the `trustDecision`.
    pub prompt_id: u32,
    /// The project root.
    pub project_root: String,
    /// What stays blocked until the person trusts the project.
    pub blocked: Vec<BlockedItem>,
}

/// Version information (RB-010-18). Absent engines are not installed or not started yet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutInfo {
    /// rBrowser's version.
    pub version: String,
    /// The git commit of the build.
    pub commit: String,
    /// The CEF version.
    pub cef: String,
    /// The Chromium version.
    pub chromium: String,
    /// The installed Firefox's version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub firefox: Option<String>,
    /// Playwright's WebKit build.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub webkit: Option<String>,
    /// The Node runtime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
    /// The build time, UTC; absent when unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub built: Option<String>,
    /// Operating system and architecture, `windows x86_64`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    /// The installed Firefox's executable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub firefox_path: Option<String>,
    /// API versions by name: `plugin` (`rbrowser:plugin@0.1.0`), `configSchema`, later `mcp`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub apis: BTreeMap<String, String>,
    /// Folders by name: `data`, `local`, `logs`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dirs: BTreeMap<String, String>,
}

/// The answer of `getTrust` / `revokeTrust` (RB-010-09).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustInfo {
    /// The open project's trust; `None` without a project.
    pub current: Option<TrustStatus>,
    /// Every trusted project, for Settings > Security > Workspace trust.
    pub projects: Vec<TrustedProject>,
}

/// The trust of the open project.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustStatus {
    /// The project root.
    pub project_root: String,
    /// Trusted in its current form.
    pub trusted: bool,
    /// Its protected parts; blocked while not trusted.
    pub blocked: Vec<BlockedItem>,
    /// The capabilities each project plugin's manifest declares (`views`, `http`, ...), by its
    /// blocked item's path: what the dialog shows before the person trusts (Charge 6).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub plugin_capabilities: BTreeMap<String, Vec<String>>,
    /// The details of what each project plugin's manifest asks for, by the same path: the
    /// dialog shows them and highlights the risky ones (review finding M1 of Charge 6).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub plugin_grants: BTreeMap<String, PluginGrantDetails>,
}

/// What a project plugin's manifest asks for in detail, read without trusting it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginGrantDetails {
    /// `evaluate`: scripts in the views - full control of the page, its network included.
    pub evaluate: bool,
    /// `cdp.domains`.
    pub cdp_domains: Vec<String>,
    /// `project-read.globs`.
    pub project_globs: Vec<String>,
    /// `sidecar.methods`.
    pub sidecar_methods: Vec<String>,
    /// `http.rate_limit_per_second`; `None` without `http`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_rate_per_second: Option<u32>,
    /// What the dialog highlights: `evaluate`, `cdp:<Domain>` for a domain that controls the
    /// page or reads its traffic, and the capabilities `network-rules` and `console-logs`.
    pub risky: Vec<String>,
}

/// A trusted project of the trust store.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustedProject {
    /// The project root.
    pub root: String,
    /// When it was trusted, RFC 3339 UTC.
    pub decided_at: String,
}

/// What `pickPath` lets the person choose.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PathKind {
    /// An existing file (an executable).
    File,
    /// An existing folder.
    Folder,
}

/// Why a field was not saved (`settingRejected`, RB-010-08).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingIssue {
    /// JSON pointer below the key (`""` for the key itself, `/2/width` in a list).
    pub path: String,
    /// The i18n key of the reason (`config.error.*`, `settings.rejected.*`).
    pub message_key: String,
}

/// A breakpoint of the config, as the shell lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BreakpointSummary {
    /// The id.
    pub id: String,
    /// The label (the id when the config has none).
    pub label: String,
    /// Width in CSS px, resolved: the breakpoint's own, else its device preset's viewport width
    /// (RB-010-12); `0` only for a breakpoint whose size does not resolve.
    pub width: u32,
    /// Height in CSS px, resolved as `width`.
    pub height: u32,
    /// The device preset it references.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
}

/// A running recording (RB-050-01).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecorderStatus {
    /// Recording or paused.
    pub phase: RecorderPhase,
    /// The recording's name.
    pub name: String,
    /// The steps so far.
    pub steps: u32,
}

/// Whether a recording takes steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RecorderPhase {
    /// Steps are recorded.
    Recording,
    /// Paused: no steps until it resumes.
    Paused,
}

/// Open DevTools (RB-010-06).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevtoolsState {
    /// The inspected view.
    pub view: rb_engine::ViewId,
    /// Docked or a window.
    pub mode: rb_config::DevtoolsMode,
}

/// A project bookmark as the toolbar and the palette list it (RB-070-06).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkSummary {
    /// The id.
    pub id: String,
    /// The label.
    pub label: String,
    /// The group of the toolbar list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Where it leads in the current environment; `None` for a path with no environment or
    /// start URL to put it after.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Its direct shortcut, as the config writes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shortcut: Option<String>,
    /// The path of a bookmark that follows the environment, as the config writes it; `None`
    /// for an absolute one (RB-080-48). The list shows it, `url` is where it leads now.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// A session of the project (RB-020-07): `default` first, then `sessions` in config order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    /// The session id.
    pub id: String,
    /// The badge colour `#rrggbb`; `None` for `default`, which shows no badge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/// Which side of a media-query boundary a detected breakpoint is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BoundaryKind {
    /// `min-width` (or `width >=`): the layout changes at `width`.
    Min,
    /// `max-width` (or `width <=`): the layout changes at `width` + 1.
    Max,
}

/// A CSS breakpoint the analyser found (RB-010-13).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedBreakpoint {
    /// The boundary in CSS px, after merging (`analyzer.mergeTolerancePx`).
    pub width: u32,
    /// `min-width` or `max-width`.
    pub kind: BoundaryKind,
    /// The framework's name for it (`md`), when one was recognised.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// How many media queries use it.
    pub count: u32,
    /// The project file that declares it and its name there (`tailwind.config.js: md`), from
    /// the static source (RB-030-07).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Declared only by a variable's name (`$bp-md`): the weaker, heuristic source.
    #[serde(default)]
    pub heuristic: bool,
}

/// Something the analyser's static source could not read (RB-030-07): a call, a variable, a
/// range or a raw query in a project file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzerHint {
    /// The project file, relative to the root.
    pub path: String,
    /// The plugin's i18n key (`plugin.breakpoint-analyzer.hints.dynamic`).
    pub key: String,
    /// Its parameters (`path`, `name`).
    #[serde(default)]
    pub params: BTreeMap<String, String>,
}

/// The analyser's proposal, the wizard's content (RB-010-13).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BreakpointProposal {
    /// Echoed in `applyBreakpoints`.
    pub proposal_id: u32,
    /// The project has breakpoints already: the wizard shows only the new ones as a diff.
    pub existing: bool,
    /// The boundaries found, by width.
    pub detected: Vec<DetectedBreakpoint>,
    /// The stylesheets read, with the number of media queries in each.
    pub sources: Vec<StylesheetSource>,
    /// The proposed breakpoints, by width.
    pub breakpoints: Vec<ProposedBreakpoint>,
    /// What the static source could not read (RB-030-07).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hints: Vec<AnalyzerHint>,
}

/// One stylesheet the analyser read.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StylesheetSource {
    /// The stylesheet's URL (inline sheets: the page URL with `#inline-<n>`).
    pub url: String,
    /// Media queries in it.
    pub count: u32,
}

/// One proposed breakpoint.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposedBreakpoint {
    /// The id it gets (`md`, `w768`).
    pub id: String,
    /// The label.
    pub label: String,
    /// Width in CSS px.
    pub width: u32,
    /// Height in CSS px.
    pub height: u32,
    /// Not in the config yet.
    pub is_new: bool,
}

/// What a project's `checks` section turned off over the user's settings, named in the findings
/// panel: a checked-in config may hide findings (RB-060-01, batch-2 security review).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectOff {
    /// The rules the project switched off that the user's settings would run, sorted.
    pub rules: Vec<String>,
    /// The project's ignore entries, which replace the user's.
    pub ignore: Vec<rb_config::IgnoreEntry>,
}

impl ProjectOff {
    /// Whether the project turned nothing off.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty() && self.ignore.is_empty()
    }

    /// How many rules and ignore entries.
    #[must_use]
    pub fn count(&self) -> usize {
        self.rules.len() + self.ignore.len()
    }
}

/// One command for the palette, the which-key overlay and the shortcut editor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandInfo {
    /// The command id.
    pub id: String,
    /// The i18n key of its title.
    pub title_key: String,
    /// The leader group.
    pub group: Group,
    /// The effective bindings in text form.
    pub bindings: Vec<String>,
    /// The default bindings in text form (what "reset" restores).
    pub default_bindings: Vec<String>,
    /// Whether the user changed the bindings.
    pub changed: bool,
    /// The MCP mapping: `{"covered": "<tool>"}` or `{"omitted": "<reason>"}`.
    pub mcp: McpCoverage,
    /// Default bindings left unbound because they were reserved or taken (a plugin's,
    /// `shortcuts.notice.binding_dropped`); the editor names them (RB-060-11).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dropped_defaults: Vec<String>,
}
