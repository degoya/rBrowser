import { describe, expect, it } from 'vitest';
import { createMockTransport } from './mockTransport';
import type { CoreToShell } from './protocol';
import type { IpcError } from './transport';

function stateOf(answer: CoreToShell) {
  if (answer.type !== 'state') {
    throw new Error(`expected state, got ${answer.type}`);
  }
  return answer;
}

describe('the mock transport', () => {
  it('answers getState with its state', async () => {
    const mock = createMockTransport();
    const state = stateOf(await mock.send({ type: 'getState' }));
    expect(state.views).toHaveLength(3);
    expect(state.url).toBe('http://localhost:5173/');
  });

  it('navigates every view and pushes the new state', async () => {
    const mock = createMockTransport();
    const pushed: CoreToShell[] = [];
    mock.subscribe((m) => pushed.push(m));
    await expect(mock.send({ type: 'navigate', url: 'http://localhost:5173/cart' })).resolves.toEqual({ type: 'ack' });
    const last = stateOf(pushed.at(-1) ?? { type: 'ack' });
    expect(last.url).toBe('http://localhost:5173/cart');
    expect(last.views.every((v) => v.url === 'http://localhost:5173/cart')).toBe(true);
  });

  it('walks back and forward through the history', async () => {
    const mock = createMockTransport();
    await mock.send({ type: 'navigate', url: 'http://localhost:5173/a' });
    await mock.send({ type: 'navigate', url: 'http://localhost:5173/b' });
    await mock.send({ type: 'back' });
    expect(mock.state.url).toBe('http://localhost:5173/a');
    await mock.send({ type: 'forward' });
    expect(mock.state.url).toBe('http://localhost:5173/b');
    await mock.send({ type: 'forward' });
    expect(mock.state.url).toBe('http://localhost:5173/b');
  });

  it('switches sync, presets and environments', async () => {
    const mock = createMockTransport();
    await mock.send({ type: 'setSync', feature: 'all', enabled: false });
    expect(mock.state.sync.enabled).toBe(false);
    await mock.send({ type: 'selectLayoutPreset', id: 'focus' });
    expect(mock.state.layout.preset).toBe('focus');
    await mock.send({ type: 'selectEnvironment', id: 'staging' });
    expect(mock.state.environment).toBe('staging');
    expect(mock.state.url).toBe('https://staging.example.com/');
  });

  it('refuses an unknown preset with 404', async () => {
    const mock = createMockTransport();
    const error = (await mock.send({ type: 'selectLayoutPreset', id: 'nope' }).catch((e: unknown) => e)) as IpcError;
    expect(error.code).toBe(404);
  });

  it('answers getSettings with the theme', async () => {
    const mock = createMockTransport({ theme: 'dark' });
    const answer = await mock.send({ type: 'getSettings' });
    expect(answer).toEqual({ type: 'settings', values: [{ key: 'ui.theme', value: 'dark', origin: 'user' }] });
  });

  it('answers what it does not know with 501 and records every request', async () => {
    const mock = createMockTransport();
    const error = (await mock.send({ type: 'runCommand', id: 'breakpoints.analyze' }).catch((e: unknown) => e)) as IpcError;
    expect(error.code).toBe(501);
    expect(mock.sent.map((m) => m.type)).toEqual(['runCommand']);
  });
});
