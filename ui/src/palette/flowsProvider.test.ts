import { describe, expect, it } from 'vitest';
import { sampleState } from '../ipc/mockState';
import { flowsProvider, RUN_FLOW_COMMAND, STEP_RUN_FLOW } from './flowsProvider';
import type { ProviderContext } from './types';

const t = (key: string, args?: Record<string, string | number>): string =>
  args === undefined ? key : `${key} ${JSON.stringify(args)}`;

function context(extra: Partial<ProviderContext> = {}): ProviderContext {
  return { state: sampleState(), commands: [], settings: null, values: new Map(), t, ...extra };
}

describe('flowsProvider', () => {
  it('offers "Run flow ..." with every flow of the project in the second step', () => {
    const entries = flowsProvider.entries(
      context({ flows: [{ name: 'login', description: 'Log in' }, { name: 'cart' }] }),
    );
    expect(entries.map((e) => e.key)).toEqual(['step:run-flow', 'flow:login', 'flow:cart']);
    expect(entries[0]?.action).toEqual({ kind: 'step', step: STEP_RUN_FLOW });
    expect(entries[1]).toMatchObject({
      scope: 'other',
      parent: STEP_RUN_FLOW,
      title: 'palette.entry.run_flow {"name":"login"}',
      group: 'palette.kind.flow',
      description: 'Log in',
      keywords: ['login'],
      commandId: RUN_FLOW_COMMAND,
      action: { kind: 'send', message: { type: 'runCommand', id: 'flows.run', args: { name: 'login' } } },
    });
    expect(entries[2]?.description).toBeUndefined();
  });

  it('offers nothing while the project has no flows or the core has not answered', () => {
    expect(flowsProvider.entries(context({ flows: [] }))).toEqual([]);
    expect(flowsProvider.entries(context())).toEqual([]);
  });
});
