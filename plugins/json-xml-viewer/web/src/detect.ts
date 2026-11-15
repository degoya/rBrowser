// Which documents the viewer takes: by `document.contentType` first, then by the shape the engine
// renders them in, so a page that only claims a type is left alone.
//
// - JSON (`application/json`, `text/json`, `*+json`): Chromium shows the text in one <pre>,
//   the first child of <body> (newer versions add a pretty-print switch after it).
// - XML (`application/xml`, `text/xml`, `*+xml` - RSS, Atom, sitemaps; not SVG or XHTML, which
//   are pages): Chromium's tree viewer moves the document's nodes into a hidden
//   `#webkit-xml-viewer-source-xml`; without it the document is the XML itself. A document with
//   an `xml-stylesheet` instruction is the author's rendering, and one with a parser error keeps
//   Chromium's message, which names the line and column.

import { XHTML } from './dom';

/** The hidden element Chromium's XML tree viewer keeps the source nodes in. */
export const XML_SOURCE_ID = 'webkit-xml-viewer-source-xml';

const PAGE_TYPES = new Set(['image/svg+xml', 'application/xhtml+xml', 'application/vnd.mozilla.xul+xml']);

export type Detected = { kind: 'json'; text: string } | { kind: 'xml'; nodes: Node[] };

/** `json`, `xml` or null for a content type (parameters and case ignored). */
export function kindOf(contentType: string): 'json' | 'xml' | null {
  const type = contentType.split(';')[0]?.trim().toLowerCase() ?? '';
  if (type === 'application/json' || type === 'text/json' || /^[a-z]+\/[\w.+-]+\+json$/.test(type)) {
    return 'json';
  }
  if (PAGE_TYPES.has(type)) return null;
  if (type === 'application/xml' || type === 'text/xml' || /^[a-z]+\/[\w.+-]+\+xml$/.test(type)) {
    return 'xml';
  }
  return null;
}

function jsonText(doc: Document): string | null {
  const first = doc.body?.firstElementChild;
  if (!first || first.localName !== 'pre' || first.namespaceURI !== XHTML) return null;
  return first.textContent ?? '';
}

function hasStyleSheet(nodes: Iterable<Node>): boolean {
  for (const node of nodes) {
    if (
      node.nodeType === Node.PROCESSING_INSTRUCTION_NODE &&
      (node as ProcessingInstruction).target === 'xml-stylesheet'
    ) {
      return true;
    }
  }
  return false;
}

function xmlNodes(doc: Document): Node[] | null {
  // A type selector without a namespace matches the element in any namespace.
  if (doc.querySelector('parsererror')) return null;
  const source = doc.getElementById(XML_SOURCE_ID);
  let nodes: Node[];
  if (source) {
    nodes = [...source.childNodes];
  } else {
    const root = doc.documentElement;
    if (!root || root.namespaceURI === XHTML) return null;
    nodes = [...doc.childNodes].filter((node) => node.nodeType !== Node.DOCUMENT_TYPE_NODE);
  }
  if (hasStyleSheet(nodes) || hasStyleSheet(doc.childNodes)) return null;
  return nodes.some((node) => node.nodeType === Node.ELEMENT_NODE) ? nodes : null;
}

/** The document's source for the viewer, or null when the viewer leaves it alone. */
export function detect(doc: Document, contentType: string = doc.contentType): Detected | null {
  switch (kindOf(contentType)) {
    case 'json': {
      const text = jsonText(doc);
      return text === null ? null : { kind: 'json', text };
    }
    case 'xml': {
      const nodes = xmlNodes(doc);
      return nodes === null ? null : { kind: 'xml', nodes };
    }
    default:
      return null;
  }
}
