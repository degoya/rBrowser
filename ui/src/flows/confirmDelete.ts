// Whether a panel asks before it deletes an entry (RB-090-91): `automation.flows.confirmDelete`
// for the Flows panel, `automation.recorder.confirmDelete` for the Recorder panel - on unless the
// setting says `false`, so a core that answers nothing still asks.

import type { IpcClient } from '../ipc/client';

/** The Flows panel's setting. */
export const FLOWS_CONFIRM_KEY = 'automation.flows.confirmDelete';
/** The Recorder panel's setting. */
export const RECORDER_CONFIRM_KEY = 'automation.recorder.confirmDelete';

/** The setting's value as the panel uses it: ask unless it is `false`. */
export function confirmOf(value: unknown): boolean {
  return value !== false;
}

/** Reads the setting `key`; asks when it cannot be read. */
export async function confirmsDelete(client: IpcClient, key: string): Promise<boolean> {
  try {
    const answer = await client.expect({ type: 'getSettings' }, 'settings');
    return confirmOf(answer.values.find((entry) => entry.key === key)?.value);
  } catch {
    return true;
  }
}
