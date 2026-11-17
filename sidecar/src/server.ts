// The server side of the stdio protocol: requests in, answers and notifications out.

import { CANCEL, ErrorCode, type ErrorObject, type Id, PROGRESS, RpcError, encode, parseLine } from './rpc.ts';
import type { Router } from './router.ts';

export class RpcServer {
  readonly #router: Router;
  readonly #write: (line: string) => void;
  readonly #running = new Map<Id, AbortController>();

  constructor(router: Router, write: (line: string) => void) {
    this.#router = router;
    this.#write = write;
  }

  /** The requests still running. */
  inflight(): number {
    return this.#running.size;
  }

  /** Handles one line; resolves when its answer is written. */
  async handleLine(line: string): Promise<void> {
    if (line.trim() === '') {
      return;
    }
    const message = parseLine(line);
    switch (message.kind) {
      case 'invalid':
        this.#error(message.id, message.error);
        return;
      case 'notification':
        this.#notification(message.method, message.params);
        return;
      case 'request':
        await this.#request(message.id, message.method, message.params);
    }
  }

  /** Writes an error without a request (a line too long to read). */
  writeError(id: Id | null, error: ErrorObject): void {
    this.#error(id, error);
  }

  notify(method: string, params: Record<string, unknown>): void {
    this.#write(encode({ jsonrpc: '2.0', method, params }));
  }

  #notification(method: string, params: unknown): void {
    if (method !== CANCEL || typeof params !== 'object' || params === null) {
      return;
    }
    const id = (params as Record<string, unknown>).id;
    if (typeof id === 'number' || typeof id === 'string') {
      this.#running.get(id)?.abort();
    }
  }

  async #request(id: Id, method: string, params: unknown): Promise<void> {
    if (this.#running.has(id)) {
      this.#error(id, { code: ErrorCode.InvalidRequest, message: `id ${String(id)} is still running` });
      return;
    }
    const controller = new AbortController();
    this.#running.set(id, controller);
    const ctx = {
      signal: controller.signal,
      progress: (progress: Record<string, unknown>) => this.notify(PROGRESS, { id, ...progress }),
    };
    try {
      const result = await this.#router.call(method, params, ctx);
      if (controller.signal.aborted) {
        this.#cancelled(id);
      } else {
        this.#write(encode({ jsonrpc: '2.0', id, result: result ?? null }));
      }
    } catch (error) {
      if (controller.signal.aborted) {
        this.#cancelled(id);
      } else if (error instanceof RpcError) {
        this.#error(id, error.toObject());
      } else {
        const message = error instanceof Error ? error.message : String(error);
        this.#error(id, { code: ErrorCode.InternalError, message });
      }
    } finally {
      this.#running.delete(id);
    }
  }

  #cancelled(id: Id): void {
    this.#error(id, { code: ErrorCode.RequestCancelled, message: 'cancelled' });
  }

  #error(id: Id | null, error: ErrorObject): void {
    this.#write(encode({ jsonrpc: '2.0', id, error }));
  }
}
