// What one criterion of the end-to-end run is (RB-099-02): a file `kNN-*.ts` (K1-K12, the
// acceptance criteria of docs/roadmap.md), `mNN-*.ts` (a milestone smoke) or `sync-*.ts`, each
// exporting one `Case`. The runner opens the project a case names, runs it and puts its outcome
// into the report; a case measures, it never decides alone that the app is broken without a
// value and its threshold.

import type { Engine } from './args.ts';
import type { Context } from './context.ts';
import type { Measurement, Outcome } from './report.ts';

/**
 * The project a case runs in: none (no app, or only `rbrowser.com`), the e2e project
 * (tests/fixtures/e2e-project, three Chromium views), the untrusted one (K9) or the perf one
 * (tests/fixtures/project-perf, 8 views, K3). The runner starts one app per project, in turn.
 */
export type ProjectKind = 'none' | 'e2e' | 'untrusted' | 'perf';

export interface Case {
  /** `K4`, `M10`, `SYNC`. */
  id: string;
  title: string;
  /** The threshold in words, for the plan and the report. */
  threshold: string;
  project: ProjectKind;
  /** Engines the case needs; one missing fails it unless `--allow-missing` names it. */
  needs: Engine[];
  /** The jobs whose end-to-end case this carries. */
  jobs: string[];
  run(ctx: Context): Promise<Outcome>;
}

export function measure(
  name: string,
  value: Measurement['value'],
  threshold: string,
  ok: boolean,
  unit?: string,
): Measurement {
  return unit === undefined ? { name, value, threshold, ok } : { name, value, threshold, ok, unit };
}

/** Passed when there is a measurement and every one holds. */
export function outcome(measurements: Measurement[], notes: string[] = []): Outcome {
  const ok = measurements.length > 0 && measurements.every((entry) => entry.ok);
  const failedOnes = measurements.filter((entry) => !entry.ok).map((entry) => entry.name);
  return ok
    ? { status: 'passed', measurements, notes }
    : {
        status: 'failed',
        measurements,
        notes,
        error: failedOnes.length > 0 ? `missed: ${failedOnes.join(', ')}` : 'nothing was measured',
      };
}

export function failed(error: string, measurements: Measurement[] = [], notes: string[] = []): Outcome {
  return { status: 'failed', measurements, notes, error };
}

export function skipped(note: string): Outcome {
  return { status: 'skipped', measurements: [], notes: [note] };
}
