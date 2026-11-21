import { afterEach, describe, expect, it } from 'vitest';
import type { SweepWindow } from './detect';
import { HOLD_POLL_MS, hold } from './hold';

/** A window of `innerWidth` whose timers fire at once, so a hold of seconds runs in a moment. */
function win(innerWidth: number): SweepWindow {
  return {
    innerWidth,
    scrollX: 0,
    getComputedStyle: (element) => window.getComputedStyle(element),
    requestAnimationFrame: (callback) => window.setTimeout(callback, 0),
    setTimeout: (callback) => window.setTimeout(callback, 0),
  };
}

/** Lets the hold run a few polls. */
async function ticks(count: number): Promise<void> {
  for (let tick = 0; tick < count; tick += 1) {
    await new Promise<void>((resolve) => window.setTimeout(resolve, 0));
  }
}

afterEach(() => {
  document.body.innerHTML = '';
});

describe('hold (the command show, RB-093-01)', () => {
  it('ends at Escape pressed after it began, not at one pressed before', async () => {
    const view = win(730);
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    const held = hold({ width: 730, ms: 60_000 }, document, view);
    await ticks(3);
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    await expect(held).resolves.toEqual({ end: 'escape', width: 730 });
  });

  it('ends when the view leaves the width: the lease ended or another width was set', async () => {
    const view = win(730);
    const held = hold({ width: 730, ms: 60_000 }, document, view);
    await ticks(3);
    view.innerWidth = 1280;
    await expect(held).resolves.toEqual({ end: 'resized', width: 730 });
  });

  it('ends by itself after the time it was given', async () => {
    const answer = await hold({ width: 730, ms: 3 * HOLD_POLL_MS }, document, win(730));
    expect(answer).toEqual({ end: 'elapsed', width: 730 });
  });

  it('takes the width the page has when it never gets the one asked for', async () => {
    const answer = await hold({ width: 640, ms: HOLD_POLL_MS }, document, win(800));
    expect(answer).toEqual({ end: 'elapsed', width: 800 });
  });
});
