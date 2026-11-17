// The Flows panel's channel on the shell's IPC (RB-090-60): the project's flows (`listFlows`,
// as the command palette asks), the run going on and the last result (`getFlowRun`), and the
// commands of the registry it runs (`flows.run`, `flows.run_all`, `flows.cancel`,
// `flows.open_results`); the core's `flowRunChanged` (a run started or ended) asks again.

import type { IpcClient } from '../ipc/client';
import type { CoreToShell, FlowRunState, FlowSummary, JsonValue } from '../ipc/protocol';
import type { Subscribe } from '../whichkey/whichkey';

export interface FlowsChannel {
  flows(): Promise<FlowSummary[]>;
  state(): Promise<FlowRunState>;
  run(id: string, args?: JsonValue): Promise<void>;
  /** Calls `listener` when a run started or ended; returns the function that stops. */
  onChanged(listener: () => void): () => void;
}

export function flowsChannel(client: IpcClient, subscribe: Subscribe): FlowsChannel {
  return {
    async flows() {
      return (await client.expect({ type: 'listFlows' }, 'flows')).items;
    },
    async state() {
      const { type: _type, ...state } = await client.expect({ type: 'getFlowRun' }, 'flowRun');
      return state;
    },
    async run(id, args) {
      await client.request(args === undefined ? { type: 'runCommand', id } : { type: 'runCommand', id, args });
    },
    onChanged(listener) {
      return subscribe((message: CoreToShell) => {
        if (message.type === 'flowRunChanged') {
          listener();
        }
      });
    },
  };
}
