// The text-measure collector the component runs in a view through `evaluate` (src/measure.rs):
// `(<this function>)({ selector, maxElements, maxLines })` in the plugin's isolated world. For
// every rendered element the selector matches - an element inside one already measured is left
// out, its text is counted there - it walks the text nodes and asks each character's box with
// `Range.getClientRects()`; a character whose middle lies inside the current line's height
// belongs to it, else it starts the next line. Collapsed white space has no box and a space at
// a soft wrap no width: neither is counted. Each line is answered with its characters and its
// box in document coordinates (for the marks). Rating is the component's.
// @ts-check
/** @param {{ selector: string, maxElements?: number, maxLines?: number, maxChars?: number }} options */
(options) => {
  'use strict';
  /** @type {NodeListOf<Element>} */
  let found;
  try {
    found = document.querySelectorAll(String(options.selector));
  } catch {
    return { invalidSelector: true };
  }
  const maxElements = Math.max(1, Math.min(Number(options.maxElements) || 500, 500));
  const maxLines = Math.max(1, Math.min(Number(options.maxLines) || 5000, 5000));
  const maxChars = Math.max(1, Math.min(Number(options.maxChars) || 200000, 200000));
  const scrollLeft = window.scrollX;
  const scrollTop = window.scrollY;
  const range = document.createRange();
  let chars = 0;
  let lineCount = 0;
  let truncated = false;

  /** @typedef {{ top: number, bottom: number, left: number, right: number, chars: number }} Line */

  /** @param {Element} element */
  const plugin = (element) => {
    for (let node = /** @type {Element | null} */ (element); node; node = node.parentElement) {
      if (node.localName.startsWith('rbrowser-')) return true;
    }
    return false;
  };

  /** A short path for the answer: the element and up to two ancestors below `html`. @param {Element} element */
  const path = (element) => {
    const parts = [];
    for (let node = /** @type {Element | null} */ (element); node && parts.length < 3; node = node.parentElement) {
      if (node.id) {
        parts.unshift(`${node.localName}#${node.id}`);
        break;
      }
      const classes = [...node.classList].slice(0, 2).map((name) => `.${name}`).join('');
      parts.unshift(node.localName + classes);
      if (node.localName === 'body') break;
    }
    return parts.join(' > ').slice(0, 200);
  };

  /** @param {Element} element @returns {Line[]} */
  const lines = (element) => {
    /** @type {Line[]} */
    const out = [];
    /** @type {Line | null} */
    let line = null;
    const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT);
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      const text = node.nodeValue ?? '';
      for (let at = 0; at < text.length; at += 1) {
        if (chars >= maxChars) {
          truncated = true;
          return out;
        }
        const code = text.charCodeAt(at);
        // A surrogate pair is one character.
        const end = code >= 0xd800 && code <= 0xdbff && at + 1 < text.length ? at + 2 : at + 1;
        range.setStart(node, at);
        range.setEnd(node, end);
        at = end - 1;
        const box = range.getClientRects()[0];
        if (!box || box.width <= 0) continue;
        chars += 1;
        const middle = box.top + box.height / 2;
        if (line && middle >= line.top && middle <= line.bottom) {
          line.chars += 1;
          line.left = Math.min(line.left, box.left);
          line.right = Math.max(line.right, box.right);
          line.top = Math.min(line.top, box.top);
          line.bottom = Math.max(line.bottom, box.bottom);
        } else {
          line = { top: box.top, bottom: box.bottom, left: box.left, right: box.right, chars: 1 };
          out.push(line);
        }
      }
    }
    return out;
  };

  /** @type {Element[]} */
  const measured = [];
  const elements = [];
  for (const element of found) {
    if (elements.length >= maxElements || lineCount >= maxLines) {
      truncated = true;
      break;
    }
    if (measured.some((outer) => outer.contains(element))) continue;
    if (element.getClientRects().length === 0 || plugin(element)) continue;
    const style = getComputedStyle(element);
    if (style.visibility === 'hidden' || style.visibility === 'collapse') continue;
    const kept = lines(element).slice(0, maxLines - lineCount);
    if (kept.length === 0) continue;
    measured.push(element);
    lineCount += kept.length;
    elements.push({
      path: path(element),
      lines: kept.map((line) => ({
        chars: line.chars,
        x: Math.round((line.left + scrollLeft) * 10) / 10,
        y: Math.round((line.top + scrollTop) * 10) / 10,
        width: Math.round((line.right - line.left) * 10) / 10,
        height: Math.round((line.bottom - line.top) * 10) / 10,
      })),
    });
  }
  return { elements, truncated };
}
