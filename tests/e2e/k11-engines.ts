// K11 (RB-040-02, RB-040-04): Firefox and WebKit windows follow navigation and scrolling within
// 300 ms (p95); properties that cannot be emulated are marked on the view. The tablet view turns
// Firefox (the installed one, RB-040-02's own end-to-end case), the mobile view WebKit; the
// desktop view leads over CDP (navigations, wheel scrolling, clicks). The replay times are the
// app's own counters (MCP `app_perf`), held to the budget by scripts/perf.sh --counters as
// docs/perf-budget.md does; the marks are each external view's `unsupported` in `views_list`.

import { join } from 'node:path';

import { measure, outcome } from './lib/case.ts';
import type { Case } from './lib/case.ts';
import { centreOf, clickAt, evaluateIn } from './lib/cdp.ts';
import { missingEngine, needSession, site } from './lib/context.ts';
import { pause, waitFor } from './lib/measure.ts';
import { runPerf, writePrivate } from './lib/perf.ts';
import type { Session } from './lib/session.ts';
import { cdpViews } from './lib/views.ts';

const BUDGET_MS = 300;
const ENGINES = { firefox: 'tablet', webkit: 'mobile' } as const;

interface Counters {
  counters?: { name?: string; count?: number }[];
}

const countOf = (answer: Counters, name: string): number =>
  answer.counters?.find((counter) => counter.name === name)?.count ?? 0;

/** The leader's input: two navigations, wheel scrolling, clicks - with the pauses a person makes. */
async function lead(session: Session, scroll: string, latency: string): Promise<void> {
  const views = await cdpViews(session);
  try {
    const { cdp, leader } = views;
    const open = async (url: string) => {
      await cdp.call('Page.navigate', { url }, leader.session);
      await waitFor(
        async () => ((await evaluateIn(cdp, leader.session, `location.href === ${JSON.stringify(url)} && document.readyState === 'complete'`)) ? true : undefined),
        { timeoutMs: 30_000, stepMs: 100, what: `${url} in the leader` },
      );
    };
    await open(scroll);
    for (let i = 0; i < 60; i += 1) {
      const deltaY = Math.floor(i / 15) % 2 === 0 ? 120 : -120;
      await cdp.call('Input.dispatchMouseEvent', { type: 'mouseWheel', x: 300, y: 300, deltaX: 0, deltaY }, leader.session);
      await pause(100);
    }
    await open(latency);
    for (let i = 1; i <= 20; i += 1) {
      const point = await centreOf(cdp, leader.session, `[data-testid="t${i}"]`);
      if (point !== null) await clickAt(cdp, leader.session, point.x, point.y);
      await pause(250);
    }
  } finally {
    views.close();
  }
}

export const k11: Case = {
  id: 'K11',
  title: 'Firefox and WebKit follow',
  threshold: `replay p95 <= ${BUDGET_MS} ms per engine; unsupported properties marked`,
  project: 'e2e',
  needs: ['firefox', 'webkit'],
  jobs: ['RB-040-02', 'RB-040-04'],
  async run(ctx) {
    const session = needSession(ctx);
    const engines = await session.mcp.value<{ firefox?: { installed?: unknown[] } }>('engines_list');
    if ((engines.firefox?.installed ?? []).length === 0) return missingEngine(ctx, 'firefox', 'engines_list finds no installation');
    // By breakpoint: a view that changes its engine may come back with another id.
    const switched: string[] = [];
    try {
      for (const [engine, breakpoint] of Object.entries(ENGINES)) {
        const set = await session.mcp.call('views_set_engine', { target: breakpoint, engine });
        if (set.isError) return missingEngine(ctx, engine as 'firefox' | 'webkit', set.text.slice(0, 200));
        switched.push(breakpoint);
      }
      await waitFor(
        async () => {
          const now = await session.views();
          return Object.entries(ENGINES).every(([engine, breakpoint]) =>
            now.some((view) => view.breakpoint === breakpoint && view.engine === engine && view.loading !== true),
          )
            ? true
            : undefined;
        },
        { timeoutMs: 60_000, stepMs: 500, what: 'the Firefox and WebKit views' },
      );
      await session.mcp.value('app_perf', { reset: true });
      await lead(session, site(ctx, '/scroll.html'), site(ctx, '/latency.html'));
      const counters = await waitFor(
        async () => {
          const answer = await session.mcp.value<Counters>('app_perf');
          return countOf(answer, 'sync.replay.firefox') > 0 && countOf(answer, 'sync.replay.webkit') > 0 ? answer : undefined;
        },
        { timeoutMs: 30_000, stepMs: 500, what: 'replays into Firefox and WebKit counted' },
      );
      const file = join(ctx.work, 'k11-app-perf.json');
      writePrivate(file, counters);
      const run = await runPerf(ctx, 'perf-k11', ['--runs', '0', '--clicks', '0', '--counters', file], await session.endpointAnswer());
      const k11 = (run.result?.k11 ?? {}) as { firefoxP95Ms?: number | null; webkitP95Ms?: number | null };
      const external = (await session.views()).filter((view) => view.engine === 'firefox' || view.engine === 'webkit');
      const marked = external.filter((view) => (view.unsupported ?? []).length > 0).map((view) => view.breakpoint);
      const p95 = (value: number | null | undefined) => (typeof value === 'number' ? value : Number.NaN);
      return outcome([
        measure('Firefox replay p95', p95(k11.firefoxP95Ms), `<= ${BUDGET_MS} ms`, p95(k11.firefoxP95Ms) <= BUDGET_MS, 'ms'),
        measure('WebKit replay p95', p95(k11.webkitP95Ms), `<= ${BUDGET_MS} ms`, p95(k11.webkitP95Ms) <= BUDGET_MS, 'ms'),
        measure('views with marks', marked.join(', ') || 'none', 'the mobile view (DPR 3, touch)', marked.includes('mobile')),
      ]);
    } finally {
      for (const breakpoint of switched) {
        await session.mcp.call('views_set_engine', { target: breakpoint, engine: 'chromium' });
      }
    }
  },
};
