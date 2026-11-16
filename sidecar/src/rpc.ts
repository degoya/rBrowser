// JSON-RPC 2.0 messages as the sidecar reads and writes them (the Rust side: crates/rb-sidecar/src/rpc.rs).

/** The error codes of JSON-RPC 2.0, `RequestCancelled` as in LSP, and the sidecar's own. */
export const ErrorCode = {
  ParseError: -32700,
  InvalidRequest: -32600,
  MethodNotFound: -32601,
  InvalidParams: -32602,
  InternalError: -32603,
  RequestCancelled: -32800,
  /** `PLAYWRIGHT_BROWSERS_PATH` is not set: rb-sidecar always sets it. */
  NoBrowsersPath: -32001,
  /** The Playwright install failed (exit code, or no completion marker afterwards). */
  InstallFailed: -32002,
  /** The pinned WebKit build is not installed (`engines.update` installs it; RB-040-04). */
  WebkitNotInstalled: -32003,
  /** WebKit did not start. */
  WebkitLaunchFailed: -32004,
  /** No WebKit view has this id. */
  UnknownView: -32005,
  /** A download does not match its pinned SHA-256; nothing was installed. */
  ChecksumMismatch: -32006,
} as const;

/** The notification that cancels a running request: `{ "id": <request id> }`. */
export const CANCEL = '$/cancel';
/** The notification that reports progress of a request: `{ "id": <request id>, ... }`. */
export const PROGRESS = '$/progress';

export type Id = number | string;

export interface ErrorObject {
  code: number;
  message: string;
  data?: unknown;
}

/** An error a handler throws to answer with a specific code. */
export class RpcError extends Error {
  readonly code: number;
  readonly data: unknown;

  constructor(code: number, message: string, data?: unknown) {
    super(message);
    this.code = code;
    this.data = data;
  }

  toObject(): ErrorObject {
    return this.data === undefined
      ? { code: this.code, message: this.message }
      : { code: this.code, message: this.message, data: this.data };
  }
}

export type Incoming =
  | { kind: 'request'; id: Id; method: string; params: unknown }
  | { kind: 'notification'; method: string; params: unknown }
  | { kind: 'invalid'; id: Id | null; error: ErrorObject };

function isId(value: unknown): value is Id {
  return typeof value === 'string' || (typeof value === 'number' && Number.isInteger(value));
}

/** Parses one line; never throws. */
export function parseLine(line: string): Incoming {
  let value: unknown;
  try {
    value = JSON.parse(line);
  } catch {
    return { kind: 'invalid', id: null, error: { code: ErrorCode.ParseError, message: 'not JSON' } };
  }
  const invalid = (id: Id | null): Incoming => ({
    kind: 'invalid',
    id,
    error: { code: ErrorCode.InvalidRequest, message: 'not a JSON-RPC 2.0 message' },
  });
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    return invalid(null);
  }
  const message = value as Record<string, unknown>;
  const id = isId(message.id) ? message.id : null;
  if (message.jsonrpc !== '2.0' || typeof message.method !== 'string') {
    return invalid(id);
  }
  if (!('id' in message)) {
    return { kind: 'notification', method: message.method, params: message.params };
  }
  if (id === null) {
    return invalid(null);
  }
  return { kind: 'request', id, method: message.method, params: message.params };
}

/** One message as a line. */
export function encode(message: object): string {
  return `${JSON.stringify(message)}\n`;
}
