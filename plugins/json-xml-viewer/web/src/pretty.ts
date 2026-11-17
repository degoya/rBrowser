// The XML document as text: as parsed (raw) and indented (pretty). The engine keeps no source
// text of an XML document, so both are serialisations of its nodes - the same markup, with the
// engine's choice of quotes and entities. JSON keeps its source text; its pretty form is
// `JSON.stringify(value, null, 2)`.

import { significant } from './xml-nodes';

const INDENT = '  ';

function escapeText(text: string): string {
  return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

function escapeAttr(text: string): string {
  return escapeText(text).replace(/"/g, '&quot;');
}

function openTag(element: Element): string {
  const attrs = [...element.attributes].map((attr) => ` ${attr.name}="${escapeAttr(attr.value)}"`);
  return `<${element.nodeName}${attrs.join('')}`;
}

/** One line of a node that is no element with children. */
function leaf(node: Node): string {
  switch (node.nodeType) {
    case Node.COMMENT_NODE:
      return `<!--${node.nodeValue ?? ''}-->`;
    case Node.CDATA_SECTION_NODE:
      return `<![CDATA[${node.nodeValue ?? ''}]]>`;
    case Node.PROCESSING_INSTRUCTION_NODE:
      return `<?${(node as ProcessingInstruction).target} ${node.nodeValue ?? ''}?>`;
    default:
      return escapeText((node.nodeValue ?? '').trim());
  }
}

/** The nodes indented, two spaces a level; whitespace-only text is left out. */
export function prettyXml(nodes: readonly Node[]): string {
  const lines: string[] = [];
  // Iterative: a closing tag is pushed as a string to print when it comes off the stack.
  const stack: [Node | string, number][] = [];
  for (let i = nodes.length - 1; i >= 0; i -= 1) {
    const node = nodes[i] as Node;
    if (significant(node)) stack.push([node, 0]);
  }
  while (stack.length > 0) {
    const [item, level] = stack.pop() as [Node | string, number];
    const pad = INDENT.repeat(level);
    if (typeof item === 'string') {
      lines.push(pad + item);
      continue;
    }
    if (item.nodeType !== Node.ELEMENT_NODE) {
      lines.push(pad + leaf(item));
      continue;
    }
    const element = item as Element;
    const kids = [...element.childNodes].filter(significant);
    const only = kids[0];
    if (kids.length === 0) {
      lines.push(`${pad}${openTag(element)}/>`);
    } else if (kids.length === 1 && only && only.nodeType !== Node.ELEMENT_NODE && only.nodeType !== Node.COMMENT_NODE) {
      lines.push(`${pad}${openTag(element)}>${leaf(only)}</${element.nodeName}>`);
    } else {
      lines.push(`${pad}${openTag(element)}>`);
      stack.push([`</${element.nodeName}>`, level]);
      for (let i = kids.length - 1; i >= 0; i -= 1) stack.push([kids[i] as Node, level + 1]);
    }
  }
  return lines.join('\n');
}

/** The nodes serialised as the engine does, top-level nodes one per line. */
export function serializeXml(nodes: readonly Node[]): string {
  const serializer = new XMLSerializer();
  return nodes
    .filter(significant)
    .map((node) => serializer.serializeToString(node))
    .join('\n');
}
