import { describe, expect, it } from 'vitest';
import { copyItem, moveItem, uniqueName } from './listOps';

describe('the operations of an entry list (RB-080-13)', () => {
  it('moves an entry to another place', () => {
    expect(moveItem(['a', 'b', 'c'], 0, 2)).toEqual(['b', 'c', 'a']);
    expect(moveItem(['a', 'b', 'c'], 2, 0)).toEqual(['c', 'a', 'b']);
    expect(moveItem(['a', 'b', 'c'], 1, 1)).toEqual(['a', 'b', 'c']);
    expect(moveItem(['a', 'b'], 0, 5)).toEqual(['a', 'b']);
  });

  it('finds a name not taken yet', () => {
    expect(uniqueName('mobile-copy', new Set(['mobile']))).toBe('mobile-copy');
    expect(uniqueName('mobile-copy', new Set(['mobile', 'mobile-copy']))).toBe('mobile-copy-2');
    expect(uniqueName('new', new Set(['new', 'new-2']))).toBe('new-3');
  });

  it('copies an entry with an id of its own and leaves the original alone', () => {
    const original = { id: 'mobile', width: 375, media: { reducedData: true } };
    const items = [original, { id: 'mobile-copy' }];
    const copy = copyItem(original, items) as { id: string; media: { reducedData: boolean } };
    expect(copy).toEqual({ id: 'mobile-copy-2', width: 375, media: { reducedData: true } });
    copy.media.reducedData = false;
    expect(original.media.reducedData).toBe(true);
    expect(copyItem({ breakpoint: 'mobile', col: 1, row: 1 }, [])).toEqual({ breakpoint: 'mobile', col: 1, row: 1 });
  });
});
