// The optional engine: the Nu HTML Checker (the W3C validator's engine) at the address the
// person set in `html-validate.nuAddress` - a local `vnu`, or validator.w3.org when they enter it
// themselves. The served source is POSTed to `<address>/?out=json` (the component builds and
// checks the endpoint, src/settings.rs) and the checker's messages are mapped to the runner's
// shape. Any failure of the address is the answer's error with its reason - never a silent
// switch to the offline engine. The request carries no cookie, no referrer and follows no
// redirect, so the source goes to the address the person set and nowhere else. Nu names lines and
// columns only; each message gets the selector of the element at the end of its range (inside
// the start tag Nu marks, src/locate.ts), so the findings panel can highlight it (RB-094-06).

import { type Place, withSelectors } from './locate';
import { type Answer, type AnswerMessage, type Failed, type Severity, cut } from './shape';

/** What the component asks for (src/script.rs builds it). */
export interface NuOptions {
  /** `<address>/?out=json`, checked by the component. */
  endpoint: string;
  source: string;
  maxMessages: number;
  maxText: number;
  /** How long the checker may take to answer. */
  timeoutMs: number;
}

/** The part of `fetch` the engine uses; the tests stub it. */
export type FetchLike = (url: string, init: RequestInit) => Promise<Response>;

/** One entry of the checker's `messages` (its JSON output format). */
interface NuMessage {
  type?: unknown;
  subType?: unknown;
  message?: unknown;
  extract?: unknown;
  firstLine?: unknown;
  lastLine?: unknown;
  firstColumn?: unknown;
  lastColumn?: unknown;
}

function text(value: unknown): string {
  return typeof value === 'string' ? value : '';
}

function whole(value: unknown): number | undefined {
  return typeof value === 'number' && Number.isInteger(value) && value > 0 ? value : undefined;
}

/** `error` is an error, `info` with the sub-type `warning` a warning, any other `info` info. */
function kind(entry: NuMessage): { rule: string; severity: Severity } {
  if (entry.type === 'error') return { rule: 'nu-error', severity: 'error' };
  if (entry.subType === 'warning') return { rule: 'nu-warning', severity: 'warning' };
  return { rule: 'nu-info', severity: 'info' };
}

function shaped(entry: NuMessage, max: number): AnswerMessage {
  // `firstLine` is left out when it equals `lastLine`; `firstColumn` when the range has none.
  const line = whole(entry.firstLine) ?? whole(entry.lastLine) ?? 0;
  const column = whole(entry.firstColumn) ?? whole(entry.lastColumn) ?? 0;
  return {
    ...kind(entry),
    message: cut(text(entry.message), max),
    line,
    column: line === 0 ? 0 : column,
    selector: null,
    extract: cut(text(entry.extract), max),
  };
}

/** The end of the message's range, where Nu has one: inside the start tag it marks. */
function rangeEnd(entry: NuMessage): Place | undefined {
  const line = whole(entry.lastLine);
  const column = whole(entry.lastColumn);
  return line !== undefined && column !== undefined ? { line, column } : undefined;
}

/** The checker's JSON as the runner's answer; with `options.source`, the source that was
 * validated, each placed message names the element at its place. */
export function mapNu(
  body: { messages?: unknown },
  options: Pick<NuOptions, 'maxMessages' | 'maxText'> & { source?: string },
): Answer {
  const entries = (Array.isArray(body.messages) ? body.messages : [])
    .filter((entry): entry is NuMessage => entry !== null && typeof entry === 'object')
    .slice(0, options.maxMessages + 1);
  const kept = entries.slice(0, options.maxMessages);
  const messages = kept.map((entry) => shaped(entry, options.maxText));
  return {
    engine: 'nu',
    version: '',
    messages:
      options.source === undefined
        ? messages
        : withSelectors(options.source, messages, kept.map(rangeEnd)),
    truncated: entries.length > options.maxMessages,
  };
}

function reason(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** Sends `options.source` to the checker and maps its answer. */
export async function validateNu(
  options: NuOptions,
  fetchImpl: FetchLike = (url, init) => fetch(url, init),
): Promise<Answer | Failed> {
  const failed = (error: string): Failed => ({ engine: 'nu', error: cut(error, 300) });
  const controller = new AbortController();
  let timedOut = false;
  const timer = setTimeout(() => {
    timedOut = true;
    controller.abort();
  }, options.timeoutMs);
  try {
    const response = await fetchImpl(options.endpoint, {
      method: 'POST',
      headers: { 'Content-Type': 'text/html; charset=utf-8' },
      body: options.source,
      credentials: 'omit',
      redirect: 'error',
      referrerPolicy: 'no-referrer',
      cache: 'no-store',
      signal: controller.signal,
    });
    if (!response.ok) return failed(`HTTP ${response.status}`);
    const raw = await response.text();
    let body: unknown;
    try {
      body = JSON.parse(raw);
    } catch {
      return failed(`no Nu JSON: ${raw.slice(0, 80)}`);
    }
    if (body === null || typeof body !== 'object' || !('messages' in body)) {
      return failed(`no Nu JSON: ${raw.slice(0, 80)}`);
    }
    const entries = (body as { messages: unknown }).messages;
    const broken = Array.isArray(entries)
      ? entries.find(
          (entry): entry is NuMessage =>
            entry !== null &&
            typeof entry === 'object' &&
            (entry as NuMessage).type === 'non-document-error',
        )
      : undefined;
    if (broken) return failed(text(broken.message) || 'non-document error');
    return mapNu(body as { messages: unknown }, options);
  } catch (error) {
    if (timedOut) return failed('timeout');
    // fetch rejects with a TypeError when the browser refuses or cannot send the request: the
    // page's CSP connect-src, mixed content, local network access, CORS, or no answer at all.
    if (error instanceof TypeError) return { ...failed(reason(error)), blocked: true };
    return failed(reason(error));
  } finally {
    clearTimeout(timer);
  }
}
