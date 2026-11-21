// The Recorder panel's ties of the recording editor (RB-092-01) to the core: the pause an added
// step starts with (`automation.recorder.insertedStepPauseMs`) and "Pick in view" - the command
// `recorder.pick_element` waits for the person's click in the active view, and the element comes
// back as `recorderElementPicked`.

import type { JsonValue, PickedElement } from '../ipc/protocol';
import type { RecorderChannel } from './host';
import { MAX_WAIT_MS } from './stepKinds';

/** The setting an added step's pause starts at. */
export const INSERTED_PAUSE_KEY = 'automation.recorder.insertedStepPauseMs';

/** `value` as an added step's pause: whole ms up to 10 s; 500 for anything that is none. */
export function insertedPauseOf(value: JsonValue | undefined): number {
  return typeof value === 'number' && Number.isFinite(value) ? Math.min(MAX_WAIT_MS, Math.max(0, Math.round(value))) : 500;
}

/**
 * The element the next click in the active view picks for the recording `name`, or `null` when
 * no recording is chosen, the pick failed (`failed` hears why) or ended without an element.
 */
export async function pickedOnce(
  channel: Pick<RecorderChannel, 'run' | 'onPicked'>,
  name: string | null,
  failed: (caught: unknown) => void,
): Promise<PickedElement | null> {
  if (name === null) {
    return null;
  }
  let picked: PickedElement | null = null;
  const stop = channel.onPicked((element) => {
    picked = element;
  });
  try {
    // The command answers once the pick ended; its element came as an event before.
    await channel.run('recorder.pick_element', { recordingId: name });
    return picked;
  } catch (caught) {
    failed(caught);
    return null;
  } finally {
    stop();
  }
}
