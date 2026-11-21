import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { HOST_TAG } from '../constants';
import { Controller } from '../controller';
import { DEFAULT_SETTINGS, type State, toSettings } from '../types';
import { Layer } from '../layer';
import { createTabOrder, focused, readingShapes, shapes } from './tab-order';
import { MARKER, MISREAD, READING, REVERSE, WARNED } from './tab-order/colors';
import type { TabItem } from './tab-order/list';

const state = (all: State['all']): State => ({ version: 1, all, views: {} });
const sink = () => ({ emit: vi.fn(), send: vi.fn() });
const on = state({ 'tab-order': { enabled: true } });

let controller: Controller | undefined;
beforeEach(() => {
  document.body.innerHTML = '<button id="one">1</button><a id="two" href="#t">2</a><input id="three">';
});
afterEach(() => {
  controller?.destroy();
  controller = undefined;
});

const item = (index: number, warnings: string[] = []): TabItem => ({
  stop: { element: document.body, tabindex: 0, frames: [] },
  index,
  rect: { x: 0, y: 0, width: 10, height: 10 },
  warnings,
});

describe('the tab order overlay', () => {
  it('numbers the stops, joins them in order and colours warnings and jumps back', () => {
    const items = [item(1), item(2, ['reverse-jump']), item(3, ['positive-tabindex'])];
    const drawn = shapes(items, [{ x: 50, y: 0 }, { x: 0, y: 0 }, { x: 0, y: 40 }], true);
    expect(drawn.markers.map((shape) => (shape.kind === 'marker' ? [shape.text, shape.color] : []))).toEqual([
      ['1', MARKER],
      ['2', REVERSE],
      ['3', WARNED],
    ]);
    expect(drawn.lines.map((shape) => shape.color)).toEqual([REVERSE, MARKER]);
    expect(shapes(items, [{ x: 0, y: 0 }, { x: 0, y: 0 }, { x: 0, y: 0 }], false).lines).toEqual([]);
  });

  it('tells the panel the count, and leaves no trace when switched off', () => {
    const events = sink();
    controller = new Controller(document, window, 1, events, [createTabOrder()]);
    const before = document.documentElement.outerHTML;
    controller.apply(on, {});
    expect(events.emit).toHaveBeenCalledWith({ type: 'hits', overlay: 'tab-order', count: 3 });
    expect(document.querySelector(HOST_TAG)).not.toBeNull();
    controller.apply(state({ 'tab-order': { enabled: false } }), {});
    expect(document.documentElement.outerHTML).toBe(before);
  });

  it('names the stop focused here to the component, once per stop, while live', () => {
    const events = sink();
    controller = new Controller(document, window, 1, events, [createTabOrder()]);
    controller.apply(on, {});
    (document.getElementById('two') as HTMLElement).focus();
    (document.getElementById('two') as HTMLElement).dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    expect(events.send).toHaveBeenCalledWith({ type: 'tab-focus', path: 'html > body > a' });
    (document.getElementById('two') as HTMLElement).dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    expect(events.send).toHaveBeenCalledTimes(1);
  });

  it('stays quiet with live off and takes a focus another view posts', () => {
    const events = sink();
    controller = new Controller(document, window, 1, events, [createTabOrder()]);
    controller.apply(on, { tabOrderLive: false });
    (document.getElementById('one') as HTMLElement).focus();
    (document.getElementById('one') as HTMLElement).dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    expect(events.send).not.toHaveBeenCalled();
    controller.receive({ type: 'tab-focus', view: 2, path: 'html > body > input' });
    controller.receive({ type: 'other' });
    expect(document.activeElement?.id).toBe('one');
  });

  it('follows the focus into open shadow roots', () => {
    const host = document.createElement('x-field');
    document.body.append(host);
    const root = host.attachShadow({ mode: 'open' });
    root.innerHTML = '<input id="inner">';
    (root.getElementById('inner') as HTMLElement).focus();
    expect(focused(document)?.id).toBe('inner');
  });

  it('reads its three settings, each true unless set', () => {
    expect([DEFAULT_SETTINGS.tabOrderShowLines, DEFAULT_SETTINGS.tabOrderLive, DEFAULT_SETTINGS.tabOrderIncludeIframes]).toEqual([true, true, true]);
    const settings = toSettings({ tabOrderShowLines: false, tabOrderLive: 'no', tabOrderIncludeIframes: false });
    expect([settings.tabOrderShowLines, settings.tabOrderLive, settings.tabOrderIncludeIframes]).toEqual([false, true, false]);
  });

  it('reads the reading order settings: off and 4 px unless set to a choice and a tolerance in range', () => {
    expect([DEFAULT_SETTINGS.tabOrderReadingOrder, DEFAULT_SETTINGS.tabOrderReadingTolerancePx]).toEqual(['off', 4]);
    const set = toSettings({ tabOrderReadingOrder: 'instead', tabOrderReadingTolerancePx: 0 });
    expect([set.tabOrderReadingOrder, set.tabOrderReadingTolerancePx]).toEqual(['instead', 0]);
    const broken = toSettings({ tabOrderReadingOrder: 'sideways', tabOrderReadingTolerancePx: 51 });
    expect([broken.tabOrderReadingOrder, broken.tabOrderReadingTolerancePx]).toEqual(['off', 4]);
  });
});

/** Gives `element` the box `x, y, width, height` (happy-dom lays nothing out). */
function place(element: Element, x: number, y: number, width = 80, height = 30): void {
  const rect = { x, y, width, height, left: x, top: y, right: x + width, bottom: y + height };
  (element as HTMLElement).getBoundingClientRect = () => ({ ...rect, toJSON: () => rect }) as DOMRect;
}

describe('the reading order arrows (RB-094-02)', () => {
  const placed = (index: number, x: number): TabItem => ({ ...item(index), rect: { x, y: 0, width: 80, height: 30 } });

  it('join the centres in source order with dashed arrows and frame an element read out of place', () => {
    const [a, b, c] = [placed(1, 96), placed(2, 0), placed(3, 192)] as [TabItem, TabItem, TabItem];
    const drawn = readingShapes([a, b, c], [a.rect, b.rect, c.rect], new Set([b]));
    const lines = drawn.arrows.filter((shape) => shape.kind === 'line');
    expect(lines).toEqual([
      { kind: 'line', x1: 136, y1: 15, x2: 40, y2: 15, color: MISREAD, dashed: true },
      { kind: 'line', x1: 40, y1: 15, x2: 232, y2: 15, color: READING, dashed: true },
    ]);
    expect(drawn.arrows.filter((shape) => shape.kind === 'head').map((shape) => shape.kind === 'head' && [shape.x, shape.y, shape.color])).toEqual([
      [40, 15, MISREAD],
      [232, 15, READING],
    ]);
    expect(drawn.marks).toEqual([{ rect: b.rect, color: MISREAD, border: 2, halo: true }]);
  });

  it('are drawn beside the tab order lines, instead of them, or not at all, as the setting says', () => {
    document.body.innerHTML = '<button id="a">A</button><button id="b">B</button><button id="c">C</button>';
    // order: -1 on the second button: B shows first, the source reads A, B, C.
    place(document.getElementById('a') as Element, 96, 0);
    place(document.getElementById('b') as Element, 0, 0);
    place(document.getElementById('c') as Element, 192, 0);
    const counts = (readingOrder: 'off' | 'beside' | 'instead'): number[] => {
      const layer = new Layer(document);
      const overlay = createTabOrder();
      const context = { doc: document, win: window, layer, settings: { ...DEFAULT_SETTINGS, tabOrderReadingOrder: readingOrder }, ...sink() };
      overlay.update(context, {});
      const found = ['tab-order-lines', 'tab-order-reading', 'tab-order-misread'].map((group) => layer.count(group));
      overlay.clear(context);
      layer.destroy();
      return found;
    };
    // Two tab order lines; two arrows of a line and a head each; one element framed.
    expect(counts('beside')).toEqual([2, 4, 1]);
    expect(counts('instead')).toEqual([0, 4, 1]);
    expect(counts('off')).toEqual([2, 0, 0]);
  });
});
