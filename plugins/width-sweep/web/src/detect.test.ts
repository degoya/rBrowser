import { afterEach, describe, expect, it } from 'vitest';
import { findClipped, findOverflow, findOverlaps, overlaps, probe, type Box, type SweepWindow } from './detect';

/** Gives `element` the box `box` (happy-dom has no layout). */
function place(element: Element, box: Box): void {
  element.getBoundingClientRect = () =>
    ({ ...box, left: box.x, top: box.y, right: box.x + box.width, bottom: box.y + box.height, toJSON: () => box }) as DOMRect;
}

/** Gives `element` its scroll and client widths. */
function widths(element: Element, scroll: number, client: number): void {
  Object.defineProperty(element, 'scrollWidth', { configurable: true, value: scroll });
  Object.defineProperty(element, 'clientWidth', { configurable: true, value: client });
}

/** A window of `innerWidth` over happy-dom's, frames at once. */
function win(innerWidth = 1024): SweepWindow {
  return {
    innerWidth,
    scrollX: 0,
    getComputedStyle: (element) => window.getComputedStyle(element),
    requestAnimationFrame: (callback) => window.setTimeout(callback, 0),
    setTimeout: (callback, ms) => window.setTimeout(callback, ms),
  };
}

function page(html: string): Document {
  document.body.innerHTML = html;
  return document;
}

afterEach(() => {
  document.body.innerHTML = '';
  delete (document.documentElement as unknown as { scrollWidth?: number }).scrollWidth;
  delete (document.documentElement as unknown as { clientWidth?: number }).clientWidth;
});

describe('overlaps', () => {
  it('needs more than a pixel of shared area each way', () => {
    const a = { x: 0, y: 0, width: 100, height: 40 };
    expect(overlaps(a, { x: 50, y: 10, width: 100, height: 40 })).toBe(true);
    expect(overlaps(a, { x: 100, y: 0, width: 100, height: 40 })).toBe(false); // touching
    expect(overlaps(a, { x: 99.5, y: 0, width: 100, height: 40 })).toBe(false); // sub-pixel
    expect(overlaps(a, { x: 0, y: 40, width: 100, height: 40 })).toBe(false); // the next row
  });
});

describe('findOverflow', () => {
  it('finds nothing while the document is no wider than the viewport', () => {
    const doc = page('<div class="hero" style="display:block"><img style="display:block"></div>');
    widths(doc.documentElement, 1024, 1024);
    expect(findOverflow(doc, win())).toEqual([]);
  });

  it('names the outermost element past the viewport, not its children', () => {
    const doc = page(
      '<main style="display:block"><div class="hero" style="display:block"><img class="wide" style="display:block"></div></main>',
    );
    widths(doc.documentElement, 1200, 1024);
    place(doc.querySelector('main')!, { x: 0, y: 0, width: 1024, height: 500 });
    place(doc.querySelector('.hero')!, { x: 0, y: 0, width: 1024, height: 500 });
    place(doc.querySelector('img')!, { x: 0, y: 0, width: 1200, height: 300 });
    const issues = findOverflow(doc, win());
    expect(issues).toHaveLength(1);
    expect(issues[0]).toMatchObject({ kind: 'overflow', selector: 'img.wide' });
    expect(issues[0]!.path).toBe('html > body > main > div > img');
  });

  it('leaves out what a clipping ancestor or a fixed box holds', () => {
    const doc = page(
      '<div class="clip" style="display:block;overflow-x:hidden"><img style="display:block"></div><nav style="display:block;position:fixed"></nav>',
    );
    widths(doc.documentElement, 1300, 1024);
    place(doc.querySelector('.clip')!, { x: 0, y: 0, width: 1024, height: 100 });
    place(doc.querySelector('img')!, { x: 0, y: 0, width: 1300, height: 100 });
    place(doc.querySelector('nav')!, { x: 0, y: 0, width: 1300, height: 50 });
    // The document still scrolls: the cause is unknown, the root is named.
    expect(findOverflow(doc, win()).map((issue) => issue.selector)).toEqual(['html']);
  });

  it('is no false report for a 100vw box beside a classic scrollbar', () => {
    const doc = page('<section class="band" style="display:block"></section>');
    // A 15 px scrollbar: the viewport is 1009 px of a 1024 px window.
    widths(doc.documentElement, 1024, 1009);
    place(doc.querySelector('.band')!, { x: 0, y: 0, width: 1024, height: 200 });
    expect(findOverflow(doc, win(1024))).toEqual([]);
  });

  it('still reports a 100vw box that pushes the page further than the scrollbar', () => {
    const doc = page('<section class="band" style="display:block;margin-left:40px"></section>');
    widths(doc.documentElement, 1064, 1009);
    place(doc.querySelector('.band')!, { x: 40, y: 0, width: 1024, height: 200 });
    expect(findOverflow(doc, win(1024)).map((issue) => issue.selector)).toEqual(['section.band']);
  });
});

describe('findClipped', () => {
  it('finds text its box cuts off, and nothing a box lets scroll or shows whole', () => {
    const doc = page(
      '<h2 class="cut" style="display:block;overflow:hidden">A heading too long for its box</h2>' +
        '<p class="dots" style="display:block;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">Long</p>' +
        '<div class="scrolls" style="display:block;overflow-x:auto">Wide table text</div>' +
        '<div class="fits" style="display:block;overflow:hidden">Short</div>' +
        '<input value="a long value in a narrow field">',
    );
    for (const selector of ['.cut', '.dots', '.scrolls', '.fits', 'input']) {
      place(doc.querySelector(selector)!, { x: 0, y: 0, width: 200, height: 30 });
    }
    widths(doc.querySelector('.cut')!, 320, 200);
    widths(doc.querySelector('.dots')!, 260, 200);
    widths(doc.querySelector('.scrolls')!, 600, 200);
    widths(doc.querySelector('.fits')!, 200, 200);
    widths(doc.querySelector('input')!, 500, 200);
    const found = findClipped(doc, win()).map((issue) => issue.selector);
    expect(found).toEqual(['h2.cut', 'p.dots']);
  });

  it('ignores a box with no text of its own and one that is not shown', () => {
    const doc = page(
      '<div class="wrap" style="display:block;overflow:hidden"><span>text in a child</span></div>' +
        '<div class="hidden" style="display:block;overflow:hidden;visibility:hidden">Hidden text</div>',
    );
    for (const selector of ['.wrap', '.hidden']) {
      place(doc.querySelector(selector)!, { x: 0, y: 0, width: 100, height: 20 });
      widths(doc.querySelector(selector)!, 300, 100);
    }
    expect(findClipped(doc, win())).toEqual([]);
  });
});

describe('findOverlaps', () => {
  it('names the later of two visible block siblings that overlap, and the other', () => {
    const doc = page(
      '<nav style="display:flex"><a class="one" style="display:block">One</a><a class="two" style="display:block">Two</a><a class="three" style="display:block">Three</a></nav>',
    );
    place(doc.querySelector('.one')!, { x: 0, y: 0, width: 120, height: 40 });
    place(doc.querySelector('.two')!, { x: 100, y: 0, width: 120, height: 40 });
    place(doc.querySelector('.three')!, { x: 220, y: 0, width: 120, height: 40 });
    const issues = findOverlaps(doc, win());
    expect(issues).toHaveLength(1);
    expect(issues[0]).toMatchObject({ kind: 'overlap', selector: 'a.two', other: 'a.one' });
  });

  it('leaves out inline, positioned, hidden and empty siblings', () => {
    const doc = page(
      '<div style="display:block">' +
        '<span class="inline" style="display:inline">a</span><span class="inline2" style="display:inline">b</span>' +
        '<div class="badge" style="display:block;position:absolute">!</div>' +
        '<div class="ghost" style="display:block;opacity:0">ghost</div>' +
        '<div class="empty" style="display:block"></div>' +
        '<div class="card" style="display:block">card</div>' +
        '</div>',
    );
    for (const selector of ['.inline', '.inline2', '.badge', '.ghost', '.card']) {
      place(doc.querySelector(selector)!, { x: 0, y: 0, width: 100, height: 40 });
    }
    place(doc.querySelector('.empty')!, { x: 0, y: 0, width: 0, height: 0 });
    expect(findOverlaps(doc, win())).toEqual([]);
  });
});

describe('probe', () => {
  it('answers the width, the issues of the kinds asked for and Escape since the reset', async () => {
    const doc = page('<div class="box" style="display:block;overflow:hidden">Too long</div>');
    place(doc.querySelector('.box')!, { x: 0, y: 0, width: 100, height: 20 });
    widths(doc.querySelector('.box')!, 180, 100);
    widths(doc.documentElement, 1024, 1024);
    const sweep = win(1024);
    const first = await probe({ width: 1024, kinds: ['overflow', 'text-clipped'], reset: true }, doc, sweep);
    expect(first).toEqual({
      width: 1024,
      escape: false,
      issues: [{ kind: 'text-clipped', selector: 'div.box', path: 'html > body > div' }],
    });
    doc.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    const second = await probe({ width: 1024, kinds: ['overlap'] }, doc, sweep);
    expect(second.escape).toBe(true);
    expect(second.issues).toEqual([]);
    const third = await probe({ width: 1024, kinds: [], reset: true }, doc, sweep);
    expect(third.escape).toBe(false);
  });

  it('gives up waiting for a width the page never takes and answers the one it has', async () => {
    const doc = page('');
    widths(doc.documentElement, 800, 800);
    const answer = await probe({ width: 640, kinds: ['overflow'] }, doc, win(800));
    expect(answer.width).toBe(800);
  });
});
