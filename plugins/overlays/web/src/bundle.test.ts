// @vitest-environment node
// The committed panel/dist is what `npm run build` writes from web/src, within its size budget
// (RB-080-86): scripts/build-plugins.sh packs it without npm.
import { describe, expect, it } from 'vitest';
import { checkBuilt } from '../../../web-common/panel-ui/vite.panel.ts';
import { PANEL } from '../vite.config.ts';

describe('the built panel', () => {
  it('is fresh and within its budget', () => {
    const { stale, over } = checkBuilt(PANEL);
    expect(stale, 'run `npm run build` in plugins/overlays/web').toEqual([]);
    expect(over).toEqual([]);
  }, 120_000);
});
