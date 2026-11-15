// Elements for the viewer's shadow root. Always in the XHTML namespace: in an XML document
// `createElement` makes elements of no namespace, which render as nothing.

export const XHTML = 'http://www.w3.org/1999/xhtml';

/** The host element's tag: a custom-element name, so it may carry a shadow root in XHTML. */
export const HOST_TAG = 'rbrowser-json-xml-viewer';

/**
 * Whether `node` is an element rBrowser added: a plugin's host (`rbrowser-*` in XHTML). In an
 * XML document without the engine's tree viewer the hosts are children of the XML root element,
 * so the tree, the queries and the pretty text leave them out.
 */
export function isPluginHost(node: Node): boolean {
  return (
    node.nodeType === Node.ELEMENT_NODE &&
    (node as Element).namespaceURI === XHTML &&
    (node as Element).localName.startsWith('rbrowser-')
  );
}

export function el<K extends keyof HTMLElementTagNameMap>(
  doc: Document,
  tag: K,
  className = '',
): HTMLElementTagNameMap[K] {
  const element = doc.createElementNS(XHTML, tag) as HTMLElementTagNameMap[K];
  if (className) element.className = className;
  return element;
}
