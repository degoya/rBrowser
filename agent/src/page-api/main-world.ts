// The page API in a view's main world (RB-020-08): `window.__rbrowser` with the view's info and
// the message bus, nothing else - no agent logic lives here. The core wraps the bundle
// (`dist/rb-page-api.js`) in `(function(__rbPageBoot){...})(<PageState>)` and registers it with
// `Page.addScriptToEvaluateOnNewDocument` (no world name) for the view's top frame.
//
// The bridge to the agent in the isolated world is DOM events on `document`, whose `detail` is
// always JSON text (a string crosses the worlds as it is): `__rbrowser:bus-out` carries a message
// to the agent, `__rbrowser:bus-in` one from another view, `__rbrowser:view` a new page state.
// The page can dispatch every one of them itself - it only fools itself: the agent checks every
// outgoing message again and can be made to do nothing but relay a bus message.

import type {
  RbrowserBus,
  RbrowserBusHandler,
  RbrowserBusMeta,
  RbrowserPageApi,
} from '../../types/rbrowser';
import type { BusSettings, PageInfo, PageState } from '../protocol';
import {
  BUS_IN_EVENT,
  BUS_OUT_EVENT,
  PAGE_API_GLOBAL,
  VIEW_EVENT,
  VIEWCHANGE_EVENT,
} from '../protocol';
import { RateLimiter, encodePayload, isBusType, isPageState } from './bridge';

type Win = Window & typeof globalThis;

function freezeInfo(info: PageInfo, bus: RbrowserBus): RbrowserPageApi {
  return Object.freeze({
    id: info.id,
    label: info.label,
    width: info.width,
    height: info.height,
    index: info.index,
    engine: info.engine,
    emulation: Object.freeze({ ...info.emulation }),
    bus,
  });
}

/** Defines `window.__rbrowser` from `boot` (a `PageState`); false in a child frame, when it is
 *  already there or `boot` is not a page state. */
export function installPageApi(win: Win, boot: unknown, now: () => number = Date.now): boolean {
  if (win !== win.top || PAGE_API_GLOBAL in win || !isPageState(boot)) return false;
  // Taken before any page script runs, so a page that replaces them later changes nothing here.
  const doc = win.document;
  const CustomEventCtor = win.CustomEvent;
  const parse = JSON.parse;
  const stringify = JSON.stringify;
  const report = (error: unknown): void => {
    if (typeof win.reportError === 'function') win.reportError(error);
    else win.setTimeout(() => {
      throw error;
    });
  };

  let settings: BusSettings = boot.bus;
  let infoText = stringify(boot.info);
  const limiter = new RateLimiter(settings.maxPerSecond, now);
  const handlers = new Map<string, Set<RbrowserBusHandler>>();

  const bus: RbrowserBus = Object.freeze({
    send(type: string, payload?: unknown): boolean {
      if (!isBusType(type)) throw new TypeError('rBrowser bus: invalid message type');
      const text = encodePayload(payload, settings.maxPayloadBytes);
      if (!settings.enabled || !limiter.take()) return false;
      const detail = `{"type":${stringify(type)},"payload":${text}}`;
      doc.dispatchEvent(new CustomEventCtor(BUS_OUT_EVENT, { detail }));
      return true;
    },
    on(type: string, handler: RbrowserBusHandler): void {
      if (!isBusType(type)) throw new TypeError('rBrowser bus: invalid message type');
      if (typeof handler !== 'function') throw new TypeError('rBrowser bus: handler is no function');
      let set = handlers.get(type);
      if (!set) handlers.set(type, (set = new Set()));
      set.add(handler);
    },
    off(type: string, handler: RbrowserBusHandler): void {
      handlers.get(type)?.delete(handler);
    },
  });

  let current = freezeInfo(boot.info, bus);
  Object.defineProperty(win, PAGE_API_GLOBAL, {
    get: () => current,
    enumerable: false,
    configurable: false,
  });

  const textOf = (event: Event): string | null =>
    event.target === doc && typeof (event as CustomEvent).detail === 'string'
      ? ((event as CustomEvent<string>).detail)
      : null;

  doc.addEventListener(VIEW_EVENT, (event) => {
    const text = textOf(event);
    if (text === null) return;
    let state: unknown;
    try {
      state = parse(text);
    } catch {
      return;
    }
    if (!isPageState(state)) return;
    const next: PageState = state;
    settings = next.bus;
    limiter.setRate(settings.maxPerSecond);
    const nextText = stringify(next.info);
    if (nextText === infoText) return;
    infoText = nextText;
    current = freezeInfo(next.info, bus);
    win.dispatchEvent(new CustomEventCtor(VIEWCHANGE_EVENT, { detail: current }));
  });

  doc.addEventListener(BUS_IN_EVENT, (event) => {
    const text = textOf(event);
    if (text === null) return;
    let message: { type?: unknown; payload?: unknown; from?: unknown };
    try {
      message = parse(text) as typeof message;
    } catch {
      return;
    }
    if (typeof message !== 'object' || message === null || !isBusType(message.type)) return;
    const set = handlers.get(message.type);
    if (!set || set.size === 0) return;
    const meta: RbrowserBusMeta = Object.freeze({
      type: message.type,
      from: typeof message.from === 'number' ? message.from : null,
    });
    const payload = message.payload === undefined ? null : message.payload;
    for (const handler of [...set]) {
      try {
        handler(payload, meta);
      } catch (error) {
        report(error);
      }
    }
  });
  return true;
}
