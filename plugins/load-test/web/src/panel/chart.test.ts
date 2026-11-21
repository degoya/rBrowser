import { describe, expect, it } from 'vitest';
import { line, topOf } from './chart';

describe('the time chart', () => {
  it('scales a line to its largest value', () => {
    expect(topOf([1, 5, null, 3])).toBe(5);
    expect(topOf([])).toBe(1);
    const drawn = line([0, 5, 10], 100, 50);
    expect(drawn.top).toBe(10);
    expect(drawn.points).toBe('0.0,50.0 50.0,25.0 100.0,0.0');
  });

  it('puts a second without a value on the floor', () => {
    expect(line([null, 4], 10, 10).points).toBe('0.0,10.0 10.0,0.0');
  });
});
