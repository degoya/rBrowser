// RB-090-17: a big page's scan stays small - the page script lists at most MAX_ITEMS hits, every
// text cut, within the answer's byte budget, and always tells the whole count.
import { describe, expect, it, vi } from 'vitest';
import { MAX_ANSWER_BYTES, MAX_TEXT_CHARS } from '../answer';
import { Layer } from '../layer';
import { DEFAULT_SETTINGS } from '../types';
import { createScanOverlay, type Hit, MAX_HITS, MAX_ITEMS, scanAnswer } from './scan';

const bytes = (value: unknown): number => new TextEncoder().encode(JSON.stringify(value)).length;

function page(count: number): Element[] {
  document.body.innerHTML = '';
  const elements: Element[] = [];
  for (let n = 0; n < count; n += 1) {
    const element = document.createElement('div');
    element.className = `stacking-context-${n}`;
    document.body.append(element);
    elements.push(element);
  }
  return elements;
}

describe('a scan of a big page', () => {
  it('answers the whole count and at most MAX_ITEMS hits, every text cut', () => {
    const label = 'z:auto 中'.repeat(200);
    const hits: Hit[] = page(MAX_ITEMS + 50).map((element) => ({ element, label }));
    const answer = scanAnswer(window, hits);
    expect(answer.count).toBe(MAX_ITEMS + 50);
    expect(answer.items.length).toBeGreaterThan(0);
    expect(answer.items.length).toBeLessThanOrEqual(MAX_ITEMS);
    for (const item of answer.items) {
      expect(Array.from(item.label).length).toBeLessThanOrEqual(MAX_TEXT_CHARS);
      expect(Array.from(item.selector).length).toBeLessThanOrEqual(MAX_TEXT_CHARS);
      expect(Array.from(item.path).length).toBeLessThanOrEqual(MAX_TEXT_CHARS);
    }
    expect(bytes(answer)).toBeLessThanOrEqual(MAX_ANSWER_BYTES + 64);
  });

  it('keeps every hit of a small page', () => {
    const hits: Hit[] = page(3).map((element) => ({ element, label: 'z:1 opacity' }));
    const answer = scanAnswer(window, hits);
    expect(answer.count).toBe(3);
    expect(answer.items.map((item) => item.label)).toEqual(['z:1 opacity', 'z:1 opacity', 'z:1 opacity']);
  });

  it('tells the panel the whole count and how many it draws', () => {
    const elements = page(MAX_HITS + 20);
    const overlay = createScanOverlay('stacking', '#7048e8', () => elements.map((element) => ({ element, label: 'z:1' })));
    const emit = vi.fn();
    const context = { doc: document, win: window, layer: new Layer(document), settings: DEFAULT_SETTINGS, emit, send: vi.fn() };
    overlay.update(context, {});
    expect(emit).toHaveBeenCalledWith({ type: 'hits', overlay: 'stacking', count: MAX_HITS + 20, shown: MAX_HITS });
    overlay.clear(context);
    context.layer.destroy();
  });
});
