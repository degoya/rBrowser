// The profile choice's logic without the DOM (RB-090-71): the choice in groups - no profile, the
// settings' profiles, the presets taken from forms (plugins/form-autofill/src/store.rs) - and the
// state of a new name typed for a stored preset (src/preset_edit.rs refuses the same names).

import { MAX_NAME } from './capture';
import type { Translate } from './model';

/** A group's heading, or a profile or preset to choose. */
export type ProfileItem = { type: 'label'; label: string } | { label: string; value: string };

/**
 * The choice in groups: "No profile" (`none`), the settings' profiles, then the stored presets a
 * settings profile does not shadow (a fill takes the settings profile of a name first). A group
 * with nothing in it is left out.
 */
export function profileGroups(t: Translate, settings: string[], presets: string[], none: string): ProfileItem[][] {
  const groups: ProfileItem[][] = [[{ label: t('panel.noProfile'), value: none }]];
  if (settings.length > 0) {
    groups.push([{ type: 'label', label: t('panel.choice.profiles') }, ...settings.map((name) => ({ label: name, value: name }))]);
  }
  const own = presets.filter((name) => !settings.includes(name));
  if (own.length > 0) {
    groups.push([{ type: 'label', label: t('panel.choice.presets') }, ...own.map((name) => ({ label: name, value: name }))]);
  }
  return groups;
}

export type RenameState = 'empty' | 'tooLong' | 'same' | 'inSettings' | 'taken' | 'ok';

/** What renaming the preset `from` to `to` does: nothing, a refusal, or the rename. */
export function renameState(to: string, from: string, settings: string[], presets: string[]): RenameState {
  const trimmed = to.trim();
  if (!trimmed) return 'empty';
  if (Array.from(trimmed).length > MAX_NAME) return 'tooLong';
  if (trimmed === from) return 'same';
  if (settings.includes(trimmed)) return 'inSettings';
  return presets.includes(trimmed) ? 'taken' : 'ok';
}
