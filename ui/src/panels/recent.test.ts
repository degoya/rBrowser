import { describe, expect, it } from 'vitest';
import { present, remember } from './recent';

// RB-090-38: the panels shown last stay loaded, hidden, so switching back does not load them.
describe('the panels shown last', () => {
  it('puts the shown panel first and keeps `size` others behind it', () => {
    let recent: string[] = [];
    recent = remember(recent, 'a', 2);
    recent = remember(recent, 'b', 2);
    recent = remember(recent, 'c', 2);
    expect(recent).toEqual(['c', 'b', 'a']);
    recent = remember(recent, 'd', 2);
    expect(recent).toEqual(['d', 'c', 'b']);
  });

  it('moves a panel shown again to the front without a second entry', () => {
    expect(remember(['b', 'a', 'c'], 'a', 2)).toEqual(['a', 'b', 'c']);
  });

  it('keeps only the shown one with size 0', () => {
    expect(remember(['b', 'a'], 'c', 0)).toEqual(['c']);
  });

  it('keeps `size` while a core panel is shown, and trims to a smaller size', () => {
    expect(remember(['c', 'b', 'a'], null, 2)).toEqual(['c', 'b']);
    expect(remember(['c', 'b', 'a'], null, 0)).toEqual([]);
  });

  // Review w30: a panel whose plugin left the list holds no place.
  it('forgets the panels no longer in the list', () => {
    expect(present(['a', 'b', 'c'], ['a', 'd'])).toEqual(['a']);
  });
});
