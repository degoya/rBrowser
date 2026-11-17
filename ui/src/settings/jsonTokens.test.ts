import { describe, expect, it } from 'vitest';
import { JSON_TOKEN_CLASSES, tokenizeJson, tokenizeJsonLines, type JsonToken } from './jsonTokens';

const kinds = (tokens: JsonToken[]): string[] => tokens.map((token) => token.kind);
const texts = (tokens: JsonToken[]): string[] => tokens.map((token) => token.text);

describe('the JSON tokenizer', () => {
  it('tells a key from a string value by the colon that follows it', () => {
    const tokens = tokenizeJson('{"a": "b", "c" : {"d": ["e"]}}');
    expect(kinds(tokens.filter((token) => token.kind !== 'space' && token.kind !== 'punct'))).toEqual([
      'key',
      'string',
      'key',
      'key',
      'string',
    ]);
    expect(texts(tokens.filter((token) => token.kind === 'key'))).toEqual(['"a"', '"c"', '"d"']);
  });

  it('keeps a string with escapes as one token', () => {
    const text = String.raw`"quote \" backslash \\ unicode \u00e9 tab \t"`;
    const tokens = tokenizeJson(`{"k": ${text}}`);
    expect(tokens.filter((token) => token.kind === 'string')).toEqual([{ kind: 'string', text }]);
  });

  it('reads numbers with sign, fraction and exponent as one token', () => {
    const tokens = tokenizeJson('[0, -1, 2.5, 1e10, -3.25E-2, 7]');
    expect(texts(tokens.filter((token) => token.kind === 'number'))).toEqual([
      '0',
      '-1',
      '2.5',
      '1e10',
      '-3.25E-2',
      '7',
    ]);
  });

  it('names true, false and null, and brackets, colons and commas as punctuation', () => {
    const tokens = tokenizeJson('{"on": true, "off": false, "none": null}');
    expect(kinds(tokens)).toEqual([
      'punct',
      'key',
      'punct',
      'space',
      'boolean',
      'punct',
      'space',
      'key',
      'punct',
      'space',
      'boolean',
      'punct',
      'space',
      'key',
      'punct',
      'space',
      'null',
      'punct',
    ]);
    expect(texts(tokens.filter((token) => token.kind === 'punct'))).toEqual([
      '{',
      ':',
      ',',
      ':',
      ',',
      ':',
      '}',
    ]);
  });

  it('loses no character: the tokens joined are the text', () => {
    const text = '{\n  "a": [1, "x\\ny", null],\n  "b": {"c": -0.5e+3}\n}\n';
    expect(texts(tokenizeJson(text)).join('')).toBe(text);
    const odd = '{"a": tru, @ "b":}';
    expect(texts(tokenizeJson(odd)).join('')).toBe(odd);
  });

  it('splits two-space JSON into lines, indentation as space tokens, no trailing empty line', () => {
    const lines = tokenizeJsonLines('{\n  "a": 1\n}\n');
    expect(lines).toEqual([
      [{ kind: 'punct', text: '{' }],
      [
        { kind: 'space', text: '  ' },
        { kind: 'key', text: '"a"' },
        { kind: 'punct', text: ':' },
        { kind: 'space', text: ' ' },
        { kind: 'number', text: '1' },
      ],
      [{ kind: 'punct', text: '}' }],
    ]);
    expect(tokenizeJsonLines('')).toEqual([]);
  });

  it('gives every kind a class of its own, the theme tokens, none to a space', () => {
    expect(JSON_TOKEN_CLASSES.key).toContain('json-key');
    expect(JSON_TOKEN_CLASSES.key).toContain('text-primary');
    expect(JSON_TOKEN_CLASSES.string).toContain('text-success');
    expect(JSON_TOKEN_CLASSES.number).toContain('text-warning');
    expect(JSON_TOKEN_CLASSES.boolean).toContain('text-info');
    expect(JSON_TOKEN_CLASSES.null).toContain('text-muted');
    expect(JSON_TOKEN_CLASSES.punct).toContain('text-dimmed');
    expect(JSON_TOKEN_CLASSES.space).toBe('');
    const colours = new Set(
      (['key', 'string', 'number', 'boolean', 'null', 'punct'] as const).map(
        (kind) => JSON_TOKEN_CLASSES[kind],
      ),
    );
    expect(colours.size).toBe(6);
  });
});
