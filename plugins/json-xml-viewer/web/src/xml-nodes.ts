// The tree's nodes over an XML document's DOM: elements, their attributes, text, CDATA,
// comments and processing instructions. Whitespace-only text between elements is left out. An
// element whose only content is one short text shows it inline (`<loc>https://…</loc>`) and has
// no rows of its own; the XPath queries still see the text node.

import { isPluginHost } from './dom';
import { type Kind, PREVIEW_CHARS, type TreeNode } from './nodes';

/** The longest text an element shows inline instead of as a child row. */
export const INLINE_CHARS = 200;

function kindOfDom(node: Node): Kind {
  switch (node.nodeType) {
    case Node.ELEMENT_NODE:
      return 'element';
    case Node.ATTRIBUTE_NODE:
      return 'attribute';
    case Node.CDATA_SECTION_NODE:
      return 'cdata';
    case Node.COMMENT_NODE:
      return 'comment';
    case Node.PROCESSING_INSTRUCTION_NODE:
      return 'instruction';
    default:
      return 'text';
  }
}

/** Whether `node` is shown at all: not whitespace-only text, a doctype or a plugin's host. */
export function significant(node: Node): boolean {
  if (node.nodeType === Node.TEXT_NODE) return (node.nodeValue ?? '').trim() !== '';
  return (
    (node.nodeType === Node.ELEMENT_NODE && !isPluginHost(node)) ||
    node.nodeType === Node.CDATA_SECTION_NODE ||
    node.nodeType === Node.COMMENT_NODE ||
    node.nodeType === Node.PROCESSING_INSTRUCTION_NODE
  );
}

function cut(text: string): string {
  return text.length > PREVIEW_CHARS ? `${text.slice(0, PREVIEW_CHARS)}…` : text;
}

/** The node test of a path step for a node kind other than element and attribute. */
const TEST: Partial<Record<Kind, string>> = {
  text: 'text()',
  cdata: 'text()',
  comment: 'comment()',
  instruction: 'processing-instruction()',
};

export class XmlNode implements TreeNode {
  readonly kind: Kind;
  readonly depth: number;
  private madeAll: XmlNode[] | null = null;
  private madeAttrs: XmlNode[] | null = null;

  constructor(
    readonly dom: Node,
    readonly parent: XmlNode | null = null,
    /** The top-level nodes, for the path of a node without a parent. */
    private readonly siblingsAtTop: readonly Node[] = [],
  ) {
    this.kind = kindOfDom(dom);
    this.depth = parent ? parent.depth + (this.kind === 'attribute' ? 0 : 1) : 0;
  }

  /** Every top-level node of a document as a root of the tree. */
  static roots(nodes: readonly Node[]): XmlNode[] {
    const kept = nodes.filter(significant);
    return kept.map((node) => new XmlNode(node, null, kept));
  }

  get name(): string | null {
    if (this.kind === 'attribute') return (this.dom as Attr).name;
    if (this.kind === 'element') return this.dom.nodeName;
    if (this.kind === 'instruction') return (this.dom as ProcessingInstruction).target || null;
    return null;
  }

  /** The local name (no prefix) of an element or attribute; '' otherwise. */
  get localName(): string {
    const name = this.name ?? '';
    return name.slice(name.indexOf(':') + 1);
  }

  /** Every significant child, text included. */
  all(): readonly XmlNode[] {
    if (!this.madeAll) {
      this.madeAll =
        this.kind === 'element'
          ? [...this.dom.childNodes].filter(significant).map((node) => new XmlNode(node, this))
          : [];
    }
    return this.madeAll;
  }

  /** The element's attributes as nodes (for XPath and search); empty otherwise. */
  attrs(): readonly XmlNode[] {
    if (!this.madeAttrs) {
      this.madeAttrs =
        this.kind === 'element'
          ? [...(this.dom as Element).attributes].map((attr) => new XmlNode(attr, this))
          : [];
    }
    return this.madeAttrs;
  }

  /** Whether the element shows its one short text inline. */
  inline(): boolean {
    if (this.kind !== 'element') return false;
    const kids = this.all();
    const only = kids[0];
    return (
      kids.length === 1 &&
      only !== undefined &&
      (only.kind === 'text' || only.kind === 'cdata') &&
      (only.dom.nodeValue ?? '').length <= INLINE_CHARS &&
      !(only.dom.nodeValue ?? '').includes('\n')
    );
  }

  hasChildren(): boolean {
    return this.childCount() > 0;
  }

  childCount(): number {
    return this.inline() ? 0 : this.all().length;
  }

  children(): readonly XmlNode[] {
    return this.inline() ? [] : this.all();
  }

  value(): string {
    if (this.kind === 'element') return this.dom.textContent ?? '';
    if (this.kind === 'attribute') return (this.dom as Attr).value;
    return this.dom.nodeValue ?? '';
  }

  preview(): string {
    if (this.kind === 'element') return this.inline() ? cut(this.dom.textContent ?? '') : '';
    return cut(this.value().trim());
  }

  /** The node's step: `name`, `name[2]`, `@name`, `text()`, `comment()[2]`. */
  private step(): string {
    if (this.kind === 'attribute') return `@${(this.dom as Attr).name}`;
    const test = this.kind === 'element' ? this.dom.nodeName : (TEST[this.kind] ?? 'node()');
    const peers = this.parent ? this.parent.all().map((node) => node.dom) : this.siblingsAtTop;
    const same = peers.filter((node) =>
      this.kind === 'element'
        ? node.nodeType === Node.ELEMENT_NODE && node.nodeName === this.dom.nodeName
        : TEST[kindOfDom(node)] === test,
    );
    return same.length > 1 ? `${test}[${same.indexOf(this.dom) + 1}]` : test;
  }

  path(): string {
    const steps: string[] = [];
    for (let node: XmlNode | null = this; node; node = node.parent) steps.push(node.step());
    return `/${steps.reverse().join('/')}`;
  }

  searchText(): string {
    if (this.kind === 'element') {
      const attrs = this.attrs().map((attr) => `${attr.name ?? ''}\u0000${attr.value()}`);
      const text = this.inline() ? [this.dom.textContent ?? ''] : [];
      return [this.dom.nodeName, ...attrs, ...text].join('\u0000');
    }
    return `${this.name ?? ''}\u0000${this.value()}`;
  }
}
