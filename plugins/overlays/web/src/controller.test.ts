import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { HOST_TAG } from './constants';
import { Controller } from './controller';
import type { Overlay } from './overlays';
import { createHighlight, wrap } from './overlays/highlight';
import { collect, DEPTH_COLORS } from './overlays/outlines';
import { signature } from './overlays/overflow';
import { effective, toSettings, type State } from './types';

function spy(id: string): Overlay & { calls: string[] } {
  const calls: string[] = [];
  return {
    id,
    calls,
    update: (_context, options) => calls.push(`update ${JSON.stringify(options)}`),
    measure: () => calls.push('measure'),
    draw: () => calls.push('draw'),
    clear: () => calls.push('clear'),
  };
}

const state = (all: State['all'], views: State['views'] = {}): State => ({ version: 1, all, views });
const sink = () => ({ emit: vi.fn(), send: vi.fn() });

let controller: Controller | undefined;
beforeEach(() => {
  document.body.innerHTML = '<main><div class="card">a</div><div class="card">b</div><p class="card">c</p></main>';
});
afterEach(() => {
  controller?.destroy();
  controller = undefined;
  vi.useRealTimers();
});

describe('the controller', () => {
  it('switches the overlays of its own view and leaves the others alone', () => {
    const overflow = spy('overflow');
    const outlines = spy('outlines');
    controller = new Controller(document, window, 2, sink(), [overflow, outlines]);
    controller.apply(state({ overflow: { enabled: true } }, { '2': { outlines: { enabled: true, options: { containersOnly: true } } } }), {});
    expect(controller.shown).toEqual(['overflow', 'outlines']);
    expect(outlines.calls).toEqual(['update {"containersOnly":true}']);
    // The same state again changes nothing.
    controller.apply(state({ overflow: { enabled: true } }, { '2': { outlines: { enabled: true, options: { containersOnly: true } } } }), {});
    expect(overflow.calls).toEqual(['update {}']);
    controller.apply(state({ overflow: { enabled: true } }), {});
    expect(outlines.calls.at(-1)).toBe('clear');
    expect(controller.shown).toEqual(['overflow']);
  });

  it('updates the active overlays when the settings change', () => {
    const overflow = spy('overflow');
    controller = new Controller(document, window, 1, sink(), [overflow]);
    controller.apply(state({ overflow: { enabled: true } }), {});
    controller.apply(state({ overflow: { enabled: true } }), { mutationThrottleMs: 1000 });
    expect(overflow.calls).toEqual(['update {}', 'update {}']);
  });

  it('measures again after a DOM change, throttled, and ignores its own layer', async () => {
    vi.useFakeTimers();
    const overflow = spy('overflow');
    controller = new Controller(document, window, 1, sink(), [overflow]);
    controller.apply(state({ overflow: { enabled: true } }), { mutationThrottleMs: 100 });
    document.body.append(document.createElement('div'));
    document.body.append(document.createElement('div'));
    await vi.advanceTimersByTimeAsync(150);
    expect(overflow.calls.filter((call) => call === 'measure')).toHaveLength(1);
  });

  it('removes its layer and stops listening when every overlay is off, and on stop', () => {
    controller = new Controller(document, window, 1, sink());
    controller.apply(state({ highlight: { enabled: true, options: { selector: '.card' } } }), {});
    expect(document.querySelector(HOST_TAG)).not.toBeNull();
    controller.apply(state({ highlight: { enabled: false, options: { selector: '.card' } } }), {});
    expect(document.querySelector(HOST_TAG)).toBeNull();
    controller.apply(state({ overflow: { enabled: true } }), {});
    controller.destroy();
    expect(document.querySelector(HOST_TAG)).toBeNull();
  });

  it('tells the panel how many elements the selector matches, and a bad selector', () => {
    const events = sink();
    controller = new Controller(document, window, 1, events, [createHighlight()]);
    controller.apply(state({ highlight: { enabled: true, options: { selector: '.card', index: 4 } } }), {});
    expect(events.emit).toHaveBeenLastCalledWith({ type: 'matches', selector: '.card', count: 3, index: 1 });
    controller.apply(state({ highlight: { enabled: true, options: { selector: 'div[' } } }), {});
    expect(events.emit).toHaveBeenLastCalledWith({ type: 'matches', selector: 'div[', count: 0, index: 0, error: 'invalid' });
  });
});

describe('the pieces', () => {
  it('wraps the highlight index around the count either way', () => {
    expect(wrap(4, 3)).toBe(1);
    expect(wrap(-1, 3)).toBe(2);
    expect(wrap(5, 0)).toBe(0);
  });

  it('collects every element with its depth, or the flex and grid containers only', () => {
    document.body.innerHTML = '<div style="display: flex"><span><i></i></span></div><section style="display: inline-grid"></section>';
    const win = { getComputedStyle: (element: Element) => ({ display: (element as HTMLElement).style.display || 'block' }) as CSSStyleDeclaration };
    expect(collect(document.body, win, false).map((entry) => [entry.element.localName, entry.depth])).toEqual([
      ['div', 1],
      ['span', 2],
      ['i', 3],
      ['section', 1],
    ]);
    expect(collect(document.body, win, true).map((entry) => entry.element.localName)).toEqual(['div', 'section']);
    expect(DEPTH_COLORS.length).toBeGreaterThan(3);
  });

  it('logs a report again only when the scroll or a cause changed', () => {
    const base = { hasHorizontalScroll: true, scrollWidth: 416, clientWidth: 375, innerWidth: 375, vw100: [] };
    const one = signature({ ...base, causes: [{ selector: '.promo', rect: { x: 0, y: 0, width: 1, height: 1 }, overhang: 41 }] });
    const moved = signature({ ...base, causes: [{ selector: '.promo', rect: { x: 0, y: 99, width: 1, height: 1 }, overhang: 41 }] });
    expect(one).toBe(moved);
    expect(signature({ ...base, hasHorizontalScroll: false, causes: [] })).toBe('none');
  });

  it('takes a view of its own over every view, and checks the settings', () => {
    const both = state({ overflow: { enabled: true } }, { '3': { overflow: { enabled: false } } });
    expect(effective(both, 3, 'overflow').enabled).toBe(false);
    expect(effective(both, 1, 'overflow').enabled).toBe(true);
    expect(effective(both, 1, 'outlines').enabled).toBe(false);
    const settings = toSettings({ mutationThrottleMs: 1, colors: { cause: 'red', vw100: '#00ff00' } });
    expect(settings.mutationThrottleMs).toBe(250);
    expect(settings.colors.cause).toBe('#e5262b');
    expect(settings.colors.vw100).toBe('#00ff00');
    expect(settings.targetSizePx).toBe(24);
    const further = toSettings({ targetSizePx: 44, smallTextPx: 100, scrollbarTolerancePx: 2.5, chaosFactor: 0.5, cssOffIncludeInline: true });
    expect([further.targetSizePx, further.smallTextPx, further.scrollbarTolerancePx, further.chaosFactor, further.cssOffIncludeInline]).toEqual([
      44, 12, 4, 0.5, true,
    ]);
  });
});
