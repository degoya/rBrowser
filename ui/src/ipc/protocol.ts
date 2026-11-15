// The shell UI <-> core protocol, contract 0.1 - the TypeScript mirror of `crates/rb-ipc`
// (`src/messages/`) and the types it embeds from rb-engine, rb-config, rb-settings and
// rb-commands. Types only. Changed only by the coordinator, together with the Rust side; the
// JSON shape is pinned by `crates/rb-ipc/src/lib_tests.rs`.
//
// Transport: shell -> core with `window.cefQuery({ request: JSON.stringify(msg), onSuccess,
// onFailure })`; the answer is a JSON `CoreToShell` (`{ type: "ack" }` when empty) or
// `onFailure(code: IpcErrorCode, message)`. Core -> shell as
// `window.dispatchEvent(new CustomEvent("rb-ipc", { detail: msg }))`.
// Wave 0.1-w1b added the messages and fields its jobs share (docs/contracts-0.1-w1b.md).

import type { CheckTarget, ChecksProgress, FindingsRun, ProjectOff } from './findings';
import type { SyncFeature, SyncState } from './sync';
import type { BoardState, CanvasRequest, ModeState } from './layout';
import type { FlowSummary, ProjectShortcut } from './project';

export type * from './findings';
export type * from './sync';
export type * from './project';
import type { ExternalView } from './externals';

export type { ExternalStatus, ExternalView, WindowRect } from './externals';
import type { RecorderStatus } from './recorder';
import type { SitemapStatus } from './sitemap';
import type { PanelMessage, PanelRequest } from '../panels/protocol';

export type { RecorderPhase, RecorderStatus } from './recorder';
export type { SitemapRunState, SitemapStatus } from './sitemap';
import type { DevicePreset, Emulation } from './emulation';
import type { ViewFrameworks } from './frameworks';
import type { AboutInfo } from './about';

export type * from './about';
import type { ViewMetrics, VitalMetric } from './metrics';

export type * from './frameworks';
export type * from './metrics';
import type { Notification, StatusMessage } from './notifications';

export type * from './notifications';
export type * from './portal';
import type { ExtensionEvent, ExtensionRequest } from './extensions';
import type { SettingsEvent, SettingsRequest } from './settingsMessages';
export type { SettingIssue, SettingsEvent, SettingsRequest, SettingValue } from './settingsMessages';
export type { ExtensionPrompt, ExtensionPromptKind, WebStoreExtension } from './extensions';

export type * from './emulation';

export const PROTOCOL_VERSION = 1;
export const EVENT_NAME = 'rb-ipc';

/** `onFailure` codes. */
export const IpcErrorCode = {
  BadRequest: 400,
  Refused: 403,
  NotFound: 404,
  Internal: 500,
  NotImplemented: 501,
} as const;
export type IpcErrorCode = (typeof IpcErrorCode)[keyof typeof IpcErrorCode];

// ---- shared types -------------------------------------------------------------------------

/** rb-engine `ViewId`. */
export type ViewId = number;
/** rb-config `EngineKind`. */
export type EngineKind = 'chromium' | 'firefox' | 'webkit';
/** rb-settings `Scope`. */
export type Scope = 'user' | 'project' | 'local';
/** rb-settings `Origin`: the badge of a field. */
export type Origin = 'default' | 'user' | 'project' | 'local';
/** rb-config `LayoutMode`. */
export type LayoutMode = 'grid' | 'focus' | 'horizontal' | 'split' | 'canvas';
/** rb-config `Zoom`. */
export type Zoom = 'fit' | number;
/** rb-commands `Group`. */
export type Group =
  | 'breakpoints'
  | 'layout'
  | 'sync'
  | 'environments'
  | 'devtools'
  | 'overlays'
  | 'capture'
  | 'checks'
  | 'reload'
  | 'panels'
  | 'flows'
  | 'view'
  | 'plugins'
  | 'settings';

/** rb-engine `CellRect`: DIP in the main window's client area. */
export interface CellRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** rb-engine `GridCell`. */
export interface GridCell {
  breakpoint: string;
  col: number;
  row: number;
  colSpan: number;
  rowSpan: number;
  zoom: Zoom;
  rect: CellRect;
  /** The strip in the gap for label, size and badges. */
  label: CellRect;
}

/** rb-engine `ViewInfo`. */
export interface ViewInfo {
  id: ViewId;
  label: string;
  breakpoint: string | null;
  cellBreakpoint: string | null;
  engine: EngineKind;
  width: number;
  height: number;
  dpr: number;
  scale: number;
  url: string;
  title: string;
  loading: boolean;
  session: string;
  cell: GridCell | null;
}

/** rb-config `Environment`. */
export interface Environment {
  id: string;
  url: string;
  color?: string;
}

export type TrustDecision = 'trust' | 'restricted';

/** rb-config `DevtoolsMode`. */
export type DevtoolsMode = 'docked-bottom' | 'docked-right' | 'window';
/** rb-config `AnalyzerMode`. */
export type AnalyzerMode = 'all' | 'simplified' | 'edges';
/** rb-config `trust::ProtectedKind` (RB-010-09). */
export type ProtectedKind =
  | 'flows'
  | 'snippets'
  | 'plugins'
  | 'mocks'
  | 'headers'
  | 'hostResolverRules'
  | 'pageScripts'
  | 'extensions'
  | 'ignoreCertificateErrors'
  | 'proxy';

/** rb-config `trust::BlockedItem`: `path` is relative to the root or a JSON pointer. */
export interface BlockedItem {
  kind: ProtectedKind;
  path: string;
}

export type PathKind = 'file' | 'folder';

export type JsonValue =
  | null
  | boolean
  | number
  | string
  | JsonValue[]
  | { [key: string]: JsonValue };

// ---- shell -> core ------------------------------------------------------------------------

export type ShellToCore =
  | { type: 'getState' }
  | { type: 'navigate'; url: string }
  | { type: 'back' }
  | { type: 'forward' }
  | { type: 'reload'; ignoreCache?: boolean }
  | { type: 'setSync'; feature: SyncFeature; enabled: boolean }
  | { type: 'selectLayoutPreset'; id: string }
  | { type: 'selectEnvironment'; id: string }
  | { type: 'setActiveView'; view: ViewId }
  | {
      type: 'setBreakpoint';
      view: ViewId;
      /** Either `breakpoint` ... */
      breakpoint?: string;
      /** ... or `width` and `height` (optionally `dpr`). */
      width?: number;
      height?: number;
      dpr?: number;
    }
  /** The toolbar wraps into `rows` rows (1 or 2, RB-080-52); `ack` when the core made the room. */
  | { type: 'toolbarRows'; rows: 1 | 2 }
  | { type: 'runCommand'; id: string; args?: JsonValue }
  | { type: 'listCommands' }
  | { type: 'openSettings'; key?: string }
  | { type: 'trustDecision'; promptId: number; decision: TrustDecision }
  | { type: 'getAbout' }
  // The shortcut editor (RB-010-17); bindings in text form.
  /** Answered with `commands`; an empty list unbinds the command. */
  | { type: 'setBindings'; id: string; bindings: string[] }
  /** Answered with `commands`; without `id` every command goes back to its defaults. */
  | { type: 'resetBinding'; id?: string }
  /** Answered with `bindingCheck`. */
  | { type: 'checkBinding'; id: string; binding: string }
  /** Answered with `bindingsExport`. */
  | { type: 'exportBindings' }
  /** Answered with `commands`; what was skipped comes as notifications. */
  | { type: 'importBindings'; json: string }
  // Wave 0.1-w1b (docs/contracts-0.1-w1b.md).
  /** Answered with `emulation` (RB-010-05). */
  | { type: 'getEmulation'; view: ViewId }
  /** Answered with `trust` (RB-010-09). */
  | { type: 'getTrust' }
  /** Answered with `trust`; granting is `trustDecision` only (RB-010-09). */
  | { type: 'revokeTrust'; root: string }
  /** Answered with `devices` (RB-010-12). */
  | { type: 'getDevices' }
  /** Answered with `ack`; the result comes as the event `breakpointProposal` (RB-010-13). */
  | { type: 'analyzeBreakpoints'; urls?: string[]; mode?: AnalyzerMode }
  /** Answered with `ack`; writes `config.json` (RB-010-13). */
  | { type: 'applyBreakpoints'; proposalId: number; selected: string[]; replace: boolean }
  // RB-060-01: answered with `ack`, the run follows as `findings`.
  | { type: 'runChecks'; target: CheckTarget; plugins?: string[]; url?: string }
  | { type: 'highlightFinding'; findingId: string; selector: string; views: ViewId[] }
  /** Answered with `ack`: marks the element behind a view's Web Vital (RB-060-10). */
  | { type: 'highlightVital'; view: ViewId; metric: VitalMetric }
  /** Answered with `ack`: the palette's popup hides, the main window has the focus (RB-060-11). */
  | { type: 'dismissPalette' }
  /** Answered with `findings` (the last run), or `ack` before the first run (RB-060-01). */
  | { type: 'getFindings' }
  /** Answered with `flows`: the project's flows for the palette (RB-060-11). */
  | { type: 'listFlows' }
  /** Answered with `projectShortcuts`: the shortcut editor's conflict lines (RB-010-16). */
  | { type: 'listProjectShortcuts' }
  /** A plugin panels window's request; answered with `pluginPanel` (RB-060-12). */
  | { type: 'pluginPanel'; request: PanelRequest }
  // The status bar (RB-080-15): its log as `statusMessages`, oldest first; a message the shell
  // raised itself into the log; the history's popup hides. The last two answer `ack`.
  | { type: 'getStatusMessages' }
  | ({ type: 'reportStatus' } & Notification)
  | { type: 'dismissHistory' }
  | { type: 'getPortal' } // answered with `portal` (RB-080-05)
  /** The extensions' requests (`./extensions.ts`): `extensionDecision`, `listWebStoreExtensions`. */
  | ExtensionRequest
  /** The settings window's requests (`./settingsMessages.ts`). */
  | SettingsRequest
  | CanvasRequest // the canvas board (RB-080-04, `layout.ts`)
  /** Answered with `checksProgress` (the run in progress or the last), or `ack` before the first (RB-080-89). */
  | { type: 'getChecksProgress' }
  /** The status bar's zoom level button: the presets open as the core's native menu with its
   *  bottom centre at `x`/`y` from the status bar's corner, above the bar (RB-090-18). `ack`. */
  | { type: 'canvasZoomMenu'; x: number; y: number };

// ---- core -> shell ------------------------------------------------------------------------

export interface ProjectInfo {
  name: string;
  root: string;
  trusted: boolean;
}

export interface LayoutState {
  preset: string | null;
  presets: string[];
  mode: LayoutMode;
  /** Focus mode: the breakpoint shown (RB-010-12). */
  focus: string | null;
  /** Horizontal, split and canvas: the toolbar's controls (RB-080-04, `layout.ts`). */
  modes?: ModeState;
  /** The hidden breakpoints with a cell to open (`layout.hidden`, RB-080-41); absent for none. */
  hidden?: string[];
}

/** A breakpoint of the config as the shell lists it. */
export interface BreakpointSummary {
  id: string;
  label: string;
  width: number;
  height: number;
  device?: string;
}

/** rb-ipc `SessionInfo`: a session of the project (RB-020-07). */
export interface SessionInfo {
  id: string;
  /** The badge colour `#rrggbb`; absent for `default`, which shows no badge. */
  color?: string;
}

/** A CSS breakpoint the analyser found (RB-010-13). */
export interface DetectedBreakpoint {
  width: number;
  /** `min-width` or `max-width`. */
  kind: 'min' | 'max';
  label?: string;
  count: number;
  /** The project file that declares it and its name there (`tailwind.config.js: md`), RB-030-07. */
  source?: string;
  /** Declared only by a variable's name (`$bp-md`): the weaker, heuristic source. */
  heuristic: boolean;
}

/** Something the analyser's static source could not read (RB-030-07). */
export interface AnalyzerHint {
  /** The project file, relative to the root. */
  path: string;
  /** The plugin's i18n key (`plugin.breakpoint-analyzer.hints.dynamic`). */
  key: string;
  params: Record<string, string>;
}

/** A project bookmark as the toolbar and the palette list it (RB-070-06). */
export interface BookmarkSummary {
  id: string;
  label: string;
  group?: string;
  /** Where it leads in the current environment; absent for a path with nothing to lie under. */
  url?: string;
  /** Its direct shortcut, as the config writes it. */
  shortcut?: string;
  /** The path of a bookmark that follows the environment; absent for an absolute one (RB-080-48). */
  path?: string;
}

/** Open DevTools (RB-010-06). */
export interface DevtoolsState {
  view: ViewId;
  mode: DevtoolsMode;
}

export interface StateSnapshot {
  project: ProjectInfo | null;
  views: ViewInfo[];
  activeView: ViewId | null;
  url: string;
  environment: string | null;
  environments: Environment[];
  layout: LayoutState;
  sync: SyncState;
  breakpoints: BreakpointSummary[];
  /** The project's sessions, `default` first (RB-020-07). */
  sessions: SessionInfo[];
  detected: DetectedBreakpoint[];
  devtools: DevtoolsState | null;
  /** The views on Firefox or WebKit; absent when none or the tiles are off (RB-040-01). */
  externals?: ExternalView[];
  /** The running recording; absent while none runs (RB-050-01). */
  recorder?: RecorderStatus;
  /** The project's bookmarks with their URLs in the current environment (RB-070-06). */
  bookmarks: BookmarkSummary[];
  /** The window's last sitemap capture; absent before the first (RB-050-05). */
  sitemap?: SitemapStatus;
  /** The Web Vitals and resources per view; absent while both are off (RB-060-10). */
  metrics?: ViewMetrics[];
  /** The frameworks each view's page runs, for the DevTools offer; absent while none is found
   *  or both are off (RB-080-24). */
  frameworks?: ViewFrameworks[];
}

export interface CommandInfo {
  id: string;
  titleKey: string;
  group: Group;
  /** Text form: `Ctrl+Shift+1`, `Ctrl+Shift+Space L 1`. */
  bindings: string[];
  /** What "reset" restores. */
  defaultBindings: string[];
  /** The user changed the bindings. */
  changed: boolean;
  mcp: McpCoverage;
  /** Default bindings left unbound, reserved or taken (rb-commands `NOTICE_BINDING_DROPPED`). */
  droppedDefaults?: string[];
}

/** rb-commands `McpCoverage`: the MCP tool, or why there is none. */
export type McpCoverage = { covered: string } | { omitted: string };

/** rb-commands `WhichKeyEntry`: a next key after the leader. */
export type WhichKeyEntry =
  | { kind: 'group'; key: string; group: Group; titleKey: string }
  | { kind: 'command'; key: string; id: string; titleKey: string };

/** rb-commands `WhichKey`: what was typed and what each next key does. */
export interface WhichKey {
  typed: string;
  entries: WhichKeyEntry[];
}

export interface BindingCheck {
  ok: boolean;
  /** i18n key of the reason (`shortcuts.reserved.*`, `shortcuts.rejected.*`). */
  reasonKey?: string;
  /** The command holding the binding. */
  conflict?: string;
}

export interface TrustPrompt {
  promptId: number;
  projectRoot: string;
  blocked: BlockedItem[];
}

export interface TrustStatus {
  projectRoot: string;
  trusted: boolean;
  blocked: BlockedItem[];
  /** The capability ids each project plugin package declares, by its blocked item's path. */
  pluginCapabilities?: Record<string, string[]>;
  /** What each project plugin package asks for in detail, by the same path. */
  pluginGrants?: Record<string, PluginGrantDetails>;
}

/** A project plugin's grants in detail, for the trust dialog (review M1 of Charge 6). */
export interface PluginGrantDetails {
  /** Runs scripts in the views: full control of the page, its network included. */
  evaluate: boolean;
  cdpDomains: string[];
  projectGlobs: string[];
  sidecarMethods: string[];
  /** Requests per second; absent without `http`. */
  httpRatePerSecond?: number;
  /** What the dialog highlights: `evaluate`, `cdp:<Domain>`, `network-rules`, `console-logs`. */
  risky: string[];
}

export interface TrustInfo {
  current: TrustStatus | null;
  projects: { root: string; decidedAt: string }[];
}

export interface BreakpointProposal {
  proposalId: number;
  existing: boolean;
  detected: DetectedBreakpoint[];
  sources: { url: string; count: number }[];
  breakpoints: { id: string; label: string; width: number; height: number; isNew: boolean }[];
  /** What the static source could not read (RB-030-07). */
  hints?: AnalyzerHint[];
}

export type CoreToShell =
  | { type: 'ack' }
  | ({ type: 'state' } & StateSnapshot)
  | {
      type: 'commands';
      leader: string;
      commands: CommandInfo[];
      /** The titles of the plugin commands from their plugins' catalogues, by key (RB-080-82). */
      texts?: Record<string, string>;
    }
  | ({ type: 'notification' } & Notification)
  | ({ type: 'trustPrompt' } & TrustPrompt)
  | ({ type: 'about' } & AboutInfo)
  /** A shell-side command the core resolved (`view.focus_address_bar`, the palette, ...). */
  | { type: 'uiCommand'; id: string }
  /** The canvas board for a camera a gesture is trying out; the next `state` is the truth. */
  | { type: 'canvasLive'; board: BoardState }
  | ({ type: 'whichKey' } & WhichKey)
  | { type: 'whichKeyHide' }
  | ({ type: 'bindingCheck' } & BindingCheck)
  | { type: 'bindingsExport'; json: string }
  // Wave 0.1-w1b (docs/contracts-0.1-w1b.md).
  | { type: 'emulation'; view: ViewId; emulation: Emulation }
  | ({ type: 'trust' } & TrustInfo)
  | { type: 'devices'; items: DevicePreset[] }
  | ({ type: 'breakpointProposal' } & BreakpointProposal)
  /** A finished check run and the texts of its plugin message keys (RB-060-01). */
  | {
      type: 'findings';
      run: FindingsRun;
      messages: Record<string, string>;
      projectOff?: ProjectOff;
      /** The run so far, while it goes (RB-080-89). */
      partial?: boolean;
    }
  /** A check run's progress, while it goes and when it ends; the answer to `getChecksProgress`. */
  | ({ type: 'checksProgress' } & ChecksProgress)
  | { type: 'flows'; items: FlowSummary[] }
  | { type: 'projectShortcuts'; items: ProjectShortcut[] }
  /** The answer to `pluginPanel`, or an event for the plugin panels (RB-060-12). */
  | { type: 'pluginPanel'; message: PanelMessage }
  /** Every notification for the status bar, beside `notification` (RB-080-15). */
  | ({ type: 'statusMessage' } & StatusMessage)
  /** The answer to `getStatusMessages`: the log, oldest first. */
  | { type: 'statusMessages'; items: StatusMessage[] }
  | ({ type: 'portal' } & import('./portal').PortalState) | { type: 'portalChanged' } // RB-080-05
  /** The extensions' events and answers (`./extensions.ts`): `extensionPrompt`, `webStoreExtensions`. */
  | ExtensionEvent
  /** The settings window's answers and events (`./settingsMessages.ts`). */
  | SettingsEvent;
