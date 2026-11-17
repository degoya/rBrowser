// The SEO panel in panel-ui's e2e (RB-080-85, RB-080-99): answered with the snapshot of the
// fixture `social-ok` (plugins/seo/tests/groups.rs writes it), one card turned small by its rule
// so the toggle to the other variant is checked in the real frame.
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const preview = JSON.parse(readFileSync(join(here, '..', '..', '..', '..', 'seo', 'tests', 'snapshots', 'social-ok.preview.json'), 'utf8'));
const facebook = preview.cards.find((card) => card.id === 'facebook');
facebook.layout = 'small';
facebook.alternate = 'large';
facebook.reason = { code: 'below', params: { width: '400', height: '400', min_width: '600', min_height: '315' } };

export default {
  title: 'Social previews',
  tooltip: '#refresh',
  answer: (call) => (call.tool === 'social_preview' ? preview : {}),
  /** The share image shows from its data: URI; the toggle shows the other variant below. */
  check: async (frame, expect) => {
    const card = frame.locator('[data-platform="facebook"]');
    await card.locator('img').first().waitFor({ timeout: 5000 });
    await card.locator('[data-toggle]').click();
    const layouts = await card.locator('[data-preview]').evaluateAll((all) => all.map((one) => one.dataset.layout));
    expect('seo: the toggle shows the other variant below the chosen one', JSON.stringify(layouts) === '["small","large"]', layouts);
  },
};
