// The tree's rows on screen: virtual rendering. Only the rows in the scroller's viewport (and a
// few around it) exist as elements, taken from a pool and placed absolutely; a spacer gives the
// scroller its full height. Past `MAX_SCROLL_PX` the scroll position is scaled, so a tree of
// millions of rows stays within the engine's largest element. An ARIA `tree` with
// `aria-activedescendant`: arrows, Home/End and Page keys move, Right/Left open and close,
// Enter and Space toggle. Texts go in as text, never as markup.

import { el } from './dom';
import { t } from './labels';
import { JsonNode, type TreeNode } from './nodes';
import { ROW_HEIGHT } from './styles';
import type { Tree } from './tree';
import type { XmlNode } from './xml-nodes';

const OVERSCAN = 8;
/** The tallest the spacer gets; Chromium caps an element's height near 2^25 px. */
export const MAX_SCROLL_PX = 15_000_000;
/** The height assumed before the scroller is laid out (tests, a hidden view). */
const FALLBACK_HEIGHT = 600;
const MAX_ATTRS = 12;
const ATTR_CHARS = 120;

function span(doc: Document, cls: string, text: string): HTMLElement {
  const node = el(doc, 'span', cls);
  node.textContent = text;
  return node;
}

function count(node: TreeNode): string {
  const n = node.childCount();
  const [one, many] =
    node.kind === 'array' ? ['item', 'items'] : node.kind === 'object' ? ['key', 'keys'] : ['child', 'children'];
  return n === 1 ? t(`viewer.${one}`) : t(`viewer.${many}`, { count: n });
}

const SCALAR_CLASS: Record<string, string> = { string: 's', number: 'n', boolean: 'l', null: 'l' };

function cut(text: string, max: number): string {
  return text.length > max ? `${text.slice(0, max)}…` : text;
}

export class TreeView {
  readonly element: HTMLElement;
  private readonly sizer: HTMLElement;
  private readonly pool: HTMLElement[] = [];
  private readonly positions = new WeakMap<TreeNode, number>();
  tree: Tree | null = null;
  active = 0;
  /** Called when the active row changes. */
  onActive: (node: TreeNode | null) => void = () => undefined;

  constructor(
    private readonly doc: Document,
    label: string,
  ) {
    this.element = el(doc, 'div', 'scroller');
    this.element.setAttribute('role', 'tree');
    this.element.setAttribute('aria-label', label);
    this.element.tabIndex = 0;
    this.sizer = el(doc, 'div', 'sizer');
    this.element.append(this.sizer);
    this.element.addEventListener('scroll', () => this.render(), { passive: true });
    this.element.addEventListener('keydown', (event) => this.onKey(event));
    this.element.addEventListener('click', (event) => this.onClick(event));
    this.element.addEventListener('dblclick', (event) => this.onClick(event, true));
  }

  setTree(tree: Tree): void {
    this.tree = tree;
    this.active = 0;
    this.element.scrollTop = 0;
    this.render();
    this.onActive(this.activeNode());
  }

  activeNode(): TreeNode | null {
    return this.tree?.nodes[this.active] ?? null;
  }

  /** The row elements now in the document, for tests. */
  visibleRows(): HTMLElement[] {
    return this.pool.filter((row) => !row.hidden);
  }

  private height(): number {
    return this.element.clientHeight || FALLBACK_HEIGHT;
  }

  /** Content pixels per scroll pixel: 1 until the tree is taller than `MAX_SCROLL_PX`. */
  private scale(): number {
    const total = (this.tree?.length ?? 0) * ROW_HEIGHT;
    const height = this.height();
    return total > MAX_SCROLL_PX ? (total - height) / (MAX_SCROLL_PX - height) : 1;
  }

  render(): void {
    const tree = this.tree;
    const length = tree?.length ?? 0;
    this.sizer.style.setProperty('height', `${Math.min(length * ROW_HEIGHT, MAX_SCROLL_PX)}px`);
    const height = this.height();
    const scrollTop = this.element.scrollTop;
    const top = scrollTop * this.scale();
    const first = Math.max(0, Math.floor(top / ROW_HEIGHT) - OVERSCAN);
    const last = Math.min(length, Math.ceil((top + height) / ROW_HEIGHT) + OVERSCAN);
    const active = this.activeNode() ? `jxv-row-${this.active}` : '';
    if (active) this.element.setAttribute('aria-activedescendant', active);
    else this.element.removeAttribute('aria-activedescendant');
    let used = 0;
    for (let index = first; index < last && tree; index += 1) {
      let row = this.pool[used];
      if (!row) {
        row = el(this.doc, 'div', 'row');
        row.setAttribute('role', 'treeitem');
        this.pool.push(row);
        this.element.append(row);
      }
      row.hidden = false;
      row.style.setProperty('top', `${index * ROW_HEIGHT - top + scrollTop}px`);
      this.fill(row, tree, index);
      used += 1;
    }
    for (let i = used; i < this.pool.length; i += 1) (this.pool[i] as HTMLElement).hidden = true;
  }

  /** The node's place among its siblings, from 1, and their number. */
  private position(tree: Tree, node: TreeNode, level: number): [number, number] {
    const siblings = level === 0 ? tree.roots : (node.parent?.children() ?? [node]);
    if (level === 0) return [siblings.indexOf(node) + 1, siblings.length];
    let at = this.positions.get(node);
    if (at === undefined) {
      siblings.forEach((sibling, i) => this.positions.set(sibling, i + 1));
      at = this.positions.get(node) ?? 1;
    }
    return [at, siblings.length];
  }

  private fill(row: HTMLElement, tree: Tree, index: number): void {
    const node = tree.nodes[index] as TreeNode;
    const level = tree.levels[index] as number;
    const [posinset, setsize] = this.position(tree, node, level);
    row.id = `jxv-row-${index}`;
    row.dataset.index = String(index);
    row.style.setProperty('--level', String(level));
    row.setAttribute('aria-level', String(level + 1));
    row.setAttribute('aria-posinset', String(posinset));
    row.setAttribute('aria-setsize', String(setsize));
    row.setAttribute('aria-selected', String(index === this.active));
    if (node.hasChildren()) row.setAttribute('aria-expanded', String(tree.isOpen(node)));
    else row.removeAttribute('aria-expanded');
    const open = tree.isOpen(node);
    const parts: Node[] = [span(this.doc, 'twisty', '')];
    if (tree.labelled && level === 0) parts.push(span(this.doc, 'p', node.path()));
    if (node instanceof JsonNode) this.json(parts, node, open, tree.labelled && level === 0);
    else this.xml(parts, node as XmlNode, open);
    row.replaceChildren(...parts);
  }

  private json(parts: Node[], node: TreeNode, open: boolean, labelled: boolean): void {
    const doc = this.doc;
    if (node.name !== null && !labelled) {
      const isIndex = node.parent?.kind === 'array';
      parts.push(span(doc, isIndex ? 'm' : 'k', isIndex ? node.name : JSON.stringify(node.name)));
      parts.push(span(doc, 'm', ': '));
    }
    if (node.kind !== 'object' && node.kind !== 'array') {
      parts.push(span(doc, SCALAR_CLASS[node.kind] ?? '', node.preview()));
      return;
    }
    const [opening, closing] = node.kind === 'array' ? ['[', ']'] : ['{', '}'];
    if (!node.hasChildren()) parts.push(span(doc, 'm', opening + closing));
    else if (open) parts.push(span(doc, 'm', opening));
    else parts.push(span(doc, 'm', `${opening}…${closing}  ${count(node)}`));
  }

  private xml(parts: Node[], node: XmlNode, open: boolean): void {
    const doc = this.doc;
    switch (node.kind) {
      case 'element': {
        parts.push(span(doc, 't', `<${node.name ?? ''}`));
        node.attrs().slice(0, MAX_ATTRS).forEach((attr) => {
          parts.push(span(doc, 'a', ` ${attr.name ?? ''}`));
          parts.push(span(doc, 'm', '='));
          parts.push(span(doc, 's', `"${cut(attr.value(), ATTR_CHARS)}"`));
        });
        if (node.attrs().length > MAX_ATTRS) parts.push(span(doc, 'm', ' …'));
        if (node.all().length === 0) parts.push(span(doc, 't', '/>'));
        else if (node.inline()) {
          parts.push(span(doc, 't', '>'));
          parts.push(doc.createTextNode(node.preview()));
          parts.push(span(doc, 't', `</${node.name ?? ''}>`));
        } else if (open) parts.push(span(doc, 't', '>'));
        else {
          parts.push(span(doc, 't', '>'));
          parts.push(span(doc, 'm', `…  ${count(node)}`));
        }
        return;
      }
      case 'attribute':
        parts.push(span(doc, 'a', `@${node.name ?? ''}`), span(doc, 'm', '='), span(doc, 's', `"${node.preview()}"`));
        return;
      case 'comment':
        parts.push(span(doc, 'c', `<!-- ${node.preview()} -->`));
        return;
      case 'cdata':
        parts.push(span(doc, 'c', '<![CDATA['), doc.createTextNode(node.preview()), span(doc, 'c', ']]>'));
        return;
      case 'instruction':
        parts.push(span(doc, 'c', `<?${node.name ?? ''} ${node.preview()}?>`));
        return;
      default:
        parts.push(doc.createTextNode(node.preview()));
    }
  }

  /** Makes `index` the active row and scrolls it into view. */
  select(index: number): void {
    const tree = this.tree;
    if (!tree || tree.length === 0) return;
    this.active = Math.max(0, Math.min(tree.length - 1, index));
    const scale = this.scale();
    const rowTop = this.active * ROW_HEIGHT;
    const top = this.element.scrollTop * scale;
    const height = this.height();
    if (rowTop < top) this.element.scrollTop = rowTop / scale;
    else if (rowTop + ROW_HEIGHT > top + height) this.element.scrollTop = (rowTop + ROW_HEIGHT - height) / scale;
    this.render();
    this.onActive(this.activeNode());
  }

  toggle(index: number): void {
    this.tree?.toggle(index);
    this.select(index);
  }

  private onKey(event: KeyboardEvent): void {
    const tree = this.tree;
    if (!tree || event.altKey || event.ctrlKey || event.metaKey) return;
    const page = Math.max(1, Math.floor(this.height() / ROW_HEIGHT) - 1);
    const node = this.activeNode();
    const index = this.active;
    switch (event.key) {
      case 'ArrowDown':
        this.select(index + 1);
        break;
      case 'ArrowUp':
        this.select(index - 1);
        break;
      case 'PageDown':
        this.select(index + page);
        break;
      case 'PageUp':
        this.select(index - page);
        break;
      case 'Home':
        this.select(0);
        break;
      case 'End':
        this.select(tree.length - 1);
        break;
      case 'ArrowRight':
        if (node?.hasChildren() && !tree.isOpen(node)) this.toggle(index);
        else if (node?.hasChildren()) this.select(index + 1);
        break;
      case 'ArrowLeft':
        if (node && tree.isOpen(node)) this.toggle(index);
        else if (tree.parentIndex(index) >= 0) this.select(tree.parentIndex(index));
        break;
      case 'Enter':
      case ' ':
        if (node?.hasChildren()) this.toggle(index);
        break;
      default:
        return;
    }
    event.preventDefault();
  }

  private onClick(event: MouseEvent, always = false): void {
    const target = event.target as Element | null;
    const row = target?.closest?.('.row') as HTMLElement | null;
    if (!row?.dataset.index) return;
    const index = Number(row.dataset.index);
    if (always || target?.classList.contains('twisty')) this.toggle(index);
    else this.select(index);
  }
}
