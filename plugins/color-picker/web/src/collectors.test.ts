// The collectors the component runs through `evaluate`: src/palette.js against the fixture page
// and src/measure.js against a laid-out text (happy-dom has no layout: the boxes are given).
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { collect, fixturePage, loadPage } from './support';

/** Every element has a box; a character's box comes from `layout`. */
function boxes(layout?: (node: Node, offset: number) => DOMRect | undefined): void {
  vi.spyOn(Element.prototype, 'getClientRects').mockImplementation(
    () => [new DOMRect(0, 0, 10, 10)] as unknown as DOMRectList,
  );
  if (!layout) return;
  vi.spyOn(Range.prototype, 'getClientRects').mockImplementation(function (this: Range) {
    const box = layout(this.startContainer, this.startOffset);
    return (box ? [box] : []) as unknown as DOMRectList;
  });
}

afterEach(() => {
  vi.restoreAllMocks();
  document.body.innerHTML = '';
});

/** Hex of a computed colour, whichever form the DOM writes. */
function hex(value: string): string {
  const rgb = /rgba?\((\d+),\s*(\d+),\s*(\d+)/.exec(value);
  if (!rgb) return value.toLowerCase();
  return `#${rgb.slice(1, 4).map((c) => Number(c).toString(16).padStart(2, '0')).join('')}`;
}

describe('the palette collector', () => {
  beforeEach(() => {
    loadPage(fixturePage('colors/index.html'));
    boxes();
  });

  it('counts the fixture five colours by use', () => {
    const answer = collect('src/palette.js', { maxElements: 20000 });
    const uses = Object.fromEntries(
      (answer.entries as Array<Record<string, number | string>>).map((entry) => [
        hex(String(entry.value)),
        Number(entry.color) + Number(entry.background) + Number(entry.border),
      ]),
    );
    expect(uses).toEqual({ '#212121': 7, '#1e88e5': 5, '#e53935': 4, '#43a047': 3, '#fdd835': 2 });
    expect(answer.truncated).toBe(false);
  });

  it('leaves out its own elements and stops at the element limit', () => {
    document.body.insertAdjacentHTML('beforeend', '<rbrowser-color-picker style="background:#123456">x</rbrowser-color-picker>');
    const answer = collect('src/palette.js', { maxElements: 20000 });
    const values = (answer.entries as Array<{ value: string }>).map((entry) => hex(entry.value));
    expect(values).not.toContain('#123456');
    expect(collect('src/palette.js', { maxElements: 3 }).truncated).toBe(true);
  });
});

/** Characters 8 px wide in lines of `perLine`, 27 px high, counted over the whole body. */
function monospace(perLine: number): (node: Node, offset: number) => DOMRect | undefined {
  return (node, offset) => {
    const text = node.nodeValue ?? '';
    if (text[offset] === '\n') return undefined;
    // The offset of this character in the document's text, collapsed spaces left out.
    let index = 0;
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
    for (let current = walker.nextNode(); current; current = walker.nextNode()) {
      if (current === node) {
        index += offset;
        break;
      }
      index += (current.nodeValue ?? '').length;
    }
    const line = Math.floor(index / perLine);
    return new DOMRect(24 + (index % perLine) * 8, 100 + line * 27, 8, 27);
  };
}

describe('the measure collector', () => {
  it('splits text into lines by the characters boxes', () => {
    document.body.innerHTML = `<p id="a">${'x'.repeat(250)}</p>`;
    boxes(monospace(100));
    const answer = collect('src/measure.js', { selector: 'p', maxElements: 500, maxLines: 5000 });
    expect(answer.elements).toHaveLength(1);
    const lines = answer.elements[0].lines as Array<Record<string, number>>;
    expect(lines.map((line) => line.chars)).toEqual([100, 100, 50]);
    expect(lines[1]).toMatchObject({ x: 24, y: 127, width: 800, height: 27 });
    expect(answer.elements[0].path).toBe('p#a');
  });

  it('counts a surrogate pair once and skips characters without a box', () => {
    document.body.innerHTML = '<p>a\u{1F600}b\nc</p>';
    boxes(monospace(100));
    const answer = collect('src/measure.js', { selector: 'p' });
    expect(answer.elements[0].lines[0].chars).toBe(4);
  });

  it('measures an element once even when its ancestor matches too', () => {
    document.body.innerHTML = '<ul><li>one <p>two</p></li></ul><p>three</p>';
    boxes(monospace(100));
    const answer = collect('src/measure.js', { selector: 'p, li' });
    expect(answer.elements.map((element: { path: string }) => element.path)).toEqual(['body > ul > li', 'body > p']);
  });

  it('answers a selector the page cannot read', () => {
    expect(collect('src/measure.js', { selector: 'p[' })).toEqual({ invalidSelector: true });
  });

  it('stops at the line limit', () => {
    document.body.innerHTML = `<p>${'x'.repeat(500)}</p><p>more</p>`;
    boxes(monospace(10));
    const answer = collect('src/measure.js', { selector: 'p', maxLines: 20 });
    expect(answer.elements[0].lines).toHaveLength(20);
    expect(answer.truncated).toBe(true);
  });
});
