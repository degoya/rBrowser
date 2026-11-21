import { describe, expect, it } from 'vitest';
import {
  PATH_MAX_MS,
  PATH_MAX_POINTS,
  PATH_MIN_DISTANCE_PX,
  PATH_MIN_INTERVAL_MS,
  PointerPath,
} from './pointer-path';

describe('the pointer path', () => {
  it('keeps the moves as points relative to the first, rounded, ending where the pointer is', () => {
    const path = new PointerPath();
    path.add(10.4, 20.6, 1_000);
    path.add(40, 20, 1_050);
    path.add(80, 60, 1_100);
    expect(path.take(100, 60, 1_150)).toEqual([
      { x: 10, y: 21, t: 0 },
      { x: 40, y: 20, t: 50 },
      { x: 80, y: 60, t: 100 },
      { x: 100, y: 60, t: 150 },
    ]);
  });

  it('starts anew after a step took it', () => {
    const path = new PointerPath();
    path.add(0, 0, 0);
    path.add(50, 0, 100);
    expect(path.take(50, 0, 100)).toHaveLength(2);
    expect(path.take(50, 0, 200)).toEqual([]);
  });

  it('samples by distance and by time', () => {
    const path = new PointerPath();
    path.add(0, 0, 0);
    // Too close.
    path.add(PATH_MIN_DISTANCE_PX - 1, 0, 100);
    // Too soon.
    path.add(100, 0, PATH_MIN_INTERVAL_MS + 100 - 1 - 100);
    path.add(200, 0, 300);
    expect(path.take(200, 0, 300).map((point) => point.x)).toEqual([0, 200]);
  });

  it('keeps at most the last points of a long way', () => {
    const path = new PointerPath();
    for (let index = 0; index < PATH_MAX_POINTS * 2; index += 1) {
      path.add(index * 10, 0, index * 20);
    }
    const way = path.take((PATH_MAX_POINTS * 2 - 1) * 10, 0, (PATH_MAX_POINTS * 2 - 1) * 20);
    expect(way).toHaveLength(PATH_MAX_POINTS);
    expect(way[0]).toEqual({ x: PATH_MAX_POINTS * 10, y: 0, t: 0 });
    expect(way.at(-1)?.x).toBe((PATH_MAX_POINTS * 2 - 1) * 10);
  });

  it('keeps no point older than the longest way', () => {
    const path = new PointerPath();
    path.add(0, 0, 0);
    path.add(100, 0, 50);
    path.add(200, 0, PATH_MAX_MS + 100);
    expect(path.take(300, 0, PATH_MAX_MS + 150)).toEqual([
      { x: 200, y: 0, t: 0 },
      { x: 300, y: 0, t: 50 },
    ]);
  });

  it('is none for a single point, and never runs backwards in time', () => {
    const path = new PointerPath();
    expect(path.take(5, 5, 0)).toEqual([]);
    path.add(0, 0, 500);
    expect(path.take(0, 0, 500)).toEqual([]);
    path.add(0, 0, 500);
    path.add(50, 0, 600);
    expect(path.take(90, 0, 400).map((point) => point.t)).toEqual([0, 100, 100]);
  });
});
