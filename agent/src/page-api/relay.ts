// The page API's end in the agent's isolated world (RB-020-08): it keeps the view's page state
// (the core's `page` command), hands it to the main world, relays the page's bus messages to the
// core and delivers the other views' messages (`bus` command) to the page.
//
// The page is untrusted. All it can reach here is one listener for `__rbrowser:bus-out` on the
// top document; whatever arrives there is checked again - JSON text, exactly `type` and
// `payload`, the type, the size, the rate - and can only ever become a bus message to the core.
// The isolated world's globals (`__rbAgent`, the binding) stay out of the page's reach.

import type { CommandResult, JsonValue, PageState } from '../protocol';
import { BUS_IN_EVENT, BUS_OUT_EVENT, VIEW_EVENT } from '../protocol';
import { MAX_ORIGIN_LENGTH, RateLimiter, encodePayload, isBusType, isPageState } from './bridge';
import { isObject } from '../../../plugins/web-common/src/is-object';

/** Room for the envelope `{"type":...,"payload":}` around a payload, in characters. */
const ENVELOPE_LENGTH = 128;

/** A page's bus message for the core. */
export interface BusOut {
  type: string;
  payload: JsonValue;
  /** The page's origin (`location.origin`). */
  origin: string;
}


/** The bus and page state of one frame's agent. */
export class BusRelay {
  private state: PageState | null = null;
  /** Made with the first page state, full. */
  private limiter: RateLimiter | null = null;

  constructor(
    private readonly win: Window,
    /** Only the top frame has the page API. */
    private readonly top: boolean,
    private readonly post: (message: BusOut) => void,
    private readonly now: () => number = Date.now,
  ) {}

  /** Listens for the page's messages; returns the uninstall. */
  start(): () => void {
    const doc = this.win.document;
    const listener = (event: Event): void => this.onOut(event);
    doc.addEventListener(BUS_OUT_EVENT, listener);
    return () => doc.removeEventListener(BUS_OUT_EVENT, listener);
  }

  /** The `page` command: keeps the state and hands it to the main world. */
  configure(input: unknown): CommandResult {
    if (!isPageState(input)) return { status: 'error', message: 'invalid page' };
    this.state = input;
    if (this.limiter) this.limiter.setRate(input.bus.maxPerSecond);
    else this.limiter = new RateLimiter(input.bus.maxPerSecond, this.now);
    if (this.top) this.toPage(VIEW_EVENT, JSON.stringify(input));
    return { status: 'done' };
  }

  /** The `bus` command: a message for this page from another view or from the core. */
  deliver(input: Record<string, unknown>): CommandResult {
    const { type, from, origin } = input;
    if (
      !isBusType(type) ||
      (from !== undefined && (typeof from !== 'number' || !Number.isInteger(from))) ||
      (origin !== undefined && typeof origin !== 'string')
    ) {
      return { status: 'error', message: 'invalid bus message' };
    }
    if (!this.top || !this.state?.bus.enabled) return { status: 'error', message: 'bus off' };
    if (origin !== undefined && origin !== this.win.location.origin) {
      return { status: 'error', message: 'other origin' };
    }
    const payload = input.payload === undefined ? null : input.payload;
    this.toPage(BUS_IN_EVENT, JSON.stringify({ type, payload, from: from ?? null }));
    return { status: 'done' };
  }

  private onOut(event: Event): void {
    const state = this.state;
    if (!this.top || !state?.bus.enabled || event.target !== this.win.document) return;
    const detail = (event as CustomEvent<unknown>).detail;
    const max = state.bus.maxPayloadBytes;
    // A UTF-8 byte per character at least: anything longer is over the limit unparsed.
    if (typeof detail !== 'string' || detail.length > max + ENVELOPE_LENGTH) return;
    let message: unknown;
    try {
      message = JSON.parse(detail);
    } catch {
      return;
    }
    if (!isObject(message) || !isBusType(message.type)) return;
    if (Object.keys(message).some((key) => key !== 'type' && key !== 'payload')) return;
    let text: string;
    try {
      text = encodePayload(message.payload, max);
    } catch {
      return;
    }
    const origin = this.win.location.origin;
    if (!origin || origin === 'null' || origin.length > MAX_ORIGIN_LENGTH) return;
    if (!this.limiter?.take()) return;
    this.post({ type: message.type, payload: JSON.parse(text) as JsonValue, origin });
  }

  private toPage(name: string, detail: string): void {
    const CustomEventCtor = (this.win as Window & typeof globalThis).CustomEvent;
    this.win.document.dispatchEvent(new CustomEventCtor(name, { detail }));
  }
}
