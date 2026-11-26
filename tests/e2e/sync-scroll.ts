// The scroll sync fixture (RB-020-04's end-to-end case): tests/fixtures/site/scroll.html with
// sections whose heights differ per breakpoint, an inner sidebar that scrolls on its own and a
// table that scrolls sideways. Wheel input in the leader - over the page, the sidebar and the
// table - must move the same thing in every other view: the page to the same section (the anchor
// mode, not the same pixels), the sidebar and the table by their own scroll positions.

import { measure, outcome } from './lib/case.ts';
import type { Case } from './lib/case.ts';
import { centreOf, evaluateIn } from './lib/cdp.ts';
import type { Cdp } from './lib/cdp.ts';
import { needSession, site } from './lib/context.ts';
import { pause, waitFor } from './lib/measure.ts';
import type { PageView } from './lib/views.ts';
import { cdpViews } from './lib/views.ts';

/** The section the leader scrolls to and where its top must be in every view: under the fixed bar. */
const SECTION = '#s5';
const TOP_BAR = 56;
const TOLERANCE = 48;

async function wheel(cdp: Cdp, leader: PageView, x: number, y: number, deltaX: number, deltaY: number, times: number) {
  for (let i = 0; i < times; i += 1) {
    await cdp.call('Input.dispatchMouseEvent', { type: 'mouseWheel', x, y, deltaX, deltaY }, leader.session);
    await pause(50);
  }
}

/** Whether every other view satisfies `expression` within the deadline. */
async function followed(cdp: Cdp, others: PageView[], expression: string): Promise<boolean> {
  return waitFor(
    async () => {
      const all = await Promise.all(others.map((page) => evaluateIn<boolean>(cdp, page.session, expression)));
      return all.every(Boolean) ? true : undefined;
    },
    { timeoutMs: 5_000, stepMs: 100, what: expression },
  ).catch(() => false);
}

export const syncScroll: Case = {
  id: 'SYNC',
  title: 'Scroll sync: page, container, sideways',
  threshold: 'the section, the sidebar and the table follow in every view',
  project: 'e2e',
  needs: [],
  jobs: ['RB-020-04'],
  async run(ctx) {
    const session = needSession(ctx);
    await session.navigateAll(site(ctx, '/scroll.html'));
    const views = await cdpViews(session);
    try {
      const { cdp, leader, others } = views;
      // The page: wheel down until the section's top sits under the bar in the leader.
      await waitFor(
        async () => {
          const top = await evaluateIn<number>(cdp, leader.session, `document.querySelector('${SECTION}').getBoundingClientRect().top`);
          if (top <= TOP_BAR + TOLERANCE) return true;
          await wheel(cdp, leader, 400, 400, 0, Math.min(240, top - TOP_BAR), 1);
          return undefined;
        },
        { timeoutMs: 20_000, stepMs: 50, what: `${SECTION} at the top of the leader` },
      );
      const page = await followed(
        cdp,
        others,
        `Math.abs(document.querySelector('${SECTION}').getBoundingClientRect().top - ${TOP_BAR}) <= ${TOLERANCE}`,
      );
      // The sidebar: scrolls on its own.
      await evaluateIn(cdp, leader.session, 'window.scrollTo(0, 0)');
      const sidebar = await centreOf(cdp, leader.session, '#sidebar');
      if (sidebar !== null) await wheel(cdp, leader, sidebar.x, sidebar.y, 0, 120, 4);
      const container = await followed(cdp, others, `document.querySelector('#sidebar').scrollTop > 0`);
      // The table: sideways.
      const table = await centreOf(cdp, leader.session, '#table-wrap');
      if (table !== null) await wheel(cdp, leader, table.x, table.y, 120, 0, 4);
      const sideways = await followed(cdp, others, `document.querySelector('#table-wrap').scrollLeft > 0`);
      return outcome([
        measure('page: same section in view', page, 'true', page),
        measure('sidebar scrolled', container, 'true', container),
        measure('table scrolled sideways', sideways, 'true', sideways),
      ]);
    } finally {
      views.close();
    }
  },
};
