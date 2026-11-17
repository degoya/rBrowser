import { afterEach, describe, expect, it } from 'vitest';
import { FILTER_ID, HOST_TAG } from './constants';
import { readSpec } from './filter';
import { Simulator } from './simulator';

const MATRIX = [0.5, 0.5, 0, 0, 0, 0.5, 0.5, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0];

function spec(value: unknown) {
  const read = readSpec(value);
  if (!read) throw new Error('spec');
  return read;
}

const root = () => document.documentElement;
const host = () => document.querySelector(HOST_TAG);

afterEach(() => {
  host()?.remove();
  root().removeAttribute('style');
  document.body.removeAttribute('style');
});

describe('Simulator', () => {
  it('filters the root element only, never the body (no containing block for position: fixed)', () => {
    const simulator = new Simulator(document);
    simulator.apply(spec({ filter: [{ op: 'matrix', values: MATRIX }, { op: 'blur', px: 2 }], overlay: null }));
    expect(root().style.getPropertyValue('filter')).toBe(`url("#${FILTER_ID}-0") blur(2px)`);
    expect(root().style.getPropertyPriority('filter')).toBe('important');
    expect(document.body.style.getPropertyValue('filter')).toBe('');
    const matrix = document.getElementById(`${FILTER_ID}-0`)?.querySelector('feColorMatrix');
    expect(matrix?.getAttribute('values')).toBe(MATRIX.join(' '));
    // RB-090-51: a layer of rBrowser's one host of page overlays, at the end of <html>.
    expect(host()?.parentElement?.localName).toBe('rbrowser-layers');
    expect(host()?.parentElement?.parentElement).toBe(root());
    simulator.destroy();
  });

  it('puts its elements into one rbrowser-simulators host, so the sync agent ignores them', () => {
    const simulator = new Simulator(document);
    simulator.apply(spec({ filter: [{ op: 'matrix', values: MATRIX }], overlay: { kind: 'glare', opacity: 0.3 } }));
    const element = host() as HTMLElement | null;
    expect(element?.tagName.toLowerCase()).toBe('rbrowser-simulators');
    expect(element?.style.getPropertyValue('pointer-events')).toBe('none');
    expect(element?.style.getPropertyValue('position')).toBe('fixed');
    expect(document.querySelectorAll(HOST_TAG)).toHaveLength(1);
    // The matrix lives in the light DOM, where url(#id) of the root element finds it.
    expect(element?.querySelector('svg filter')?.id).toBe(`${FILTER_ID}-0`);
    simulator.destroy();
  });

  it('restores what the page had on its root element', () => {
    root().style.setProperty('filter', 'grayscale(0.2)');
    const simulator = new Simulator(document);
    simulator.apply(spec({ filter: [{ op: 'brightness', amount: 0.64 }], overlay: null }));
    expect(root().style.getPropertyValue('filter')).toBe('brightness(0.64)');
    simulator.apply(null);
    expect(root().style.getPropertyValue('filter')).toBe('grayscale(0.2)');
    expect(root().style.getPropertyPriority('filter')).toBe('');
    expect(host()).toBeNull();
  });

  it('leaves no style attribute where the page had none', () => {
    const simulator = new Simulator(document);
    simulator.apply(spec({ filter: [{ op: 'contrast', amount: 0.5 }], overlay: null }));
    simulator.destroy();
    expect(root().hasAttribute('style')).toBe(false);
  });

  it('switches from one simulation to the next without leftovers', () => {
    const simulator = new Simulator(document);
    simulator.apply(spec({ filter: [{ op: 'matrix', values: MATRIX }], overlay: { kind: 'glare', opacity: 0.3 } }));
    simulator.apply(spec({ filter: [], overlay: { kind: 'tunnel', clear: 0.24, fade: 0.25, opacity: 0.95 } }));
    expect(root().style.getPropertyValue('filter')).toBe('');
    expect(document.getElementById(`${FILTER_ID}-0`)).toBeNull();
    expect(document.querySelectorAll(HOST_TAG)).toHaveLength(1);
    expect(simulator.overlayKind()).toBe('tunnel');
    simulator.destroy();
    expect(host()).toBeNull();
  });

  it('draws each overlay without pointer events', () => {
    const simulator = new Simulator(document);
    for (const overlay of [
      { kind: 'tunnel', clear: 0.24, fade: 0.25, opacity: 0.95 },
      { kind: 'spot', size: 0.25, blurPx: 12, opacity: 0.85 },
      { kind: 'glare', opacity: 0.35 },
    ]) {
      simulator.apply(spec({ filter: [], overlay }));
      const layer = simulator.overlayElement();
      expect(layer?.style.getPropertyValue('pointer-events')).toBe('none');
      expect(layer?.style.getPropertyValue('background-image') || layer?.style.getPropertyValue('background')).toContain(
        'radial-gradient',
      );
    }
    simulator.destroy();
  });

  it('applies itself again when the page rewrites the root style or removes the host', async () => {
    const simulator = new Simulator(document);
    simulator.apply(spec({ filter: [{ op: 'contrast', amount: 0.5 }], overlay: { kind: 'glare', opacity: 0.3 } }));
    root().setAttribute('style', 'color: red');
    // The page takes rBrowser's layers away (the host's parent, a child of <html>).
    const layers = host()?.parentElement;
    layers?.remove();
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(root().style.getPropertyValue('filter')).toBe('contrast(0.5)');
    expect(host()?.isConnected).toBe(true);
    expect(host()?.parentElement?.parentElement).toBe(root());
    simulator.destroy();
    root().setAttribute('style', 'color: blue');
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(root().style.getPropertyValue('filter')).toBe('');
  });
});
