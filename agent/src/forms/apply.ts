// Applying the leader's form state in a follower view (RB-020-05): the core's `apply` and
// `focus` commands. The agent sets the state in its isolated world and dispatches the events
// frameworks listen to, instead of CDP input: a select's native popup, a date picker or a file
// chooser cannot be driven by input events at all, and frameworks do not check `isTrusted`.
//
// - Values go through the prototype's setter, past an own property a framework put on the
//   element (React's value tracker), then `input` and `change` bubble - so React's onChange
//   (which compares with its tracker) and Vue's v-model (`input`/`change`) both see them.
// - A checkbox or radio whose state differs is clicked (`click`, `input`, `change`, like a user's
//   click: React reads checkboxes from `click`). A state already reached - by the replayed click
//   or label click before it - changes nothing, so a control toggles exactly once.
// - A file input is never touched; a contenteditable only when it holds plain text.
// - Nothing in an answer quotes a value: it may be a password.

import type { ChangeData, CommandResult, ElementTarget } from '../protocol';
import { locate } from '../resolve';
import { editingHost } from './capture';

const DONE: CommandResult = { status: 'done' };

/** The setter of `property` on `element`'s prototype chain, past an own property. */
function protoSetter(element: Element, property: string): ((value: unknown) => void) | null {
  for (let proto = Object.getPrototypeOf(element); proto; proto = Object.getPrototypeOf(proto)) {
    const setter = Object.getOwnPropertyDescriptor(proto, property)?.set;
    if (setter) return (value) => setter.call(element, value);
  }
  return null;
}

/** Dispatches `input` (composed, like the engine's) and `change`, both bubbling. */
function fire(element: Element, change = true): void {
  const view = element.ownerDocument.defaultView;
  const EventClass = view?.Event ?? Event;
  element.dispatchEvent(new EventClass('input', { bubbles: true, composed: true }));
  if (change) element.dispatchEvent(new EventClass('change', { bubbles: true }));
}

function setValue(element: HTMLInputElement | HTMLTextAreaElement, value: string | undefined): CommandResult {
  if (value === undefined || element.value === value) return DONE;
  const setter = protoSetter(element, 'value');
  if (setter) setter(value);
  else element.value = value;
  fire(element);
  return DONE;
}

function setChecked(input: HTMLInputElement, checked: boolean | undefined): CommandResult {
  if (checked === undefined || input.checked === checked) return DONE;
  input.click();
  if (input.checked !== checked) {
    // A radio cannot be clicked off; a page may cancel the click: set it as a script would.
    protoSetter(input, 'checked')?.(checked);
    fire(input);
  }
  return DONE;
}

function setSelect(select: HTMLSelectElement, data: ChangeData): CommandResult {
  const options = Array.from(select.options);
  let wanted: boolean[];
  if (data.selected && data.selected.every((index) => index < options.length)) {
    const selected = select.multiple ? data.selected : data.selected.slice(0, 1);
    wanted = options.map((_, index) => selected.includes(index));
  } else if (data.value !== undefined && options.some((option) => option.value === data.value)) {
    wanted = options.map((option) => option.value === data.value);
  } else {
    return { status: 'not_found' };
  }
  if (options.every((option, index) => option.selected === wanted[index])) return DONE;
  if (select.multiple) {
    options.forEach((option, index) => {
      option.selected = wanted[index] ?? false;
    });
  } else {
    select.selectedIndex = wanted.indexOf(true);
  }
  fire(select);
  return DONE;
}

function setEditable(host: HTMLElement, value: string | undefined): CommandResult {
  if (value === undefined || host.textContent === value) return DONE;
  // Markup is an editor's own model: typed text reached it already (RB-020-03).
  if (Array.from(host.children).some((child) => child.localName !== 'br')) return DONE;
  host.textContent = value;
  fire(host, false);
  return DONE;
}

/** Sets the control `data.target` names to `data`'s state. */
export function applyChange(data: ChangeData, doc: Document): CommandResult {
  const element = locate(data.target, doc, false);
  if (typeof element === 'string') return { status: element };
  if ((element as HTMLInputElement).disabled) return DONE;
  switch (element.localName) {
    case 'select':
      return setSelect(element as HTMLSelectElement, data);
    case 'textarea':
      return setValue(element as HTMLTextAreaElement, data.value);
    case 'input': {
      const input = element as HTMLInputElement;
      if (input.type === 'file') return DONE;
      if (input.type === 'checkbox' || input.type === 'radio') return setChecked(input, data.checked);
      return setValue(input, data.value);
    }
    default: {
      const host = editingHost(element);
      return host ? setEditable(host, data.value) : DONE;
    }
  }
}

/** Focuses the element `target` names, without scrolling it into view. */
export function applyFocus(target: ElementTarget, doc: Document): CommandResult {
  const element = locate(target, doc);
  if (typeof element === 'string') return { status: element };
  (element as HTMLElement).focus?.({ preventScroll: true });
  return DONE;
}

/** Whether `value` is a well-formed `apply` state (types only; the values are never shown). */
export function isChangeState(value: Record<string, unknown>): boolean {
  const { value: text, checked, selected } = value;
  return (
    (text === undefined || typeof text === 'string') &&
    (checked === undefined || typeof checked === 'boolean') &&
    (selected === undefined ||
      (Array.isArray(selected) &&
        selected.every((index) => Number.isInteger(index) && (index as number) >= 0)))
  );
}
