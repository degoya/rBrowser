// K2 (RB-020-02): when the click's target is missing in a view, that view shows the "desynced"
// badge within 1 s and the other views perform the click. tests/fixtures/site/mobile-menu.html
// removes its desktop menu below 700 px: a click on "Pricing" in the desktop view is `not_found`
// in the mobile view (393 px) and replayed in the tablet view (768 px). The badge is the view's
// `desync` in `views_list`; the clicks are the page's own log, window.__clicks.

import { measure, outcome } from './lib/case.ts';
import type { Case } from './lib/case.ts';
import { centreOf, clickAt, evaluateIn } from './lib/cdp.ts';
import { needSession, site } from './lib/context.ts';
import { waitFor } from './lib/measure.ts';
import { cdpViews } from './lib/views.ts';

const BADGE_MS = 1000;
const NARROW = 700;
const TARGET = '[data-testid="nav-pricing"]';
const CLICKED = `(window.__clicks || []).some((entry) => entry.target === 'nav-pricing')`;

export const k02: Case = {
  id: 'K2',
  title: 'Desync badge for a missing target',
  threshold: `badge <= ${BADGE_MS} ms in the narrow view, the click replayed in the others`,
  project: 'e2e',
  needs: [],
  jobs: ['RB-020-02'],
  async run(ctx) {
    const session = needSession(ctx);
    await session.navigateAll(site(ctx, '/mobile-menu.html'));
    const views = await cdpViews(session);
    try {
      const point = await centreOf(views.cdp, views.leader.session, TARGET);
      if (point === null) throw new Error(`the leader has no ${TARGET}`);
      const started = Date.now();
      await clickAt(views.cdp, views.leader.session, point.x, point.y);
      const narrow = await waitFor(
        async () => {
          const desynced = (await session.views()).filter((view) => view.width < NARROW && view.desync !== null);
          return desynced.length > 0 ? desynced : undefined;
        },
        { timeoutMs: 10_000, stepMs: 50, what: 'the desync badge of the narrow view' },
      );
      const badgeMs = Date.now() - started;
      const wide = views.others.filter((page) => page.width >= NARROW);
      const followed = await waitFor(
        async () => {
          const done = await Promise.all(wide.map((page) => evaluateIn<boolean>(views.cdp, page.session, CLICKED)));
          return done.every(Boolean) ? true : undefined;
        },
        { timeoutMs: 5_000, stepMs: 50, what: 'the click in the wide views' },
      ).catch(() => false);
      // The badge goes again for the next case.
      await session.mcp.call('sync_set', { action: 'resync', target: 'all' });
      return outcome(
        [
          measure('badge after', badgeMs, `<= ${BADGE_MS} ms`, badgeMs <= BADGE_MS, 'ms'),
          measure('desync reason', narrow[0]?.desync?.reason ?? null, 'set', narrow[0]?.desync?.reason !== undefined),
          measure('other views clicked', followed, 'true', followed && wide.length > 0),
        ],
        ['the badge time includes one views_list round trip: an upper estimate'],
      );
    } finally {
      views.close();
    }
  },
};
