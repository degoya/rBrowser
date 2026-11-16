// The behaviour of the overlays' one host `<rbrowser-layers>` (RB-090-51), run against both
// copies: web-common's `layers.ts` (bundled page scripts, `evaluate`) and the prologue's
// `rbPlugin.layers` (plain page scripts). Each world adds and removes its own layers; the DOM is
// all they share.
import { afterEach, describe, expect, it } from 'vitest';

/** One copy of the logic, as a page script of plugin `owner` calls it. */
export interface LayersImpl {
  add(host: Element): boolean;
  remove(host: Element): void;
}

const containers = () => Array.from(document.documentElement.children).filter((child) => child.localName === 'rbrowser-layers');

function host(tag: string): HTMLElement {
  const made = document.createElement(tag);
  made.attachShadow({ mode: 'closed' });
  return made;
}

/** The suite for the copy `name`; `make(owner)` answers its functions for plugin `owner`. */
export function describeLayers(name: string, make: (owner: string) => LayersImpl): void {
  describe(`the overlays' one host (${name})`, () => {
    afterEach(() => {
      for (const container of containers()) container.remove();
    });

    it('puts a layer into one <rbrowser-layers> at the end of <html>, out of every box and of the accessibility tree', () => {
      const ruler = host('rbrowser-rulers-guides');
      expect(make('rulers-guides').add(ruler)).toBe(true);
      const [layers] = containers();
      expect(containers()).toHaveLength(1);
      expect(document.documentElement.lastElementChild).toBe(layers);
      expect(ruler.parentElement).toBe(layers);
      expect(ruler.getAttribute('data-rbrowser-plugin')).toBe('rulers-guides');
      expect(layers?.getAttribute('aria-hidden')).toBe('true');
      const style = (layers as HTMLElement).style;
      expect(style.getPropertyValue('display')).toBe('contents');
      expect(style.getPropertyPriority('display')).toBe('important');
      // No stacking context, no containing block: a fixed layer stays fixed to the viewport.
      for (const name of ['position', 'transform', 'filter', 'contain', 'z-index', 'will-change']) {
        expect(['', 'initial', 'none', 'auto', 'static']).toContain(style.getPropertyValue(name));
      }
    });

    it('shares the container between plugins and adds a layer only once', () => {
      const peek = host('rbrowser-peek');
      const marks = host('rbrowser-color-picker-marks');
      make('peek').add(peek);
      make('color-picker').add(marks);
      make('peek').add(peek);
      expect(containers()).toHaveLength(1);
      expect(Array.from(containers()[0]?.children ?? [])).toEqual([peek, marks]);
    });

    it('takes a container another world made', () => {
      const made = document.createElement('rbrowser-layers');
      document.documentElement.append(made);
      const fonts = host('rbrowser-fonts');
      make('fonts').add(fonts);
      expect(containers()).toEqual([made]);
      expect(fonts.parentElement).toBe(made);
    });

    it('removes the container with its last layer', () => {
      const one = host('rbrowser-image-overlay');
      const two = host('rbrowser-image-overlay');
      const layers = make('image-overlay');
      layers.add(one);
      layers.add(two);
      layers.remove(one);
      expect(containers()).toHaveLength(1);
      layers.remove(two);
      expect(containers()).toHaveLength(0);
      expect(two.isConnected).toBe(false);
      // A layer that is not in the page is no failure.
      layers.remove(two);
    });

    it('makes the container anew when the page replaced <html> or removed it', () => {
      const doc = document.implementation.createHTMLDocument('');
      const tip = doc.createElement('rbrowser-peek');
      const layers = make('peek');
      layers.add(tip);
      doc.documentElement.remove();
      const fresh = doc.createElement('html');
      doc.append(fresh);
      expect(tip.isConnected).toBe(false);
      expect(layers.add(tip)).toBe(true);
      expect(tip.parentElement?.localName).toBe('rbrowser-layers');
      expect(tip.parentElement?.parentElement).toBe(fresh);
      tip.parentElement?.remove();
      layers.add(tip);
      expect(fresh.children).toHaveLength(1);
      expect(tip.parentElement?.parentElement).toBe(fresh);
    });

    it('waits while the document has no root element', () => {
      const doc = document.implementation.createHTMLDocument('');
      doc.documentElement.remove();
      const tip = doc.createElement('rbrowser-peek');
      expect(make('peek').add(tip)).toBe(false);
      expect(tip.isConnected).toBe(false);
    });
  });
}
