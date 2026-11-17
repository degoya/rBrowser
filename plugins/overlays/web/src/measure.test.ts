import { beforeEach, describe, expect, it } from 'vitest';
import { measure, measureElements, type MeasureWindow } from './measure';

type Box = { x: number; y: number; width: number; height: number };

function place(element: Element, box: Box): void {
  const rect = { ...box, left: box.x, top: box.y, right: box.x + box.width, bottom: box.y + box.height };
  (element as HTMLElement).getBoundingClientRect = () => ({ ...rect, toJSON: () => rect }) as DOMRect;
}

function viewport(clientWidth: number, scrollWidth: number): void {
  const root = document.documentElement;
  Object.defineProperty(root, 'clientWidth', { value: clientWidth, configurable: true });
  Object.defineProperty(root, 'scrollWidth', { value: scrollWidth, configurable: true });
}

/** A window whose computed style is the inline style: happy-dom lays nothing out. */
function fakeWindow(innerWidth: number): MeasureWindow {
  return {
    innerWidth,
    scrollX: 0,
    getComputedStyle: (element: Element) => {
      const style = (element as HTMLElement).style;
      return {
        display: style.display || 'block',
        position: style.position || 'static',
        overflowX: style.overflowX || style.overflow || 'visible',
      } as CSSStyleDeclaration;
    },
    matchMedia: () => ({ matches: true }) as MediaQueryList,
  };
}

const $ = (selector: string): Element => {
  const found = document.querySelector(selector);
  if (!found) throw new Error(`no ${selector}`);
  return found;
};

beforeEach(() => {
  document.head.innerHTML = '';
  document.body.innerHTML = `
    <main class="page">
      <div class="card">one</div>
      <div class="promo"><div class="inner">wide</div></div>
      <div class="clipped" style="overflow-x: hidden"><div class="huge">clipped</div></div>
      <div class="bar" style="position: fixed"><div class="fixed-wide">fixed</div></div>
      <div class="gone" style="display: none"><div class="gone-wide">gone</div></div>
    </main>`;
  place(document.body, { x: 0, y: 0, width: 375, height: 600 });
  place($('main'), { x: 16, y: 0, width: 343, height: 600 });
  place($('.card'), { x: 16, y: 0, width: 343, height: 40 });
  place($('.promo'), { x: 16, y: 40, width: 400, height: 80 });
  place($('.inner'), { x: 16, y: 40, width: 400, height: 80 });
  place($('.clipped'), { x: 16, y: 120, width: 343, height: 40 });
  place($('.huge'), { x: 16, y: 120, width: 2000, height: 40 });
  place($('.bar'), { x: 0, y: 0, width: 375, height: 10 });
  place($('.fixed-wide'), { x: 0, y: 0, width: 3000, height: 10 });
  place($('.gone'), { x: 0, y: 0, width: 0, height: 0 });
  place($('.gone-wide'), { x: 0, y: 0, width: 900, height: 10 });
});

describe('the overflow measurement', () => {
  it('names exactly the outermost element that passes the viewport, with its overhang', () => {
    viewport(375, 416);
    const measured = measureElements(document, fakeWindow(375), { show100vw: false });
    expect(measured.causes).toEqual([$('.promo')]);
    const report = measured.report;
    expect(report.hasHorizontalScroll).toBe(true);
    expect(report.causes).toHaveLength(1);
    expect(report.causes[0]?.selector).toBe('div.promo');
    expect(report.causes[0]?.overhang).toBe(41);
    expect(report.causes[0]?.rect).toEqual({ x: 16, y: 40, width: 400, height: 80 });
  });

  it('looks for no cause while the document does not scroll', () => {
    viewport(1440, 1440);
    const report = measure({ show100vw: false }, document, fakeWindow(1440));
    expect(report.hasHorizontalScroll).toBe(false);
    expect(report.causes).toEqual([]);
  });

  it('skips the element it is told to leave out (its own overlay)', () => {
    viewport(375, 416);
    const measured = measureElements(document, fakeWindow(375), { show100vw: false, exclude: $('.promo') });
    expect(measured.causes).toEqual([]);
  });

  it('measures in document coordinates while the page is scrolled sideways', () => {
    viewport(375, 416);
    // Scrolled 41 px to the right: every box but the fixed one moves 41 px to the left.
    place(document.body, { x: -41, y: 0, width: 375, height: 600 });
    for (const selector of ['main', '.card', '.clipped']) place($(selector), { x: -25, y: 0, width: 343, height: 40 });
    place($('.promo'), { x: -25, y: 40, width: 400, height: 80 });
    place($('.inner'), { x: -25, y: 40, width: 400, height: 80 });
    const scrolled = { ...fakeWindow(375), scrollX: 41 };
    const report = measure({ show100vw: false }, document, scrolled);
    expect(report.causes[0]?.overhang).toBe(41);
    expect(report.causes[0]?.rect.x).toBe(16);
  });
});

describe('the 100vw hint', () => {
  it('marks an element whose stylesheet rule says width: 100vw, without any scroll', () => {
    viewport(1440, 1440);
    const style = document.createElement('style');
    style.textContent = '.hero { width: 100vw; } .half { width: 50vw; }';
    document.head.append(style);
    document.body.insertAdjacentHTML('beforeend', '<section class="hero"></section><section class="half"></section>');
    place($('.hero'), { x: 0, y: 600, width: 1440, height: 200 });
    place($('.half'), { x: 0, y: 800, width: 720, height: 200 });
    const measured = measureElements(document, fakeWindow(1440), { show100vw: true });
    expect(measured.vw100).toEqual([$('.hero')]);
    expect(measured.report.vw100[0]?.selector).toBe('section.hero');
    expect(measured.report.hasHorizontalScroll).toBe(false);
  });

  it('marks an inline 100vw and, beside a classic scrollbar, every element as wide as the window', () => {
    viewport(1425, 1440);
    document.body.insertAdjacentHTML(
      'beforeend',
      '<div class="inline" style="min-width: 100vw"></div><div class="banner"></div><div class="empty" style="width: 100vw"></div>',
    );
    place($('.inline'), { x: 0, y: 600, width: 1440, height: 20 });
    place($('.banner'), { x: 0, y: 620, width: 1440, height: 20 });
    place($('.empty'), { x: 0, y: 640, width: 0, height: 0 });
    const measured = measureElements(document, fakeWindow(1440), { show100vw: true });
    expect(measured.vw100).toContain($('.inline'));
    expect(measured.vw100).toContain($('.banner'));
    expect(measured.vw100).not.toContain($('.empty'));
  });

  it('is left out when switched off', () => {
    viewport(1425, 1440);
    document.body.insertAdjacentHTML('beforeend', '<div class="inline" style="width: 100vw"></div>');
    place($('.inline'), { x: 0, y: 600, width: 1440, height: 20 });
    expect(measure({ show100vw: false }, document, fakeWindow(1440)).vw100).toEqual([]);
  });
});
