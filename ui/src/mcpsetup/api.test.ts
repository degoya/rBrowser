import { describe, expect, it } from 'vitest';
import type { McpSetupAnswer, ShellToCore } from '../ipc/protocol';
import { fakeCore } from '../firstrun/testing';
import { createMcpSetupApi } from './api';

/** A core that answers every `mcpSetup` request with `answer(request)` as an event, after `ack`. */
function core(answer: (request: Extract<ShellToCore, { type: 'mcpSetup' }>['request']) => McpSetupAnswer) {
  const fake = fakeCore((message) => {
    if (message.type !== 'mcpSetup') return undefined;
    // The event comes after the ack, as the core sends it from the blocking pool.
    setTimeout(() => fake.emit({ type: 'mcpSetup', answer: answer(message.request) }), 0);
    return { type: 'ack' };
  });
  return fake;
}

describe('the MCP setup api', () => {
  it('takes only the event with its own request number', async () => {
    const fake = core((request) => ({ op: 'check', requestId: request.requestId, enabled: true, running: true, port: 7420, calls: 0 }));
    const api = createMcpSetupApi(fake.client);
    // An answer to another request first: never taken.
    setTimeout(() => fake.emit({ type: 'mcpSetup', answer: { op: 'check', requestId: 9999, enabled: false, running: false, calls: 0 } }), 0);
    const answer = await api.check();
    expect(answer).toMatchObject({ op: 'check', enabled: true, port: 7420 });
  });

  it('sends the plan with the project folder and the write with the person\'s choice', async () => {
    const fake = core((request) =>
      request.op === 'plan'
        ? { op: 'plan', requestId: request.requestId, enabled: false, clients: [] }
        : { op: 'written', requestId: request.requestId, files: [] },
    );
    const api = createMcpSetupApi(fake.client);
    await api.plan('C:/work/shop');
    await api.write({ client: 'cursor', root: 'C:/work/shop', project: true, user: false });
    await api.plan(null);
    const requests = fake.sent.flatMap((message) => (message.type === 'mcpSetup' ? [message.request] : []));
    expect(requests[0]).toMatchObject({ op: 'plan', root: 'C:/work/shop' });
    expect(requests[1]).toMatchObject({ op: 'write', client: 'cursor', root: 'C:/work/shop', project: true, user: false });
    expect(requests[2]).not.toHaveProperty('root');
    expect(new Set(requests.map((request) => request.requestId)).size).toBe(3);
  });

  it('switches the server on and remembers the client as user settings, the person\'s writes', async () => {
    const fake = fakeCore();
    const api = createMcpSetupApi(fake.client);
    expect(await api.enable()).toBe(true);
    await api.setClient('codex');
    expect(fake.sent).toEqual([
      { type: 'setSetting', key: 'mcp.enabled', value: true, scope: 'user' },
      { type: 'setSetting', key: 'mcp.client', value: 'codex', scope: 'user' },
    ]);
  });

  it('says when the server could not be switched on', async () => {
    const fake = fakeCore(() => ({ type: 'settingRejected', key: 'mcp.enabled', issues: [] }));
    expect(await createMcpSetupApi(fake.client).enable()).toBe(false);
  });
});
