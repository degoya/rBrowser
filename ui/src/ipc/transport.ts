// The transport between the shell and the Rust core (contract: `protocol.ts`, docs/contracts-0.1.md
// section 5): shell -> core through CEF's message router (`window.cefQuery`), core -> shell as the
// `rb-ipc` CustomEvent on `window`.

import { EVENT_NAME, IpcErrorCode, type CoreToShell, type ShellToCore } from './protocol';

/** What `window.cefQuery` takes (CEF's message router, `CefMessageRouterRendererSide`). */
export interface CefQueryOptions {
  request: string;
  persistent?: boolean;
  onSuccess: (response: string) => void;
  onFailure: (code: number, message: string) => void;
}

/** The part of `window` the transport touches. */
export interface CefWindow extends EventTarget {
  cefQuery: (options: CefQueryOptions) => number;
  cefQueryCancel?: (id: number) => void;
}

/** How one request is sent. */
export interface SendOptions {
  /** How long to wait for its answer instead of the transport's default (a plugin panel's
   * call, RB-080-90). */
  timeoutMs?: number;
}

/** Sends requests to the core and hears its events. */
export interface Transport {
  send(message: ShellToCore, options?: SendOptions): Promise<CoreToShell>;
  /** Returns the function that stops listening. */
  subscribe(listener: (message: CoreToShell) => void): () => void;
}

/** A request the core refused or did not answer; `code` is an `IpcErrorCode`. */
export class IpcError extends Error {
  readonly code: number;

  constructor(code: number, message: string) {
    super(message);
    this.name = 'IpcError';
    this.code = code;
  }
}

export interface CefTransportOptions {
  /** A request without an answer after this long fails with 500. */
  timeoutMs?: number;
}

export const DEFAULT_TIMEOUT_MS = 10_000;

export function hasCefQuery(target: EventTarget): target is CefWindow {
  return typeof (target as Partial<CefWindow>).cefQuery === 'function';
}

/** A `CoreToShell` has at least a string `type`; anything else is dropped or refused. */
export function isCoreMessage(value: unknown): value is CoreToShell {
  return (
    typeof value === 'object' &&
    value !== null &&
    !Array.isArray(value) &&
    typeof (value as { type?: unknown }).type === 'string'
  );
}

export function parseResponse(response: string): CoreToShell {
  if (response.trim() === '') {
    return { type: 'ack' };
  }
  let value: unknown;
  try {
    value = JSON.parse(response);
  } catch {
    throw new IpcError(IpcErrorCode.Internal, 'the core answered with invalid JSON');
  }
  if (!isCoreMessage(value)) {
    throw new IpcError(IpcErrorCode.Internal, 'the core answered with something not a message');
  }
  return value;
}

/** Subscribes to the `rb-ipc` event on `target`. */
export function subscribeEvents(
  target: EventTarget,
  listener: (message: CoreToShell) => void,
): () => void {
  const handler = (event: Event): void => {
    const detail: unknown = (event as CustomEvent<unknown>).detail;
    if (isCoreMessage(detail)) {
      listener(detail);
    }
  };
  target.addEventListener(EVENT_NAME, handler);
  return () => target.removeEventListener(EVENT_NAME, handler);
}

export function createCefTransport(win: CefWindow, options: CefTransportOptions = {}): Transport {
  const defaultMs = options.timeoutMs ?? DEFAULT_TIMEOUT_MS;
  return {
    send(message, sent) {
      const timeoutMs = sent?.timeoutMs ?? defaultMs;
      return new Promise<CoreToShell>((resolve, reject) => {
        let settled = false;
        const settle = (finish: () => void): void => {
          if (!settled) {
            settled = true;
            clearTimeout(timer);
            finish();
          }
        };
        const timer = setTimeout(() => {
          settle(() => {
            win.cefQueryCancel?.(id);
            reject(new IpcError(IpcErrorCode.Internal, `no answer to ${message.type} in ${timeoutMs} ms`));
          });
        }, timeoutMs);
        const id = win.cefQuery({
          request: JSON.stringify(message),
          persistent: false,
          onSuccess: (response) => {
            settle(() => {
              try {
                resolve(parseResponse(response));
              } catch (error) {
                reject(error);
              }
            });
          },
          onFailure: (code, text) => settle(() => reject(new IpcError(code, text))),
        });
      });
    },
    subscribe: (listener) => subscribeEvents(win, listener),
  };
}
