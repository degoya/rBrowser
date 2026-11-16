// The which-key state (RB-010-17's overlay, built here): after the leader the core sends
// `whichKey` with what was typed and every next key, and `whichKeyHide` when the chord ran, was
// cancelled or timed out. The bar in the shell strip shows it (`WhichKeyBar.vue`).

import { onScopeDispose, shallowRef, type ShallowRef } from 'vue';
import type { CoreToShell, WhichKey } from '../ipc/protocol';
import { subscribeEvents } from '../ipc/transport';

export type Subscribe = (listener: (message: CoreToShell) => void) => () => void;

/** Subscribes to the core's events on the window (CEF's transport). */
export const windowEvents: Subscribe = (listener) => subscribeEvents(window, listener);

/** The overlay's content; `null` while hidden. */
export function useWhichKey(subscribe: Subscribe = windowEvents): ShallowRef<WhichKey | null> {
  const current = shallowRef<WhichKey | null>(null);
  const stop = subscribe((message) => {
    if (message.type === 'whichKey') {
      const { type: _type, ...which } = message;
      current.value = which;
    } else if (message.type === 'whichKeyHide') {
      current.value = null;
    }
  });
  onScopeDispose(stop);
  return current;
}
