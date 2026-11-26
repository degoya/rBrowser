import { describe, expect, it } from 'vitest';

import { measure, outcome } from './case.ts';
import type { Case } from './case.ts';
import { grouped, planText, select } from './plan.ts';

const make = (id: string, project: Case['project'], needs: Case['needs'] = []): Case => ({
  id,
  title: `title ${id}`,
  threshold: `threshold ${id}`,
  project,
  needs,
  jobs: ['RB-000-00'],
  run: async () => outcome([]),
});
const cases = [make('K1', 'e2e'), make('K3', 'perf'), make('K5', 'none'), make('K9', 'untrusted'), make('K11', 'e2e', ['firefox'])];

describe('select', () => {
  it('keeps every case without --only, else the named ones in the registry order', () => {
    expect(select(cases, null).map((c) => c.id)).toEqual(['K1', 'K3', 'K5', 'K9', 'K11']);
    expect(select(cases, ['K11', 'K1']).map((c) => c.id)).toEqual(['K1', 'K11']);
  });
});

describe('grouped', () => {
  it('runs the cases without an app first, then one app per project, the perf one last', () => {
    expect(grouped(cases).map((group) => [group.project, group.cases.map((c) => c.id)])).toEqual([
      ['none', ['K5']],
      ['e2e', ['K1', 'K11']],
      ['untrusted', ['K9']],
      ['perf', ['K3']],
    ]);
  });

  it('drops the projects nothing asked for', () => {
    expect(grouped(select(cases, ['K5'])).map((group) => group.project)).toEqual(['none']);
  });
});

describe('planText', () => {
  it('lists each step with its threshold and what it needs', () => {
    const text = planText(grouped(select(cases, ['K11', 'K5'])), {
      mode: 'wsl',
      app: '/mnt/c/x/app',
      allowMissing: new Set(['firefox']),
    });
    expect(text).toContain('mode wsl');
    expect(text).toContain('1. no app');
    expect(text).toContain('   K5  title K5 - threshold K5');
    expect(text).toContain('2. app on tests/fixtures/e2e-project');
    expect(text).toContain('   K11  title K11 - threshold K11 [needs firefox: allowed missing]');
  });
});

describe('outcome', () => {
  it('passes only with every measurement held', () => {
    expect(outcome([measure('p95', 120, '<= 150 ms', true)]).status).toBe('passed');
    const missed = outcome([measure('p95', 200, '<= 150 ms', false), measure('missed', 0, '= 0', true)]);
    expect(missed).toMatchObject({ status: 'failed', error: 'missed: p95' });
    expect(outcome([])).toMatchObject({ status: 'failed', error: 'nothing was measured' });
  });
});
