// @vitest-environment node
// The committed builds are what `npm run build` writes from web/src: the page script and the
// analysis (esbuild, scripts/bundle.mjs) and the panel (Vite), each within its size budget.
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { checkBuilt } from '../../../web-common/panel-ui/vite.panel.ts';
import { BUNDLES, bundle } from '../scripts/bundle.mjs';
import { PANEL } from '../vite.config.ts';

const here = dirname(fileURLToPath(import.meta.url));

describe('the committed bundles', () => {
  it.each(BUNDLES)('$out is built from the sources (npm run build)', async (spec) => {
    const committed = readFileSync(join(here, '..', spec.out), 'utf8');
    expect(committed === (await bundle(spec)), `run \`npm run build\` in plugins/container-queries/web`).toBe(true);
  });

  it('the analysis defines __rbContainers with analyze, resize and reset', () => {
    const analysis = readFileSync(join(here, '..', '..', 'assets', 'analyze.js'), 'utf8');
    expect(analysis).toContain('var __rbContainers');
    for (const name of ['analyze', 'resize', 'reset']) expect(analysis).toContain(name);
  });

  it('the panel is fresh and within its budget', () => {
    const { stale, over } = checkBuilt(PANEL);
    expect(stale, 'run `npm run build` in plugins/container-queries/web').toEqual([]);
    expect(over).toEqual([]);
  }, 120_000);
});
