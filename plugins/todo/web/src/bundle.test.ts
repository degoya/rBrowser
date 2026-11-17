// @vitest-environment node
// The committed panel/dist is what `npm run build` writes from web/src, within its size budget
// (RB-080-83): scripts/build-plugins.sh packs it without npm.
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { checkBuilt } from '../../../web-common/panel-ui/vite.panel.ts';
import { PANEL } from '../vite.config.ts';

describe('the built panel', () => {
  it("is fresh and within its budget", () => {
    const { stale, over } = checkBuilt(PANEL);
    expect(stale, 'run `npm run build` in plugins/todo/web').toEqual([]);
    expect(over).toEqual([]);
  }, 120_000);

  it('carries the icons of the rows, the six heading levels included (RB-110-15)', () => {
    // The tests run in plugins/todo/web (`npm test`).
    const script = readFileSync(join(process.cwd(), '..', 'panel', 'dist', 'panel.js'), 'utf8');
    for (const name of ['folder', 'text', 'heading-1', 'heading-2', 'heading-3', 'heading-4', 'heading-5', 'heading-6']) {
      expect(script, name).toContain(`"${name}":{"width":24`);
    }
  });
});
