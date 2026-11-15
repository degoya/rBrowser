import { describe, expect, it } from 'vitest';
import { BUNDLES, bundle } from '../scripts/bundle.mjs';
import pageScript from '../../page/viewer.js?raw';

describe('the committed page script', () => {
  it('is built from the sources (npm run build)', async () => {
    const spec = BUNDLES[0];
    expect(spec?.out).toBe('../page/viewer.js');
    expect(pageScript === (await bundle(spec as (typeof BUNDLES)[number]))).toBe(true);
  });

  it('leaves no global behind and does nothing on a page that is no JSON or XML', async () => {
    const before = new Set(Object.getOwnPropertyNames(window));
    const sent: unknown[] = [];
    const rbPlugin = {
      id: 'json-xml-viewer',
      view: 1,
      maxMessageBytes: 65536,
      send: (message: unknown) => {
        sent.push(message);
        return Promise.resolve({});
      },
      emit: () => undefined,
      on: () => () => undefined,
      onStop: () => () => undefined,
    };
    // As the runtime runs it: one function in strict mode with `rbPlugin` in scope.
    new Function('rbPlugin', `'use strict';\n${pageScript}`)(rbPlugin);
    await Promise.resolve();
    const added = Object.getOwnPropertyNames(window).filter((name) => !before.has(name));
    expect(added).toEqual([]);
    expect(sent).toEqual([]);
  });
});
