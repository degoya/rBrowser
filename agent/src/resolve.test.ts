import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ElementTarget } from './protocol';
import { isVisible, locate, resolve } from './resolve';
import { makeTarget } from './selector';
import { el, giveBoxes, loadPage, setViewport } from './test-page';

beforeEach(() => {
  vi.restoreAllMocks();
  setViewport(1440);
  loadPage();
  giveBoxes();
});

const saveTarget: ElementTarget = {
  element: { kind: 'testid', testid: 'save', path: 'nowhere' },
  offset: { x: 0.25, y: 0.5 },
};

describe('resolve', () => {
  it('gives the viewport point of the offset', () => {
    expect(resolve(saveTarget, document)).toEqual({ status: 'ok', x: 35, y: 40, scrolled: false });
  });

  it('reports not_found for a missing element', () => {
    const target: ElementTarget = {
      element: { kind: 'testid', testid: 'missing', path: '#missing' },
      offset: { x: 0.5, y: 0.5 },
    };
    expect(resolve(target, document)).toEqual({ status: 'not_found' });
  });

  it('reports not_visible for an element under display:none', () => {
    const target = makeTarget(el('[data-testid="hidden-button"]'), 20, 30);
    expect(resolve(target, document)).toEqual({ status: 'not_visible' });
  });

  it('falls back to the CSS path when the primary kind finds nothing', () => {
    const target: ElementTarget = {
      element: { kind: 'testid', testid: 'renamed', path: '#login > button' },
      offset: { x: 0.5, y: 0.5 },
    };
    expect(resolve(target, document)).toMatchObject({ status: 'ok', x: 60, y: 40 });
  });

  it('scrolls an element outside the viewport into view', () => {
    vi.restoreAllMocks();
    giveBoxes({ left: 10, top: 2000, width: 100, height: 40 });
    const scroll = vi.fn();
    el('[data-testid="save"]').scrollIntoView = scroll;
    expect(resolve(saveTarget, document)).toMatchObject({ status: 'ok', scrolled: true });
    expect(scroll).toHaveBeenCalledOnce();
  });

  it('resolves through open shadow roots', () => {
    const root = el('#host').attachShadow({ mode: 'open' });
    root.innerHTML = '<button>Inside</button>';
    const target = makeTarget(root.querySelector('button') as Element, 60, 40);
    expect(resolve(target, document)).toMatchObject({ status: 'ok', x: 60, y: 40 });
  });
});

describe('a selector from a 1440 px view in a 393 px view', () => {
  it('finds the same element after the layout changed', () => {
    const cancel = el('button.kGfCyH');
    const target = makeTarget(cancel, 60, 40);
    setViewport(393);
    expect(isVisible(cancel)).toBe(true);
    expect(resolve(target, document)).toMatchObject({ status: 'ok', x: 60, y: 40 });
  });

  it('takes the visible twin of a link the narrow layout hides', () => {
    const desktopDocs = el('nav.nav-desktop a[href="/docs"]');
    const target = makeTarget(desktopDocs, 60, 40);
    expect(target.element).toMatchObject({ kind: 'role', role: 'link', name: 'Docs' });
    setViewport(393);
    expect(isVisible(desktopDocs)).toBe(false);
    const mobileDocs = el('nav.nav-mobile a[href="/docs"]');
    expect(isVisible(mobileDocs)).toBe(true);
    expect(locate(target, document)).toBe(mobileDocs);
    expect(resolve(target, document)).toMatchObject({ status: 'ok' });
  });
});

describe('a burger label a wider breakpoint hides (RB-090-93)', () => {
  const BURGER = `<!doctype html><html><head><style>
    .sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0, 0, 0, 0); }
    @media (min-width: 1024px) { .lg-hidden { display: none; } }
  </style></head><body><header>
    <input type="checkbox" id="nav-toggle" class="sr-only">
    <label for="nav-toggle" class="lg-hidden"><span></span><span></span><span></span><span class="sr-only">Menu</span></label>
  </header></body></html>`;

  it('resolves where the label shows and is not_visible where it has no box', () => {
    setViewport(768);
    loadPage(BURGER);
    const label = el('label[for="nav-toggle"]');
    const target = makeTarget(el('label[for="nav-toggle"] span'), 40, 30);
    expect(target.element).toMatchObject({ kind: 'text', tag: 'label', text: 'Menu' });
    expect(resolve(target, document)).toMatchObject({ status: 'ok' });
    setViewport(1024);
    expect(isVisible(label)).toBe(false);
    expect(resolve(target, document)).toEqual({ status: 'not_visible' });
  });
});
