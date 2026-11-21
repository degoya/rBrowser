// The kinds of step the Recorder panel's editor offers (RB-092-01), grouped as the add-step
// popover shows them - Navigation, Interaction, Input, Wait, Assert -, with what each needs: an
// element, a value and of which sort. A kind's action is built from an element and a value;
// changing a step's kind keeps its element and value where the new kind has them.

import type { Action, Locator } from './recording';
import { stepKind, stepValue, targetOf } from './recording';

/** A kind, the last part of its i18n key `recorder.panel.kind.<kind>`. */
export type StepKind =
  | 'navigate'
  | 'click'
  | 'dblclick'
  | 'hover'
  | 'check'
  | 'uncheck'
  | 'scroll'
  | 'fill'
  | 'select'
  | 'press'
  | 'waitForUrl'
  | 'waitFor'
  | 'wait'
  | 'assertVisible'
  | 'assertText'
  | 'assertValue'
  | 'assertUrl';

/** A group of kinds, the last part of its i18n key `recorder.editor.group.<group>`. */
export type KindGroup = 'navigation' | 'interaction' | 'input' | 'wait' | 'assert';

export const KIND_GROUPS: readonly { group: KindGroup; kinds: readonly StepKind[] }[] = [
  { group: 'navigation', kinds: ['navigate'] },
  { group: 'interaction', kinds: ['click', 'dblclick', 'hover', 'check', 'uncheck', 'scroll'] },
  { group: 'input', kinds: ['fill', 'select', 'press'] },
  { group: 'wait', kinds: ['waitForUrl', 'waitFor', 'wait'] },
  { group: 'assert', kinds: ['assertVisible', 'assertText', 'assertValue', 'assertUrl'] },
];

/** What a kind's value is: a URL, text, a key, an option of a `<select>`, a time in ms. */
export type ValueSort = 'url' | 'text' | 'key' | 'option' | 'ms';

const NO_ELEMENT: readonly StepKind[] = ['navigate', 'waitForUrl', 'wait', 'assertUrl'];
/** An element that may be absent: a key goes to the page, a scroll scrolls the page. */
const ELEMENT_OPTIONAL: readonly StepKind[] = ['press', 'scroll'];

const VALUES: Partial<Record<StepKind, ValueSort>> = {
  navigate: 'url',
  waitForUrl: 'url',
  assertUrl: 'url',
  fill: 'text',
  assertText: 'text',
  assertValue: 'text',
  select: 'option',
  press: 'key',
  wait: 'ms',
};

/** The keys a "Press key" step proposes; any other `KeyboardEvent.key` may be typed. */
export const PRESS_KEYS: readonly string[] = [
  'Enter',
  'Escape',
  'Tab',
  'Backspace',
  'Delete',
  ' ',
  'ArrowUp',
  'ArrowDown',
  'ArrowLeft',
  'ArrowRight',
  'Home',
  'End',
  'PageUp',
  'PageDown',
];

/** The longest fixed wait and pause before a step, ms (the exporter's cap). */
export const MAX_WAIT_MS = 10_000;

/** Whether the kind acts on an element: `required`, `optional` or `none`. */
export function elementOf(kind: StepKind): 'required' | 'optional' | 'none' {
  if (NO_ELEMENT.includes(kind)) {
    return 'none';
  }
  return ELEMENT_OPTIONAL.includes(kind) ? 'optional' : 'required';
}

/** The sort of the kind's value; `null` for a kind without one. */
export function valueOf(kind: StepKind): ValueSort | null {
  return VALUES[kind] ?? null;
}

/** The group a kind is in. */
export function groupOf(kind: StepKind): KindGroup {
  return KIND_GROUPS.find((entry) => entry.kinds.includes(kind))?.group ?? 'interaction';
}

/** A fixed wait's ms from the typed value: a whole number from 0 to [`MAX_WAIT_MS`]. */
function milliseconds(value: string): number | null {
  const ms = Number(value);
  return Number.isInteger(ms) && ms >= 0 && ms <= MAX_WAIT_MS ? ms : null;
}

/**
 * The action of `kind` on `target` with `value`; `null` while something it needs is missing (an
 * element, a URL, a key, a time) - the editor says what.
 */
export function actionFor(kind: StepKind, target: Locator | null, value: string): Action | null {
  if (elementOf(kind) === 'required' && target === null) {
    return null;
  }
  const sort = valueOf(kind);
  if ((sort === 'url' || sort === 'key') && value.trim() === '') {
    return null;
  }
  const element = target ?? undefined;
  switch (kind) {
    case 'navigate':
    case 'waitForUrl':
      return { type: kind, url: value.trim() };
    case 'wait': {
      const ms = milliseconds(value);
      return ms === null ? null : { type: 'wait', ms };
    }
    case 'press':
      return element === undefined ? { type: 'press', key: value } : { type: 'press', target: element, key: value };
    case 'scroll':
      return element === undefined ? { type: 'scroll', x: 0, y: 0 } : { type: 'scroll', target: element, x: 0, y: 0 };
    case 'fill':
      return target === null ? null : { type: 'fill', target, value };
    case 'select':
      return target === null ? null : { type: 'select', target, value, indexes: [] };
    case 'assertVisible':
      return target === null ? null : { type: 'assert', assertion: { kind: 'visible', target } };
    case 'assertText':
      return target === null ? null : { type: 'assert', assertion: { kind: 'text', target, text: value } };
    case 'assertValue':
      return target === null ? null : { type: 'assert', assertion: { kind: 'value', target, value } };
    case 'assertUrl':
      return value.trim() === '' ? null : { type: 'assert', assertion: { kind: 'url', url: value.trim() } };
    default:
      return target === null ? null : { type: kind, target };
  }
}

/** The kind of `action`, as the editor offers it. */
export function kindOf(action: Action): StepKind {
  return stepKind(action) as StepKind;
}

/** The value the editor shows for `action`: its URL, text, key, option or ms; empty without. */
export function editorValue(action: Action): string {
  if (action.type === 'wait') {
    return String(action.ms);
  }
  return stepValue(action) ?? '';
}

/**
 * `action` as a step of `kind`: its element and value kept where the new kind has them; `null`
 * while the new kind needs something it does not have.
 */
export function changeKind(action: Action, kind: StepKind): Action | null {
  const target = targetOf(action) ?? null;
  const sort = valueOf(kindOf(action));
  const value = sort !== null && sort === valueOf(kind) ? editorValue(action) : '';
  return actionFor(kind, target, value);
}
