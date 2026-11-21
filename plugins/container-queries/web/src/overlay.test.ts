// The overlay against the fixture with 3 containers (tests/fixtures/containers.html): framed with
// their size and matching conditions, a drag on the handle pins the width - (min-width: 500px)
// stops matching at 400 px - and a double click resets it (RB-094-01's acceptance).
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { loadPage, pluginFile } from '../../../web-common/src/load';
import { fixtureEnv } from './fixture-env';
import { HOST_TAG, Overlay, labelInk } from './overlay';
import { ORIGINAL_ATTRIBUTE } from './resize';
import { DEFAULT_SETTINGS } from './types';

let overlay: Overlay;
let emitted: unknown[];

function boxes(): HTMLElement[] {
  return Array.from(overlay.root.firstElementChild?.children ?? []) as HTMLElement[];
}

const labelOf = (box: HTMLElement | undefined): string => (box?.children[0] as HTMLElement | undefined)?.textContent ?? '';
const handleOf = (box: HTMLElement | undefined): HTMLElement => box?.children[1] as HTMLElement;

function pointer(target: HTMLElement, type: string, clientX: number): void {
  target.dispatchEvent(new PointerEvent(type, { clientX, button: 0, pointerId: 1, bubbles: true, cancelable: true }));
}

beforeEach(() => {
  loadPage(pluginFile('tests/fixtures/containers.html'));
  emitted = [];
  overlay = new Overlay(document, window, {
    layers: {
      add: (host) => {
        document.documentElement.append(host);
        return true;
      },
      remove: (host) => host.remove(),
    },
    emit: (message) => emitted.push(message),
  }, fixtureEnv);
});

afterEach(() => overlay.destroy());

describe('the container overlay', () => {
  it('frames the 3 containers with their names, sizes and matching conditions', () => {
    overlay.apply(true, DEFAULT_SETTINGS);
    expect(document.querySelector(HOST_TAG)).not.toBeNull();
    expect(boxes()).toHaveLength(3);
    expect(boxes().map(labelOf)).toEqual(['shell · 1000 × 600', 'card · 640 × 300 · ✓ (min-width: 500px)', '300 × 250 · ✓ (min-height: 200px)']);
    expect(handleOf(boxes()[0]).style.getPropertyValue('pointer-events')).toBe('auto');
  });

  it('leaves the labels out when the setting says so and draws in its colour', () => {
    overlay.apply(true, { labels: false, color: '#123456' });
    expect(boxes().map(labelOf)).toEqual(['', '', '']);
    expect(boxes()[0]?.style.getPropertyValue('border')).toContain('#123456');
  });

  it('drags a container to 400 px: (min-width: 500px) no longer matches; a double click resets it', () => {
    overlay.apply(true, DEFAULT_SETTINGS);
    const hero = document.getElementById('hero') as HTMLElement;
    const before = hero.style.cssText;
    const handle = handleOf(boxes()[1]);
    pointer(handle, 'pointerdown', 700);
    pointer(handle, 'pointermove', 460);
    pointer(handle, 'pointerup', 460);
    expect(hero.style.getPropertyValue('width')).toBe('400px');
    expect(labelOf(boxes()[1])).toBe('↔ · card · 400 × 300');
    expect(emitted).toEqual([{ type: 'resized', path: 'html > body > main > article', width: 400 }]);
    handleOf(boxes()[1]).dispatchEvent(new MouseEvent('dblclick', { bubbles: true, cancelable: true }));
    expect(hero.hasAttribute(ORIGINAL_ATTRIBUTE)).toBe(false);
    expect(hero.style.cssText).toBe(before);
    expect(labelOf(boxes()[1])).toBe('card · 640 × 300 · ✓ (min-width: 500px)');
    expect(emitted[1]).toEqual({ type: 'reset', path: 'html > body > main > article', width: 640 });
  });

  it('takes its layer away when switched off, keeping a width for the panel to reset', () => {
    overlay.apply(true, DEFAULT_SETTINGS);
    const handle = handleOf(boxes()[1]);
    pointer(handle, 'pointerdown', 700);
    pointer(handle, 'pointermove', 600);
    pointer(handle, 'pointerup', 600);
    overlay.apply(false, DEFAULT_SETTINGS);
    expect(document.querySelector(HOST_TAG)).toBeNull();
    expect(boxes()).toHaveLength(0);
    expect(document.getElementById('hero')?.hasAttribute(ORIGINAL_ATTRIBUTE)).toBe(true);
  });

  it('resets every width it set when the page script stops', () => {
    overlay.apply(true, DEFAULT_SETTINGS);
    const handle = handleOf(boxes()[1]);
    pointer(handle, 'pointerdown', 700);
    pointer(handle, 'pointermove', 600);
    pointer(handle, 'pointerup', 600);
    overlay.destroy();
    expect(document.getElementById('hero')?.hasAttribute(ORIGINAL_ATTRIBUTE)).toBe(false);
  });

  it('writes its labels in the ink with the higher contrast on the frame colour', () => {
    expect(labelInk('#0d9488')).toBe('#000000');
    expect(labelInk('#1e1b4b')).toBe('#ffffff');
    expect(labelInk('#ffff00')).toBe('#000000');
  });
});
