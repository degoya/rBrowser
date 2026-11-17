import { describe, expect, it } from 'vitest';
import { parseSize } from './size';

describe('the free size', () => {
  it('reads width, height and an optional DPR', () => {
    expect(parseSize('500x800')).toEqual({ width: 500, height: 800 });
    expect(parseSize(' 500 X 800 ')).toEqual({ width: 500, height: 800 });
    expect(parseSize('500×800@2')).toEqual({ width: 500, height: 800, dpr: 2 });
    expect(parseSize('500*800 @ 2.5x')).toEqual({ width: 500, height: 800, dpr: 2.5 });
  });

  it('refuses what is no size', () => {
    for (const bad of [
      '',
      '500',
      'x800',
      '500x',
      '0x800',
      '500x0',
      '500x800@0',
      '500x800@11',
      '500x800@abc',
      '99999x800',
      '-5x800',
      '500x800x2',
      '500x800@2@3',
    ]) {
      expect(parseSize(bad), bad).toBeNull();
    }
  });
});
