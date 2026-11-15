// Form controls on the capture side (RB-020-05): the state a `change` carries per control type,
// the clicks that are not captured because they open a native picker (a select's popup, the file
// chooser, the colour and date pickers would open in every view), and a contenteditable's text,
// reported as a `change` when it loses the focus - the way an input reports its value.

import type { ChangeData } from '../protocol';
import { actionable } from '../selector';

/** A contenteditable longer than this is not reported (a document editor, not a field). */
export const MAX_EDITABLE_TEXT = 10_000;

/** Input types whose click opens a native picker. */
const PICKER_TYPES = new Set(['file', 'color', 'date', 'datetime-local', 'month', 'week', 'time']);

/** Whether `element` is a password field: its value travels, marked `sensitive`. */
export function isSensitive(element: Element): boolean {
  return element.localName === 'input' && (element as HTMLInputElement).type === 'password';
}

/** The state of a form control after `change`, without its target. */
export function changeState(element: Element): Omit<ChangeData, 'target'> {
  if (element.localName === 'select') {
    const select = element as HTMLSelectElement;
    return {
      value: select.value,
      selected: Array.from(select.selectedOptions, (option) => option.index),
    };
  }
  if (element.localName === 'input') {
    const input = element as HTMLInputElement;
    // A file input's value is a local path: never captured, never synced.
    if (input.type === 'file') return {};
    if (input.type === 'checkbox' || input.type === 'radio') {
      return { value: input.value, checked: input.checked };
    }
    return isSensitive(input) ? { value: input.value, sensitive: true } : { value: input.value };
  }
  if (element.localName === 'textarea') return { value: (element as HTMLTextAreaElement).value };
  return {};
}

/** The control a label stands for: `for`, else the first control inside it. */
function labelled(label: HTMLLabelElement): Element | null {
  if (label.htmlFor) return label.ownerDocument.getElementById(label.htmlFor);
  return label.querySelector('input, select, textarea, button');
}

/** Whether a click on `element` (through its label) opens a native picker. */
export function opensPicker(element: Element): boolean {
  let subject: Element | null = actionable(element);
  if (subject.localName === 'label') subject = labelled(subject as HTMLLabelElement);
  if (!subject) return false;
  if (subject.closest('select')) return true;
  return subject.localName === 'input' && PICKER_TYPES.has((subject as HTMLInputElement).type);
}

/** The editing host `element` belongs to, if any. */
export function editingHost(element: Element): HTMLElement | null {
  const host = element.closest('[contenteditable]');
  if (!(host instanceof HTMLElement)) return null;
  return host.getAttribute('contenteditable') === 'false' ? null : host;
}

/**
 * Tracks the text of the focused contenteditable: `focusin` remembers it, `focusout` answers it
 * when it changed (and is not too long), else null.
 */
export function editableTracker(): {
  focusin(element: Element): void;
  focusout(element: Element): { host: HTMLElement; value: string } | null;
} {
  let editing: { host: HTMLElement; text: string } | null = null;
  return {
    focusin(element) {
      const host = editingHost(element);
      editing = host ? { host, text: host.textContent ?? '' } : null;
    },
    focusout(element) {
      const host = editingHost(element);
      const before = editing;
      editing = null;
      if (!host || !before || before.host !== host) return null;
      const value = host.textContent ?? '';
      if (value === before.text || value.length > MAX_EDITABLE_TEXT) return null;
      return { host, value };
    },
  };
}
