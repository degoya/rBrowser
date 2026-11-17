import { describe, expect, it } from 'vitest';
import { BUNDLES, bundle } from '../scripts/bundle.mjs';
import pageScript from '../../page/bridge.js?raw';
import reactPanel from '../../panel/dist/react.js?raw';
import vuePanel from '../../panel/dist/vue.js?raw';

const committed: Record<string, string> = {
  '../page/bridge.js': pageScript,
  '../panel/dist/react.js': reactPanel,
  '../panel/dist/vue.js': vuePanel,
};

describe('the committed bundles', () => {
  it('are built from the sources (npm run build) and within their budgets', async () => {
    expect(BUNDLES.map((spec) => spec.out)).toEqual(Object.keys(committed));
    for (const spec of BUNDLES) {
      const fresh = await bundle(spec);
      expect(committed[spec.out] === fresh, `${spec.out} is stale`).toBe(true);
      expect(new TextEncoder().encode(fresh).length).toBeLessThanOrEqual(spec.budget);
    }
  });

  it('open no port and no socket', () => {
    for (const [name, text] of Object.entries(committed)) {
      expect(text.includes('new WebSocket('), `${name} opens a WebSocket`).toBe(false);
      expect(text.includes('EventSource('), `${name} opens an EventSource`).toBe(false);
    }
  });

  it('leaves no global behind as a page script and relays nothing on its own', () => {
    const before = new Set(Object.getOwnPropertyNames(window));
    const emitted: unknown[] = [];
    const rbPlugin = {
      id: 'framework-devtools',
      view: 1,
      maxMessageBytes: 65536,
      send: () => Promise.resolve({}),
      emit: (message: unknown) => {
        emitted.push(message);
      },
      on: () => () => undefined,
      onStop: () => () => undefined,
    };
    // As the runtime runs it: one function in strict mode with `rbPlugin` in scope.
    new Function('rbPlugin', `'use strict';\n${pageScript}`)(rbPlugin);
    const added = Object.getOwnPropertyNames(window).filter((name) => !before.has(name));
    expect(added).toEqual([]);
    expect(emitted).toEqual([]);
  });
});
