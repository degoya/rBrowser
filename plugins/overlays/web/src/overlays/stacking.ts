// Stacking contexts: every element that opens one, labelled with its z-index and the reason
// (`z:10 position + z-index`, `z:auto opacity +2`). Nothing on the page changes.

import { type Finder, createScanOverlay, elementsBelowBody, value } from './scan';

type Style = Pick<CSSStyleDeclaration, 'getPropertyValue'>;

const COLOR = '#7048e8';
/** Properties that open a stacking context unless they hold the value given. */
const UNLESS: [string, string][] = [
  ['transform', 'none'],
  ['translate', 'none'],
  ['rotate', 'none'],
  ['scale', 'none'],
  ['filter', 'none'],
  ['backdrop-filter', 'none'],
  ['perspective', 'none'],
  ['clip-path', 'none'],
  ['mask-image', 'none'],
  ['mix-blend-mode', 'normal'],
  ['isolation', 'auto'],
];
const WILL_CHANGE = /\b(transform|translate|rotate|scale|opacity|filter|backdrop-filter|perspective|clip-path|mask|isolation|mix-blend-mode|z-index|position)\b/;
const CONTAIN = /\b(layout|paint|strict|content)\b/;
const FLEX_OR_GRID = /(^|-)(flex|grid)$/;

/** Why an element with `style` (its parent's `parent`) opens a stacking context; none when it
 *  does not. */
export function stackingReasons(style: Style, parent?: Style): string[] {
  const reasons: string[] = [];
  const z = value(style, 'z-index');
  const zSet = z !== '' && z !== 'auto';
  const position = value(style, 'position');
  if (position === 'fixed' || position === 'sticky') reasons.push(`position: ${position}`);
  else if ((position === 'absolute' || position === 'relative') && zSet) reasons.push('position + z-index');
  else if (zSet && parent && FLEX_OR_GRID.test(value(parent, 'display'))) reasons.push('flex/grid item + z-index');
  const opacity = Number.parseFloat(value(style, 'opacity'));
  if (opacity < 1) reasons.push('opacity');
  for (const [name, none] of UNLESS) {
    const current = value(style, name);
    if (current !== '' && current !== none) reasons.push(name);
  }
  if (WILL_CHANGE.test(value(style, 'will-change'))) reasons.push('will-change');
  if (CONTAIN.test(value(style, 'contain'))) reasons.push('contain');
  const container = value(style, 'container-type');
  if (container === 'size' || container === 'inline-size') reasons.push('container-type');
  return reasons;
}

/** The label of a stacking context: its z-index, its first reason and how many more. */
export function stackingLabel(z: string, reasons: string[]): string {
  const more = reasons.length > 1 ? ` +${reasons.length - 1}` : '';
  return `z:${z || 'auto'} ${reasons[0] ?? ''}${more}`;
}

export const findStacking: Finder = ({ doc, win }) =>
  elementsBelowBody(doc).flatMap((element) => {
    const style = win.getComputedStyle(element);
    const parent = element.parentElement ? win.getComputedStyle(element.parentElement) : undefined;
    const reasons = stackingReasons(style, parent);
    return reasons.length ? [{ element, label: stackingLabel(value(style, 'z-index'), reasons) }] : [];
  });

export const createStacking = () => createScanOverlay('stacking', COLOR, findStacking);
