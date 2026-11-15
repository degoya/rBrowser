import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import scrollPage from '../../../tests/fixtures/site/scroll.html?raw';
import type { ScrollData, SyncEvent } from '../protocol';
import { el, loadPage } from '../test-page';
import { ScrollCapture, snapshot } from './capture';
import type { Scroller } from './anchor';
import { Layout, stack } from './test-layout';

let frames: (() => void)[];
let emitted: SyncEvent[];
let on: boolean;
let debounce: number;
let capture: ScrollCapture;

/** Runs the frame callbacks queued so far. */
function frame(): void {
  const due = frames;
  frames = [];
  for (const run of due) run();
}

function make(): ScrollCapture {
  return new ScrollCapture({
    enabled: () => on,
    emit: (event) => emitted.push(event),
    debounceMs: () => debounce,
    frame: (run) => frames.push(run),
    snapshot: (scroller: Scroller): ScrollData =>
      scroller instanceof Element
        ? { container: { element: { kind: 'css', path: `#${scroller.id}` }, offset: { x: 0.5, y: 0.5 } }, x: 0, y: 1, maxX: 0, maxY: 9 }
        : { x: 0, y: 1, maxX: 0, maxY: 9 },
  });
}

beforeEach(() => {
  vi.useFakeTimers();
  loadPage(scrollPage);
  frames = [];
  emitted = [];
  on = true;
  debounce = 150;
  capture = make();
});

afterEach(() => {
  capture.dispose();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe('the scroll throttle', () => {
  it('sends one event per scroller and frame, however many scroll events came', () => {
    for (let i = 0; i < 10; i += 1) capture.onScroll(document);
    expect(emitted).toHaveLength(0);
    expect(frames).toHaveLength(1);
    frame();
    expect(emitted).toEqual([{ type: 'scroll', x: 0, y: 1, maxX: 0, maxY: 9 }]);
    capture.onScroll(document);
    capture.onScroll(el('#sidebar'));
    capture.onScroll(document);
    frame();
    expect(emitted).toHaveLength(3);
    expect(emitted[2]).toMatchObject({ container: { element: { path: '#sidebar' } } });
  });

  it('sends the end position once more after the scrolling rested', () => {
    capture.onScroll(document);
    capture.onScroll(el('#sidebar'));
    frame();
    vi.advanceTimersByTime(100);
    capture.onScroll(document);
    frame();
    expect(emitted).toHaveLength(3);
    // Each scroll restarts the wait.
    vi.advanceTimersByTime(100);
    expect(emitted).toHaveLength(3);
    vi.advanceTimersByTime(50);
    // Both scrollers of the gesture, once each.
    expect(emitted).toHaveLength(5);
    vi.advanceTimersByTime(1000);
    expect(emitted).toHaveLength(5);
  });

  it('sends no end position with a debounce of 0', () => {
    debounce = 0;
    capture.onScroll(document);
    frame();
    vi.advanceTimersByTime(1000);
    expect(emitted).toHaveLength(1);
  });

  it('captures nothing while off, and asks for no frame', () => {
    on = false;
    capture.onScroll(document);
    expect(frames).toHaveLength(0);
    vi.advanceTimersByTime(1000);
    expect(emitted).toHaveLength(0);
  });

  it('drops what is pending when switched off before the frame', () => {
    capture.onScroll(document);
    on = false;
    frame();
    vi.advanceTimersByTime(1000);
    expect(emitted).toHaveLength(0);
  });

  it('stops when disposed', () => {
    capture.onScroll(document);
    capture.dispose();
    frame();
    vi.advanceTimersByTime(1000);
    expect(emitted).toHaveLength(0);
  });
});

describe('the scroll snapshot', () => {
  let layout: Layout;

  beforeEach(() => {
    layout = new Layout();
  });

  afterEach(() => {
    layout.restore();
  });

  it('of the document carries the position and the anchor', () => {
    const sections = Array.from(document.querySelectorAll('main > section'));
    const bottom = stack(layout, sections, Array(8).fill(500), { left: 288, top: 72, width: 1128 });
    layout.page.height = bottom;
    layout.install();
    window.scrollTo({ top: 572 + 250 });
    expect(snapshot(document)).toEqual({
      x: 0,
      y: 822,
      maxX: 0,
      maxY: bottom - 800,
      anchor: { element: { kind: 'css', path: '#s2' }, offset: { x: -0.255319, y: 0.5 } },
    });
  });

  it('of a container names it, without an anchor where none qualifies', () => {
    const sidebar = el('#sidebar');
    layout.box(sidebar, { left: 24, top: 72, width: 240, height: 400 });
    layout.scroller(sidebar, 240, 1200);
    layout.install();
    sidebar.scrollTo({ top: 300 });
    const data = snapshot(sidebar);
    expect(data).toMatchObject({ x: 0, y: 300, maxX: 0, maxY: 800 });
    expect(data.container?.element).toEqual({ kind: 'css', path: '#sidebar' });
    expect(data).not.toHaveProperty('anchor');
  });
});
