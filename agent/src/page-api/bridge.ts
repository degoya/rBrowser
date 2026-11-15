// The checks both ends of the page API's bridge make (RB-020-08): the main world before it hands a
// bus message over, the agent in the isolated world again (it trusts nothing the page sends) -
// bus type names, payload size in UTF-8 bytes of the JSON text, the page state, and the rate
// limit per view. The Rust core holds the same limits (`crates/rb-sync/src/bus.rs`).

import type { BusSettings, PageInfo, PageState } from '../protocol';
import { isObject } from '../../../plugins/web-common/src/is-object';

/** The longest bus type name. */
export const MAX_TYPE_LENGTH = 64;
/** The highest `bus.maxPayloadBytes`: the message with its envelope stays under the binding's
 *  256 KiB. */
export const MAX_PAYLOAD_LIMIT = 196_608;
/** The highest `bus.maxPerSecond`. */
export const MAX_RATE_LIMIT = 1000;
/** The longest page origin a bus message carries. */
export const MAX_ORIGIN_LENGTH = 2048;

const TYPE_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]*$/;
const ENGINES = ['chromium', 'firefox', 'webkit'];


function isCount(value: unknown, min: number, max: number): value is number {
  return typeof value === 'number' && Number.isInteger(value) && value >= min && value <= max;
}

/** Whether `value` is a bus type: 1 to 64 letters, digits and `_ . : / -`, not starting with a
 *  separator. */
export function isBusType(value: unknown): value is string {
  return (
    typeof value === 'string' && value.length <= MAX_TYPE_LENGTH && TYPE_PATTERN.test(value)
  );
}

/** The length of `text` in UTF-8 bytes. */
export function utf8Length(text: string): number {
  let bytes = 0;
  for (let i = 0; i < text.length; i += 1) {
    const code = text.charCodeAt(i);
    if (code < 0x80) bytes += 1;
    else if (code < 0x800) bytes += 2;
    else if (code >= 0xd800 && code <= 0xdbff && i + 1 < text.length) {
      const next = text.charCodeAt(i + 1);
      if (next >= 0xdc00 && next <= 0xdfff) {
        bytes += 4;
        i += 1;
      } else bytes += 3;
    } else bytes += 3;
  }
  return bytes;
}

/** The JSON text of a bus payload (`undefined` is `null`). Throws a `TypeError` for what JSON
 *  cannot carry (cycles, functions, BigInt) and a `RangeError` above `maxBytes`. */
export function encodePayload(payload: unknown, maxBytes: number): string {
  let text: string | undefined;
  try {
    text = JSON.stringify(payload === undefined ? null : payload);
  } catch {
    throw new TypeError('rBrowser bus: the payload is not JSON-serialisable');
  }
  if (typeof text !== 'string') {
    throw new TypeError('rBrowser bus: the payload is not JSON-serialisable');
  }
  const size = utf8Length(text);
  if (size > maxBytes) {
    throw new RangeError(`rBrowser bus: the payload is ${size} bytes, over ${maxBytes}`);
  }
  return text;
}

/** Whether `value` holds a complete, in-range `BusSettings`. */
export function isBusSettings(value: unknown): value is BusSettings {
  return (
    isObject(value) &&
    typeof value.enabled === 'boolean' &&
    isCount(value.maxPayloadBytes, 1, MAX_PAYLOAD_LIMIT) &&
    isCount(value.maxPerSecond, 1, MAX_RATE_LIMIT)
  );
}

function isPageInfo(value: unknown): value is PageInfo {
  if (!isObject(value) || !isObject(value.emulation)) return false;
  const emulation = value.emulation;
  return (
    isCount(value.id, 0, 0xffff_ffff) &&
    typeof value.label === 'string' &&
    isCount(value.width, 0, 100_000) &&
    isCount(value.height, 0, 100_000) &&
    isCount(value.index, 0, 10_000) &&
    ENGINES.includes(value.engine as string) &&
    typeof emulation.dpr === 'number' &&
    typeof emulation.mobile === 'boolean' &&
    typeof emulation.touch === 'boolean'
  );
}

/** Whether `value` is a `PageState` (the `page` command, the main-world boot). */
export function isPageState(value: unknown): value is PageState {
  return isObject(value) && isPageInfo(value.info) && isBusSettings(value.bus);
}

/** A token bucket: `perSecond` messages a second, at most one second's worth stored. */
export class RateLimiter {
  private rate: number;
  private tokens: number;
  private last: number;
  private readonly now: () => number;

  constructor(perSecond: number, now: () => number = Date.now) {
    this.now = now;
    this.rate = perSecond;
    this.tokens = perSecond;
    this.last = now();
  }

  /** A new rate; what is stored is capped to it. */
  setRate(perSecond: number): void {
    this.refill();
    this.rate = perSecond;
    this.tokens = Math.min(this.tokens, perSecond);
  }

  /** Whether one more message may go now. */
  take(): boolean {
    this.refill();
    if (this.tokens < 1) return false;
    this.tokens -= 1;
    return true;
  }

  private refill(): void {
    const now = this.now();
    const elapsed = Math.max(0, now - this.last);
    this.last = now;
    this.tokens = Math.min(this.rate, this.tokens + (elapsed * this.rate) / 1000);
  }
}
