// What the page script, the panels and the core's main-world scripts agree on (RB-080-24): the
// DOM event names and the envelope of `agent/src/protocol.ts` (mirrored here: the plugin is a
// package of its own), and the relay over the plugin bridge, whose messages are at most 64 KiB
// of JSON - a React `operations` message for a large tree can be more, so a long text goes in
// numbered chunks the panel puts together again.

import { isObject } from '../../../web-common/src/is-object';

/** The frameworks whose DevTools rBrowser brings. */
export type Framework = 'vue' | 'react';
export const FRAMEWORKS: readonly Framework[] = ['vue', 'react'];

/** Main world -> page script: the probe's report, `detail` the JSON text of a `FrameworkReport`. */
export const FRAMEWORK_EVENT = '__rbrowser:framework';
/** Main world -> page script: a message of the framework's backend, `detail` JSON text. */
export const outEvent = (framework: Framework): string => `__rbrowser:${framework}-devtools-out`;
/** Page script -> main world: a message for the framework's backend, `detail` JSON text. */
export const inEvent = (framework: Framework): string => `__rbrowser:${framework}-devtools-in`;

/** What the probe found of a framework in a page. */
export interface FrameworkInfo {
  version?: string;
  /** rBrowser's hook is in the document. */
  hook: boolean;
  /** The framework registered with a hook. */
  devtools: boolean;
}

/** The probe's report. */
export interface FrameworkReport {
  vue?: FrameworkInfo;
  react?: FrameworkInfo;
}

/** A message between a backend and its panel. */
export type Envelope = { t: 'connect' } | { t: 'disconnect' } | { t: 'm'; d: unknown };

/** The longest text one relay message carries, in UTF-16 units: at most three UTF-8 bytes
 *  each, so a chunk stays well under the bridge's 64 KiB with its envelope. */
export const CHUNK_CHARS = 16_000;

/** A message over the plugin bridge, page script -> panel: a backend's envelope as JSON text
 *  (`text` alone, or chunk `i` of `n` of message `id`), or the probe's report (`probe`). */
export interface Relay {
  framework: Framework | 'probe';
  text: string;
  id?: number;
  i?: number;
  n?: number;
}

/** A message over the plugin bridge, panel -> page script (the component's post). */
export interface Post {
  framework: Framework;
  envelope: Envelope;
}

export function isFramework(value: unknown): value is Framework {
  return value === 'vue' || value === 'react';
}


/** Whether `value` is an envelope a backend or panel may send. */
export function isEnvelope(value: unknown): value is Envelope {
  if (!isObject(value)) return false;
  if (value.t === 'connect' || value.t === 'disconnect') return true;
  return value.t === 'm' && 'd' in value && value.d !== undefined;
}

/** Whether `value` is a relay message. */
export function isRelay(value: unknown): value is Relay {
  if (!isObject(value) || typeof value.text !== 'string') return false;
  if (!isFramework(value.framework) && value.framework !== 'probe') return false;
  if (value.id === undefined) return value.i === undefined && value.n === undefined;
  return (
    typeof value.id === 'number' &&
    typeof value.i === 'number' &&
    typeof value.n === 'number' &&
    Number.isInteger(value.i) &&
    Number.isInteger(value.n) &&
    value.n >= 1 &&
    value.i >= 0 &&
    value.i < value.n
  );
}

/** Whether `value` is a post of the component. */
export function isPost(value: unknown): value is Post {
  return isObject(value) && isFramework(value.framework) && isEnvelope(value.envelope);
}

/** `text` in pieces of at most `max` units; one piece for a short text, none for an empty one. */
export function chunks(text: string, max = CHUNK_CHARS): string[] {
  const pieces: string[] = [];
  for (let at = 0; at < text.length; at += max) pieces.push(text.slice(at, at + max));
  return pieces;
}

/** The relay messages for one backend message `text` of `framework`: whole, or chunked under
 *  `id`. */
export function relays(framework: Framework, text: string, id: number, max = CHUNK_CHARS): Relay[] {
  const pieces = chunks(text, max);
  if (pieces.length <= 1) return [{ framework, text }];
  return pieces.map((piece, i) => ({ framework, text: piece, id, i, n: pieces.length }));
}

/** Puts chunked relay messages together again, per message id. A whole message passes through;
 *  a chunk completes its message once every piece arrived, in any order. At most `limit`
 *  messages wait at once; the oldest is dropped beyond it. */
export class Reassembler {
  private readonly waiting = new Map<number, (string | undefined)[]>();
  private readonly limit: number;

  constructor(limit = 32) {
    this.limit = limit;
  }

  /** The complete text, or null while pieces are missing. */
  take(relay: Relay): string | null {
    if (relay.id === undefined || relay.i === undefined || relay.n === undefined) return relay.text;
    let pieces = this.waiting.get(relay.id);
    if (!pieces) {
      if (this.waiting.size >= this.limit) {
        const oldest = this.waiting.keys().next().value;
        if (oldest !== undefined) this.waiting.delete(oldest);
      }
      pieces = new Array<string | undefined>(relay.n).fill(undefined);
      this.waiting.set(relay.id, pieces);
    }
    if (pieces.length !== relay.n) return null;
    pieces[relay.i] = relay.text;
    if (pieces.some((piece) => piece === undefined)) return null;
    this.waiting.delete(relay.id);
    return pieces.join('');
  }
}
