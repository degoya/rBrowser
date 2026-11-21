// The page API rBrowser puts in every view's main world (RB-020-08): `window.__rbrowser`, frozen,
// with the view's info and a message bus to the same page in the other views. Absent outside
// rBrowser and with `pageApi.enabled: false`. Copy this file into a project to type it:
//
//   if (window.__rbrowser) {
//     window.__rbrowser.bus.on('canvas:pan', (delta) => pan(delta as { x: number }));
//     window.addEventListener('rbrowser:viewchange', (event) => relayout(event.detail.width));
//   }
//
// The contract is docs/sync-protocol.md, "Page API and message bus".

/** The emulation of the view. */
export interface RbrowserEmulation {
  /** The device pixel ratio; 0 keeps the display's. */
  readonly dpr: number;
  readonly mobile: boolean;
  readonly touch: boolean;
  readonly orientation?: 'portrait' | 'landscape';
  readonly colorScheme?: 'light' | 'dark';
  readonly reducedMotion?: boolean;
  readonly locale?: string;
}

/** Where a bus message came from. */
export interface RbrowserBusMeta {
  /** The message's type. */
  readonly type: string;
  /** The view that sent it; `null` when rBrowser itself did (MCP's `bus_send`). */
  readonly from: number | null;
}

/** Called with the payload (the JSON value, parsed once per message) of every message of its type. */
export type RbrowserBusHandler = (payload: unknown, meta: RbrowserBusMeta) => void;

/** The message bus: a message goes to the same page in every other view, never back to the
 *  sender, and only to pages of the same origin. */
export interface RbrowserBus {
  /**
   * Sends `payload` (JSON-serialisable, at most `bus.maxPayloadBytes` of JSON text, 64 KiB by
   * default) as `type` (1 to 64 of `A-Z a-z 0-9 _ . : / -`). Returns `false` when the bus is off
   * or the view sent more than `bus.maxPerSecond` messages this second (the message is dropped).
   * @throws TypeError for a bad type or a payload JSON cannot carry
   * @throws RangeError for a payload over the limit
   */
  send(type: string, payload?: unknown): boolean;
  /** Calls `handler` for every message of `type` from another view. */
  on(type: string, handler: RbrowserBusHandler): void;
  /** Stops calling `handler` for `type`. */
  off(type: string, handler: RbrowserBusHandler): void;
}

/** `window.__rbrowser`: a frozen snapshot, replaced when the view changes. */
export interface RbrowserPageApi {
  /** The view id, as MCP's `target` names it. */
  readonly id: number;
  /** The breakpoint label, or the size after a free size. */
  readonly label: string;
  /** The emulated width, CSS pixels. */
  readonly width: number;
  /** The emulated height, CSS pixels. */
  readonly height: number;
  /** The view's place in the grid, from 0. */
  readonly index: number;
  readonly engine: 'chromium' | 'firefox' | 'webkit' | 'android' | 'safari' | 'ios';
  readonly emulation: RbrowserEmulation;
  readonly bus: RbrowserBus;
}

declare global {
  interface Window {
    /** rBrowser's page API; `undefined` outside rBrowser or when it is switched off. */
    readonly __rbrowser?: RbrowserPageApi;
  }
  interface WindowEventMap {
    /** The breakpoint or emulation changed; `detail` is the new `window.__rbrowser`. */
    'rbrowser:viewchange': CustomEvent<RbrowserPageApi>;
  }
}
