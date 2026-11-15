// @vitest-environment node
// The committed builds are what `npm run build` writes from web/src: the page part (esbuild,
// scripts/bundle.mjs) and the panel (Vite, RB-080-83), each within its size budget.
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { checkBuilt } from '../../../web-common/panel-ui/vite.panel.ts';
import { PANEL } from '../vite.config.ts';
import { BUNDLES, bundle } from '../scripts/bundle.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const page = readFileSync(join(here, '..', '..', 'src', 'page.js'), 'utf8');

describe('the committed bundles', () => {
  it('the page part is built from the sources (npm run build)', async () => {
    expect(BUNDLES.map((spec) => spec.out)).toEqual(['../src/page.js']);
    expect(page === (await bundle(BUNDLES[0] as (typeof BUNDLES)[number]))).toBe(true);
  });

  it('the page part defines FontsPage.run, mark and clear for the component', () => {
    expect(page).toContain('var FontsPage');
    for (const name of ['run', 'mark', 'clear']) expect(page).toContain(name);
  });

  it('the marks live in an rbrowser-fonts element, which the sync agent ignores', () => {
    expect(page).toContain('"rbrowser-fonts"');
  });

  it('the panel is fresh and within its budget', () => {
    const { stale, over } = checkBuilt(PANEL);
    expect(stale, 'run `npm run build` in plugins/fonts/web').toEqual([]);
    expect(over).toEqual([]);
  }, 120_000);
});
