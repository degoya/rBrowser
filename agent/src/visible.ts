// Whether an element is shown (RB-020-01): the resolution in a target view prefers a visible
// match, and the selector of a recorded element counts its visible matches (RB-090-81).

/** The parent of `node` across a shadow boundary. */
function composedParent(node: Element): Element | null {
  if (node.parentElement) return node.parentElement;
  const root = node.getRootNode();
  return root instanceof ShadowRoot ? root.host : null;
}

/** Whether `element` is rendered and has a box: no `display:none` above it, not hidden. */
export function isVisible(element: Element): boolean {
  if (!element.isConnected) return false;
  const view = element.ownerDocument.defaultView;
  if (!view) return false;
  for (let node: Element | null = element; node; node = composedParent(node)) {
    if (view.getComputedStyle(node).display === 'none') return false;
  }
  const visibility = view.getComputedStyle(element).visibility;
  if (visibility === 'hidden' || visibility === 'collapse') return false;
  const rect = element.getBoundingClientRect();
  return rect.width > 0 && rect.height > 0;
}
