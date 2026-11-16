// The method registry: every module (sidecar, playwright; WebKit, flows and Lighthouse in their
// jobs) registers its methods here.

import { ErrorCode, RpcError } from './rpc.ts';

/** What a handler gets besides its parameters. */
export interface CallContext {
  /** Aborted when rBrowser cancels the request (`$/cancel`). */
  signal: AbortSignal;
  /** Sends a `$/progress` notification tagged with the request id. */
  progress(params: Record<string, unknown>): void;
}

export type MethodHandler = (params: unknown, ctx: CallContext) => unknown;

export class Router {
  readonly #methods = new Map<string, MethodHandler>();

  register(method: string, handler: MethodHandler): void {
    if (this.#methods.has(method)) {
      throw new Error(`method ${method} registered twice`);
    }
    this.#methods.set(method, handler);
  }

  methods(): string[] {
    return [...this.#methods.keys()].sort();
  }

  async call(method: string, params: unknown, ctx: CallContext): Promise<unknown> {
    const handler = this.#methods.get(method);
    if (handler === undefined) {
      throw new RpcError(ErrorCode.MethodNotFound, `unknown method ${method}`);
    }
    return await handler(params, ctx);
  }
}
