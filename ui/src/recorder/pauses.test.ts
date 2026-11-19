import { describe, expect, it } from 'vitest';
import { pauseMap, seconds } from './pauses';

describe('the pauses the panel shows (RB-090-84)', () => {
  it("are the exporter's, by step id", () => {
    const map = pauseMap([
      { step: 1, ms: 0 },
      { step: 4, ms: 2_504 },
    ]);
    expect(map.get(4)).toBe(2_504);
    expect(map.get(2)).toBeUndefined();
  });

  it('read as seconds', () => {
    expect(seconds(2_504)).toBe('2.5');
    expect(seconds(60)).toBe('0.1');
  });
});
