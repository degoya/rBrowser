import { createServer } from 'node:http';
import type { AddressInfo } from 'node:net';

import { describe, expect, it } from 'vitest';

import { leakedTargets, pageTargets, statusWithout } from './cdp.ts';

const infos = [
  { targetId: '1', type: 'page', url: 'http://127.0.0.1:18766/grid.html' },
  { targetId: '2', type: 'page', url: 'rbrowser://shell/index.html' },
  { targetId: '3', type: 'page', url: 'devtools://devtools/bundled/inspector.html' },
  { targetId: '4', type: 'service_worker', url: 'http://127.0.0.1:18766/sw.js' },
  { targetId: '5', type: 'page', url: 'about:blank#rb-render-3' },
  { targetId: '6', type: 'page', url: 'about:blank' },
];

describe('the targets an outside client sees (K10)', () => {
  it('names every one rBrowser keeps to itself: its scheme, DevTools, its render views', () => {
    expect(leakedTargets(infos).map((info) => info.targetId)).toEqual(['2', '3', '5']);
  });

  it('counts the pages, not the workers', () => {
    expect(pageTargets(infos).map((info) => info.targetId)).toEqual(['1', '2', '3', '5', '6']);
  });
});

describe('statusWithout', () => {
  it('answers the HTTP status of /json/version asked with no token', async () => {
    const seen: (string | undefined)[] = [];
    const server = createServer((request, response) => {
      seen.push(request.headers.authorization);
      response.writeHead(request.url === '/json/version' ? 401 : 404);
      response.end();
    });
    await new Promise<void>((done) => server.listen(0, '127.0.0.1', () => done()));
    const { port } = server.address() as AddressInfo;
    try {
      expect(await statusWithout(`http://127.0.0.1:${port}`)).toBe(401);
      expect(seen).toEqual([undefined]);
    } finally {
      server.close();
    }
  });

  it('strips a token in the path form before it asks', async () => {
    const paths: string[] = [];
    const server = createServer((request, response) => {
      paths.push(request.url ?? '');
      response.writeHead(403);
      response.end();
    });
    await new Promise<void>((done) => server.listen(0, '127.0.0.1', () => done()));
    const { port } = server.address() as AddressInfo;
    try {
      expect(await statusWithout(`http://127.0.0.1:${port}/`)).toBe(403);
      expect(paths).toEqual(['/json/version']);
    } finally {
      server.close();
    }
  });
});
