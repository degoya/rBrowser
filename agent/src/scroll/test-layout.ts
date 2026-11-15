// Test helper for the scroll tests (RB-020-04): happy-dom lays nothing out, so this gives the
// document and chosen elements boxes, scroll positions and sizes, and answers
// `getBoundingClientRect`, `elementsFromPoint` and `scrollTo` from them. A box is in the content
// coordinates of the nearest scroll container registered above it (the document by default);
// `fixed` boxes are in viewport coordinates. Every programmatic scroll fires a `scroll` event,
// as the browser does, and is counted.

import { vi } from 'vitest';
import { el } from '../test-page';

/** A box: left, top, width, height in CSS px. */
export interface Box {
  left: number;
  top: number;
  width: number;
  height: number;
  /** In viewport coordinates, never scrolled (`position: fixed`). */
  fixed?: boolean;
}

interface Scroller {
  x: number;
  y: number;
  /** Content size. */
  width: number;
  height: number;
}

/** A laid-out page; `install()` before use, `restore()` after. */
export class Layout {
  readonly boxes = new Map<Element, Box>();
  readonly scrollers = new Map<Element, Scroller>();
  readonly page: Scroller = { x: 0, y: 0, width: 0, height: 0 };
  viewport = { width: 1440, height: 800 };
  /** How many `scroll` events programmatic scrolls fired. */
  scrollEvents = 0;
  private readonly restorers: (() => void)[] = [];

  /** Gives `element` the box `box`. */
  box(element: Element, box: Box): this {
    this.boxes.set(element, box);
    return this;
  }

  /** Makes `element` a scroll container with content of `width` x `height`. */
  scroller(element: Element, width: number, height: number): this {
    this.scrollers.set(element, { x: 0, y: 0, width, height });
    return this;
  }

  /** The scroll container `element` scrolls in: a registered element, else null (document). */
  containerOf(element: Element): Element | null {
    for (let node = element.parentElement; node; node = node.parentElement) {
      if (this.scrollers.has(node)) return node;
    }
    return null;
  }

  /** `element`'s box in viewport coordinates. */
  rect(element: Element): DOMRect {
    const box = this.boxes.get(element) ?? { left: 0, top: 0, width: 0, height: 0 };
    let left = box.left;
    let top = box.top;
    if (!box.fixed) {
      const container = this.containerOf(element);
      if (container) {
        const outer = this.rect(container);
        const state = this.scrollers.get(container) as Scroller;
        left += outer.left - state.x;
        top += outer.top - state.y;
      } else {
        left -= this.page.x;
        top -= this.page.y;
      }
    }
    return {
      left,
      top,
      x: left,
      y: top,
      width: box.width,
      height: box.height,
      right: left + box.width,
      bottom: top + box.height,
      toJSON: () => box,
    } as DOMRect;
  }

  /** The elements with a box at (`x`, `y`): the fixed ones (painted on top), then the deepest
   *  first. */
  hits(x: number, y: number): Element[] {
    const depth = (element: Element) => {
      let count = 0;
      for (let node = element.parentElement; node; node = node.parentElement) count += 1;
      return count;
    };
    return [...this.boxes.keys()]
      .filter((element) => {
        const rect = this.rect(element);
        return x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom;
      })
      .sort((a, b) => {
        const fixed = Number(Boolean(this.boxes.get(b)?.fixed)) - Number(Boolean(this.boxes.get(a)?.fixed));
        return fixed || depth(b) - depth(a);
      });
  }

  private define(target: object, name: string, descriptor: PropertyDescriptor): void {
    const own = Object.getOwnPropertyDescriptor(target, name);
    const writable = 'value' in descriptor ? { writable: true } : {};
    Object.defineProperty(target, name, { configurable: true, ...writable, ...descriptor });
    this.restorers.push(() => {
      if (own) Object.defineProperty(target, name, own);
      else delete (target as Record<string, unknown>)[name];
    });
  }

  private scrolled(target: EventTarget): void {
    this.scrollEvents += 1;
    target.dispatchEvent(new Event('scroll', { bubbles: target === document }));
  }

  /** Answers the layout APIs from this layout. */
  install(): this {
    const layout = this as Layout;
    vi.spyOn(Element.prototype, 'getBoundingClientRect').mockImplementation(function (
      this: Element,
    ) {
      return layout.rect(this);
    });
    const page = this.page;
    const clamp = (value: number, max: number) => Math.min(Math.max(0, value), Math.max(0, max));
    this.define(window, 'innerWidth', { get: () => layout.viewport.width });
    this.define(window, 'innerHeight', { get: () => layout.viewport.height });
    this.define(window, 'scrollX', { get: () => page.x });
    this.define(window, 'scrollY', { get: () => page.y });
    this.define(window, 'scrollTo', {
      value: (options: ScrollToOptions) => {
        page.x = clamp(options.left ?? page.x, page.width - layout.viewport.width);
        page.y = clamp(options.top ?? page.y, page.height - layout.viewport.height);
        layout.scrolled(document);
      },
    });
    const root = document.documentElement;
    this.define(root, 'scrollWidth', { get: () => Math.max(page.width, layout.viewport.width) });
    this.define(root, 'scrollHeight', { get: () => Math.max(page.height, layout.viewport.height) });
    this.define(root, 'clientWidth', { get: () => layout.viewport.width });
    this.define(root, 'clientHeight', { get: () => layout.viewport.height });
    this.define(document, 'elementsFromPoint', {
      value: (x: number, y: number) => layout.hits(x, y),
    });
    for (const [element, state] of this.scrollers) {
      const box = () => this.boxes.get(element) ?? { left: 0, top: 0, width: 0, height: 0 };
      this.define(element, 'scrollLeft', { get: () => state.x });
      this.define(element, 'scrollTop', { get: () => state.y });
      this.define(element, 'scrollWidth', { get: () => Math.max(state.width, box().width) });
      this.define(element, 'scrollHeight', { get: () => Math.max(state.height, box().height) });
      this.define(element, 'clientWidth', { get: () => box().width });
      this.define(element, 'clientHeight', { get: () => box().height });
      this.define(element, 'clientLeft', { get: () => 0 });
      this.define(element, 'clientTop', { get: () => 0 });
      this.define(element, 'scrollTo', {
        value: (options: ScrollToOptions) => {
          state.x = clamp(options.left ?? state.x, state.width - box().width);
          state.y = clamp(options.top ?? state.y, state.height - box().height);
          layout.scrolled(element);
        },
      });
    }
    return this;
  }

  /** Undoes `install()` (the `getBoundingClientRect` spy goes with `vi.restoreAllMocks`). */
  restore(): void {
    for (const undo of this.restorers.reverse()) undo();
    this.restorers.length = 0;
  }
}

/** Boxes stacked from `top` down, each `heights[i]` tall and `width` wide at `left`. */
export function stack(
  layout: Layout,
  elements: Element[],
  heights: number[],
  { left = 0, top = 0, width = 1440 } = {},
): number {
  let y = top;
  elements.forEach((element, index) => {
    const height = heights[index] ?? 0;
    layout.box(element, { left, top: y, width, height });
    y += height;
  });
  return y;
}

function sections(): HTMLElement[] {
  return Array.from(document.querySelectorAll<HTMLElement>('main > section'));
}

/** The fixture `scroll.html` at 1440 x 800: sections of 720 and 480 px from y 72 (section 5
 *  from 2472), a heading of 48 px atop each, the fixed header over the top edge. */
export function desktop(target: Layout): void {
  target.viewport = { width: 1440, height: 800 };
  target.box(el('.topbar'), { left: 0, top: 0, width: 1440, height: 56, fixed: true });
  const all = sections();
  const heights = all.map((_, index) => (index % 2 === 0 ? 720 : 480));
  const bottom = stack(target, all, heights, { left: 288, top: 72, width: 1128 });
  headings(target, all, 48);
  target.page.width = 1440;
  target.page.height = bottom + 24;
}

/** The fixture at 390 x 844: the sidebar above, sections of 1600 and 1200 px from y 496
 *  (section 5 from 6096), headings of 80 px. */
export function mobile(target: Layout): void {
  target.viewport = { width: 390, height: 844 };
  target.box(el('.topbar'), { left: 0, top: 0, width: 390, height: 56, fixed: true });
  const all = sections();
  const heights = all.map((_, index) => (index % 2 === 0 ? 1600 : 1200));
  const bottom = stack(target, all, heights, { left: 24, top: 496, width: 342 });
  headings(target, all, 80);
  target.page.width = 390;
  target.page.height = bottom + 24;
}

function headings(target: Layout, all: HTMLElement[], height: number): void {
  for (const section of all) {
    const box = target.boxes.get(section);
    const heading = section.querySelector('h2');
    if (box && heading) target.box(heading, { ...box, height });
  }
}
