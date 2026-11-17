// How a recording plays back (RB-090-66), as `automation.recorder.playbackPace` takes it: `fast`
// runs the steps back to back, each waiting only for its element; `recorded` keeps the pauses
// between the steps as they were recorded, each at most 10 s. The Recorder panel's choice beside
// "Play" starts at the setting.

import type { JsonValue } from '../ipc/protocol';

export const PACES = ['fast', 'recorded'] as const;
export type Pace = (typeof PACES)[number];

/** The setting the choice starts at. */
export const PACE_KEY = 'automation.recorder.playbackPace';

/** `value` as a pace; `fast` for anything that is none. */
export function paceOf(value: JsonValue | undefined): Pace {
  return value === 'recorded' ? 'recorded' : 'fast';
}
