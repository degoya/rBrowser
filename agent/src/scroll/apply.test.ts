import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import scrollPage from '../../../tests/fixtures/site/scroll.html?raw';
import { Agent } from '../agent';
import type { AgentMessage, ScrollData } from '../protocol';
import { el, loadPage } from '../test-page';
import { applyScroll, pointOf, targetAxis } from './apply';
import { snapshot } from './capture';
import { Layout, desktop, mobile, stack } from './test-layout';

let layout: Layout;

/** The leader's scroll in the desktop layout, as the follower gets it. */
function captured(scrollY: number): ScrollData {
  const leader = new Layout();
  desktop(leader);
  leader.install();
  window.scrollTo({ top: scrollY });
  const data = snapshot(document);
  leader.restore();
  vi.restoreAllMocks();
  return data;
}

beforeEach(() => {
  loadPage(scrollPage);
  layout = new Layout();
});

afterEach(() => {
  layout.restore();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe('the offset math', () => {
  it('lands exactly at the start and at the end', () => {
    expect(targetAxis(0, 3000, 500, 9000, 120)).toBe(0);
    expect(targetAxis(0.4, 3000, 500, 9000, null)).toBe(0);
    expect(targetAxis(3000, 3000, 500, 9000, -40)).toBe(9000);
    expect(targetAxis(2999.5, 3000, 500, 9000, null)).toBe(9000);
  });

  it('moves by the anchor, else by the share of the scroll height', () => {
    expect(targetAxis(1500, 3000, 500, 9000, 1220)).toBe(1720);
    expect(targetAxis(1500, 3000, 500, 9000, null)).toBe(4500);
    expect(targetAxis(1500, 3000, 500, 9000, -900)).toBe(0);
    expect(targetAxis(1500, 3000, 8900, 9000, 400)).toBe(9000);
  });

  it('stays at 0 where nothing scrolls', () => {
    expect(targetAxis(200, 400, 0, 0, 50)).toBe(0);
  });
});

describe('a scroll in a follower', () => {
  it('brings section 5 to the top of a mobile view as it is in the desktop view', () => {
    // Desktop: section 5 from 2472, 720 px tall; the leader shows it from 180 px in (25 %).
    const data = captured(2472 + 180);
    expect(data.anchor?.element).toEqual({ kind: 'css', path: '#s5' });
    mobile(layout);
    layout.install();
    expect(applyScroll(data, document)).toEqual({ status: 'done' });
    // Mobile: section 5 from 496 + 1600 + 1200 + 1600 + 1200 = 6096, 1600 px tall.
    const s5 = el('#s5').getBoundingClientRect();
    expect(Math.abs(window.scrollY - (6096 + 0.25 * 1600))).toBeLessThanOrEqual(1);
    expect(Math.abs(s5.top + 0.25 * s5.height)).toBeLessThanOrEqual(10);
  });

  it('is exactly at the top and at the bottom', () => {
    const top = captured(0);
    const bottom = captured(1e9);
    expect(bottom.y).toBe(bottom.maxY);
    mobile(layout);
    layout.install();
    window.scrollTo({ top: 3000 });
    applyScroll(top, document);
    expect(window.scrollY).toBe(0);
    applyScroll(bottom, document);
    expect(window.scrollY).toBe(layout.page.height - 844);
  });

  it('falls back to the share of the scroll height without its anchor', () => {
    mobile(layout);
    layout.install();
    const data: ScrollData = {
      x: 0,
      y: 1000,
      maxX: 0,
      maxY: 4000,
      anchor: { element: { kind: 'css', path: '#gone' }, offset: { x: 0, y: 0.5 } },
    };
    expect(applyScroll(data, document)).toEqual({ status: 'done' });
    expect(window.scrollY).toBe(Math.round((layout.page.height - 844) / 4));
  });

  it('scrolls the same inner sidebar, and the page stays', () => {
    const sidebar = el('#sidebar');
    layout.box(sidebar, { left: 24, top: 72, width: 240, height: 400 });
    layout.scroller(sidebar, 240, 24 * 48);
    stack(layout, Array.from(sidebar.querySelectorAll('li')), Array(24).fill(48), { width: 240 });
    layout.install();
    sidebar.scrollTo({ top: 100 });
    const data = snapshot(sidebar);
    expect(data.anchor?.element).toEqual({ kind: 'css', path: '#item-3' });
    layout.restore();
    vi.restoreAllMocks();

    // The follower's items are 64 px tall.
    const follower = new Layout();
    follower.box(sidebar, { left: 24, top: 496, width: 342, height: 400 });
    follower.scroller(sidebar, 342, 24 * 64);
    stack(follower, Array.from(sidebar.querySelectorAll('li')), Array(24).fill(64), {
      width: 342,
    });
    follower.install();
    expect(applyScroll(data, document)).toEqual({ status: 'done' });
    expect(Math.abs(sidebar.scrollTop - (2 * 64 + 0.083333 * 64))).toBeLessThanOrEqual(1);
    expect(window.scrollY).toBe(0);
    expect(follower.scrollEvents).toBe(1);
    follower.restore();
  });

  it('mirrors a horizontal scroll of a table', () => {
    const wrap = el('#table-wrap');
    const row = el('#table-wrap tr');
    layout.box(wrap, { left: 288, top: 100, width: 1000, height: 60 });
    layout.scroller(wrap, 2400, 60);
    layout.box(row, { left: 0, top: 0, width: 2400, height: 60 });
    layout.install();
    wrap.scrollTo({ left: 500 });
    const data = snapshot(wrap);
    expect(data).toMatchObject({ x: 500, maxX: 1400, y: 0, maxY: 0 });
    layout.restore();
    vi.restoreAllMocks();

    const follower = new Layout();
    follower.viewport = { width: 390, height: 844 };
    follower.box(wrap, { left: 24, top: 100, width: 342, height: 60 });
    follower.scroller(wrap, 2400, 60);
    follower.box(row, { left: 0, top: 0, width: 2400, height: 60 });
    follower.install();
    applyScroll(data, document);
    expect(wrap.scrollLeft).toBe(500);
    expect(window.scrollY).toBe(0);
    follower.restore();
  });

  it('answers not_found when the container is missing, and scrolls nothing', () => {
    mobile(layout);
    layout.install();
    const data: ScrollData = {
      container: { element: { kind: 'css', path: '#nope' }, offset: { x: 0.5, y: 0.5 } },
      x: 0,
      y: 100,
      maxX: 0,
      maxY: 400,
    };
    expect(applyScroll(data, document)).toEqual({ status: 'not_found' });
    expect(layout.scrollEvents).toBe(0);
  });

  it('scrolls without a smooth animation and not at all when already there', () => {
    mobile(layout);
    layout.install();
    const scrollTo = vi.spyOn(window, 'scrollTo');
    applyScroll({ x: 0, y: 2000, maxX: 0, maxY: 4000 }, document);
    expect(scrollTo).toHaveBeenCalledWith({ left: 0, top: expect.any(Number), behavior: 'instant' });
    applyScroll({ x: 0, y: 2000, maxX: 0, maxY: 4000 }, document);
    expect(scrollTo).toHaveBeenCalledOnce();
  });
});

describe('the scroll command', () => {
  it('causes no echo: the scroll it makes is not captured', () => {
    vi.useFakeTimers();
    mobile(layout);
    layout.install();
    const sent: AgentMessage[] = [];
    const agent = new Agent({
      win: window,
      send: (payload) => sent.push(JSON.parse(payload) as AgentMessage),
      config: {
        active: true,
        capture: {
          click: true,
          hover: true,
          scroll: true,
          keyboard: true,
          input: true,
          forms: true,
          focus: false,
        },
        hoverThrottleMs: 100,
      },
      trusted: () => true,
    });
    agent.start();
    expect(agent.command({ cmd: 'scroll', x: 0, y: 2000, maxX: 0, maxY: 4000 })).toEqual({
      status: 'done',
    });
    expect(layout.scrollEvents).toBe(1);
    vi.advanceTimersByTime(1000);
    expect(sent.filter((message) => message.kind === 'event')).toHaveLength(0);
    agent.stop();
  });
});

describe('the wheel point', () => {
  it('is the target point, inside the viewport, or the centre without a target', () => {
    mobile(layout);
    layout.install();
    window.scrollTo({ top: 6096 });
    const s5 = { element: { kind: 'css' as const, path: '#s5' }, offset: { x: 0.5, y: 0.1 } };
    expect(pointOf(s5, document)).toEqual({ status: 'ok', x: 195, y: 160, scrolled: false });
    const s1 = { element: { kind: 'css' as const, path: '#s1' }, offset: { x: 0.5, y: 0.5 } };
    // Section 1 is above the viewport: the nearest point of the viewport.
    expect(pointOf(s1, document)).toMatchObject({ status: 'ok', y: 0, scrolled: false });
    const gone = { element: { kind: 'css' as const, path: '#gone' }, offset: { x: 0.5, y: 0.5 } };
    expect(pointOf(gone, document)).toEqual({ status: 'ok', x: 195, y: 422, scrolled: false });
  });
});
