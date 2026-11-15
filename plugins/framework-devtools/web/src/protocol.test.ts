import { describe, expect, it } from 'vitest';
import { Reassembler, chunks, isEnvelope, isPost, isRelay, relays } from './protocol';

describe('chunks and relays', () => {
  it('cuts a long text into pieces and keeps a short one whole', () => {
    expect(chunks('', 4)).toEqual([]);
    expect(chunks('abc', 4)).toEqual(['abc']);
    expect(chunks('abcdefghij', 4)).toEqual(['abcd', 'efgh', 'ij']);
    expect(relays('react', 'abc', 7, 4)).toEqual([{ framework: 'react', text: 'abc' }]);
    expect(relays('vue', 'abcdefghij', 7, 4)).toEqual([
      { framework: 'vue', text: 'abcd', id: 7, i: 0, n: 3 },
      { framework: 'vue', text: 'efgh', id: 7, i: 1, n: 3 },
      { framework: 'vue', text: 'ij', id: 7, i: 2, n: 3 },
    ]);
  });
});

describe('Reassembler', () => {
  it('passes a whole message through and completes chunks in any order', () => {
    const r = new Reassembler();
    expect(r.take({ framework: 'react', text: 'whole' })).toBe('whole');
    const [a, b, c] = relays('react', 'abcdefghij', 1, 4);
    expect(r.take(c!)).toBeNull();
    expect(r.take(a!)).toBeNull();
    expect(r.take(b!)).toBe('abcdefghij');
    // The id is free again.
    expect(r.take(a!)).toBeNull();
  });

  it('keeps messages apart by id and drops the oldest beyond its limit', () => {
    const r = new Reassembler(2);
    const first = relays('vue', 'aaaabbbb', 1, 4);
    const second = relays('vue', 'ccccdddd', 2, 4);
    const third = relays('vue', 'eeeeffff', 3, 4);
    expect(r.take(first[0]!)).toBeNull();
    expect(r.take(second[0]!)).toBeNull();
    // A third message drops the first, the oldest.
    expect(r.take(third[0]!)).toBeNull();
    expect(r.take(second[1]!)).toBe('ccccdddd');
    // The first message's second piece starts it over; it is incomplete.
    expect(r.take(first[1]!)).toBeNull();
    expect(r.take(third[1]!)).toBe('eeeeffff');
    expect(r.take(first[0]!)).toBe('aaaabbbb');
  });

  it('ignores a chunk whose count disagrees with its message', () => {
    const r = new Reassembler();
    expect(r.take({ framework: 'vue', text: 'a', id: 1, i: 0, n: 2 })).toBeNull();
    expect(r.take({ framework: 'vue', text: 'b', id: 1, i: 1, n: 3 })).toBeNull();
    expect(r.take({ framework: 'vue', text: 'b', id: 1, i: 1, n: 2 })).toBe('ab');
  });
});

describe('the message checks', () => {
  it('know an envelope', () => {
    expect(isEnvelope({ t: 'connect' })).toBe(true);
    expect(isEnvelope({ t: 'm', d: null })).toBe(true);
    expect(isEnvelope({ t: 'm' })).toBe(false);
    expect(isEnvelope({ t: 'other' })).toBe(false);
    expect(isEnvelope('connect')).toBe(false);
  });

  it('know a relay message', () => {
    expect(isRelay({ framework: 'react', text: 'x' })).toBe(true);
    expect(isRelay({ framework: 'probe', text: '{}' })).toBe(true);
    expect(isRelay({ framework: 'vue', text: 'x', id: 1, i: 0, n: 1 })).toBe(true);
    expect(isRelay({ framework: 'vue', text: 'x', id: 1, i: 1, n: 1 })).toBe(false);
    expect(isRelay({ framework: 'vue', text: 'x', i: 0, n: 1 })).toBe(false);
    expect(isRelay({ framework: 'svelte', text: 'x' })).toBe(false);
    expect(isRelay({ framework: 'vue' })).toBe(false);
  });

  it('know a post', () => {
    expect(isPost({ framework: 'vue', envelope: { t: 'connect' } })).toBe(true);
    expect(isPost({ framework: 'vue', envelope: { t: 'nope' } })).toBe(false);
    expect(isPost({ framework: 'angular', envelope: { t: 'connect' } })).toBe(false);
  });
});
