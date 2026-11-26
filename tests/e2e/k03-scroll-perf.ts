// K3 (RB-020-09): with 8 views, 2 of them at DPR 3 (tests/fixtures/project-perf), the active view
// scrolls at 50 fps or more and every rBrowser process together stays under 3 GB. Measured by
// scripts/perf.sh (three runs of 10 s, the runs no more than 10 % apart) - docs/perf-budget.md.
// perf.sh sums the memory of every rbrowser.exe: the run starts this app last and alone.

import { failed, measure, outcome } from './lib/case.ts';
import type { Case } from './lib/case.ts';
import { needSession } from './lib/context.ts';
import { runPerf } from './lib/perf.ts';

const FPS = 50;
const MEMORY_MB = 3072;

export const k03: Case = {
  id: 'K3',
  title: 'Scrolling with 8 views',
  threshold: `>= ${FPS} fps (min of 3 runs), < ${MEMORY_MB} MB, 8 views with 2 at DPR 3`,
  project: 'perf',
  needs: [],
  jobs: ['RB-020-09'],
  async run(ctx) {
    if (ctx.processesBefore > 0) {
      return failed(`${ctx.processesBefore} other rbrowser.exe ran before this app started: perf.sh would add their memory`);
    }
    const session = needSession(ctx);
    const run = await runPerf(
      ctx,
      'perf-k3',
      ['--views', '8', '--dpr3', '2', '--runs', '3', '--seconds', '10', '--clicks', '0'],
      await session.endpointAnswer(),
    );
    const result = run.result as {
      fps?: { min?: number; spreadPct?: number };
      memoryMb?: { max?: number };
      seen?: { views?: number; dpr3?: number };
    } | null;
    if (result?.fps?.min === undefined) return failed(`scripts/perf.sh wrote no result (exit ${String(run.code)})`);
    const memory = result.memoryMb?.max ?? Number.NaN;
    const spread = result.fps.spreadPct ?? Number.NaN;
    return outcome([
      measure('fps (min)', result.fps.min, `>= ${FPS}`, result.fps.min >= FPS),
      measure('memory (max)', memory, `< ${MEMORY_MB} MB`, memory < MEMORY_MB, 'MB'),
      measure('views seen', `${result.seen?.views ?? 0} (${result.seen?.dpr3 ?? 0} at DPR 3)`, '8 (2 at DPR 3)',
        result.seen?.views === 8 && result.seen.dpr3 === 2),
      measure('spread', spread, '< 10 %', spread < 10, '%'),
    ]);
  },
};
