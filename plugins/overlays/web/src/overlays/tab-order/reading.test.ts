import { afterEach, describe, expect, it } from 'vitest';
import fixture from '../../../../../../tests/fixtures/reading-order/cases.json';
import { before, contentRtl, outOfPlace, readingBox } from './reading';

const toRect = ([x, y, width, height]: number[]) => ({ x: x ?? 0, y: y ?? 0, width: width ?? 0, height: height ?? 0 });

afterEach(() => {
  document.documentElement.removeAttribute('dir');
  document.body.removeAttribute('dir');
});

describe('the reading order against the source order', () => {
  // The outline plugin's `reading-order` rule decides the same cases (RB-094-02).
  for (const each of fixture.cases) {
    it(`finds the cases of the shared fixture alike: ${each.name}`, () => {
      expect(outOfPlace(each.boxes.map(toRect), each.dir === 'rtl', each.tolerance)).toEqual(each.outOfPlace);
    });
  }

  it('reads a box on the same line before when it ends left of the start, an earlier line when above and not in a column to the right', () => {
    expect(before([96, 0, 80, 30], [0, 0, 80, 30], 4)).toBe(true);
    expect(before([0, 0, 80, 30], [96, 0, 80, 30], 4)).toBe(false);
    expect(before([0, 100, 80, 30], [0, 0, 80, 30], 4)).toBe(true);
    expect(before([0, 100, 80, 30], [200, 0, 80, 30], 4)).toBe(false);
  });

  it('measures in whole px, mirrored for right to left', () => {
    expect(readingBox({ x: 10.4, y: 2.6, width: 80, height: Number.NaN }, false)).toEqual([10, 3, 80, 0]);
    expect(readingBox({ x: 10, y: 0, width: 80, height: 30 }, true)).toEqual([-90, 0, 80, 30]);
  });
});

describe('the direction of the content', () => {
  it('is the nearest dir attribute, else the CSS direction', () => {
    expect(contentRtl(document, window)).toBe(false);
    document.documentElement.setAttribute('dir', 'rtl');
    expect(contentRtl(document, window)).toBe(true);
    document.body.setAttribute('dir', 'ltr');
    expect(contentRtl(document, window)).toBe(false);
  });
});
