import { describe, expect, it } from 'vitest';
import { chordKeys } from './shortcut';

describe("a menu entry's shortcut", () => {
  it('splits a leader chord into its steps and names the arrows as UKbd draws them', () => {
    expect(chordKeys('Ctrl+Shift+Space P Left')).toEqual(['Ctrl+Shift+Space', 'P', 'arrowleft']);
    expect(chordKeys('Ctrl+Shift+Space P Up')).toEqual(['Ctrl+Shift+Space', 'P', 'arrowup']);
    expect(chordKeys('Ctrl+Shift+Space P Z')).toEqual(['Ctrl+Shift+Space', 'P', 'Z']);
  });

  it('is empty without a binding', () => {
    expect(chordKeys(null)).toEqual([]);
    expect(chordKeys(undefined)).toEqual([]);
    expect(chordKeys('')).toEqual([]);
  });
});
