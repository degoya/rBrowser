// Content chaos: every text of the page made `factor` times as long (words repeated, one very
// long word added) or as short (cut at a word), to see how the layout copes with other content.
// It changes the page's text nodes by definition; each changed node remembers its own text and
// gets it back when the overlay is switched off - unless the page changed it again meanwhile.

import { MAX_ELEMENTS } from './scan';
import type { Overlay, OverlayContext } from './overlay';

/** The long word added when text grows: as long as real long words in German or Finnish get. */
export const LONG_WORD = 'Pneumonoultramicroscopicsilicovolcanoconiosis';
const SKIP = new Set(['script', 'style', 'noscript', 'template', 'textarea', 'title', 'code', 'pre']);

/** `text` made `factor` times as long, its surrounding white space kept. */
export function chaosText(text: string, factor: number): string {
  const match = /^(\s*)([\s\S]*?)(\s*)$/.exec(text);
  const [, lead = '', core = '', trail = ''] = match ?? [];
  if (!core || factor === 1) return text;
  const words = core.split(/\s+/);
  if (factor > 1) {
    const target = Math.ceil(core.length * factor);
    const grown = [words[0] ?? '', LONG_WORD, ...words.slice(1)];
    for (let index = 1; grown.join(' ').length < target; index++) grown.push(words[index % words.length] ?? '');
    return `${lead}${grown.join(' ')}${trail}`;
  }
  const target = Math.max(1, Math.round(core.length * factor));
  const cut = core.slice(0, target);
  const space = cut.lastIndexOf(' ');
  return `${lead}${space > 0 ? cut.slice(0, space) : cut}${trail}`;
}

/** The text nodes below <body> content chaos changes. */
function textNodes(doc: Document): Text[] {
  if (!doc.body) return [];
  const walker = doc.createTreeWalker(doc.body, 4 /* NodeFilter.SHOW_TEXT */);
  const nodes: Text[] = [];
  for (let node = walker.nextNode(); node && nodes.length < MAX_ELEMENTS; node = walker.nextNode()) {
    const parent = node.parentElement;
    if (parent && !SKIP.has(parent.localName) && !parent.isContentEditable && node.nodeValue?.trim()) nodes.push(node as Text);
  }
  return nodes;
}

export function createContentChaos(): Overlay {
  const changed = new Map<Text, { original: string; changed: string }>();
  let factor = 2;

  const restore = (): void => {
    for (const [node, entry] of changed) if (node.nodeValue === entry.changed) node.nodeValue = entry.original;
    changed.clear();
  };

  return {
    id: 'content-chaos',
    update(context: OverlayContext, options: Record<string, unknown>) {
      const next = typeof options.factor === 'number' && options.factor >= 0.1 && options.factor <= 10 ? options.factor : context.settings.chaosFactor;
      if (next !== factor) restore();
      factor = next;
      this.measure(context);
    },
    measure(context: OverlayContext) {
      for (const node of textNodes(context.doc)) {
        const entry = changed.get(node);
        if (entry && node.nodeValue === entry.changed) continue;
        const original = node.nodeValue ?? '';
        const text = chaosText(original, factor);
        if (text === original) continue;
        changed.set(node, { original, changed: text });
        node.nodeValue = text;
      }
    },
    draw() {},
    clear() {
      restore();
    },
  };
}
