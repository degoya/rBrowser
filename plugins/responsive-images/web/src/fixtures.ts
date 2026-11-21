// The acceptance fixture of RB-093-02, as the collector reads it in three breakpoints: a page with
// four problems - an image twice as large as needed, one too small, a srcset with widths and no
// sizes, a lazy LCP image - and one correctly sized image. Each breakpoint's view has its own
// density, and the browser picked each file for it, so the sizes differ per view while the
// problems stay. fixture.test.ts writes the measured answers the Rust fixture test reads
// (tests/fixtures/<breakpoint>.json).

import type { ImageFacts, Size } from './types';

export interface Breakpoint {
  id: string;
  width: number;
  height: number;
  dpr: number;
}

export const BREAKPOINTS: Breakpoint[] = [
  { id: 'mobile', width: 390, height: 844, dpr: 3 },
  { id: 'tablet', width: 768, height: 1024, dpr: 2 },
  { id: 'desktop', width: 1440, height: 900, dpr: 1 },
];

export const PAGE_URL = 'https://example.test/';

/** An image of `rendered` CSS pixels whose file has `scale` times the pixels the density needs. */
export function image(selector: string, rendered: Size, dpr: number, scale: number, more: Partial<ImageFacts> = {}): ImageFacts {
  const file = { width: Math.round(rendered.width * dpr * scale), height: Math.round(rendered.height * dpr * scale) };
  const name = selector.replace(/[^a-z0-9]+/gi, '-').replace(/^-+|-+$/g, '');
  return {
    selector,
    path: '',
    src: `${PAGE_URL}img/${name}.jpg`,
    currentSrc: `${PAGE_URL}img/${name}-${file.width}.jpg`,
    srcset: '',
    sizes: '',
    picture: false,
    source: null,
    sourceWithoutSizes: false,
    loading: '',
    fetchpriority: '',
    rendered,
    natural: { width: file.width / dpr, height: file.height / dpr },
    file,
    fileFrom: 'probe',
    vector: false,
    loaded: true,
    visible: true,
    lcp: false,
    ...more,
  };
}

/** The fixture page's images as `breakpoint` shows them. */
export function fixtureImages(breakpoint: Breakpoint): ImageFacts[] {
  const { width, dpr } = breakpoint;
  const column = Math.min(width - 32, 600);
  return [
    image('#hero', { width, height: Math.round(width / 2) }, dpr, 1, {
      path: '1,HTML,1,BODY,0,MAIN,0,IMG',
      srcset: 'hero-800.jpg 800w, hero-1600.jpg 1600w, hero-3200.jpg 3200w',
      sizes: '100vw',
      loading: 'lazy',
      fetchpriority: 'high',
      lcp: true,
    }),
    image('#large', { width: 300, height: 200 }, dpr, 2, { path: '1,HTML,1,BODY,0,MAIN,1,IMG' }),
    image('#small', { width: column, height: Math.round(column * 0.75) }, dpr, 0.5, { path: '1,HTML,1,BODY,0,MAIN,2,IMG' }),
    image('#no-sizes', { width: 320, height: 240 }, dpr, 1, {
      path: '1,HTML,1,BODY,0,MAIN,3,IMG',
      srcset: 'card-320.jpg 320w, card-640.jpg 640w, card-960.jpg 960w',
    }),
    image('#right', { width: 200, height: 200 }, dpr, 1, {
      path: '1,HTML,1,BODY,0,MAIN,4,IMG',
      srcset: 'logo-200.jpg 200w, logo-400.jpg 400w, logo-600.jpg 600w',
      sizes: '200px',
      loading: 'lazy',
    }),
  ];
}
