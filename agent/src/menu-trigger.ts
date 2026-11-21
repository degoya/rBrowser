// The menu trigger of a click (RB-090-76): a link inside a submenu that opens only on hover
// (`li:hover > ul`) can be clicked only after the pointer rests on the menu item. A recording
// that has the click but not that hover waits in a replay for an element that never shows. The
// pointer's hover chain is known here, at the click: every ancestor of the clicked element is
// `:hover`. So at a mouse's pointer-down the capture reports a `hover` on the trigger first.
//
// The heuristic: of the clicked element's ancestors that are a list item (`li`) or carry
// `aria-haspopup`, each in a menu context - inside `nav`, `header`, `[role=navigation]`,
// `[role=menubar]` or `[role=menu]`, or itself carrying or holding an `aria-haspopup` - whose
// clicked descendant lies in a nested list or menu (`ul`, `ol`, `menu`, `[role=menu]`, ...)
// that the item's own link or button is not part of names that link or button; the outermost
// such item wins - the top-level menu entry. A click on a top-level entry, on a sibling in the
// same list, outside every list or in a link tree outside a menu (a footer sitemap, a comment
// thread) has no trigger. One level only: a submenu inside a submenu gets the hover of the
// outermost entry.
//
// RB-090-87: while a recording runs, a hover where the pointer rests is a step when it is on a
// menu's entry (`isMenuEntry`): one inside a submenu of a menu entry (`menuTrigger` names its
// trigger), one inside a `[role=menu]` or `[role=menubar]`, or one whose hover shows a submenu
// (`opensSubmenu`): its menu entry's nested list or menu is shown, it is `aria-expanded`, or what
// its `aria-controls` names is shown. `hasSubmenu` says whether one could still open (a menu
// with a hover delay).

import { actionable } from './selector';

/** The elements that can be a menu entry's own link or button. */
const TRIGGER = [
  'a[href]', 'button', 'summary', '[aria-haspopup]:not([aria-haspopup="false"])',
  '[role="button"]', '[role="link"]', '[role="menuitem"]',
].join(',');

/** The containers of a submenu. */
const LIST = [
  'ul', 'ol', 'menu', '[role="menu"]', '[role="menubar"]', '[role="listbox"]', '[role="group"]',
].join(',');

/** A popup's opener. */
const POPUP = '[aria-haspopup]:not([aria-haspopup="false"])';

/** A menu entry: a list item or an element with a popup. */
const ITEM = `li, ${POPUP}`;

/** Where a list item is a menu entry. */
const MENU = 'nav, header, [role="navigation"], [role="menubar"], [role="menu"]';

/** The link or button whose hover opens the submenu `clicked` lies in; null when there is none. */
export function menuTrigger(clicked: Element): Element | null {
  const subject = actionable(clicked);
  let trigger: Element | null = null;
  for (let item = subject.parentElement; item; item = item.parentElement) {
    if (!item.matches(ITEM) || !inMenu(item)) continue;
    const own = ownTrigger(item, subject);
    if (own) trigger = own;
  }
  return trigger;
}

/** Whether `item` is a menu's entry, not a list item of a link tree. */
function inMenu(item: Element): boolean {
  return item.closest(MENU) !== null || item.matches(POPUP) || item.querySelector(POPUP) !== null;
}

/** Whether `element` is an entry of a menu: in a submenu, in an ARIA menu, or showing one. */
export function isMenuEntry(element: Element): boolean {
  const subject = actionable(element);
  return (
    subject.closest(ARIA_MENU) !== null || menuTrigger(subject) !== null || opensSubmenu(subject)
  );
}

/** Whether the hover on `element` shows a submenu now. */
export function opensSubmenu(element: Element): boolean {
  const subject = actionable(element);
  if (subject.getAttribute('aria-expanded') === 'true') return true;
  const controls = subject.getAttribute('aria-controls');
  const controlled = controls ? subject.ownerDocument.getElementById(controls) : null;
  if (controlled && shown(controlled)) return true;
  return submenusOf(subject).some(shown);
}

/** Whether `element` has a submenu, shown or not: a hover with a delay may still open it. */
export function hasSubmenu(element: Element): boolean {
  const subject = actionable(element);
  return (
    subject.hasAttribute('aria-expanded') ||
    subject.hasAttribute('aria-controls') ||
    submenusOf(subject).length > 0
  );
}

/** The ARIA menus, whose every entry counts. */
const ARIA_MENU = '[role="menu"], [role="menubar"]';

/** The nested lists or menus of the menu entry whose own link or button `subject` is. */
function submenusOf(subject: Element): Element[] {
  const item = subject.matches(ITEM) ? subject : subject.closest(ITEM);
  if (!item || !inMenu(item)) return [];
  return [...item.querySelectorAll(LIST)].filter((list) => !list.contains(subject));
}

/** Whether `element` is shown: no `display: none` above it, not hidden, not transparent. The
 *  style alone, so a submenu that is still unfolding counts. */
function shown(element: Element): boolean {
  const view = element.ownerDocument.defaultView;
  if (!view || !element.isConnected) return false;
  for (let node: Element | null = element; node; node = node.parentElement) {
    const style = view.getComputedStyle(node);
    if (style.display === 'none' || style.opacity === '0') return false;
  }
  const visibility = view.getComputedStyle(element).visibility;
  return visibility !== 'hidden' && visibility !== 'collapse';
}

/** `item`'s own link or button outside the submenu that holds `subject`. */
function ownTrigger(item: Element, subject: Element): Element | null {
  const submenu = outermostList(item, subject);
  if (!submenu) return null;
  for (const candidate of item.querySelectorAll(TRIGGER)) {
    if (!submenu.contains(candidate) && !candidate.contains(subject)) return candidate;
  }
  return null;
}

/** The outermost list or menu between `item` (excluded) and `subject`. */
function outermostList(item: Element, subject: Element): Element | null {
  let found: Element | null = null;
  for (let node = subject.parentElement; node && node !== item; node = node.parentElement) {
    if (node.matches(LIST)) found = node;
  }
  return found;
}
