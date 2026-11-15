import { afterEach, describe, expect, it } from 'vitest';
import { highlighter, markedTokens } from './highlight';

// RB-080-84: the highlighter in a Nuxt UI panel, its tokens cut again by marks of the panel's
// own (Coverage's used parts).
afterEach(() => {
  highlighter().configure({});
});

describe('marked tokens', () => {
  it('cut the coloured tokens at the marks and lose nothing', () => {
    const text = 'a { color: red }\nb { margin: 0 }';
    const pieces = markedTokens(text, 'css', [[0, 16]]);
    expect(pieces.map((piece) => piece.text).join('')).toBe(text);
    expect(pieces.filter((piece) => piece.marked).map((piece) => piece.text).join('')).toBe('a { color: red }');
    expect(pieces.some((piece) => piece.kind !== 'plain')).toBe(true);
    expect(pieces.some((piece) => piece.kind === 'punct')).toBe(false);
  });

  it('split one token where a mark starts or ends inside it', () => {
    expect(markedTokens('abcdef', null, [[2, 4]])).toEqual([
      { text: 'ab', kind: 'plain', marked: false },
      { text: 'cd', kind: 'plain', marked: true },
      { text: 'ef', kind: 'plain', marked: false },
    ]);
  });

  it('stay plain with highlighting off', () => {
    highlighter().configure({ 'ui.syntaxHighlight': false });
    expect(markedTokens('a { b: c }', 'css', [])).toEqual([{ text: 'a { b: c }', kind: 'plain', marked: false }]);
  });
});
