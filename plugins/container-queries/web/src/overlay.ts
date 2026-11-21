// The container overlay of one view: every size container framed, with its name, its size and
// the @container conditions that match now, and a handle on its right edge that drags its width
// (src/resize.ts); a double click on the handle resets it. Drawn in a closed shadow root of one
// layer of the page's `<rbrowser-layers>` (docs/plugins.md, "Overlays"), fixed over the viewport,
// every style set through the CSSOM so no page CSP refuses it. Only the handles take the pointer.

import { type Env, type Found, scan } from './collect';
import { resetAll, resetElement, resizeElement } from './resize';
import { DEFAULT_SETTINGS, type PageSettings, type Report } from './types';

/** The tag of the overlay's layer. */
export const HOST_TAG = 'rbrowser-container-queries';
/** The most matching conditions a label names; the rest are counted. */
const LABEL_CONDITIONS = 3;
/** The least time between two scans of the page, in ms. */
const SCAN_INTERVAL_MS = 150;

export interface Bridge {
  layers: { add(host: Element): boolean; remove(host: Element): void };
  /** An event for the plugin's panel. */
  emit(message: unknown): void;
}

/** The text colour (black or white) with the higher WCAG 2 contrast on `color` (`#rrggbb`). */
export function labelInk(color: string): string {
  const channel = (offset: number): number => {
    const value = parseInt(color.slice(offset, offset + 2), 16) / 255;
    return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
  };
  const luminance = 0.2126 * channel(1) + 0.7152 * channel(3) + 0.0722 * channel(5);
  return (luminance + 0.05) / 0.05 >= 1.05 / (luminance + 0.05) ? '#000000' : '#ffffff';
}

function set(element: HTMLElement, styles: Record<string, string>): void {
  for (const [name, value] of Object.entries(styles)) element.style.setProperty(name, value, 'important');
}

/** The label of a container: its names, its size and the conditions that match now. */
export function labelText(found: Found, report: Report): string {
  const { info } = found;
  const size = `${Math.round(info.inline)} × ${Math.round(info.block)}`;
  const queries = info.matching.map((index) => report.conditions[index]?.query ?? '').filter(Boolean);
  const shown = queries.slice(0, LABEL_CONDITIONS).map((query) => `✓ ${query}`);
  if (queries.length > LABEL_CONDITIONS) shown.push(`+${queries.length - LABEL_CONDITIONS}`);
  return [info.resized ? '↔' : '', info.names.join(' '), size, ...shown].filter(Boolean).join(' · ');
}

export class Overlay {
  readonly host: HTMLElement;
  /** The closed shadow root: the page cannot reach it, the tests can. */
  readonly root: ShadowRoot;
  private readonly frame: HTMLElement;
  private on = false;
  private settings: PageSettings = DEFAULT_SETTINGS;
  private found: Found[] = [];
  private report: Report | null = null;
  private observer: MutationObserver | null = null;
  private sizes: ResizeObserver | null = null;
  private observed = new Set<Element>();
  private timer: ReturnType<typeof setTimeout> | null = null;
  private lastScan = 0;
  private dragging = false;
  private readonly onViewport = (): void => this.schedule(false);

  constructor(
    private readonly doc: Document,
    private readonly win: Window,
    private readonly bridge: Bridge,
    private readonly env: Env,
  ) {
    this.host = doc.createElement(HOST_TAG);
    this.host.style.setProperty('all', 'initial', 'important');
    set(this.host, { position: 'fixed', top: '0', left: '0', width: '0', height: '0', overflow: 'visible', display: 'block', 'pointer-events': 'none', 'z-index': '2147483647' });
    this.root = this.host.attachShadow({ mode: 'closed' });
    this.frame = doc.createElement('div');
    this.root.append(this.frame);
  }

  /** Shows or hides the overlay with `settings`. */
  apply(on: boolean, settings: PageSettings): void {
    this.settings = settings;
    if (on === this.on) {
      if (on) this.draw();
      return;
    }
    this.on = on;
    if (on) this.start();
    else this.stop();
  }

  /** Whether the overlay shows. */
  get shown(): boolean {
    return this.on;
  }

  /** The containers drawn, for the tests. */
  get containers(): readonly Found[] {
    return this.found;
  }

  private start(): void {
    this.win.addEventListener('scroll', this.onViewport, true);
    this.win.addEventListener('resize', this.onViewport);
    this.observer = new MutationObserver(() => this.schedule(true));
    this.observer.observe(this.doc.documentElement, { subtree: true, childList: true, attributes: true, attributeFilter: ['class', 'style', 'id'] });
    if (typeof ResizeObserver === 'function') this.sizes = new ResizeObserver(() => this.schedule(true));
    this.rescan();
  }

  private stop(): void {
    this.win.removeEventListener('scroll', this.onViewport, true);
    this.win.removeEventListener('resize', this.onViewport);
    this.observer?.disconnect();
    this.observer = null;
    this.sizes?.disconnect();
    this.sizes = null;
    this.observed = new Set();
    if (this.timer !== null) clearTimeout(this.timer);
    this.timer = null;
    this.frame.replaceChildren();
    this.found = [];
    this.report = null;
    this.bridge.layers.remove(this.host);
  }

  /** Scans again (a page change) or only redraws (a scroll), at most every {@link SCAN_INTERVAL_MS}. */
  private schedule(rescan: boolean): void {
    if (!this.on) return;
    if (!rescan) {
      this.draw();
      return;
    }
    if (this.timer !== null || this.dragging) return;
    const wait = Math.max(0, this.lastScan + SCAN_INTERVAL_MS - Date.now());
    this.timer = setTimeout(() => {
      this.timer = null;
      this.rescan();
    }, wait);
  }

  /** Scans the page now and draws. */
  rescan(): void {
    if (!this.on) return;
    this.lastScan = Date.now();
    const { found, report } = scan(this.doc, this.env, { exclude: this.host });
    this.found = found;
    this.report = report;
    if (this.sizes) {
      // Only the containers new to it: observing again reports every size once more, which would
      // scan again and again.
      const now = new Set(found.map((entry) => entry.element));
      for (const element of this.observed) if (!now.has(element)) this.sizes.unobserve(element);
      for (const element of now) if (!this.observed.has(element)) this.sizes.observe(element);
      this.observed = now;
    }
    this.draw();
  }

  private draw(): void {
    if (!this.on || !this.report) return;
    if (!this.host.isConnected) this.bridge.layers.add(this.host);
    const report = this.report;
    while (this.frame.children.length > this.found.length) this.frame.lastElementChild?.remove();
    this.found.forEach((found, index) => {
      let box = this.frame.children[index] as HTMLElement | undefined;
      if (!box) {
        box = this.newBox();
        this.frame.append(box);
      }
      this.drawBox(box, found, report);
    });
  }

  private newBox(): HTMLElement {
    const box = this.doc.createElement('div');
    const label = this.doc.createElement('span');
    const handle = this.doc.createElement('div');
    box.append(label, handle);
    handle.addEventListener('pointerdown', (event) => this.drag(event, box));
    handle.addEventListener('dblclick', (event) => {
      event.preventDefault();
      const found = this.found[Array.prototype.indexOf.call(this.frame.children, box)];
      if (found && resetElement(found.element)) this.changed(found, 'reset');
    });
    return box;
  }

  private drawBox(box: HTMLElement, found: Found, report: Report): void {
    const rect = found.element.getBoundingClientRect();
    const color = this.settings.color;
    set(box, {
      position: 'absolute',
      left: `${rect.left}px`,
      top: `${rect.top}px`,
      width: `${Math.max(rect.width, 0)}px`,
      height: `${Math.max(rect.height, 0)}px`,
      'box-sizing': 'border-box',
      border: `2px ${found.info.resized ? 'dashed' : 'solid'} ${color}`,
      background: `${color}14`,
      margin: '0',
      padding: '0',
    });
    const [label, handle] = Array.from(box.children) as [HTMLElement, HTMLElement];
    label.textContent = this.settings.labels ? labelText(found, report) : '';
    set(label, {
      display: this.settings.labels ? 'block' : 'none',
      position: 'absolute',
      left: '-2px',
      top: rect.top >= 18 ? '-18px' : '0',
      'max-width': `${Math.max(rect.width, 160)}px`,
      overflow: 'hidden',
      'text-overflow': 'ellipsis',
      'white-space': 'nowrap',
      font: '600 11px/16px system-ui, sans-serif',
      padding: '0 4px',
      background: color,
      color: labelInk(color),
      'border-radius': '2px',
    });
    set(handle, {
      position: 'absolute',
      top: '0',
      right: '-6px',
      width: '10px',
      height: '100%',
      cursor: 'ew-resize',
      'pointer-events': 'auto',
      background: `${color}99`,
      'border-radius': '3px',
    });
  }

  /** A drag on the handle of `box`: the width follows the pointer until it is let go. */
  private drag(event: PointerEvent, box: HTMLElement): void {
    const found = this.found[Array.prototype.indexOf.call(this.frame.children, box)];
    if (!found || event.button !== 0) return;
    event.preventDefault();
    event.stopPropagation();
    const handle = event.currentTarget as HTMLElement;
    handle.setPointerCapture?.(event.pointerId);
    const startX = event.clientX;
    const startWidth = found.info.inline;
    this.dragging = true;
    let width = startWidth;
    const move = (next: PointerEvent): void => {
      width = Math.max(0, startWidth + next.clientX - startX);
      resizeElement(found.element, width);
      found.info.inline = Math.round(width);
      found.info.resized = true;
      this.draw();
    };
    const end = (): void => {
      handle.removeEventListener('pointermove', move);
      handle.removeEventListener('pointerup', end);
      handle.removeEventListener('pointercancel', end);
      this.dragging = false;
      if (width !== startWidth) this.changed(found, 'resized');
      else this.rescan();
    };
    handle.addEventListener('pointermove', move);
    handle.addEventListener('pointerup', end);
    handle.addEventListener('pointercancel', end);
  }

  /** Tells the panel that a container's width changed in this view. */
  private changed(found: Found, type: 'resized' | 'reset'): void {
    this.rescan();
    const now = this.found.find((entry) => entry.element === found.element) ?? found;
    this.bridge.emit({ type, path: now.info.path, width: now.info.inline });
  }

  /**
   * Removes everything the plugin added, the widths it set included: the host stopped the page
   * script. Switching the overlay off keeps them, for the panel's reset.
   */
  destroy(): void {
    this.stop();
    this.on = false;
    resetAll(this.doc);
  }
}
