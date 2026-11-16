// The sync part of the protocol (RB-020-02/04/05/06/07): the TypeScript mirror of rb-config
// `SyncConfig` and rb-ipc `SyncState`, split from `protocol.ts`, which re-exports it.

/** A view id, as in `protocol.ts`. */
type ViewId = number;

/** rb-config `SyncConfig`. */
export interface SyncConfig {
  navigation: boolean;
  scroll: boolean;
  click: boolean;
  input: boolean;
  keyboard: boolean;
  hover: boolean;
  leaderOnlySubmit: boolean;
  /** With `leaderOnlySubmit`, one leader per session (RB-020-07). */
  leaderPerSession: boolean;
  /** A click into a view makes it the active view (RB-020-02). */
  activateOnFocus: boolean;
  hoverThrottleMs: number;
  desyncTimeoutMs: number;
  contextMenu: boolean;
  /** Form controls' state follows the active view (RB-020-05). */
  forms: boolean;
  /** The focus follows the active view (RB-020-05; off by default). */
  focus: boolean;
  /** RB-020-06. */
  navigationSettleMs: number;
  navigationIncludeHash: boolean;
  detectBrowsersync: boolean;
  /** How scroll sync moves the other views (RB-020-04). */
  scrollMode: 'anchor' | 'wheel';
  scrollDebounceMs: number;
}

export type SyncFeature =
  | 'all'
  | 'navigation'
  | 'scroll'
  | 'click'
  | 'input'
  | 'keyboard'
  | 'hover'
  | 'forms'
  | 'focus';

/** rb-sync `DesyncReason`. */
export type DesyncReason = 'notFound' | 'notVisible' | 'timeout' | 'failed';

/** rb-sync `ViewDesync`: a view that could not follow (RB-020-02). */
export interface ViewDesync {
  view: ViewId;
  reason: DesyncReason;
  /** The CSS path of the element the event aimed at. */
  selector?: string;
  /** The event type that could not follow (`click`). */
  event: string;
}

export interface SyncState {
  enabled: boolean;
  features: SyncConfig;
  /** Views taken out of sync; absent when none (RB-020-02). */
  excluded?: ViewId[];
  /** Desynced views; absent when none (RB-020-02). */
  desynced?: ViewDesync[];
  /** Views whose navigation is decoupled: their label strip is an address field (RB-020-06). */
  navigationDecoupled?: ViewId[];
  /** The site whose Browsersync switched sync off; "sync anyway" while sync is off (RB-020-06). */
  browsersync?: string;
  /** The active view's next click is not synced (`Leader S N`); absent when not (RB-020-05). */
  skipNextClick?: boolean;
}
