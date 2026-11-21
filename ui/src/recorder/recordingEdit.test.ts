// RB-092-01: the editor's edits of a recording.
import { describe, expect, it } from 'vitest';
import type { Recording } from './recording';
import {
  duplicateStep,
  insertStep,
  newStep,
  nextId,
  pauseOf,
  replaceStep,
  setDisabled,
  withOnlyIn,
  withPause,
} from './recordingEdit';

function search(): Recording {
  return {
    version: 1,
    name: 'search',
    breakpoints: [
      { id: 'mobile', width: 390, height: 844, deviceScaleFactor: 3 },
      { id: 'desktop', width: 1440, height: 900, deviceScaleFactor: 1 },
    ],
    startedAtMs: 0,
    appVersion: '0.9.2',
    steps: [
      { id: 1, action: { type: 'navigate', url: 'https://example.test/' }, view: 'mobile', atMs: 0 },
      { id: 4, action: { type: 'press', key: 'Enter' }, view: 'desktop', atMs: 900, path: [{ x: 1, y: 2, t: 0 }] },
      { id: 2, action: { type: 'wait', ms: 300 }, view: 'mobile', atMs: 2000 },
    ],
  };
}

const ids = (recording: Recording) => recording.steps.map((step) => step.id);

function stepAt(recording: Recording, index: number) {
  const step = recording.steps[index];
  if (step === undefined) {
    throw new Error(`no step at ${index}`);
  }
  return step;
}

describe('a step added in the editor', () => {
  it('takes the next id and the view and time of the step before it', () => {
    const recording = search();
    expect(nextId(recording)).toBe(5);
    const step = newStep(recording, 4, { type: 'wait', ms: 1 }, 500);
    expect(step).toEqual({ id: 5, action: { type: 'wait', ms: 1 }, view: 'desktop', atMs: 900, pauseMs: 500 });
    expect(newStep(recording, 0, { type: 'wait', ms: 1 }, null)).toEqual({
      id: 5,
      action: { type: 'wait', ms: 1 },
      view: 'mobile',
      atMs: 0,
    });
    const empty = { ...recording, steps: [] };
    expect(newStep(empty, null, { type: 'wait', ms: 1 }, null).view).toBe('mobile');
  });

  it('lands at its place: after a step, at the start or at the end', () => {
    const recording = search();
    const step = newStep(recording, 1, { type: 'wait', ms: 1 }, null);
    expect(ids(insertStep(recording, 1, step))).toEqual([1, 5, 4, 2]);
    expect(ids(insertStep(recording, 0, step))).toEqual([5, 1, 4, 2]);
    expect(ids(insertStep(recording, null, step))).toEqual([1, 4, 2, 5]);
    expect(ids(recording)).toEqual([1, 4, 2]);
  });
});

describe('a step changed in the editor', () => {
  it('is replaced under its id', () => {
    const recording = search();
    const changed = replaceStep(recording, 4, { ...stepAt(recording, 1), id: 99, action: { type: 'press', key: 'Tab' } });
    expect(stepAt(changed, 1)).toMatchObject({ id: 4, action: { type: 'press', key: 'Tab' } });
  });

  it('is switched off and on, on written as no field', () => {
    const off = setDisabled(search(), 1, true);
    expect(stepAt(off, 0).disabled).toBe(true);
    const on = setDisabled(off, 1, false);
    expect('disabled' in stepAt(on, 0)).toBe(false);
  });

  it('takes a pause of whole ms up to 10 s, or none', () => {
    expect(pauseOf(1234.6)).toBe(1235);
    expect(pauseOf(25_000)).toBe(10_000);
    expect(pauseOf(-5)).toBe(0);
    expect(pauseOf(null)).toBeNull();
    const step = stepAt(search(), 0);
    expect(withPause(step, 700).pauseMs).toBe(700);
    expect('pauseMs' in withPause(withPause(step, 700), null)).toBe(false);
  });

  it('runs in some breakpoints, or in all of them without the field', () => {
    const step = stepAt(search(), 0);
    const all = ['mobile', 'desktop'];
    expect(withOnlyIn(step, ['desktop'], all).onlyIn).toEqual(['desktop']);
    expect('onlyIn' in withOnlyIn(step, [], all)).toBe(false);
    expect('onlyIn' in withOnlyIn(step, ['desktop', 'mobile'], all)).toBe(false);
  });

  it('is duplicated right after itself under a new id, without its pointer way', () => {
    const copied = duplicateStep(search(), 4);
    expect(ids(copied)).toEqual([1, 4, 5, 2]);
    expect(stepAt(copied, 2).path).toBeUndefined();
    expect(stepAt(copied, 2).action).toEqual({ type: 'press', key: 'Enter' });
    expect(duplicateStep(search(), 9)).toEqual(search());
  });
});
