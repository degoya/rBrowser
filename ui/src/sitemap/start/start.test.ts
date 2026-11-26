import { describe, expect, it } from 'vitest';
import { IpcClient } from '../../ipc/client';
import type { CoreToShell, ShellToCore, SitemapDiscovery } from '../../ipc/protocol';
import { CHOICE_KEY, createStart, loadChoice, saveChoice } from './start';

const FOUND: SitemapDiscovery = {
  discoveryId: '1-1',
  source: 'https://a.test/',
  pages: ['https://a.test/'],
  tree: [{ name: 'https://a.test', first: 0, count: 1, children: [{ name: '/', first: 0, count: 1 }] }],
  truncated: false,
  maxPages: 100_000,
  views: { all: 2, visible: 1 },
};

function client(answers: Array<() => Promise<CoreToShell>>, asked: ShellToCore[] = []): IpcClient {
  return new IpcClient({
    send: (message) => {
      asked.push(message);
      const next = answers.shift();
      return next === undefined ? Promise.resolve({ type: 'ack' }) : next();
    },
    subscribe: () => () => undefined,
  });
}

describe('the start page state', () => {
  it('drops the answer of a discovery that was cancelled', async () => {
    let answer: (value: CoreToShell) => void = () => undefined;
    const late = new Promise<CoreToShell>((resolve) => (answer = resolve));
    const start = createStart(client([() => late]));
    const asked = start.discover();
    start.cancel();
    answer({ type: 'sitemapDiscovery', ...FOUND });
    await asked;
    expect(start.state.value.phase).toBe('cancelled');
  });

  it('starts once the pages are found and says when the core refuses', async () => {
    const asked: ShellToCore[] = [];
    const start = createStart(
      client(
        [() => Promise.resolve({ type: 'sitemapDiscovery', ...FOUND }), () => Promise.reject(new Error('sitemap.error.busy: running'))],
        asked,
      ),
    );
    await start.discover();
    expect(start.state.value).toEqual({ phase: 'found', discovery: FOUND });
    await start.start({ discovery: '1-1', urls: ['https://a.test/'] });
    expect(start.state.value.phase).toBe('refused');
    expect(asked[1]).toEqual({ type: 'runCommand', id: 'sitemap.capture', args: { discovery: '1-1', urls: ['https://a.test/'] } });
  });

  it('remembers the views and the environment, and reads nothing broken', () => {
    const kept = new Map<string, string>();
    const storage = { getItem: (key: string) => kept.get(key) ?? null, setItem: (key: string, value: string) => void kept.set(key, value) };
    expect(loadChoice(storage)).toEqual({ views: 'all', env: null });
    saveChoice(storage, { views: 'visible', env: 'local' });
    expect(loadChoice(storage)).toEqual({ views: 'visible', env: 'local' });
    kept.set(CHOICE_KEY, '{broken');
    expect(loadChoice(storage)).toEqual({ views: 'all', env: null });
    expect(loadChoice(null)).toEqual({ views: 'all', env: null });
  });
});
