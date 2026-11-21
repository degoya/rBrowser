// The presenter's click on a viewer's page (RB-096-05, `portal.presentation.pointer`): a ring
// that grows and fades at the middle of the element the presenter clicked, then goes. It takes
// no pointer events and is hidden from assistive technology; its styles are inline, so the
// page's own styles cannot hide it and it adds no sheet to the page.

/** The attribute that marks the ring (and the tests find it by). */
export const FLASH_ATTRIBUTE = 'data-rbrowser-flash';
/** How long the ring stays, in ms. */
export const FLASH_MS = 700;
/** The ring's size in CSS pixels. */
const SIZE = 36;

/** Flashes the ring at `element` in `doc`; `false` when the page has nowhere to draw it. */
export function flashAt(element: Element, doc: Document): boolean {
  const parent = doc.body ?? doc.documentElement;
  if (parent === null) return false;
  const box = element.getBoundingClientRect();
  const ring = doc.createElement('div');
  ring.setAttribute(FLASH_ATTRIBUTE, '');
  ring.setAttribute('aria-hidden', 'true');
  const style = ring.style;
  style.position = 'fixed';
  style.left = `${box.left + box.width / 2}px`;
  style.top = `${box.top + box.height / 2}px`;
  style.width = `${SIZE}px`;
  style.height = `${SIZE}px`;
  style.margin = `${-SIZE / 2}px 0 0 ${-SIZE / 2}px`;
  style.borderRadius = '50%';
  style.border = '3px solid #f59e0b';
  style.boxShadow = '0 0 0 4px rgba(245, 158, 11, 0.35)';
  style.pointerEvents = 'none';
  style.zIndex = '2147483647';
  style.transition = `transform ${FLASH_MS}ms ease-out, opacity ${FLASH_MS}ms ease-out`;
  parent.append(ring);
  const view = doc.defaultView;
  view?.requestAnimationFrame?.(() => {
    style.transform = 'scale(1.6)';
    style.opacity = '0';
  });
  (view ?? globalThis).setTimeout(() => ring.remove(), FLASH_MS);
  return true;
}
