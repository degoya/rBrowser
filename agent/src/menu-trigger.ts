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
