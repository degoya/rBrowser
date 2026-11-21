// @vitest-environment node
// The committed builds are what `npm run build` writes from web/src: the collector (esbuild,
// scripts/bundle.mjs) and the panel (Vite, RB-093-02), each within its size budget.
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { checkBuilt } from '../../../web-common/panel-ui/vite.panel.ts';
import { PANEL } from '../vite.config.ts';
import { BUNDLES, bundle } from '../scripts/bundle.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const collector = readFileSync(join(here, '..', '..', 'src', 'collect.js'), 'utf8');

describe('the committed bundles', () => {
  it('the collector is built from the sources (npm run build)', async () => {
    expect(BUNDLES.map((spec) => spec.out)).toEqual(['../src/collect.js']);
    expect(collector === (await bundle(BUNDLES[0] as (typeof BUNDLES)[number]))).toBe(true);
  });

  it('the collector defines ResponsiveImagesCollect.run for the component', () => {
    expect(collector).toContain('var ResponsiveImagesCollect');
    expect(collector).toContain('run');
  });

  it('the panel is fresh and within its budget', () => {
    const { stale, over } = checkBuilt(PANEL);
    expect(stale, 'run `npm run build` in plugins/responsive-images/web').toEqual([]);
    expect(over).toEqual([]);
  }, 120_000);
});
