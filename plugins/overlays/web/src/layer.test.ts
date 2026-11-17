import { afterEach, describe, expect, it } from 'vitest';
import { HOST_TAG } from './constants';
import { Layer, labelText } from './layer';

let layer: Layer | undefined;
afterEach(() => layer?.destroy());

describe('the overlay layer', () => {
  it("is one fixed host without size or pointer events in rBrowser's one host of page overlays", () => {
    layer = new Layer(document);
    layer.render('overflow', []);
    const layers = document.documentElement.lastElementChild as HTMLElement;
    expect(layers.localName).toBe('rbrowser-layers');
    const host = layers.lastElementChild as HTMLElement;
    expect(host.localName).toBe(HOST_TAG);
    expect(host.getAttribute('data-rbrowser-plugin')).toBe('overlays');
    for (const [name, value] of [
      ['position', 'fixed'],
      ['width', '0px'],
      ['height', '0px'],
      ['pointer-events', 'none'],
      ['z-index', '2147483647'],
    ]) {
      expect(host.style.getPropertyValue(name as string)).toBe(value);
      expect(host.style.getPropertyPriority(name as string)).toBe('important');
    }
    // The shadow root is closed: the page cannot reach the boxes.
    expect(host.shadowRoot).toBeNull();
    expect(host.getAttribute('style')).not.toBeNull();
  });

  it('draws, reuses and clears the boxes of a group', () => {
    layer = new Layer(document);
    const item = { rect: { x: 10, y: 40, width: 100, height: 20 }, color: '#e5262b', label: '+41px' };
    layer.render('overflow', [item, item, item]);
    expect(layer.count('overflow')).toBe(3);
    layer.render('overflow', [item]);
    expect(layer.count('overflow')).toBe(1);
    layer.render('outlines', [item]);
    layer.clear('overflow');
    expect(layer.count('overflow')).toBe(0);
    expect(layer.count('outlines')).toBe(1);
  });

  it('comes back when the page removes it, and goes with destroy', () => {
    layer = new Layer(document);
    layer.render('a', []);
    layer.host.remove();
    layer.render('a', []);
    expect(layer.host.isConnected).toBe(true);
    layer.destroy();
    expect(document.querySelector(HOST_TAG)).toBeNull();
    expect(document.querySelector('rbrowser-layers')).toBeNull();
  });

  it('labels in black or white, whichever contrasts more', () => {
    expect(labelText('#f5b800')).toBe('#000000');
    expect(labelText('#1f6feb')).toBe('#ffffff');
    expect(labelText('#e5262b')).toBe('#000000');
    expect(labelText('#c026d3')).toBe('#ffffff');
  });
});
