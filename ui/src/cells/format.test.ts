import { describe, expect, it } from 'vitest';
import { formatCellSize, formatDpr, formatScale } from './format';

describe('cell label numbers', () => {
  it.each([
    [3, '3'],
    [1, '1'],
    [2.625, '2.625'],
    [1.3333333, '1.333'],
  ])('dpr %s reads %s', (dpr, text) => {
    expect(formatDpr(dpr)).toBe(text);
  });

  it.each([
    [1, null],
    [1.2, null],
    [0.5, '50'],
    [0.4567, '46'],
  ])('scale %s shows %s percent', (scale, text) => {
    expect(formatScale(scale)).toBe(text);
  });

  it('writes the size as the label shows it', () => {
    expect(formatCellSize(393, 852, 3)).toBe('393 \u00d7 852 @3x');
    expect(formatCellSize(1440, 900, 1)).toBe('1440 \u00d7 900 @1x');
  });
});
