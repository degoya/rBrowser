// Small text: every element whose own text is set below `smallTextPx` (12 by default, or the
// option `px`), labelled with its computed font size. Nothing on the page changes.

import { type Finder, MAX_ELEMENTS, createScanOverlay, intOption } from './scan';

const COLOR = '#1c7ed6';
const SKIP = new Set(['script', 'style', 'noscript', 'template', 'title']);

export const findSmallText: Finder = ({ doc, win, settings, options }) => {
  const limit = intOption(options, 'px', 6, 32, settings.smallTextPx);
  if (!doc.body) return [];
  const walker = doc.createTreeWalker(doc.body, 4 /* NodeFilter.SHOW_TEXT */);
  const seen = new Set<Element>();
  const hits = [];
  for (let node = walker.nextNode(), visited = 0; node && visited < MAX_ELEMENTS; node = walker.nextNode(), visited++) {
    const parent = node.parentElement;
    if (!parent || seen.has(parent) || SKIP.has(parent.localName) || !node.nodeValue?.trim()) continue;
    seen.add(parent);
    const size = Number.parseFloat(win.getComputedStyle(parent).fontSize);
    if (size > 0 && size < limit) hits.push({ element: parent, label: `${Math.round(size * 10) / 10}px` });
  }
  return hits;
};

export const createSmallText = () => createScanOverlay('small-text', COLOR, findSmallText);
