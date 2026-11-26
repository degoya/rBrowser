// Which cases run, in which order (RB-099-02): `--only` picks, the cases without an app run
// first, then one app per project - the perf project (K3) last and alone, since
// scripts/perf.sh sums the memory of every rbrowser.exe that runs.

import type { Engine, Mode } from './args.ts';
import type { Case, ProjectKind } from './case.ts';

export const GROUP_ORDER: readonly ProjectKind[] = ['none', 'e2e', 'untrusted', 'perf'];

/** Where each project comes from (copied into the work folder before its app starts). */
export const PROJECT_SOURCE: Record<ProjectKind, string> = {
  none: '',
  e2e: 'tests/fixtures/e2e-project',
  untrusted: 'tests/fixtures/untrusted-project',
  perf: 'tests/fixtures/project-perf',
};

export interface Group {
  project: ProjectKind;
  cases: Case[];
}

export function select(cases: readonly Case[], only: readonly string[] | null): Case[] {
  return only === null ? [...cases] : cases.filter((entry) => only.includes(entry.id));
}

export function grouped(cases: readonly Case[]): Group[] {
  return GROUP_ORDER.map((project) => ({ project, cases: cases.filter((entry) => entry.project === project) })).filter(
    (group) => group.cases.length > 0,
  );
}

export function planText(
  groups: readonly Group[],
  options: { mode: Mode; app: string | null; allowMissing: ReadonlySet<Engine> },
): string {
  const lines = [`rBrowser end-to-end plan: mode ${options.mode}, app ${options.app ?? '(staged by scripts/e2e-windows.sh)'}`];
  groups.forEach((group, index) => {
    const where = group.project === 'none' ? 'no app' : `app on ${PROJECT_SOURCE[group.project]}`;
    lines.push(`${index + 1}. ${where}`);
    for (const entry of group.cases) {
      const needs = entry.needs
        .map((engine) => `needs ${engine}${options.allowMissing.has(engine) ? ': allowed missing' : ''}`)
        .join(', ');
      lines.push(`   ${entry.id}  ${entry.title} - ${entry.threshold}${needs ? ` [${needs}]` : ''}`);
    }
  });
  return `${lines.join('\n')}\n`;
}
