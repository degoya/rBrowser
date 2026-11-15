import { describe, expect, it } from 'vitest';

import { ErrorCode, RpcError } from './rpc.ts';
import { Router } from './router.ts';
import { RpcServer } from './server.ts';

function setup() {
  const router = new Router();
  const out: unknown[] = [];
  const server = new RpcServer(router, (line) => {
    expect(line.endsWith('\n')).toBe(true);
    expect(line.slice(0, -1)).not.toContain('\n');
    out.push(JSON.parse(line));
  });
  return { router, server, out };
}

describe('RpcServer', () => {
  it('answers a request with the handler result', async () => {
    const { router, server, out } = setup();
    router.register('echo', (params) => ({ got: params }));
    await server.handleLine('{"jsonrpc":"2.0","id":1,"method":"echo","params":{"a":1}}');
    expect(out).toEqual([{ jsonrpc: '2.0', id: 1, result: { got: { a: 1 } } }]);
  });

  it('answers null for a handler without a result', async () => {
    const { router, server, out } = setup();
    router.register('noop', () => undefined);
    await server.handleLine('{"jsonrpc":"2.0","id":"x","method":"noop"}');
    expect(out).toEqual([{ jsonrpc: '2.0', id: 'x', result: null }]);
  });

  it('reports an unknown method', async () => {
    const { server, out } = setup();
    await server.handleLine('{"jsonrpc":"2.0","id":2,"method":"nope"}');
    expect(out).toEqual([
      { jsonrpc: '2.0', id: 2, error: { code: ErrorCode.MethodNotFound, message: 'unknown method nope' } },
    ]);
  });

  it('reports a parse error with a null id', async () => {
    const { server, out } = setup();
    await server.handleLine('{not json');
    expect(out).toEqual([
      { jsonrpc: '2.0', id: null, error: { code: ErrorCode.ParseError, message: 'not JSON' } },
    ]);
  });

  it('reports an invalid request and keeps its id', async () => {
    const { server, out } = setup();
    await server.handleLine('{"jsonrpc":"1.0","id":3,"method":"x"}');
    await server.handleLine('[1,2]');
    expect(out).toEqual([
      { jsonrpc: '2.0', id: 3, error: { code: ErrorCode.InvalidRequest, message: 'not a JSON-RPC 2.0 message' } },
      { jsonrpc: '2.0', id: null, error: { code: ErrorCode.InvalidRequest, message: 'not a JSON-RPC 2.0 message' } },
    ]);
  });

  it('ignores blank lines and unknown notifications', async () => {
    const { server, out } = setup();
    await server.handleLine('   ');
    await server.handleLine('{"jsonrpc":"2.0","method":"something"}');
    expect(out).toEqual([]);
  });

  it('passes an RpcError through with its code and data', async () => {
    const { router, server, out } = setup();
    router.register('bad', () => {
      throw new RpcError(ErrorCode.InvalidParams, 'browser must be webkit', { browser: 'x' });
    });
    await server.handleLine('{"jsonrpc":"2.0","id":4,"method":"bad"}');
    expect(out).toEqual([
      {
        jsonrpc: '2.0',
        id: 4,
        error: { code: ErrorCode.InvalidParams, message: 'browser must be webkit', data: { browser: 'x' } },
      },
    ]);
  });

  it('turns any other exception into an internal error', async () => {
    const { router, server, out } = setup();
    router.register('boom', async () => {
      throw new Error('disk full');
    });
    await server.handleLine('{"jsonrpc":"2.0","id":5,"method":"boom"}');
    expect(out).toEqual([
      { jsonrpc: '2.0', id: 5, error: { code: ErrorCode.InternalError, message: 'disk full' } },
    ]);
  });

  it('cancels a running request on $/cancel', async () => {
    const { router, server, out } = setup();
    router.register('wait', (_params, ctx) => new Promise((_resolve, reject) => {
      ctx.signal.addEventListener('abort', () => reject(new Error('aborted')));
    }));
    const running = server.handleLine('{"jsonrpc":"2.0","id":6,"method":"wait"}');
    expect(server.inflight()).toBe(1);
    await server.handleLine('{"jsonrpc":"2.0","method":"$/cancel","params":{"id":6}}');
    await running;
    expect(server.inflight()).toBe(0);
    expect(out).toEqual([
      { jsonrpc: '2.0', id: 6, error: { code: ErrorCode.RequestCancelled, message: 'cancelled' } },
    ]);
  });

  it('refuses a second request with an id still running', async () => {
    const { router, server, out } = setup();
    let release = () => {};
    router.register('hold', () => new Promise<void>((resolve) => { release = resolve; }));
    const first = server.handleLine('{"jsonrpc":"2.0","id":7,"method":"hold"}');
    await server.handleLine('{"jsonrpc":"2.0","id":7,"method":"hold"}');
    release();
    await first;
    expect(out).toEqual([
      { jsonrpc: '2.0', id: 7, error: { code: ErrorCode.InvalidRequest, message: 'id 7 is still running' } },
      { jsonrpc: '2.0', id: 7, result: null },
    ]);
  });

  it('sends progress as $/progress notifications tagged with the request id', async () => {
    const { router, server, out } = setup();
    router.register('work', (_params, ctx) => {
      ctx.progress({ percent: 50 });
      return 'done';
    });
    await server.handleLine('{"jsonrpc":"2.0","id":8,"method":"work"}');
    expect(out).toEqual([
      { jsonrpc: '2.0', method: '$/progress', params: { id: 8, percent: 50 } },
      { jsonrpc: '2.0', id: 8, result: 'done' },
    ]);
  });
});

describe('Router', () => {
  it('refuses a method registered twice', () => {
    const router = new Router();
    router.register('a', () => null);
    expect(() => router.register('a', () => null)).toThrow('method a registered twice');
  });

  it('lists its methods sorted', () => {
    const router = new Router();
    router.register('b.x', () => null);
    router.register('a.y', () => null);
    expect(router.methods()).toEqual(['a.y', 'b.x']);
  });
});
