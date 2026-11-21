// The keyboard half of the shell's accessibility tests (RB-090-08): what axe-core does not check
// on a DOM without layout. Every control is in the Tab order, or one stop of a roving group
// (tabs, radios, a menu or toolbar) whose current member is; every focusable element that is no
// native control has a role and a name (docs/design.md, section 7).

/** What a pointer can operate. */
const CONTROLS = [
  'button',
  'a[href]',
  'input:not([type="hidden"])',
  'select',
  'textarea',
  'summary',
  '[role="button"]',
  '[role="link"]',
  '[role="checkbox"]',
  '[role="radio"]',
  '[role="switch"]',
  '[role="tab"]',
  '[role="menuitem"]',
  '[role="option"]',
  '[role="slider"]',
  '[role="combobox"]',
].join(', ');

/** Composite widgets whose members rove: one stop in the Tab order, the arrows inside. */
const ROVING = '[role="tablist"], [role="radiogroup"], [role="listbox"], [role="menu"], [role="menubar"], [role="toolbar"], [role="grid"], [role="tree"]';

/** Native controls, named by their label or content. */
const NATIVE = new Set(['BUTTON', 'A', 'INPUT', 'SELECT', 'TEXTAREA', 'SUMMARY']);

/** Whether `el` is out of reach on purpose: hidden, inert, or disabled. */
function outOfReach(el: Element): boolean {
  if (el.closest('[hidden], [inert], [aria-hidden="true"]') !== null) return true;
  if ((el as HTMLButtonElement).disabled === true) return true;
  return el.closest('[aria-disabled="true"], fieldset[disabled]') !== null;
}

function tabbable(el: Element): boolean {
  return (el as HTMLElement).tabIndex >= 0 && !outOfReach(el);
}

/** A short handle of `el` for a failed assertion. */
function describeElement(el: Element): string {
  const id = el.getAttribute('data-testid') ?? el.getAttribute('aria-label') ?? el.textContent?.trim().slice(0, 40) ?? '';
  return `${el.tagName.toLowerCase()}${el.getAttribute('role') === null ? '' : `[role=${el.getAttribute('role')}]`} "${id}"`;
}

/**
 * Whether a control out of the Tab order is still one stop away: its roving group or a
 * focus-forwarding container (reka's roving root) is a stop, its widget's own input is (a number
 * field's steppers, a menu's trigger - the arrows work there), or a stop drives its list
 * through `aria-controls` (a combobox and its options).
 */
function reachedThroughStop(el: Element): boolean {
  for (let up = el.parentElement; up !== null; up = up.parentElement) {
    if (tabbable(up) && !NATIVE.has(up.tagName)) return true;
    if (up.matches(ROVING) && [...up.querySelectorAll(CONTROLS)].some(tabbable)) return true;
    if (up.matches('[data-slot="root"]') && [...up.querySelectorAll(CONTROLS)].some(tabbable)) return true;
    if (up.id !== '' && [...el.ownerDocument.querySelectorAll(`[aria-controls~="${up.id}"]`)].some(tabbable)) return true;
  }
  return false;
}

/** Controls in `root` that neither Tab nor a stop next to them reaches. */
export function unreachableControls(root: Element = document.body): string[] {
  return [...root.querySelectorAll(CONTROLS)]
    .filter((el) => !outOfReach(el) && !tabbable(el) && !reachedThroughStop(el))
    .map(describeElement);
}

/** Roles whose name may come from their content; every other role needs a label. */
const NAME_FROM_CONTENT = new Set(['button', 'link', 'tab', 'menuitem', 'option', 'checkbox', 'radio', 'switch', 'treeitem', 'row', 'cell', 'gridcell']);

/** The accessible name of a non-native focusable, as far as a DOM without layout tells. */
function nameOf(el: Element): string {
  const label = el.getAttribute('aria-label')?.trim() ?? '';
  if (label !== '') return label;
  const ids = el.getAttribute('aria-labelledby')?.split(/\s+/) ?? [];
  return ids
    .map((id) => el.ownerDocument.getElementById(id)?.textContent?.trim() ?? '')
    .join(' ')
    .trim();
}

/**
 * Focusable elements in `root` that are no native control and lack a role or a name. A roving
 * root, which hands the focus on to one of its controls at once, is never where the reader stays.
 */
export function unnamedFocusables(root: Element = document.body): string[] {
  return [...root.querySelectorAll('[tabindex]')]
    .filter((el) => !NATIVE.has(el.tagName) && tabbable(el) && el.querySelector(CONTROLS) === null)
    .filter((el) => {
      const role = el.getAttribute('role');
      if (role === null) return true;
      if (nameOf(el) !== '') return false;
      return !NAME_FROM_CONTENT.has(role) || (el.textContent?.trim() ?? '') === '';
    })
    .map(describeElement);
}
