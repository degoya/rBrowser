// K1 (RB-020-03): a click in one Chromium view is replayed on the same element in every other
// Chromium view within 150 ms (p95). Measured by scripts/perf.sh's click run
// (scripts/lib/perf-k1.mjs: 100 clicks on tests/fixtures/site/latency.html, each view's
// timestamp log), so the figure is the one docs/perf-budget.md records.

import { measure, outcome, failed } from './lib/case.ts';
import type { Case } from './lib/case.ts';
import { needSession, site } from './lib/context.ts';
import { runPerf } from './lib/perf.ts';

const CLICKS = 100;
const BUDGET_MS = 150;

export const k01: Case = {
  id: 'K1',
  title: 'Click replay latency',
  threshold: `p95 <= ${BUDGET_MS} ms over ${CLICKS} clicks, none missed`,
  project: 'e2e',
  needs: [],
  jobs: ['RB-020-03'],
  async run(ctx) {
    const session = needSession(ctx);
    // perf-k1.mjs opens latency.html on the leader's origin: start on the fixture server.
    await session.navigateAll(site(ctx, '/sync.html'));
    const run = await runPerf(ctx, 'perf-k1', ['--runs', '0', '--clicks', String(CLICKS)], await session.endpointAnswer());
    const k1 = run.result?.k1 as { p95Ms?: number; missed?: number; pairs?: number } | undefined;
    if (k1?.p95Ms === undefined) return failed(`scripts/perf.sh wrote no click result (exit ${String(run.code)})`);
    return outcome([
      measure('p95', k1.p95Ms, `<= ${BUDGET_MS} ms`, k1.p95Ms <= BUDGET_MS, 'ms'),
      measure('missed clicks', k1.missed ?? 0, '= 0', (k1.missed ?? 0) === 0),
      measure('pairs', k1.pairs ?? 0, '> 0', (k1.pairs ?? 0) > 0),
    ]);
  },
};
