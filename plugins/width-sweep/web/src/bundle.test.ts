import { describe, expect, it } from 'vitest';
import { BUNDLES, bundle } from '../scripts/bundle.mjs';
import detector from '../../src/detect.js?raw';

describe('the committed detector', () => {
  for (const spec of BUNDLES) {
    it(`${spec.out} is built from the sources (npm run build)`, async () => {
      expect(detector === (await bundle(spec))).toBe(true);
    });
  }

  it('defines WidthSweepDetect.probe for the component', () => {
    expect(detector).toContain('var WidthSweepDetect');
    expect(detector).toContain('probe');
  });
});
