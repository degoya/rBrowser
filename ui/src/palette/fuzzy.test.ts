import { describe, expect, it } from 'vitest';
import { fuzzyScore } from './fuzzy';

describe('fuzzyScore', () => {
  it('matches a subsequence and refuses what is not one', () => {
    expect(fuzzyScore('ovfl', 'Toggle overflow detector')).not.toBeNull();
    expect(fuzzyScore('xyz', 'Toggle overflow detector')).toBeNull();
    expect(fuzzyScore('lfvo', 'Toggle overflow detector')).toBeNull();
  });

  it('ignores case', () => {
    expect(fuzzyScore('SYNC', 'Toggle sync')).not.toBeNull();
  });

  it('prefers word starts and consecutive letters', () => {
    const wordStart = fuzzyScore('rel', 'Reload all views');
    const inner = fuzzyScore('rel', 'Thresholds are lower');
    expect(wordStart).not.toBeNull();
    expect(inner).not.toBeNull();
    expect(wordStart as number).toBeGreaterThan(inner as number);
  });

  it('prefers a tight match over a scattered one', () => {
    const tight = fuzzyScore('zoom', 'Zoom the active view in');
    const scattered = fuzzyScore('zoom', 'Zebra of other mammals');
    expect(tight as number).toBeGreaterThan(scattered as number);
  });

  it('finds the best alignment, not the first', () => {
    // A greedy match takes the `o` of `Toggle`; the best one takes the word start of `overflow`.
    const best = fuzzyScore('ov', 'Toggle overflow');
    const plain = fuzzyScore('ov', 'Togglexoverflow');
    expect(best as number).toBeGreaterThan(plain as number);
  });

  it('counts a camelCase hump and a dot as a word start', () => {
    const hump = fuzzyScore('lt', 'leaderTimeout');
    const flat = fuzzyScore('lt', 'leadertimeout');
    expect(hump as number).toBeGreaterThan(flat as number);
    const dotted = fuzzyScore('st', 'sync.toggle');
    const undotted = fuzzyScore('st', 'syncxtoggle');
    expect(dotted as number).toBeGreaterThan(undotted as number);
  });

  it('matches the empty needle with score 0', () => {
    expect(fuzzyScore('', 'anything')).toBe(0);
  });
});
