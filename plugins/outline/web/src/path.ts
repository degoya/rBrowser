// The element path the findings' stable id is made of (docs/plugins.md, "Check plugins"),
// copied from plugins/a11y-axe/runner/src/path.ts so the same element has the same path in
// outline, a11y-axe and seo and their findings with a shared id collapse into one. Keep the
// three in step.

function step(node: Element, siblings: ArrayLike<Element>): string {
  const tag = node.localName.toLowerCase();
  const same = Array.from(siblings).filter((other) => other.localName === node.localName);
  return same.length > 1 ? `${tag}:nth-of-type(${same.indexOf(node) + 1})` : tag;
}

/** The path of `element` from the root of its document. */
export function elementPath(element: Element): string {
  const steps: string[] = [];
  let node: Element = element;
  for (;;) {
    const parent = node.parentNode;
    if (parent && parent.nodeType === Node.ELEMENT_NODE) {
      steps.unshift(step(node, (parent as Element).children));
      node = parent as Element;
      continue;
    }
    if (parent && parent.nodeType === Node.DOCUMENT_FRAGMENT_NODE && 'host' in parent) {
      steps.unshift(step(node, (parent as ShadowRoot).children));
      return `${elementPath((parent as ShadowRoot).host)} >>> ${steps.join(' > ')}`;
    }
    steps.unshift(node.localName.toLowerCase());
    return steps.join(' > ');
  }
}
