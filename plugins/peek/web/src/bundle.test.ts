import { describe, expect, it } from 'vitest';
import { BUNDLES, bundle } from '../scripts/bundle.mjs';
import collector from '../../src/collect.js?raw';
import pageScript from '../../page/peek.js?raw';

const COMMITTED: Record<string, string> = {
  '../page/peek.js': pageScript,
  '../src/collect.js': collector,
};

describe('the committed bundles', () => {
  for (const spec of BUNDLES) {
    it(`${spec.out} is built from the sources (npm run build)`, async () => {
      expect(COMMITTED[spec.out] === (await bundle(spec))).toBe(true);
    });
  }

  it('the collector defines PeekCollect.inspect for the component', () => {
    expect(collector).toContain('var PeekCollect');
    expect(collector).toContain('inspect');
  });
});
