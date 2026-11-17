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

/** The prefix of a choice row's id; the rest is the choice's place in the menu. */
const CHOICE = 'choice:';
/** A place in the menu: digits without a leading zero. */
const PLACE = /^(0|[1-9][0-9]*)$/;

/**
 * The rows of a pulldown of choices and what each one chooses (RB-090-57). A row's id is its
 * place, `choice:<n>`, never the choice's value: the core takes row ids of at most `MAX_TEXT`
 * characters, and one long environment or preset name must not break the whole menu (as the
 * bookmarks, RB-090-54). A row chosen maps back through the rows built last.
 */
export interface ChoiceMenu {
  /** The choices as check rows, the one in effect (`current`) checked. */
  rows: (choices: readonly Choice[], current: string | null) => NativeMenuRow[];
  /** The value the row `id` of the rows built last chooses; `undefined` for any other id. */
  valueOf: (id: string) => string | undefined;
}

/** A pulldown of choices whose rows carry their place (`ChoiceMenu`). */
export function choiceMenu(): ChoiceMenu {
  let shown: readonly string[] = [];
  return {
    rows(choices, current) {
      shown = choices.map((choice) => choice.value);
      return choices.map((choice, index) => ({
        kind: 'item',
        id: `${CHOICE}${index}`,
        label: choice.label,
        checked: choice.value === current,
      }));
    },
    valueOf(id) {
      const place = id.startsWith(CHOICE) ? id.slice(CHOICE.length) : '';
      return PLACE.test(place) ? shown[Number(place)] : undefined;
    },
  };
}

/** What the button shows: the choice in effect, `none` without one, `empty` without choices. */
export function shownChoice(choices: readonly Choice[], current: string | null, none: string, empty: string): string {
  if (choices.length === 0) {
    return empty;
  }
  return choices.find((choice) => choice.value === current)?.label ?? none;
}
