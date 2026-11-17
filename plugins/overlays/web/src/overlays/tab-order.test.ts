import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { HOST_TAG } from '../constants';
import { Controller } from '../controller';
import { DEFAULT_SETTINGS, type State, toSettings } from '../types';
import { createTabOrder, focused, shapes } from './tab-order';
import { MARKER, REVERSE, WARNED } from './tab-order/colors';
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
});
