// K4 (RB-020-02): a form submit produces exactly 1 POST at the server; every other view then
// shows the target URL. tests/fixtures/site/form.html posts to /submit, which the fixture server
// counts and answers with 303 to /form.html?posts=<n>; the submit is a real click in the leader.
// The other views arriving at the target URL is the barrier: a second POST from one of them
// would have reached the server before it.

import { measure, outcome } from './lib/case.ts';
import type { Case } from './lib/case.ts';
import { centreOf, clickAt } from './lib/cdp.ts';
import { needSession, site } from './lib/context.ts';
import { waitFor } from './lib/measure.ts';
import { cdpViews } from './lib/views.ts';

const TARGET = '/form.html?posts=1';

export const k04: Case = {
  id: 'K4',
  title: 'Leader-only submit',
  threshold: 'exactly 1 POST; every view on the target URL',
  project: 'e2e',
  needs: [],
  jobs: ['RB-020-02'],
  async run(ctx) {
    const session = needSession(ctx);
    await session.navigateAll(site(ctx, '/form.html'));
    ctx.server.reset();
    const views = await cdpViews(session);
    try {
      const point = await centreOf(views.cdp, views.leader.session, '[data-testid="submit"]');
      if (point === null) throw new Error('form.html has no submit button');
      await clickAt(views.cdp, views.leader.session, point.x, point.y);
      const arrived = await waitFor(
        async () => {
          const urls = await session.evaluateAll<string>('location.pathname + location.search');
          return urls.length > 0 && urls.every((entry) => entry.value === TARGET) ? urls.length : undefined;
        },
        { timeoutMs: 15_000, stepMs: 100, what: `every view on ${TARGET}` },
      ).catch(() => 0);
      const total = (await session.views()).length;
      const posts = ctx.server.posts();
      return outcome([
        measure('POSTs at the server', posts, '= 1', posts === 1),
        measure('views on the target URL', `${arrived} of ${total}`, 'all', arrived === total && total > 1),
      ]);
    } finally {
      views.close();
    }
  },
};
