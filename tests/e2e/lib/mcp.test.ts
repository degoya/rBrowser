import { PassThrough } from 'node:stream';
import { createInterface } from 'node:readline';

import { describe, expect, it } from 'vitest';

import { McpClient, toolAnswer } from './mcp.ts';

/** A fake `rbrowser.com mcp`: answers each request line with `reply(request)`. */
function fake(reply: (request: { id?: number; method: string; params?: unknown }) => unknown) {
  const toServer = new PassThrough();
  const fromServer = new PassThrough();
  const seen: string[] = [];
  createInterface({ input: toServer }).on('line', (line) => {
    seen.push(line);
    const request = JSON.parse(line) as { id?: number; method: string; params?: unknown };
    if (request.id === undefined) return;
    const answer = reply(request);
    if (answer !== undefined) fromServer.write(`${JSON.stringify({ jsonrpc: '2.0', id: request.id, ...answer })}\n`);
  });
  return { client: new McpClient(toServer, fromServer), seen };
}

const text = (value: unknown, isError = false) => ({
  result: { content: [{ type: 'text', text: JSON.stringify(value) }], isError },
});

describe('McpClient', () => {
  it('initializes, says it is initialized and calls a tool with its arguments', async () => {
    const { client, seen } = fake((request) => {
      if (request.method === 'initialize') return { result: { protocolVersion: '2025-06-18', capabilities: {} } };
      if (request.method === 'tools/call') return text({ settled: true, views: [{ id: 1, url: 'http://a/' }] });
      return undefined;
    });
    await client.initialize();
    const answer = await client.call('navigate', { url: 'http://a/', target: 'all' });
    expect(answer).toEqual({ isError: false, value: { settled: true, views: [{ id: 1, url: 'http://a/' }] }, text: expect.any(String) });
    const methods = seen.map((line) => (JSON.parse(line) as { method: string }).method);
    expect(methods).toEqual(['initialize', 'notifications/initialized', 'tools/call']);
    expect(JSON.parse(seen[2] ?? '{}').params).toEqual({ name: 'navigate', arguments: { url: 'http://a/', target: 'all' } });
  });

  it('answers a tool error as isError with its code, not as a throw', async () => {
    const { client } = fake(() => text({ code: 'evaluate.disabled', error: 'off' }, true));
    const answer = await client.call('evaluate', { expression: '1' });
    expect(answer.isError).toBe(true);
    expect(answer.value).toEqual({ code: 'evaluate.disabled', error: 'off' });
  });

  it('throws a JSON-RPC error with its message', async () => {
    const { client } = fake(() => ({ error: { code: -32000, message: 'no running rBrowser instance for C:\\p' } }));
    await expect(client.call('views_list')).rejects.toThrow('no running rBrowser instance for C:\\p');
  });

  it('gives up on a call after its deadline', async () => {
    const { client } = fake(() => undefined);
    await expect(client.call('views_list', {}, 20)).rejects.toThrow('views_list: no answer in 20 ms');
  });

  it('ignores notifications between the answers', async () => {
    const toServer = new PassThrough();
    const fromServer = new PassThrough();
    const client = new McpClient(toServer, fromServer);
    createInterface({ input: toServer }).on('line', (line) => {
      const { id } = JSON.parse(line) as { id: number };
      fromServer.write('{"jsonrpc":"2.0","method":"notifications/tools/list_changed"}\n');
      fromServer.write(`${JSON.stringify({ jsonrpc: '2.0', id, ...text([1, 2]) })}\n`);
    });
    expect((await client.call('views_list')).value).toEqual([1, 2]);
  });

  it('fails the waiting and every later call at once when the connection ends', async () => {
    const fromServer = new PassThrough();
    const client = new McpClient(new PassThrough(), fromServer);
    const waiting = client.call('views_list');
    fromServer.end();
    await expect(waiting).rejects.toThrow('views_list: the MCP connection ended');
    await expect(client.call('navigate')).rejects.toThrow('navigate: the MCP connection ended');
  });
});

describe('toolAnswer', () => {
  it('keeps a text that is no JSON as text', () => {
    expect(toolAnswer({ content: [{ type: 'text', text: 'plain' }] })).toEqual({ isError: false, value: 'plain', text: 'plain' });
  });
});
