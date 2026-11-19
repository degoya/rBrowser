// The sync agent's protocol, version 1 (RB-020-01): what the agent in a view's isolated world
// reports to the core over the binding, the commands the core evaluates in that world, and the
// agent's configuration. The Rust mirror is `crates/rb-sync/src/event.rs`; both are held to
// `protocol-fixtures.json` (`protocol.test.ts`, `crates/rb-sync/tests/protocol_fixtures.rs`).
// The contract, with the rules around it, is `docs/sync-protocol.md`.

/** Raised on every incompatible change; the core refuses messages of another version. */
export const PROTOCOL_VERSION = 1;
/** The isolated world the agent runs in (`Page.addScriptToEvaluateOnNewDocument.worldName`). */
export const WORLD_NAME = 'rbrowser';
/** The binding the agent reports through, visible only in that world (`Runtime.addBinding`). */
export const BINDING_NAME = '__rbEmit';
/** The agent's handle in its world: `globalThis.__rbAgent.command(<AgentCommand>)`. */
export const AGENT_GLOBAL = '__rbAgent';
/** The boot configuration the core puts in front of the bundle. */
export const BOOT_GLOBAL = '__rbAgentBoot';

/** The page API in the page's main world (RB-020-08): `window.__rbrowser`. */
export const PAGE_API_GLOBAL = '__rbrowser';
/** The parameter the core wraps the main-world bundle in: `(function(__rbPageBoot){...})(<PageState>)`. */
export const PAGE_BOOT_PARAM = '__rbPageBoot';
/** Main world -> agent: a bus message, `detail` the JSON text `{ type, payload }`. */
export const BUS_OUT_EVENT = '__rbrowser:bus-out';
/** Agent -> main world: a bus message of another view, `detail` the JSON text `{ type, payload, from }`. */
export const BUS_IN_EVENT = '__rbrowser:bus-in';
/** Agent -> main world: the view changed, `detail` the JSON text of the `PageState`. */
export const VIEW_EVENT = '__rbrowser:view';
/** The page's event on `window` when the view info changed; `detail` is the new `window.__rbrowser`. */
export const VIEWCHANGE_EVENT = 'rbrowser:viewchange';
/** Main world -> agent: the framework probe's report (RB-080-24), `detail` the JSON text of a `FrameworkReport`. */
export const FRAMEWORK_EVENT = '__rbrowser:framework';
/** The frameworks whose DevTools rBrowser brings (RB-080-24). */
export type Framework = 'react' | 'vue';
/** Main world -> plugin page script: a message of a framework's DevTools backend, `detail` the JSON text of a `DevtoolsEnvelope`. */
export const devtoolsOutEvent = (framework: Framework): string => `__rbrowser:${framework}-devtools-out`;
/** Plugin page script -> main world: a message for a framework's DevTools backend, `detail` the JSON text of a `DevtoolsEnvelope`. */
export const devtoolsInEvent = (framework: Framework): string => `__rbrowser:${framework}-devtools-in`;
/** The marker rBrowser's DevTools hooks set on `__REACT_DEVTOOLS_GLOBAL_HOOK__` and `__VUE_DEVTOOLS_GLOBAL_HOOK__`. */
export const HOOK_MARKER = 'rbrowser';

/** CDP modifier bits, as `Input.dispatchMouseEvent` / `Input.dispatchKeyEvent` take them. */
export const MOD_ALT = 1;
export const MOD_CTRL = 2;
export const MOD_META = 4;
export const MOD_SHIFT = 8;

/**
 * How an element is found again in another view, tried in this order when it is made:
 * `data-testid`, ARIA role and accessible name, tag and text, CSS path. `nth` picks among
 * several matches (document order, omitted for the first); `path` is the CSS path, the
 * fallback of every other kind. A role or text selector with an `nth` carries `visibleNth`,
 * its index among the visible matches, when the element is shown (RB-090-81). `within` is the
 * CSS selector of an ancestor inside which a role or text selector names the element alone,
 * when the page shows the name more than once (RB-090-84); the whole page where it is missing.
 */
export type ElementSelector =
  | { kind: 'testid'; testid: string; nth?: number; path: string }
  | {
      kind: 'role';
      role: string;
      name: string;
      nth?: number;
      visibleNth?: number;
      within?: string;
      path: string;
    }
  | { kind: 'text'; tag: string; text: string; nth?: number; visibleNth?: number; within?: string; path: string }
  | { kind: 'css'; path: string };

/** A point in an element: its selector, the open shadow hosts above it and the offset. */
export interface ElementTarget {
  /** The open shadow hosts from the document inward, each relative to the root before it. */
  hosts?: ElementSelector[];
  element: ElementSelector;
  /** The point in the element's border box, each axis 0 to 1. */
  offset: { x: number; y: number };
}

/** A mouse button, named as CDP names it. */
export type MouseButton = 'none' | 'left' | 'middle' | 'right' | 'back' | 'forward';

/** `click`, `dblclick` and `contextmenu`. */
export interface MouseData {
  button: MouseButton;
  /** The click count (`UIEvent.detail`). */
  detail: number;
  modifiers: number;
  target: ElementTarget;
}

/** `keydown` and `keyup`. */
export interface KeyData {
  key: string;
  code: string;
  /** `KeyboardEvent.keyCode`, the `windowsVirtualKeyCode` of CDP. */
  keyCode: number;
  location: number;
  repeat: boolean;
  modifiers: number;
  /** A key in a password field: never logged, never shown to MCP or the recorder in clear. */
  sensitive?: boolean;
  /** The focused element; omitted when it is the body. */
  target?: ElementTarget;
}

/**
 * Where the document or a scroll container stands (RB-020-04). The pixel position alone does not
 * carry over to another width - the same content is taller in a narrow view - so the anchor
 * says which content sits at the container's top-left edge.
 */
export interface ScrollData {
  /** The scrolled element; omitted for the document. */
  container?: ElementTarget;
  /**
   * The element at the container's top edge and the point of it that sits at the top-left
   * corner of the container's visible area: `offset.y` is the share of its height above the
   * edge (0 to 1), `offset.x` the share of its width left of it (may lie outside 0 to 1 when the
   * element does not reach the left edge). Omitted when no element qualifies.
   */
  anchor?: ElementTarget;
  /** `scrollLeft`. */
  x: number;
  /** `scrollTop`. */
  y: number;
  /** The largest `scrollLeft`. */
  maxX: number;
  /** The largest `scrollTop`. */
  maxY: number;
}

/**
 * A form control's state after `change` (RB-020-05): what the other views set (`apply`). A file
 * input carries none of the values: it is never synced.
 */
export interface ChangeData {
  /** The value: of an input, a text area or a select; the text of a contenteditable. */
  value?: string;
  /** A checkbox's or radio's state. */
  checked?: boolean;
  /** The selected option indexes of a `<select>`. */
  selected?: number[];
  /** A password field: never logged, never shown to MCP or the recorder in clear. */
  sensitive?: boolean;
  target: ElementTarget;
}

/** One captured event. Coordinates are CSS pixels of the view's (emulated) page. */
export type SyncEvent =
  | {
      type: 'pointer';
      phase: 'down' | 'up';
      button: MouseButton;
      buttons: number;
      pointerType: string;
      modifiers: number;
      target: ElementTarget;
    }
  | ({ type: 'click' } & MouseData)
  | ({ type: 'dblclick' } & MouseData)
  | ({ type: 'contextmenu' } & MouseData)
  | {
      type: 'hover';
      target: ElementTarget;
      /** Not a move: the hover on a menu entry at a press into its hover-only submenu
       *  (`menu-trigger.ts`, RB-090-76). */
      menu?: true;
    }
  | {
      type: 'wheel';
      deltaX: number;
      deltaY: number;
      deltaMode: number;
      modifiers: number;
      target: ElementTarget;
    }
  | ({ type: 'scroll' } & ScrollData)
  | ({ type: 'keydown' } & KeyData)
  | ({ type: 'keyup' } & KeyData)
  | {
      type: 'input';
      inputType: string;
      data?: string;
      /** The field's value after the input (inputs and text areas, not file inputs). */
      value?: string;
      /** A password field: never logged, never shown to MCP or the recorder in clear. */
      sensitive?: boolean;
      target: ElementTarget;
    }
  | ({ type: 'change' } & ChangeData)
  | { type: 'submit'; method: string; action: string; target: ElementTarget }
  | { type: 'focus'; target: ElementTarget };

/** Where a message comes from: the top frame, or a frame with its path when it is known. */
interface Origin {
  v: number;
  /** Whether the agent runs in the top frame. */
  top: boolean;
  /** The CSS paths of the iframes from the top document down, when every hop is same-origin. */
  frame?: string[];
}

/** A JSON value: what a bus payload may be. */
export type JsonValue =
  | null
  | boolean
  | number
  | string
  | JsonValue[]
  | { [key: string]: JsonValue };
/** A tool in the page that syncs the views by itself (RB-020-06). */
export type DetectedTool = 'browsersync';

/** What the main-world probe found of a framework (RB-080-24). */
export interface FrameworkInfo {
  /** The framework's version, when it says (Vue's `app.version`, a React renderer's `version`). */
  version?: string;
  /** rBrowser's DevTools hook is in this document, so the panel can connect; else a reload is needed. */
  hook: boolean;
  /** The framework talks to DevTools: false for a Vue production build without `__VUE_PROD_DEVTOOLS__`
   *  and for React loaded before any hook. */
  devtools: boolean;
}

/** The frameworks of the top document (RB-080-24); a framework not in use is absent. */
export interface FrameworkReport {
  vue?: FrameworkInfo;
  react?: FrameworkInfo;
}

/** A message between a framework's DevTools backend (main world) and its panel (RB-080-24). */
export type DevtoolsEnvelope =
  /** The panel is listening: the backend may start. */
  | { t: 'connect' }
  /** The panel is gone: the backend stops. */
  | { t: 'disconnect' }
  /** A message of the backend's or frontend's own protocol. */
  | { t: 'm'; d: JsonValue };

/** A Web Vital the agent measures (RB-060-10). */
export type VitalMetric = 'lcp' | 'cls' | 'inp' | 'fcp' | 'ttfb';

/**
 * The Web Vitals of the top document's current segment (RB-060-10): a document load starts
 * segment 0, a soft navigation (a new path through the history API) the next one, where only CLS
 * and INP are measured. Times in ms; a metric is absent until measured; `unsupported` names what
 * the engine cannot measure. Selectors are CSS paths (`selector.ts`).
 */
export interface VitalsData {
  segment: number;
  soft: boolean;
  url: string;
  lcp?: { value: number; element?: string };
  cls?: { value: number; sources: string[] };
  inp?: { value: number; target?: string; interactions: number };
  fcp?: number;
  ttfb?: number;
  unsupported: VitalMetric[];
}

/** Agent -> core, the binding's payload (JSON text). */
export type AgentMessage =
  | ({ kind: 'ready'; url: string } & Origin)
  | ({ kind: 'event'; seq: number; time: number; event: SyncEvent } & Origin)
  /** A page's bus message (RB-020-08): only from the top frame, `origin` the page's origin. */
  | ({ kind: 'bus'; time: number; origin: string; type: string; payload: JsonValue } & Origin)
  /** Browsersync's client is in the document: the core switches its own sync off (RB-020-06). */
  | ({ kind: 'detected'; tool: DetectedTool } & Origin)
  /** The top document's Web Vitals, at most once a second while they change (RB-060-10). */
  | ({ kind: 'vitals' } & VitalsData & Origin)
  /** The frameworks of the top document, whenever the probe's finding changes (RB-080-24). */
  | ({ kind: 'framework' } & FrameworkReport & Origin);

/** What the agent captures; the core sets it from `sync.*`. */
export interface AgentConfig {
  /** Only the active view captures. */
  active: boolean;
  capture: {
    /** Pointer down/up, click, double click, context menu (`sync.click`). */
    click: boolean;
    /** Hover (`sync.hover`). */
    hover: boolean;
    /** Scroll and wheel (`sync.scroll`). */
    scroll: boolean;
    /** Key down/up (`sync.keyboard`). */
    keyboard: boolean;
    /** Input and submit (`sync.input`). */
    input: boolean;
    /** Change of form controls (`sync.forms`, RB-020-05). */
    forms: boolean;
    /** Focus (`sync.focus`, RB-020-05; off by default). */
    focus: boolean;
  };
  /** The least time between two hover events (`sync.hoverThrottleMs`). */
  hoverThrottleMs: number;
  /**
   * How long scrolling rests before its end position is sent once more, freshly anchored
   * (`sync.scrollDebounceMs`; 0: never). Absent: `DEFAULT_SCROLL_DEBOUNCE_MS`.
   */
  scrollDebounceMs?: number;
  /**
   * How scroll sync moves the other views (`sync.scrollMode`, RB-090-33): `anchor` replays the
   * scroll position, so no wheel is captured; `wheel` captures the wheel. Absent: `anchor`.
   */
  scrollMode?: ScrollMode;
}

/** `sync.scrollMode`. */
export type ScrollMode = 'anchor' | 'wheel';

/** The emulation a page sees in `window.__rbrowser.emulation`. */
export interface PageEmulation {
  /** The device pixel ratio; 0 keeps the display's. */
  dpr: number;
  mobile: boolean;
  touch: boolean;
  orientation?: 'portrait' | 'landscape';
  colorScheme?: 'light' | 'dark';
  reducedMotion?: boolean;
  locale?: string;
}

/** What a page learns about its view (`window.__rbrowser`, without the bus). */
export interface PageInfo {
  /** The view id (as in MCP's `target`). */
  id: number;
  /** The breakpoint label, or the size after a free size. */
  label: string;
  /** The emulated width and height, CSS pixels. */
  width: number;
  height: number;
  /** The view's place in the grid, from 0. */
  index: number;
  engine: 'chromium' | 'firefox' | 'webkit';
  emulation: PageEmulation;
}

/** The message bus switch and limits (`bus.*`, with `pageApi.enabled`). */
export interface BusSettings {
  /** Off when `bus.enabled` or `pageApi.enabled` is off. */
  enabled: boolean;
  /** The largest payload, bytes of its JSON text. */
  maxPayloadBytes: number;
  /** Messages per second and view; more are dropped. */
  maxPerSecond: number;
}

/** The page API's state of a view: the core sends it with `page` and wraps the main-world bundle in it. */
export interface PageState {
  info: PageInfo;
  bus: BusSettings;
}

/** Core -> agent, evaluated as `__rbAgent.command(<json>)` in the agent's world. */
export type AgentCommand =
  | { cmd: 'configure'; config: AgentConfig }
  /** The replay marker: nothing is captured for `ms` (0 ends it), so replays cause no echo. */
  | { cmd: 'replay'; ms: number }
  | { cmd: 'resolve'; target: ElementTarget }
  /** Sets a form control to the leader's state (RB-020-05): `done`, `not_found`. */
  | ({ cmd: 'apply' } & ChangeData)
  /** Focuses the element without scrolling (RB-020-05): `done`, `not_found`, `not_visible`. */
  | { cmd: 'focus'; target: ElementTarget }
  | { cmd: 'ping' }
  /** The page API's state (RB-020-08): the agent keeps it and hands it to the main world. */
  | { cmd: 'page'; page: PageState }
  /** A bus message for this page: from another view (`from`) or the core (MCP, no `from`);
   *  delivered only when the bus is on and the page's origin is `origin` (when given). */
  | { cmd: 'bus'; from?: number; origin?: string; type: string; payload: JsonValue }
  /** Moves the document to `url` (same origin) through the history API, no reload (RB-020-06). */
  | { cmd: 'navigate'; url: string }
  /** Scroll sync (RB-020-04): bring the same content to the container's top edge. */
  | ({ cmd: 'scroll' } & ScrollData)
  /** The point of a target inside the viewport, never scrolling (wheel replay, RB-020-04). */
  | { cmd: 'point'; target: ElementTarget };

/** Where a target is in a view, in CSS pixels of the agent's frame viewport. */
export type ResolveResult =
  | { status: 'ok'; x: number; y: number; scrolled: boolean }
  | { status: 'not_found' }
  | { status: 'not_visible' };

/** The answer to a command. */
export type CommandResult =
  | { status: 'done' }
  | ResolveResult
  | { status: 'error'; message: string };

/** `AgentConfig.scrollDebounceMs` when the core sends none. */
export const DEFAULT_SCROLL_DEBOUNCE_MS = 150;

/** The configuration until the core sends its own: capture off, every kind on but focus. */
export const DEFAULT_CONFIG: AgentConfig = {
  active: false,
  capture: {
    click: true, hover: true, scroll: true, keyboard: true, input: true, forms: true, focus: false,
  },
  hoverThrottleMs: 100,
  scrollDebounceMs: DEFAULT_SCROLL_DEBOUNCE_MS,
  scrollMode: 'anchor',
};
