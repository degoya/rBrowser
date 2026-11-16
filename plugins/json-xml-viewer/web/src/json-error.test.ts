import { describe, expect, it } from 'vitest';
import { lineColumn, locateJsonError, parseJson } from './json-error';

describe('locateJsonError', () => {
  it.each([
    '{}',
    '[]',
    ' {"a": [1, -2.5e+3, true, false, null, "x\\n\\u00e9"], "b": {}} ',
    '"text"',
    '0',
    '[[[[]]]]',
  ])('finds nothing wrong in %s', (text) => {
    expect(locateJsonError(text)).toBeNull();
  });

  it.each([
    ['{"a": 1,}', 8, 'unexpected'],
    ['{"a" 1}', 5, 'unexpected'],
    ['[1 2]', 3, 'unexpected'],
    ['[1,', 3, 'end'],
    ['{"a": tru}', 6, 'unexpected'],
    ['{"a": 01}', 7, 'unexpected'],
    ['{"a": "x\ty"}', 8, 'unexpected'],
    ['{"a": "\\x"}', 8, 'unexpected'],
    ['{} []', 3, 'unexpected'],
    ['', 0, 'end'],
    ["{'a': 1}", 1, 'unexpected'],
    ['[1.]', 3, 'unexpected'],
    ['"open', 5, 'end'],
  ])('%j fails at offset %i (%s)', (text, offset, reason) => {
    expect(locateJsonError(text)).toEqual({ offset, reason });
  });
});

describe('lineColumn', () => {
  it('counts lines and columns from 1', () => {
    expect(lineColumn('ab\ncd\r\nef', 0)).toEqual({ line: 1, column: 1 });
    expect(lineColumn('ab\ncd\r\nef', 4)).toEqual({ line: 2, column: 2 });
    expect(lineColumn('ab\ncd\r\nef', 8)).toEqual({ line: 3, column: 2 });
  });
});

describe('parseJson', () => {
  it('answers the value', () => {
    expect(parseJson('{"a": [1]}')).toEqual({ ok: true, value: { a: [1] } });
  });

  it('answers the error with its line and column', () => {
    expect(parseJson('{\n  "a": 1,\n}')).toEqual({
      ok: false,
      error: { offset: 12, reason: 'unexpected', line: 3, column: 1 },
    });
  });
});
