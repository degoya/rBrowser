import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import scrollPage from '../../../tests/fixtures/site/scroll.html?raw';
import { el, loadPage } from '../test-page';
import { anchorAbove, anchorTarget, chooseAnchor, positionOf, viewportOf } from './anchor';
import { Layout, desktop, stack } from './test-layout';

let layout: Layout;

beforeEach(() => {
  loadPage(scrollPage);
  layout = new Layout();
});

afterEach(() => {
  layout.restore();
  vi.restoreAllMocks();
});

describe('the scroll anchor', () => {
  it('is the element at the top edge, under the fixed header', () => {
    desktop(layout);
    layout.install();
    // Section 5 starts at 72 + 720 + 480 + 720 + 480 = 2472; 120 px of it are above the edge.
    layout.page.y = 2472 + 120;
    const anchor = chooseAnchor(document);
    expect(anchor).toBe(el('#s5'));
    expect(anchorTarget(el('#s5'), document)).toEqual({
      element: { kind: 'css', path: '#s5' },
      offset: { x: -0.255319, y: 0.166667 },
    });
  });

  it('prefers the innermost heading, section or element with an id or test id', () => {
    desktop(layout);
    layout.install();
    layout.page.y = 2472 + 12;
    expect(chooseAnchor(document)).toBe(el('#s5 > h2'));
    expect(anchorTarget(el('#s5 > h2'), document).element).toEqual({
      kind: 'css',
      path: '#s5 > h2',
    });
    expect(anchorTarget(el('#s5 > h2'), document).offset.y).toBe(0.25);
  });

  it('climbs from an element with a generated class to its section', () => {
    desktop(layout);
    const card = document.createElement('div');
    card.className = 'css-1x2y3z';
    el('#s2').append(card);
    layout.box(card, { left: 288, top: 900, width: 1128, height: 200 });
    layout.install();
    layout.page.y = 950;
    expect(chooseAnchor(document)).toBe(el('#s2'));
  });

  it('names an element with a test id by it', () => {
    desktop(layout);
    layout.install();
    expect(anchorTarget(el('[data-testid="wide-table"]'), document).element).toMatchObject({
      kind: 'testid',
      testid: 'wide-table',
    });
  });

  it('is missing where no content is at the edge', () => {
    desktop(layout);
    layout.install();
    layout.page.y = 0;
    layout.boxes.clear();
    expect(chooseAnchor(document)).toBeNull();
  });

  it('of a container is found inside it, at its own top edge', () => {
    const sidebar = el('#sidebar');
    layout.box(sidebar, { left: 24, top: 72, width: 240, height: 400 });
    layout.scroller(sidebar, 240, 24 * 48);
    stack(layout, Array.from(sidebar.querySelectorAll('li')), Array(24).fill(48), { width: 240 });
    layout.install();
    sidebar.scrollTo({ top: 100 });
    expect(viewportOf(sidebar)).toEqual({ left: 24, top: 72, width: 240, height: 400 });
    expect(positionOf(sidebar)).toEqual({ x: 0, y: 100, maxX: 0, maxY: 752 });
    const anchor = chooseAnchor(sidebar);
    expect(anchor).toBe(el('#item-3'));
    expect(anchorTarget(el('#item-3'), sidebar).offset).toEqual({ x: 0, y: 0.083333 });
  });

  it('of the document is never inside an inner scroll container', () => {
    const sidebar = el('#sidebar');
    sidebar.style.position = 'static';
    layout.box(sidebar, { left: 24, top: 72, width: 240, height: 400 });
    layout.scroller(sidebar, 240, 24 * 48);
    stack(layout, Array.from(sidebar.querySelectorAll('li')), Array(24).fill(48), { width: 240 });
    layout.install();
    expect(anchorAbove(el('#item-3'), document)).toBe(sidebar);
    expect(anchorAbove(el('#item-3'), sidebar)).toBe(el('#item-3'));
    // Sticky, the sidebar does not move with the page: nothing in it anchors the document.
    sidebar.style.position = 'sticky';
    expect(anchorAbove(el('#item-3'), document)).toBeNull();
  });

  it('skips sticky elements', () => {
    desktop(layout);
    el('#s5 > h2').style.position = 'sticky';
    layout.install();
    layout.page.y = 2472 + 12;
    expect(chooseAnchor(document)).toBe(el('#s5'));
  });
});
