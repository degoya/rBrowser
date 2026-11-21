import { describe, expect, it } from 'vitest';
import { changedOnDisk, entryOf } from './draftGuard';

const LIST = { project: true, recordings: [{ name: 'search', steps: 3, modifiedMs: 10 }] };

describe('the guard of unsaved edits (RB-092-01)', () => {
  it('finds the chosen entry', () => {
    expect(entryOf(LIST, 'search')?.steps).toBe(3);
    expect(entryOf(LIST, 'cart')).toBeUndefined();
    expect(entryOf(null, 'search')).toBeUndefined();
    expect(entryOf(LIST, null)).toBeUndefined();
  });

  it('sees a file written since by its time or its step count', () => {
    const known = { name: 'search', steps: 3, modifiedMs: 10 };
    expect(changedOnDisk(known, { ...known })).toBe(false);
    expect(changedOnDisk(known, { ...known, modifiedMs: 11 })).toBe(true);
    expect(changedOnDisk(known, { ...known, steps: 4 })).toBe(true);
    expect(changedOnDisk(undefined, known)).toBe(false);
    expect(changedOnDisk(known, undefined)).toBe(false);
  });
});
