// Milestone 0.8, layouts (M10, docs/roadmap.md "0.8": the layout modes horizontal, split and
// canvas): each mode is set over MCP and keeps every view; the grid comes back at the end.

import { measure, outcome } from './lib/case.ts';
import type { Case } from './lib/case.ts';
import { needSession } from './lib/context.ts';
import type { Measurement } from './lib/report.ts';

const MODES = ['horizontal', 'split', 'canvas'];

export const m10: Case = {
  id: 'M10',
  title: 'Milestone 0.8 layout modes',
  threshold: 'horizontal, split and canvas set without losing a view',
  project: 'e2e',
  needs: [],
  jobs: ['RB-080-04'],
  async run(ctx) {
    const session = needSession(ctx);
    const before = (await session.views()).length;
    const measurements: Measurement[] = [];
    try {
      for (const mode of MODES) {
        const set = await session.mcp.call('layout_set', { mode });
        const after = (await session.views()).length;
        if (set.isError) ctx.log(`${mode}: ${set.text.slice(0, 300)}`);
        measurements.push(measure(mode, set.isError ? 'refused' : `${after} views`, `${before} views`, !set.isError && after === before));
      }
    } finally {
      await session.mcp.call('layout_set', { mode: 'grid' });
    }
    return outcome(measurements);
  },
};
