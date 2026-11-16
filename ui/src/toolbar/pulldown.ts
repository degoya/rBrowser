// The toolbar's pulldowns as buttons (RB-090-45, `PulldownButton.vue`): the environment, the layout
// preset and the bookmarks show their value and a chevron, and their rows open as the core's
// native menu below the button (`nativeMenu.ts`), like the theme (RB-090-41) - a `<select>`'s list
// was Chromium's popup, which followed neither light nor dark. In a narrow window the group draws
// the button as its icon alone (`ToolbarGroup.vue`, `useGroupLook`) before it leaves the row.

import { computed, inject, type InjectionKey, type Ref } from 'vue';
import { ICONS } from '../icons';
import type { NativeMenuRow } from '../ipc/protocol';

/** The chevron after a pulldown's value. */
export const PULLDOWN_CHEVRON = ICONS.pulldown;

/** How the group around a pulldown draws it: `compact`, only `icon`. */
export interface GroupLook {
  compact: Readonly<Ref<boolean>>;
  icon: Readonly<Ref<string | undefined>>;
}

export const GROUP_LOOK_KEY: InjectionKey<GroupLook> = Symbol('toolbar-group-look');

/** The look of the group this pulldown is in; outside the toolbar, always in full. */
export function useGroupLook(): GroupLook {
  return inject(GROUP_LOOK_KEY, () => ({ compact: computed(() => false), icon: computed(() => undefined) }), true);
}

/** One choice of a pulldown. */
export interface Choice {
  value: string;
  label: string;
}

/** The choices as check rows, the one in effect (`current`) checked. */
export function choiceRows(choices: readonly Choice[], current: string | null): NativeMenuRow[] {
  return choices.map((choice) => ({
    kind: 'item',
    id: choice.value,
    label: choice.label,
    checked: choice.value === current,
  }));
}

/** What the button shows: the choice in effect, `none` without one, `empty` without choices. */
export function shownChoice(choices: readonly Choice[], current: string | null, none: string, empty: string): string {
  if (choices.length === 0) {
    return empty;
  }
  return choices.find((choice) => choice.value === current)?.label ?? none;
}
