// "Save layout as preset" (RB-080-04): the command, the user setting of its default file and the
// rules of a preset name - the same as the core's `layout::save_preset::preset_id`, which checks
// again.

import type { JsonValue, LayoutState } from '../ipc/protocol';

/** The command; without arguments the core asks the shell for the name (`uiCommand`). */
export const SAVE_PRESET_COMMAND = 'layout.save_preset';
/** The user setting naming the default file, `local` or `project`. */
export const SAVE_PRESET_TO_KEY = 'layoutModes.savePresetTo';
/** The longest preset name, in characters. */
export const MAX_PRESET_NAME = 40;

const ALLOWED = /^[\p{L}\p{N} ._-]+$/u;

/** `raw` trimmed when it is a preset name the core takes, else `null`. */
export function presetName(raw: string): string | null {
  const name = raw.trim();
  const length = [...name].length;
  return length > 0 && length <= MAX_PRESET_NAME && ALLOWED.test(name) ? name : null;
}

/** The name the field starts with: the selected preset's, else the mode's. */
export function suggestedName(layout: LayoutState | undefined): string {
  return layout?.preset ?? layout?.mode ?? 'grid';
}

/** Whether the setting says to share a saved preset with the team (`config.json`). */
export function sharesByDefault(settings: Record<string, JsonValue>): boolean {
  return settings[SAVE_PRESET_TO_KEY] === 'project';
}
