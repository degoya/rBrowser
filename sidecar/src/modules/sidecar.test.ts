import { describe, expect, it } from 'vitest';

import { Router } from '../router.ts';
import { registerSidecar } from './sidecar.ts';

const ctx = { signal: new AbortController().signal, progress: () => {} };

describe('sidecar module', () => {
  it('answers a ping', async () => {
    const router = new Router();
    registerSidecar(router, { playwrightVersion: () => '1.65.0', browsers: () => [] });
    expect(await router.call('sidecar.ping', undefined, ctx)).toEqual({ pong: true });
  });

  it('reports Node, Playwright, the browsers and the methods', async () => {
    const router = new Router();
    const browsers = [{ name: 'webkit', revision: '2388', version: '27.2', installed: false, path: null }];
    registerSidecar(router, { playwrightVersion: () => '1.65.0', browsers: () => browsers });
    expect(await router.call('sidecar.versions', {}, ctx)).toEqual({
      node: process.versions.node,
      playwright: '1.65.0',
      browsers,
      methods: ['sidecar.ping', 'sidecar.versions'],
    });
  });
});
