// The element path a finding's stable id is made of (docs/plugins.md, "Check plugins"), one for
// every TypeScript bundle (RB-094-10): the lower-case tags from `html` down to the element, joined
// by " > ", each with `:nth-of-type(n)` (1-based) only when its parent holds more than one child
// of that tag; no ids, no classes, so the same element has the same path in every view and in
// every plugin that reports it. A shadow root is crossed with " >>> " after its host's path.
// tests/fixtures/element-path/cases.json holds this, the Rust copy (plugins/schema-org/src/path.rs)
// and the page copy of plugins/seo/src/collect.js to the same cases.

function step(node: Element, siblings: ArrayLike<Element>): string {
  const tag = node.localName.toLowerCase();
  const same = Array.from(siblings).filter((other) => other.localName === node.localName);
  return same.length > 1 ? `${tag}:nth-of-type(${same.indexOf(node) + 1})` : tag;
}

/** The path of `element` from the root of its document, through its shadow hosts. */
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
