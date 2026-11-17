import { describe, expect, it } from 'vitest';
import { isHeld, isModifierKey, modifierOf, type ModifierState } from './modifier';

function state(over: Partial<ModifierState> = {}): ModifierState {
  return { altKey: false, ctrlKey: false, shiftKey: false, metaKey: false, ...over };
}

describe('the modifier', () => {
  it('Alt alone holds Alt', () => {
    expect(isHeld(state({ altKey: true }), 'Alt')).toBe(true);
    expect(isHeld(state(), 'Alt')).toBe(false);
    expect(isHeld(state({ altKey: true, shiftKey: true }), 'Alt')).toBe(false);
    expect(isHeld(state({ altKey: true, metaKey: true }), 'Alt')).toBe(false);
  });

  it('AltGr (Ctrl+Alt, or the AltGraph state) is never Alt or Ctrl', () => {
    const altGr = state({ altKey: true, ctrlKey: true });
    expect(isHeld(altGr, 'Alt')).toBe(false);
    expect(isHeld(altGr, 'Ctrl')).toBe(false);
    const graph = state({ altKey: true, getModifierState: (key) => key === 'AltGraph' });
    expect(isHeld(graph, 'Alt')).toBe(false);
  });

  it('Ctrl and Shift alone', () => {
    expect(isHeld(state({ ctrlKey: true }), 'Ctrl')).toBe(true);
    expect(isHeld(state({ ctrlKey: true, shiftKey: true }), 'Ctrl')).toBe(false);
    expect(isHeld(state({ shiftKey: true }), 'Shift')).toBe(true);
    expect(isHeld(state({ shiftKey: true, ctrlKey: true }), 'Shift')).toBe(false);
  });

  it('the key itself and the config value', () => {
    expect(isModifierKey({ key: 'Alt' }, 'Alt')).toBe(true);
    expect(isModifierKey({ key: 'AltGraph' }, 'Alt')).toBe(false);
    expect(isModifierKey({ key: 'Control' }, 'Ctrl')).toBe(true);
    expect(modifierOf('Shift')).toBe('Shift');
    expect(modifierOf('Ctrl+Alt')).toBe('Alt');
    expect(modifierOf(undefined)).toBe('Alt');
  });
});
