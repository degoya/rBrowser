import { describe, expect, it } from 'vitest';
import { highlight } from './highlight';

describe('the export preview highlighting (RB-090-60)', () => {
  it('marks comments, strings, keywords and numbers and keeps the text whole', () => {
    const code = "// Step 1: navigate\nawait page.goto('/login');\nconst n = 42; // done\n";
    const tokens = highlight(code);
    expect(tokens.map((token) => token.text).join('')).toBe(code);
    expect(tokens).toContainEqual({ kind: 'comment', text: '// Step 1: navigate' });
    expect(tokens).toContainEqual({ kind: 'keyword', text: 'await' });
    expect(tokens).toContainEqual({ kind: 'string', text: "'/login'" });
    expect(tokens).toContainEqual({ kind: 'keyword', text: 'const' });
    expect(tokens).toContainEqual({ kind: 'number', text: '42' });
    expect(tokens).toContainEqual({ kind: 'comment', text: '// done' });
  });

  it('reads escaped quotes, template strings and block comments', () => {
    const code = "a('it\\'s', `x ${y}`) /* note */";
    const tokens = highlight(code);
    expect(tokens.map((token) => token.text).join('')).toBe(code);
    expect(tokens).toContainEqual({ kind: 'string', text: "'it\\'s'" });
    expect(tokens).toContainEqual({ kind: 'string', text: '`x ${y}`' });
    expect(tokens).toContainEqual({ kind: 'comment', text: '/* note */' });
  });

  it('does not read a keyword inside a word and ends an open string at the end', () => {
    const tokens = highlight("awaited 'open");
    expect(tokens).toEqual([
      { kind: 'plain', text: 'awaited ' },
      { kind: 'string', text: "'open" },
    ]);
  });
});
