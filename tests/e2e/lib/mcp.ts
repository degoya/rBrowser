// An MCP client over stdio, as an agent is one (RB-099-02): `rbrowser.com mcp --project <dir>`
// forwards each JSON-RPC line to the project's running rBrowser (crates/rb-mcp/src/stdio.rs);
// from WSL that is the way across the interop (K7). One request per line, answers by id,
// notifications ignored. A tool's answer is the JSON in its first text content; a tool error
// (`isError`) is an answer, a JSON-RPC error a throw. Nothing here logs an answer: some carry a
// token (cdp_endpoint).

import { createInterface } from 'node:readline';
import type { Readable, Writable } from 'node:stream';

/** The MCP protocol version the client asks for. */
const PROTOCOL = '2025-06-18';
/** The default deadline of a call; `navigate` waits up to mcp.navigationTimeoutMs (15 s). */
const CALL_TIMEOUT_MS = 60_000;

export interface ToolAnswer {
  isError: boolean;
  value: unknown;
  text: string;
}

interface Pending {
  method: string;
  resolve(result: unknown): void;
  reject(error: Error): void;
}

/** The answer of a `tools/call` result. */
export function toolAnswer(result: unknown): ToolAnswer {
  const fields = (typeof result === 'object' && result !== null ? result : {}) as {
    content?: { type?: string; text?: string }[];
    isError?: boolean;
  };
  const text = fields.content?.find((part) => part.type === 'text')?.text ?? '';
  let value: unknown = text;
  try {
    value = JSON.parse(text);
  } catch {
    // A plain text answer stays text.
  }
  return { isError: fields.isError === true, value, text };
}

export class McpClient {
  private next = 0;
  private readonly pending = new Map<number, Pending>();
  private readonly input: Writable;
  private ended = false;

  constructor(input: Writable, output: Readable) {
    this.input = input;
    createInterface({ input: output })
      .on('line', (line) => this.receive(line))
      .on('close', () => this.end());
  }

  /** The forwarder's output ended (the app or `rbrowser.com mcp` is gone): every call fails now. */
  private end(): void {
    this.ended = true;
    for (const waiting of this.pending.values()) {
      waiting.reject(new Error(`${waiting.method}: the MCP connection ended`));
    }
    this.pending.clear();
  }

  private receive(line: string): void {
    let message: { id?: number; result?: unknown; error?: { message?: string } };
    try {
      message = JSON.parse(line) as typeof message;
    } catch {
      return;
    }
    if (typeof message.id !== 'number') return;
    const waiting = this.pending.get(message.id);
    if (waiting === undefined) return;
    this.pending.delete(message.id);
    if (message.error !== undefined) {
      waiting.reject(new Error(`${waiting.method}: ${message.error.message ?? 'error'}`));
    } else {
      waiting.resolve(message.result);
    }
  }

  private send(message: object): void {
    this.input.write(`${JSON.stringify({ jsonrpc: '2.0', ...message })}\n`);
  }

  /** A JSON-RPC request; rejects after `timeoutMs`. */
  request(method: string, params: object, timeoutMs = CALL_TIMEOUT_MS, label = method): Promise<unknown> {
    this.next += 1;
    const id = this.next;
    if (this.ended) return Promise.reject(new Error(`${label}: the MCP connection ended`));
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`${label}: no answer in ${timeoutMs} ms`));
      }, timeoutMs);
      this.pending.set(id, {
        method: label,
        resolve: (result) => {
          clearTimeout(timer);
          resolve(result);
        },
        reject: (error) => {
          clearTimeout(timer);
          reject(error);
        },
      });
      this.send({ id, method, params });
    });
  }

  async initialize(): Promise<unknown> {
    const result = await this.request('initialize', {
      protocolVersion: PROTOCOL,
      capabilities: {},
      clientInfo: { name: 'rbrowser-e2e', version: '1' },
    });
    this.send({ method: 'notifications/initialized' });
    return result;
  }

  /** Calls the tool `name`; a tool error is an answer with `isError`. */
  async call(name: string, args: object = {}, timeoutMs = CALL_TIMEOUT_MS): Promise<ToolAnswer> {
    return toolAnswer(await this.request('tools/call', { name, arguments: args }, timeoutMs, name));
  }

  /** Calls the tool `name` and throws on a tool error, with its code and text. */
  async value<T = unknown>(name: string, args: object = {}, timeoutMs = CALL_TIMEOUT_MS): Promise<T> {
    const answer = await this.call(name, args, timeoutMs);
    if (answer.isError) throw new Error(`${name}: ${answer.text.slice(0, 500)}`);
    return answer.value as T;
  }
}
