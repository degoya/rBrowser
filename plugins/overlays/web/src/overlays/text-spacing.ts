// Text spacing (WCAG 1.4.12): the spacing the criterion asks text to survive - line height 1.5,
// paragraph spacing 2em, letter spacing 0.12em, word spacing 0.16em - applied through a sheet of
// the plugin's own (removed again when switched off), and every box whose text is then clipped
// (`overflow: hidden|clip`) or spills out of its fixed height marked.

import { type Finder, createScanOverlay, elementsBelowBody, value } from './scan';
import { PageSheet } from './sheet';

type Style = Pick<CSSStyleDeclaration, 'getPropertyValue'>;
export interface TextBox {
  clientWidth: number;
  clientHeight: number;
  scrollWidth: number;
  scrollHeight: number;
}

const COLOR = '#e5262b';
export const SPACING_CSS =
  '*, *::before, *::after { line-height: 1.5 !important; letter-spacing: 0.12em !important; word-spacing: 0.16em !important; } p { margin-bottom: 2em !important; }';
const SCROLLS = new Set(['auto', 'scroll']);
const CLIPS = new Set(['hidden', 'clip']);

/** The label of a box whose text no longer fits (`clipped +12px`, `overflows +8px`); none when
 *  it fits or scrolls. */
export function clippedLabel(box: TextBox, style: Style): string | undefined {
  const overflowY = value(style, 'overflow-y');
  const overflowX = value(style, 'overflow-x');
  const overY = box.scrollHeight - box.clientHeight;
  const overX = box.scrollWidth - box.clientWidth;
  if (box.clientHeight > 0 && overY > 1 && !SCROLLS.has(overflowY)) {
    return `${CLIPS.has(overflowY) ? 'clipped' : 'overflows'} +${Math.round(overY)}px`;
  }
  if (box.clientWidth > 0 && overX > 1 && CLIPS.has(overflowX)) return `clipped +${Math.round(overX)}px`;
  return undefined;
}

export const findClipped: Finder = ({ doc, win }) =>
  elementsBelowBody(doc).flatMap((element) => {
    if (!element.textContent?.trim()) return [];
    const label = clippedLabel(element, win.getComputedStyle(element));
    return label ? [{ element, label }] : [];
  });

export function createTextSpacing() {
  const sheet = new PageSheet(SPACING_CSS);
  return createScanOverlay('text-spacing', COLOR, findClipped, {
    refresh: (context) => sheet.attach(context.doc, context.win),
    off: (context) => sheet.detach(context.doc),
  });
}
