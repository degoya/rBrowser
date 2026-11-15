// Draws one simulation in the page: a CSS `filter` on the root element and, for the types that
// hide part of the view, a layer.
//
// - The filter goes on <html> and nowhere else: the root element is the one element whose
//   `filter` makes no containing block for `position: fixed` (Filter Effects 1), so a fixed
//   header stays where it is. The page's own inline value is put back afterwards.
// - Everything else lives in one host element, <rbrowser-simulators>, at the end of <html>: fixed
//   over the viewport, without pointer events (clicks and the sync agent's events reach the page
//   as before; the agent ignores `rbrowser-*` anyway). The SVG colour matrices sit in its light
//   DOM, where the root's `url(#id)` finds them, shown through a <slot>; the layer sits in its
//   closed shadow root.
// - Every style is set through the CSSOM (`style.setProperty`), never as a style attribute or a
//   <style> element, so a page's CSP (`style-src`) cannot refuse it.
// - A page that rewrites the root's style attribute or removes the host gets them back.

import { FILTER_ID, HOST_TAG } from './constants';
import { cssFilter } from './filter';
import type { Overlay, Spec } from './types';

const SVG = 'http://www.w3.org/2000/svg';

type Styles = Record<string, string>;

function set(element: HTMLElement | SVGElement, styles: Styles): void {
  for (const [name, value] of Object.entries(styles)) element.style.setProperty(name, value, 'important');
}

const HOST_STYLE: Styles = {
  position: 'fixed',
  top: '0',
  left: '0',
  right: '0',
  bottom: '0',
  margin: '0',
  padding: '0',
  border: '0',
  display: 'block',
  overflow: 'hidden',
  'pointer-events': 'none',
  'z-index': '2147483647',
  transform: 'none',
  filter: 'none',
};

const n = (value: number): string => String(Math.round(value * 1000) / 1000);

/** The layer's styles for `overlay`. */
export function overlayStyles(overlay: Overlay): Styles {
  const base: Styles = { position: 'absolute', 'pointer-events': 'none', margin: '0', padding: '0', border: '0' };
  switch (overlay.kind) {
    case 'tunnel':
      return {
        ...base,
        inset: '0',
        'background-image': `radial-gradient(circle at 50% 50%, rgba(0, 0, 0, 0) ${n(overlay.clear * 100)}%, rgba(0, 0, 0, ${n(overlay.opacity)}) ${n((overlay.clear + overlay.fade) * 100)}%)`,
      };
    case 'spot': {
      const size = `calc(${n(overlay.size)} * min(100vw, 100vh))`;
      const tint = (alpha: number) => `rgba(40, 40, 40, ${n(alpha)})`;
      return {
        ...base,
        left: '50%',
        top: '50%',
        width: size,
        height: size,
        transform: 'translate(-50%, -50%)',
        'border-radius': '50%',
        'backdrop-filter': `blur(${n(overlay.blurPx)}px)`,
        '-webkit-backdrop-filter': `blur(${n(overlay.blurPx)}px)`,
        'background-image': `radial-gradient(circle, ${tint(overlay.opacity)} 0%, ${tint(overlay.opacity * 0.7)} 50%, ${tint(0)} 100%)`,
      };
    }
    case 'glare': {
      const haze = (alpha: number) => `rgba(255, 255, 245, ${n(alpha)})`;
      return {
        ...base,
        inset: '0',
        'background-image': `radial-gradient(circle at 50% 45%, ${haze(overlay.opacity)} 0%, ${haze(overlay.opacity * 0.45)} 55%, ${haze(overlay.opacity * 0.3)} 100%)`,
      };
    }
  }
}

export class Simulator {
  private host: HTMLElement | null = null;
  private layer: HTMLElement | null = null;
  private spec: Spec | null = null;
  /** The root's filter as the browser reads back what we set; compared, so a re-apply never loops. */
  private applied = '';
  /** The page's own inline `filter` on the root, put back when the simulation ends. */
  private saved: { value: string; priority: string; attribute: string | null } | null = null;
  private observer: MutationObserver | null = null;

  constructor(private readonly doc: Document) {}

  /** Draws `spec`, or removes every trace of the last one for null. */
  apply(spec: Spec | null): void {
    this.clear();
    this.spec = spec;
    if (!spec) return;
    const root = this.doc.documentElement;
    if (!root) {
      // `document-start`: the root element comes a moment later.
      this.watch(this.doc, { childList: true });
      return;
    }
    this.draw(root, spec);
    this.watch(root, { attributes: true, attributeFilter: ['style'], childList: true });
  }

  /** Ends the simulation (the page script was switched off). */
  destroy(): void {
    this.apply(null);
  }

  /** The kind of the layer drawn, for the tests. */
  overlayKind(): Overlay['kind'] | null {
    return this.spec?.overlay?.kind ?? null;
  }

  /** The layer drawn, for the tests (the shadow root is closed). */
  overlayElement(): HTMLElement | null {
    return this.layer;
  }

  private draw(root: HTMLElement, spec: Spec): void {
    const host = this.doc.createElement(HOST_TAG);
    host.style.setProperty('all', 'initial', 'important');
    set(host, HOST_STYLE);
    host.setAttribute('aria-hidden', 'true');
    const shadow = host.attachShadow({ mode: 'closed' });
    shadow.append(this.doc.createElement('slot'));
    const matrices = spec.filter.flatMap((step) => (step.op === 'matrix' ? [step.values] : []));
    if (matrices.length > 0) {
      const svg = this.doc.createElementNS(SVG, 'svg');
      svg.setAttribute('width', '0');
      svg.setAttribute('height', '0');
      set(svg, { position: 'absolute', width: '0', height: '0', overflow: 'hidden' });
      matrices.forEach((values, index) => {
        const filter = this.doc.createElementNS(SVG, 'filter');
        filter.id = `${FILTER_ID}-${index}`;
        filter.setAttribute('color-interpolation-filters', 'linearRGB');
        const matrix = this.doc.createElementNS(SVG, 'feColorMatrix');
        matrix.setAttribute('type', 'matrix');
        matrix.setAttribute('values', values.join(' '));
        filter.append(matrix);
        svg.append(filter);
      });
      host.append(svg);
    }
    if (spec.overlay) {
      this.layer = this.doc.createElement('div');
      set(this.layer, overlayStyles(spec.overlay));
      shadow.append(this.layer);
    }
    this.host = host;
    root.append(host);
    const css = cssFilter(spec.filter, (index) => `${FILTER_ID}-${index}`);
    if (css) {
      const style = root.style;
      this.saved = {
        value: style.getPropertyValue('filter'),
        priority: style.getPropertyPriority('filter'),
        attribute: root.getAttribute('style'),
      };
      style.setProperty('filter', css, 'important');
      this.applied = style.getPropertyValue('filter');
    }
  }

  /** Puts back what the page took away. */
  private repair(): void {
    const root = this.doc.documentElement;
    if (!this.spec || !root) return;
    if (!this.host) {
      this.observer?.disconnect();
      this.observer = null;
      this.draw(root, this.spec);
      this.watch(root, { attributes: true, attributeFilter: ['style'], childList: true });
      return;
    }
    if (!this.host.isConnected) root.append(this.host);
    if (this.saved && root.style.getPropertyValue('filter') !== this.applied) {
      const css = cssFilter(this.spec.filter, (index) => `${FILTER_ID}-${index}`);
      root.style.setProperty('filter', css, 'important');
      this.applied = root.style.getPropertyValue('filter');
    }
  }

  private watch(target: Node, options: MutationObserverInit): void {
    const view = this.doc.defaultView;
    if (!view) return;
    this.observer = new view.MutationObserver(() => this.repair());
    this.observer.observe(target, options);
  }

  private clear(): void {
    this.observer?.disconnect();
    this.observer = null;
    this.host?.remove();
    this.host = null;
    this.layer = null;
    const root = this.doc.documentElement;
    if (this.saved && root) {
      if (this.saved.value) root.style.setProperty('filter', this.saved.value, this.saved.priority);
      else root.style.removeProperty('filter');
      // A root without a style attribute before gets none back. Chromium writes the inline style
      // into the attribute lazily: reading it first makes the removal stick.
      if (this.saved.attribute === null && root.style.length === 0) {
        root.getAttribute('style');
        root.removeAttribute('style');
      }
    }
    this.saved = null;
    this.applied = '';
  }
}
