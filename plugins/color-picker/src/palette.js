// The palette collector the component runs in a view through `evaluate` (src/palette.rs):
// `(<this function>)({ maxElements })` in the plugin's isolated world, whose DOM is the page's.
// It counts, by their computed text, the `color` of every rendered element with text of its
// own, its `background-color` when not transparent and the colour of each visible border side
// (once per element). Rendered means it has a box and is not `visibility: hidden`; the plugin's
// own elements (`rbrowser-*`) are left out. Grouping and sorting are the component's.
// @ts-check
/** @param {{ maxElements?: number }} options */
(options) => {
  'use strict';
  const MAX_ENTRIES = 2000;
  const max = Math.max(1, Math.min(Number(options && options.maxElements) || 20000, 20000));
  const SIDES = ['top', 'right', 'bottom', 'left'];
  /** @type {Map<string, { value: string, color: number, background: number, border: number }>} */
  const counts = new Map();
  let truncated = false;

  /** @param {string} value @param {'color' | 'background' | 'border'} kind */
  const add = (value, kind) => {
    if (!value || value === 'transparent' || value === 'rgba(0, 0, 0, 0)') return;
    let entry = counts.get(value);
    if (!entry) {
      if (counts.size >= MAX_ENTRIES) {
        truncated = true;
        return;
      }
      entry = { value, color: 0, background: 0, border: 0 };
      counts.set(value, entry);
    }
    entry[kind] += 1;
  };

  /** @param {Element} element */
  const ownText = (element) =>
    [...element.childNodes].some((node) => node.nodeType === 3 && /\S/.test(node.nodeValue ?? ''));

  /** @param {Element} element */
  const plugin = (element) => {
    for (let node = /** @type {Element | null} */ (element); node; node = node.parentElement) {
      if (node.localName.startsWith('rbrowser-')) return true;
    }
    return false;
  };

  let elements = 0;
  for (const element of document.querySelectorAll('*')) {
    if (elements >= max) {
      truncated = true;
      break;
    }
    if (element.getClientRects().length === 0 || plugin(element)) continue;
    const style = getComputedStyle(element);
    if (style.visibility === 'hidden' || style.visibility === 'collapse') continue;
    elements += 1;
    if (ownText(element)) add(style.color, 'color');
    add(style.backgroundColor, 'background');
    const borders = new Set();
    for (const side of SIDES) {
      const width = Number.parseFloat(style.getPropertyValue(`border-${side}-width`)) || 0;
      const line = style.getPropertyValue(`border-${side}-style`);
      if (width > 0 && line !== 'none' && line !== 'hidden') borders.add(style.getPropertyValue(`border-${side}-color`));
    }
    for (const value of borders) add(value, 'border');
  }
  return { elements, truncated, entries: [...counts.values()] };
}
