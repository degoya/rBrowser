// The last choice of a test run's views per function (RB-099-10): `checks`, `visual`, `flows`.
// Kept in the shell page's `localStorage` (the shell's own origin), a per-user convenience and no
// setting; a blocked storage keeps it while the page lives. The breakpoints a choice names that
// the project lost fall away, and chosen ones of which none is left are every view again.

import { ref, watch, type Ref } from 'vue';
import {
  keepKnown,
  recallScope,
  rememberScope,
  type ScopeChoice,
} from '../../../plugins/web-common/panel-ui/src/scope';

type ScopeStorage = Pick<Storage, 'getItem' | 'setItem'>;

/** The window's storage, or `null` where it cannot be reached. */
export function scopeStorage(): ScopeStorage | null {
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

/** Function `name`'s choice, remembered as it changes; `known` the project's breakpoint ids. */
export function useScope(name: string, known: () => string[], storage: ScopeStorage | null = scopeStorage()): Ref<ScopeChoice> {
  const choice = ref<ScopeChoice>(recallScope(storage, name));
  watch(
    known,
    (ids) => {
      if (ids.length > 0) {
        choice.value = keepKnown(choice.value, ids);
      }
    },
    { immediate: true },
  );
  watch(choice, (now) => rememberScope(storage, name, now), { deep: true });
  return choice;
}
