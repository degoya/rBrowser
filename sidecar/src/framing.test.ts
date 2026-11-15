import { describe, expect, it } from 'vitest';

import { LineSplitter } from './framing.ts';

function collect(maxBytes = 64) {
  const lines: string[] = [];
  const overflows: number[] = [];
  const splitter = new LineSplitter(
    maxBytes,
    (line) => lines.push(line),
    (bytes) => overflows.push(bytes),
  );
  return { splitter, lines, overflows };
}

describe('LineSplitter', () => {
  it('emits one line per newline, across chunk borders', () => {
    const { splitter, lines } = collect();
    splitter.push(Buffer.from('{"a":1}\n{"b"'));
    splitter.push(Buffer.from(':2}\n'));
    expect(lines).toEqual(['{"a":1}', '{"b":2}']);
  });

  it('strips a carriage return before the newline', () => {
    const { splitter, lines } = collect();
    splitter.push(Buffer.from('x\r\n'));
    expect(lines).toEqual(['x']);
  });

  it('keeps a multi-byte character split across chunks whole', () => {
    const { splitter, lines } = collect();
    const bytes = Buffer.from('"é"\n');
    splitter.push(bytes.subarray(0, 2));
    splitter.push(bytes.subarray(2));
    expect(lines).toEqual(['"é"']);
  });

  it('drops a line longer than the limit and reports it once', () => {
    const { splitter, lines, overflows } = collect(8);
    splitter.push(Buffer.from('0123456789'));
    splitter.push(Buffer.from('abc\nok\n'));
    expect(overflows).toEqual([13]);
    expect(lines).toEqual(['ok']);
  });
});
