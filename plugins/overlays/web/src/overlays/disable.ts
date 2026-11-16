// CSS off and images off: the page without its style sheets, or without its images. Both change
// the page by definition and both are undone when switched off:
// - CSS off disables every style sheet of the document (`CSSStyleSheet.disabled`, no attribute),
//   remembering which it disabled, and with `cssOffIncludeInline` (or the option `includeInline`)
//   reverts the style attributes through a sheet of the plugin's own;
// - images off hides images through a sheet of the plugin's own: <img>, <picture>, SVG images,
//   image buttons and CSS background images. (Blocking the requests waits for the host's network
//   rules, RB-070-02.)
// JavaScript off has no page side: the component switches it over CDP (src/effects.rs).

import { HOST_TAG } from '../constants';
import type { Overlay, OverlayContext } from './overlay';
import { PageSheet, isOwn } from './sheet';

export const INLINE_OFF_CSS = `[style]:not(${HOST_TAG}) { all: revert !important; }`;
export const IMAGES_OFF_CSS =
  'img, picture, svg image, input[type="image"] { visibility: hidden !important; } *, *::before, *::after { background-image: none !important; }';

/** Every style sheet of `doc` that is not the plugin's own: its <style> and <link> sheets and
 *  its adopted ones. */
export function pageSheets(doc: Document): CSSStyleSheet[] {
  const sheets = [...Array.from(doc.styleSheets), ...(doc.adoptedStyleSheets ?? [])] as CSSStyleSheet[];
  return sheets.filter((sheet) => !isOwn(sheet));
}

export function createCssOff(): Overlay {
  const disabled = new Set<CSSStyleSheet>();
  const inline = new PageSheet(INLINE_OFF_CSS);
  let includeInline: boolean | undefined;

  const enableAgain = (): void => {
    for (const sheet of disabled) sheet.disabled = false;
    disabled.clear();
  };

  return {
    id: 'css-off',
    update(context: OverlayContext, options: Record<string, unknown>) {
      includeInline = typeof options.includeInline === 'boolean' ? options.includeInline : undefined;
      this.measure(context);
    },
    measure(context: OverlayContext) {
      for (const sheet of pageSheets(context.doc)) {
        if (sheet.disabled) continue;
        sheet.disabled = true;
        disabled.add(sheet);
      }
      if (includeInline ?? context.settings.cssOffIncludeInline) inline.attach(context.doc, context.win);
      else inline.detach(context.doc);
    },
    draw() {},
    clear(context: OverlayContext) {
      enableAgain();
      inline.detach(context.doc);
    },
  };
}

export function createImagesOff(): Overlay {
  const sheet = new PageSheet(IMAGES_OFF_CSS);
  return {
    id: 'images-off',
    update(context: OverlayContext) {
      this.measure(context);
    },
    measure(context: OverlayContext) {
      sheet.attach(context.doc, context.win);
    },
    draw() {},
    clear(context: OverlayContext) {
      sheet.detach(context.doc);
    },
  };
}
