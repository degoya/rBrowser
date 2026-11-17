// The placeholdifier: the layout without its content. A sheet of the plugin's own makes the
// page's text transparent and its media invisible (removed again when switched off); the overlay
// layer draws a grey block over every line of text and a framed placeholder with its size over
// every image, video, canvas, SVG and frame.

import type { Item } from '../layer';
import { MAX_ELEMENTS } from './scan';
import { type Overlay, type OverlayContext, toRect } from './overlay';
import { PageSheet } from './sheet';

const GROUP = 'placeholdifier';
const TEXT_COLOR = '#adb5bd';
const MEDIA_COLOR = '#495057';
const MEDIA = 'img, picture, video, canvas, svg, iframe, object, embed';
export const PLACEHOLDER_CSS = `body, body * { color: transparent !important; -webkit-text-fill-color: transparent !important; text-shadow: none !important; } ${MEDIA} { opacity: 0 !important; }`;
/** The most boxes drawn at once. */
export const MAX_BOXES = 1500;
const SKIP = new Set(['script', 'style', 'noscript', 'template', 'title']);

export function createPlaceholdifier(): Overlay {
  const sheet = new PageSheet(PLACEHOLDER_CSS);
  let texts: Text[] = [];
  let media: Element[] = [];

  return {
    id: 'placeholdifier',
    update(context: OverlayContext) {
      this.measure(context);
    },
    measure(context: OverlayContext) {
      sheet.attach(context.doc, context.win);
      texts = [];
      const body = context.doc.body;
      if (body) {
        const walker = context.doc.createTreeWalker(body, 4 /* NodeFilter.SHOW_TEXT */);
        for (let node = walker.nextNode(); node && texts.length < MAX_ELEMENTS; node = walker.nextNode()) {
          const parent = node.parentElement;
          if (parent && !SKIP.has(parent.localName) && node.nodeValue?.trim()) texts.push(node as Text);
        }
      }
      media = body ? Array.from(body.querySelectorAll(MEDIA)).filter((element) => !element.parentElement?.closest('svg')).slice(0, MAX_ELEMENTS) : [];
      this.draw(context);
    },
    draw(context: OverlayContext) {
      const { win, doc } = context;
      const items: Item[] = [];
      const shows = (rect: DOMRect): boolean =>
        rect.width > 0 && rect.height > 0 && rect.bottom >= 0 && rect.top <= win.innerHeight && rect.right >= 0 && rect.left <= win.innerWidth;
      for (const element of media) {
        if (items.length >= MAX_BOXES) break;
        const rect = element.getBoundingClientRect();
        if (element.isConnected && shows(rect)) {
          items.push({ rect: toRect(rect), color: MEDIA_COLOR, fill: true, border: 1, label: `${Math.round(rect.width)}x${Math.round(rect.height)}` });
        }
      }
      const range = doc.createRange();
      for (const node of texts) {
        if (items.length >= MAX_BOXES) break;
        if (!node.isConnected) continue;
        range.selectNodeContents(node);
        for (const rect of Array.from(range.getClientRects())) {
          if (items.length >= MAX_BOXES) break;
          if (shows(rect)) items.push({ rect: toRect(rect), color: TEXT_COLOR, fill: 'solid', border: 0 });
        }
      }
      context.layer.render(GROUP, items);
    },
    clear(context: OverlayContext) {
      texts = [];
      media = [];
      context.layer.clear(GROUP);
      sheet.detach(context.doc);
    },
  };
}
