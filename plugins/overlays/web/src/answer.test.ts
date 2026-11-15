import { describe, expect, it } from 'vitest';
import { cut, fitting, MAX_TEXT_CHARS } from './answer';

describe('an answer of the page script (RB-090-17)', () => {
  it('cuts a text to MAX_TEXT_CHARS characters, a multi-byte one included', () => {
    expect(cut('short')).toBe('short');
    expect(Array.from(cut('中'.repeat(MAX_TEXT_CHARS + 5)))).toHaveLength(MAX_TEXT_CHARS);
    expect(cut('x'.repeat(MAX_TEXT_CHARS + 1))).toHaveLength(MAX_TEXT_CHARS);
  });

  it('keeps the first items whose JSON bytes fit the budget, commas included', () => {
    // '"ab"' is four bytes, five with its comma; '"中"' is five, six with it.
    expect(fitting(['ab', 'ab', 'ab'], 10)).toEqual(['ab', 'ab']);
    expect(fitting(['中', 'ab'], 6)).toEqual(['中']);
    expect(fitting(['ab'], 4)).toEqual([]);
    expect(fitting([], 10)).toEqual([]);
  });
});
