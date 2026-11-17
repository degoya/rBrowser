import { describe, expect, it } from 'vitest';
import { ClsWindows, MAX_SOURCES } from './cls';

describe('ClsWindows', () => {
  it('is zero before any shift and ignores shifts after input', () => {
    const cls = new ClsWindows();
    expect(cls.value()).toEqual({ value: 0, sources: [] });
    cls.add({ value: 0.3, startTime: 100, hadRecentInput: true, sources: ['#a'] });
    expect(cls.value()).toEqual({ value: 0, sources: [] });
  });

  it('sums the shifts of one session window', () => {
    const cls = new ClsWindows();
    cls.add({ value: 0.05, startTime: 100, hadRecentInput: false, sources: ['#banner'] });
    cls.add({ value: 0.07, startTime: 800, hadRecentInput: false, sources: ['#banner', 'main'] });
    expect(cls.value().value).toBeCloseTo(0.12);
    expect(cls.value().sources).toEqual(['#banner', 'main']);
  });

  it('starts a new window after a gap of a second and keeps the largest', () => {
    const cls = new ClsWindows();
    cls.add({ value: 0.2, startTime: 100, hadRecentInput: false, sources: ['#first'] });
    cls.add({ value: 0.05, startTime: 1500, hadRecentInput: false, sources: ['#second'] });
    cls.add({ value: 0.05, startTime: 1700, hadRecentInput: false, sources: ['#second'] });
    expect(cls.value().value).toBeCloseTo(0.2);
    expect(cls.value().sources).toEqual(['#first']);
    cls.add({ value: 0.15, startTime: 2000, hadRecentInput: false, sources: ['#third'] });
    expect(cls.value().value).toBeCloseTo(0.25);
    expect(cls.value().sources).toEqual(['#third', '#second']);
  });

  it('caps a window at five seconds', () => {
    const cls = new ClsWindows();
    for (let time = 0; time <= 6000; time += 500) {
      cls.add({ value: 0.01, startTime: time, hadRecentInput: false, sources: [] });
    }
    // 0 ... 4500 is one window (10 shifts), 5000 ... 6000 the next (3 shifts).
    expect(cls.value().value).toBeCloseTo(0.1);
  });

  it('names at most the largest sources', () => {
    const cls = new ClsWindows();
    for (let index = 0; index < MAX_SOURCES + 3; index += 1) {
      cls.add({
        value: 0.01 * (index + 1),
        startTime: index * 10,
        hadRecentInput: false,
        sources: [`#s${index}`],
      });
    }
    const { sources } = cls.value();
    expect(sources).toHaveLength(MAX_SOURCES);
    expect(sources[0]).toBe(`#s${MAX_SOURCES + 2}`);
  });

  it('starts over on reset', () => {
    const cls = new ClsWindows();
    cls.add({ value: 0.4, startTime: 100, hadRecentInput: false, sources: ['#a'] });
    cls.reset();
    expect(cls.value()).toEqual({ value: 0, sources: [] });
  });
});
