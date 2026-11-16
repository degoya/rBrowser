// The modifier the inspector is held with (`peek.modifier`). Each is one key held alone, so no
// reserved combination is taken: AltGr - which Windows reports as Ctrl+Alt - never counts as Alt
// or Ctrl, so typing @ or { on a German layout leaves Peek asleep.

import type { Modifier } from './types';

/** The state of the modifier keys an event carries. */
export interface ModifierState {
  altKey: boolean;
  ctrlKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
  getModifierState?(key: string): boolean;
}

const KEYS: Record<Modifier, string> = { Alt: 'Alt', Ctrl: 'Control', Shift: 'Shift' };

function altGraph(event: ModifierState): boolean {
  return Boolean(event.getModifierState?.('AltGraph')) || (event.altKey && event.ctrlKey);
}

/** Whether `modifier`, and only it, is held during `event`. */
export function isHeld(event: ModifierState, modifier: Modifier): boolean {
  if (event.metaKey || altGraph(event)) return false;
  switch (modifier) {
    case 'Alt':
      return event.altKey && !event.shiftKey;
    case 'Ctrl':
      return event.ctrlKey && !event.shiftKey;
    case 'Shift':
      return event.shiftKey && !event.altKey && !event.ctrlKey;
  }
}

/** Whether `event` is the press or release of the `modifier` key itself. */
export function isModifierKey(event: { key: string }, modifier: Modifier): boolean {
  return event.key === KEYS[modifier];
}

/** The modifier of a config value; Alt for anything else. */
export function modifierOf(value: unknown): Modifier {
  return value === 'Ctrl' || value === 'Shift' ? value : 'Alt';
}
