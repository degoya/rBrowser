// The overlay layer: one host element at the end of <html> with a closed shadow root. The host
// is fixed at 0,0 with no size, so it never takes part in the page's layout or its scrollable
// overflow; the boxes inside are placed in viewport coordinates. Every style is set through the
// CSSOM (`style.setProperty`), never as a style attribute or <style> element, so a page's CSP
// (`style-src`) cannot refuse it, and the page's CSS cannot reach into the shadow root.

import { HOST_TAG } from './constants';
import { type Shape, shapeStyle } from './shapes';
import type { Rect } from './types';

/** One box to draw, in viewport coordinates. */
export interface Item {
  rect: Rect;
  /** `#rrggbb`. */
  color: string;
  /** Border width in px (default 2). */
  border?: number;
  /** A translucent fill of the colour, or the colour itself (`solid`). */
  fill?: boolean | 'solid';
  /** A light and a dark ring around the border, so the box shows on light and dark pages. */
  halo?: boolean;
  /** A short text beside the box: numbers and units only, the page has no locale. */
  label?: string;
}

/** The relative luminance of `#rrggbb` (WCAG 2). */
export function luminance(color: string): number {
  const channel = (offset: number): number => {
    const value = parseInt(color.slice(offset, offset + 2), 16) / 255;
    return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel(1) + 0.7152 * channel(3) + 0.0722 * channel(5);
}

/** The contrast ratio of two `#rrggbb` colours (WCAG 2), 1 to 21. */
export function contrast(one: string, two: string): number {
  const [light, dark] = [luminance(one), luminance(two)].sort((a, b) => b - a) as [number, number];
  return (light + 0.05) / (dark + 0.05);
}

/** The text colour (black or white) with the higher contrast on `color` (WCAG 2 luminance). */
export function labelText(color: string): string {
  return contrast(color, '#000000') >= contrast(color, '#ffffff') ? '#000000' : '#ffffff';
}

function set(element: HTMLElement, styles: Record<string, string>): void {
  for (const [name, value] of Object.entries(styles)) element.style.setProperty(name, value, 'important');
}

const HOST_STYLE: Record<string, string> = {
  position: 'fixed',
  top: '0',
  left: '0',
  width: '0',
  height: '0',
  margin: '0',
  padding: '0',
  border: '0',
  overflow: 'visible',
  display: 'block',
  'pointer-events': 'none',
  'z-index': '2147483647',
  transform: 'none',
  contain: 'none',
};

export class Layer {
  readonly host: HTMLElement;
  private readonly container: HTMLElement;
  private readonly groups = new Map<string, HTMLElement>();

  constructor(private readonly doc: Document) {
    this.host = doc.createElement(HOST_TAG);
    this.host.style.setProperty('all', 'initial', 'important');
    set(this.host, HOST_STYLE);
    const root = this.host.attachShadow({ mode: 'closed' });
    this.container = doc.createElement('div');
    set(this.container, { position: 'absolute', top: '0', left: '0', width: '0', height: '0', overflow: 'visible' });
    root.append(this.container);
  }

  /** Puts the host back at the end of <html> when the page removed it. */
  attach(): void {
    if (!this.host.isConnected) this.doc.documentElement.append(this.host);
  }

  /** Draws `items` as the boxes of `group`, reusing the boxes it drew before. */
  render(group: string, items: readonly Item[]): void {
    this.fill(group, items.length, (box, index) => this.drawBox(box, items[index] as Item));
  }

  /** Draws `shapes` (markers and lines) as `group`, reusing the elements it drew before. */
  renderShapes(group: string, shapes: readonly Shape[]): void {
    this.fill(group, shapes.length, (box, index) => {
      const shape = shapes[index] as Shape;
      box.firstElementChild?.remove();
      set(box, shapeStyle(shape));
      box.textContent = shape.kind === 'marker' ? shape.text : '';
    });
  }

  /** The holder of `group` with exactly `count` children, each drawn by `draw`. */
  private fill(group: string, count: number, draw: (box: HTMLElement, index: number) => void): void {
    this.attach();
    let holder = this.groups.get(group);
    if (!holder) {
      holder = this.doc.createElement('div');
      holder.dataset.group = group;
      this.container.append(holder);
      this.groups.set(group, holder);
    }
    while (holder.children.length > count) holder.lastElementChild?.remove();
    for (let index = 0; index < count; index += 1) {
      let box = holder.children[index] as HTMLElement | undefined;
      if (!box) {
        box = this.doc.createElement('div');
        holder.append(box);
      }
      draw(box, index);
    }
  }

  /** Removes the boxes of `group`. */
  clear(group: string): void {
    this.groups.get(group)?.remove();
    this.groups.delete(group);
  }

  /** The number of boxes `group` shows. */
  count(group: string): number {
    return this.groups.get(group)?.children.length ?? 0;
  }

  /** Removes the layer from the page. */
  destroy(): void {
    for (const holder of this.groups.values()) holder.remove();
    this.groups.clear();
    this.host.remove();
  }

  private drawBox(box: HTMLElement, item: Item): void {
    const border = item.border ?? 2;
    set(box, {
      position: 'absolute',
      left: `${item.rect.x}px`,
      top: `${item.rect.y}px`,
      width: `${Math.max(item.rect.width, 0)}px`,
      height: `${Math.max(item.rect.height, 0)}px`,
      'box-sizing': 'border-box',
      border: `${border}px solid ${item.color}`,
      background: item.fill === 'solid' ? item.color : item.fill ? `${item.color}33` : 'transparent',
      'box-shadow': item.halo ? '0 0 0 1px rgba(255,255,255,0.9), inset 0 0 0 1px rgba(0,0,0,0.6)' : 'none',
      margin: '0',
      padding: '0',
    });
    let label = box.firstElementChild as HTMLElement | null;
    if (!item.label) {
      label?.remove();
      return;
    }
    if (!label) {
      label = this.doc.createElement('span');
      box.append(label);
    }
    label.textContent = item.label;
    set(label, {
      position: 'absolute',
      left: `${-border}px`,
      top: item.rect.y >= 18 ? '-18px' : '0',
      font: '600 11px/16px system-ui, sans-serif',
      padding: '0 4px',
      'white-space': 'nowrap',
      background: item.color,
      color: labelText(item.color),
      'border-radius': '2px',
    });
  }
}
