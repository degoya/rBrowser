// The Recorder panel's guard of unsaved edits (RB-092-01 review): which entry of the list is the
// chosen recording, and whether its file changed since the panel read it - an agent's step tools
// (`recorder_step_*`) and a continued recording write it while the panel shows it.

import type { RecordingEntry, RecordingList } from '../ipc/protocol';

/** The list's entry of the recording `name`. */
export function entryOf(list: RecordingList | null, name: string | null): RecordingEntry | undefined {
  return name === null ? undefined : list?.recordings.find((entry) => entry.name === name);
}

/** Whether the file of `known` (as the panel read it) was written since: `now` is newer or other. */
export function changedOnDisk(known: RecordingEntry | undefined, now: RecordingEntry | undefined): boolean {
  if (known === undefined || now === undefined) {
    return false;
  }
  return known.modifiedMs !== now.modifiedMs || known.steps !== now.steps;
}
