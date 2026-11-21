// The steps of the recording that runs (RB-090-89): each push replaces the list from its place,
// an end (saved, or the setting off) lets it go, and the pause before each step is the one
// recorded.
import { describe, expect, it } from 'vitest';
import { MAX_PAUSE_MS, livePauses, withLiveSteps, type LiveRecording } from './live';
import type { Step } from './recording';

function step(id: number, atMs: number, url = `http://localhost:3000/${id}`): Step {
  return { id, action: { type: 'navigate', url }, view: 'w2000', atMs };
}

describe('the live steps', () => {
  it('starts with the first push and replaces the list from each push on', () => {
    const started = withLiveSteps(null, { name: 'rec', state: 'recording', from: 0, steps: [] });
    expect(started).toEqual({ name: 'rec', steps: [] });
    const two = withLiveSteps(started, { name: 'rec', state: 'recording', from: 0, steps: [step(1, 0), step(2, 400)] });
    const cleaned = withLiveSteps(two, { name: 'rec', state: 'recording', from: 1, steps: [step(2, 400, 'http://localhost:3000/b')] });
    expect(cleaned?.steps.map((each) => each.id)).toEqual([1, 2]);
    expect(cleaned?.steps[1]?.action).toEqual({ type: 'navigate', url: 'http://localhost:3000/b' });
    const shorter = withLiveSteps(cleaned, { name: 'rec', state: 'recording', from: 1, steps: [] });
    expect(shorter?.steps.map((each) => each.id)).toEqual([1]);
  });

  it('ends when it was saved or hidden, and another recording starts afresh', () => {
    const live: LiveRecording = { name: 'rec', steps: [step(1, 0)] };
    expect(withLiveSteps(live, { name: 'rec', state: 'ended', from: 0, steps: [] })).toBeNull();
    expect(withLiveSteps(live, { name: 'rec', state: 'hidden', from: 0, steps: [] })).toBeNull();
    expect(withLiveSteps(live, { name: 'next', state: 'recording', from: 1, steps: [step(2, 9)] })).toEqual({
      name: 'next',
      steps: [step(2, 9)],
    });
  });

  it('gives each step the pause recorded before it, at most 10 s, none before the first', () => {
    const pauses = livePauses([step(1, 100), step(2, 1_900), step(3, 60_000), step(4, 50_000)]);
    expect([...pauses.entries()]).toEqual([
      [1, 0],
      [2, 1_800],
      [3, MAX_PAUSE_MS],
      [4, 0],
    ]);
  });
});

describe('a recording continued from a step (RB-092-01)', () => {
  it('keeps the step its new steps follow', () => {
    const live = withLiveSteps(null, { name: 'search', from: 0, steps: [], state: 'recording', continueAfter: 3 });
    expect(live).toEqual({ name: 'search', steps: [], continueAfter: 3 });
    expect(withLiveSteps(null, { name: 'search', from: 0, steps: [], state: 'recording' })).toEqual({ name: 'search', steps: [] });
  });
});
