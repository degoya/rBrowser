// The project's flows in the palette (RB-060-11, RB-040-07): "Run flow ..." opens a second step
// with every flow of `.rbrowser/flows` (`listFlows`, asked at every open); choosing one runs
// `flows.run { name }`. Listing is not running: the core's trust gate stays on `flows.run`.

import { STEP_RUN_FLOW, type PaletteEntry, type PaletteProvider } from './types';

/** The command a flow entry runs (`Ctrl+Enter` opens it in the shortcut editor). */
export const RUN_FLOW_COMMAND = 'flows.run';

export { STEP_RUN_FLOW };

export const flowsProvider: PaletteProvider = {
  id: 'flows',
  entries({ flows, t }): PaletteEntry[] {
    if (flows === undefined || flows.length === 0) {
      return [];
    }
    return [
      {
        key: `step:${STEP_RUN_FLOW}`,
        scope: 'other',
        title: t('palette.step.run_flow'),
        group: t('palette.kind.step'),
        commandId: RUN_FLOW_COMMAND,
        action: { kind: 'step', step: STEP_RUN_FLOW },
      },
      ...flows.map(
        (flow): PaletteEntry => ({
          key: `flow:${flow.name}`,
          scope: 'other',
          parent: STEP_RUN_FLOW,
          title: t('palette.entry.run_flow', { name: flow.name }),
          group: t('palette.kind.flow'),
          ...(flow.description === undefined ? {} : { description: flow.description }),
          keywords: [flow.name],
          commandId: RUN_FLOW_COMMAND,
          action: { kind: 'send', message: { type: 'runCommand', id: RUN_FLOW_COMMAND, args: { name: flow.name } } },
        }),
      ),
    ];
  },
};
