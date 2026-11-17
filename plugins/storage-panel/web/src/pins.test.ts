// page/pins.js as it runs at document start in a view's isolated world.
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import { runScript } from './support';

const ORIGIN = 'http://127.0.0.1:18767';
const RECORD = 'rbrowser:storage-panel:pins';

interface Mock {
  sent: unknown[];
  answer: unknown;
  stop?: () => void;
}

function install(answer: unknown): Mock {
  const mock: Mock = { sent: [], answer };
  const api = {
    id: 'storage-panel',
    view: 1,
    maxMessageBytes: 65536,
    send: (message: unknown) => {
      mock.sent.push(message);
      return mock.answer instanceof Error ? Promise.reject(mock.answer) : Promise.resolve(mock.answer);
    },
    emit: () => undefined,
    on: () => () => undefined,
    onStop: (listener: () => void) => {
      mock.stop = listener;
      return () => undefined;
    },
  };
  (globalThis as Record<string, unknown>).rbPlugin = api;
  return mock;
}

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

describe('the pins page script', () => {
  beforeEach(() => {
    localStorage.clear();
    sessionStorage.clear();
  });
  afterEach(() => {
    delete (globalThis as Record<string, unknown>).rbPlugin;
  });

  it('sets the kept pins synchronously, before any page script could run', () => {
    sessionStorage.setItem(RECORD, JSON.stringify({ origin: ORIGIN, local: { featureFlag: 'on' }, session: { step: '2' } }));
    localStorage.setItem('featureFlag', 'off');
    install(new Promise(() => undefined));
    runScript('page/pins.js');
    expect(localStorage.getItem('featureFlag')).toBe('on');
    expect(sessionStorage.getItem('step')).toBe('2');
  });

  it('tells the plugin and keeps the record it answers', async () => {
    const record = { local: { featureFlag: 'beta' }, origin: ORIGIN, session: {} };
    const mock = install({ record });
    runScript('page/pins.js');
    expect(mock.sent).toEqual([{ type: 'loaded', origin: ORIGIN }]);
    await settle();
    expect(localStorage.getItem('featureFlag')).toBe('beta');
    expect(JSON.parse(sessionStorage.getItem(RECORD) ?? 'null')).toEqual(record);
  });

  it('does not set the values again when the answer matches the kept record', async () => {
    sessionStorage.setItem(RECORD, JSON.stringify({ origin: ORIGIN, local: { featureFlag: 'on' }, session: {} }));
    install({ record: { local: { featureFlag: 'on' }, origin: ORIGIN, session: {} } });
    runScript('page/pins.js');
    // The page changes the value after the start; the same pins do not overwrite it.
    localStorage.setItem('featureFlag', 'page');
    await settle();
    expect(localStorage.getItem('featureFlag')).toBe('page');
  });

  it('drops the record when nothing is pinned and ignores one of another origin', async () => {
    sessionStorage.setItem(RECORD, JSON.stringify({ origin: 'https://example.com', local: { featureFlag: 'x' } }));
    install({ record: null });
    runScript('page/pins.js');
    expect(localStorage.getItem('featureFlag')).toBeNull();
    await settle();
    expect(sessionStorage.getItem(RECORD)).toBeNull();
  });

  it('keeps the record when the plugin does not answer, and drops it when switched off', async () => {
    sessionStorage.setItem(RECORD, JSON.stringify({ origin: ORIGIN, local: { a: 'b' }, session: {} }));
    const mock = install(new Error('plugins.error.busy'));
    runScript('page/pins.js');
    await settle();
    expect(sessionStorage.getItem(RECORD)).not.toBeNull();
    mock.stop?.();
    expect(sessionStorage.getItem(RECORD)).toBeNull();
  });

  it('does nothing without the plugin bridge', () => {
    sessionStorage.setItem(RECORD, JSON.stringify({ origin: ORIGIN, local: { featureFlag: 'on' }, session: {} }));
    runScript('page/pins.js');
    expect(localStorage.getItem('featureFlag')).toBeNull();
  });
});
