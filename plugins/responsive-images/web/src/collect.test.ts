// The collector of RB-093-02 against a page in happy-dom, which has no layout and loads no
// image: the geometry, the decoded sizes, the probe and the LCP element come through `Env`.
import { afterEach, describe, expect, it } from 'vitest';
import { collect, MAX_IMAGES, nodePath, type Env } from './collect';
import { run } from './collect-entry';
import type { Size } from './types';

interface Fake {
  rendered?: Record<string, Size>;
  natural?: Record<string, Size>;
  current?: Record<string, string>;
  probe?: Record<string, Size | null>;
  lcp?: string | null;
  dpr?: number;
  hidden?: string[];
}

/** An Env whose images are found by id; an image without an entry is 100x100 and loaded. */
function env(fake: Fake = {}): Env {
  const id = (image: HTMLImageElement): string => image.id;
  return {
    dpr: () => fake.dpr ?? 1,
    viewport: () => ({ width: 1280, height: 800 }),
    rendered: (image) => fake.rendered?.[id(image)] ?? { width: 100, height: 100 },
    visible: (image) => !(fake.hidden ?? []).includes(id(image)),
    natural: (image) => fake.natural?.[id(image)] ?? { width: 100, height: 100 },
    loaded: (image) => (fake.natural?.[id(image)]?.width ?? 100) > 0,
    currentSrc: (image) => fake.current?.[id(image)] ?? new URL(image.getAttribute('src') ?? '', document.baseURI).href,
    matches: (media) => media !== '(min-width: 2000px)',
    probe: async (url) => (fake.probe && url in fake.probe ? (fake.probe[url] ?? null) : null),
    lcpElement: async () => (fake.lcp === undefined ? { known: false, element: null } : { known: true, element: fake.lcp ? document.getElementById(fake.lcp) : null }),
  };
}

const BASE = document.baseURI;
const abs = (path: string): string => new URL(path, BASE).href;

afterEach(() => {
  document.body.innerHTML = '';
});

describe('collect', () => {
  it('reads each image: sources, attributes, sizes, the probe', async () => {
    document.body.innerHTML = `<main>
      <img id="a" src="a.jpg" srcset="a-400.jpg 400w, a-800.jpg 800w" sizes="50vw" loading="lazy" fetchpriority="high" alt="">
      <img id="b" src="b.jpg">
    </main>`;
    const page = await collect(
      { dpr: 2, oversizeFactor: 1.5, undersizeFactor: 1, checkLcp: true },
      env({ lcp: null, current: { a: abs('a-800.jpg') }, probe: { [abs('a-800.jpg')]: { width: 800, height: 600 } }, rendered: { a: { width: 400, height: 300 } } }),
    );
    expect(page.dpr).toBe(2);
    expect(page.images).toHaveLength(2);
    const [a, b] = page.images;
    expect(a).toMatchObject({
      selector: '#a',
      currentSrc: abs('a-800.jpg'),
      srcset: 'a-400.jpg 400w, a-800.jpg 800w',
      sizes: '50vw',
      loading: 'lazy',
      fetchpriority: 'high',
      picture: false,
      source: null,
      file: { width: 800, height: 600 },
      fileFrom: 'probe',
      needed: { width: 800, height: 600 },
      ratio: 1,
      issues: [],
    });
    expect(b?.fileFrom).toBe('natural');
    expect(b?.file).toEqual({ width: 100, height: 100 });
  });

  it('takes the file size from the descriptor when the probe fails', async () => {
    document.body.innerHTML = `<img id="w" src="w.jpg" srcset="w-400.jpg 400w, w-1200.jpg 1200w" sizes="400px">
      <img id="x" src="x.jpg" srcset="x.jpg 1x, x-2.jpg 2x">`;
    const page = await collect(
      { dpr: 1 },
      env({
        lcp: null,
        current: { w: abs('w-1200.jpg'), x: abs('x-2.jpg') },
        natural: { w: { width: 400, height: 300 }, x: { width: 150, height: 100 } },
      }),
    );
    expect(page.images.map((one) => [one.fileFrom, one.file])).toEqual([
      ['descriptor', { width: 1200, height: 900 }],
      ['descriptor', { width: 300, height: 200 }],
    ]);
    expect(page.images[0]?.issues.map((one) => one.rule)).toEqual(['oversized']);
  });

  it('names the source of a picture the image came from', async () => {
    document.body.innerHTML = `<picture>
      <source media="(min-width: 2000px)" srcset="huge.avif 3000w" sizes="100vw" type="image/avif">
      <source srcset="mid.webp 800w, big.webp 1600w" type="image/webp">
      <img id="p" src="fallback.jpg">
    </picture>`;
    const page = await collect({ dpr: 1 }, env({ current: { p: abs('big.webp') } }));
    expect(page.images[0]).toMatchObject({
      picture: true,
      source: { media: '', type: 'image/webp', srcset: 'mid.webp 800w, big.webp 1600w', sizes: '' },
      sourceWithoutSizes: true,
    });
    expect(page.images[0]?.issues.map((one) => `${one.rule}.${one.variant}`)).toContain('sizes-missing.source');
  });

  it('marks the LCP element; without one the largest image in the first viewport', async () => {
    document.body.innerHTML = '<img id="s" src="s.jpg"><img id="l" src="l.jpg"><img id="v" src="v.svg">';
    const told = await collect({ dpr: 1 }, env({ lcp: 's' }));
    expect(told.lcpKnown).toBe(true);
    expect(told.images.map((one) => one.lcp)).toEqual([true, false, false]);
    const guessed = await collect({ dpr: 1 }, env({ rendered: { l: { width: 600, height: 400 } } }));
    expect(guessed.lcpKnown).toBe(false);
    expect(guessed.images.map((one) => one.lcp)).toEqual([false, true, false]);
    expect(guessed.images[2]?.vector).toBe(true);
    const off = await collect({ dpr: 1, checkLcp: false }, env({ lcp: 's' }));
    expect(off.images.every((one) => one.issues.length === 0)).toBe(true);
  });

  it('lists a hidden or pending image without measuring it', async () => {
    document.body.innerHTML = '<img id="h" src="h.jpg"><img id="n" src="n.jpg" loading="lazy">';
    const page = await collect({ dpr: 2 }, env({ hidden: ['h'], natural: { n: { width: 0, height: 0 } } }));
    expect(page.images.map((one) => [one.visible, one.loaded, one.file, one.ratio])).toEqual([
      [false, true, { width: 100, height: 100 }, null],
      [true, false, null, null],
    ]);
  });

  it('lists at most MAX_IMAGES and counts the rest', async () => {
    document.body.innerHTML = Array.from({ length: MAX_IMAGES + 3 }, (_, index) => `<img src="i${index}.jpg">`).join('');
    const page = await collect({ dpr: 1 }, env());
    expect(page.images).toHaveLength(MAX_IMAGES);
    expect(page.more).toBe(3);
  });

  it('takes the page density when the request has none', async () => {
    document.body.innerHTML = '<img id="d" src="d.jpg">';
    expect((await collect({}, env({ dpr: 3 }))).dpr).toBe(3);
    expect((await collect({ dpr: 0 }, env({ dpr: 1.5 }))).dpr).toBe(1.5);
  });
});

describe('nodePath', () => {
  it('writes the path as Lighthouse does: index among the siblings, blank text skipped', () => {
    // Lighthouse's pages have a doctype, the HTML element's elder sibling.
    if (!document.doctype) document.insertBefore(document.implementation.createDocumentType('html', '', ''), document.documentElement);
    document.body.innerHTML = '\n  <p>t</p>\n  text <main>\n <div></div><img id="z" src="z.jpg"></main>';
    const image = document.getElementById('z') as Element;
    // body's children: <p> (0), the text " text " (1), <main> (2); main's: <div> (0), <img> (1).
    expect(nodePath(image)).toBe('1,HTML,1,BODY,2,MAIN,1,IMG');
  });
});

describe('run', () => {
  it('takes a malformed request for the defaults', async () => {
    document.body.innerHTML = '<img id="r" src="r.jpg">';
    const page = await run('nonsense', env());
    expect(page.images).toHaveLength(1);
    expect(page.dpr).toBe(1);
  });
});
