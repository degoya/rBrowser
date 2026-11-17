import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import fixture from '../../../../tests/fixtures/site/fonts/index.html?raw';
import { collect, type Env, type FaceLike, firstFamily, lineHeightRatio, MAX_CANDIDATES, parseSrc, weightOf } from './collect';
import { run } from './collect-entry';
import type { Box } from './types';

const URL = 'http://localhost:8080/fonts/index.html';

/** The statuses `document.fonts` gives the fixture's faces in Chromium. */
const STATUSES: FaceLike[] = [
  { family: 'Inter', weight: '400', style: 'normal', display: 'swap', unicodeRange: 'U+0-10FFFF', status: 'loaded' },
  { family: 'Inter', weight: '600', style: 'normal', display: 'auto', unicodeRange: 'U+0-10FFFF', status: 'loaded' },
  { family: 'Roboto', weight: '400', style: 'normal', display: 'swap', unicodeRange: 'U+0-10FFFF', status: 'loaded' },
  { family: 'Legacy Serif', weight: '400', style: 'normal', display: 'swap', unicodeRange: 'U+0-10FFFF', status: 'error' },
  { family: 'Unused Display', weight: '400', style: 'normal', display: 'swap', unicodeRange: 'U+0-10FFFF', status: 'unloaded' },
  { family: '"Inter ASCII"', weight: '400', style: 'normal', display: 'swap', unicodeRange: 'U+0-7F', status: 'loaded' },
];

/** happy-dom has no layout: every element a 10x10 box in the first screen. */
function env(overrides: Partial<Env> = {}): Env {
  return {
    rect: (): Box => [10, 10, 10, 10],
    viewport: () => ({ width: 375, height: 800 }),
    scrollY: () => 0,
    shown: () => true,
    fontFaces: () => STATUSES,
    resources: () => [
      { name: 'http://localhost:8080/fonts/inter-400.woff2', initiatorType: 'link', encodedBodySize: 17156, transferSize: 17456, decodedBodySize: 17156 },
      { name: 'http://localhost:8080/fonts/index.css', initiatorType: 'link', encodedBodySize: 900, transferSize: 1200, decodedBodySize: 900 },
      { name: 'https://fonts.gstatic.com/s/roboto/v30/KFOmCnqEu92Fr1Mu4mxK.woff2', initiatorType: 'css', encodedBodySize: 0, transferSize: 0, decodedBodySize: 0 },
    ],
    ...overrides,
  };
}

function load(html: string): void {
  const happy = (window as unknown as { happyDOM: { setURL(url: string): void } }).happyDOM;
  happy.setURL(URL);
  document.head.innerHTML =
    `<style>${/<style>([\s\S]*?)<\/style>/.exec(html)?.[1] ?? ''}</style>` +
    (/<link rel="preload"[^>]*>/.exec(html)?.[0] ?? '');
  document.body.innerHTML = /<body>([\s\S]*)<\/body>/.exec(html)?.[1] ?? '';
}

describe('collect against tests/fixtures/site/fonts/', () => {
  beforeEach(() => load(fixture));
  afterEach(() => {
    document.head.innerHTML = '';
    document.body.innerHTML = '';
  });

  it('reads every @font-face with its src, format, display and status', () => {
    const fonts = collect({}, env());
    expect(fonts.faces.map((f) => `${f.family} ${f.weight} ${f.style} ${f.display} ${f.status}`)).toEqual([
      'Inter 400 normal swap loaded',
      'Inter 600 normal auto loaded',
      'Roboto 400 normal swap loaded',
      'Legacy Serif 400 normal swap error',
      'Unused Display 400 normal swap unloaded',
      'Inter ASCII 400 normal swap loaded',
    ]);
    expect(fonts.faces[0]?.src).toEqual([{ url: 'http://localhost:8080/fonts/inter-400.woff2', format: 'woff2', local: false }]);
    expect(fonts.faces[2]?.src[0]?.url).toBe('https://fonts.gstatic.com/s/roboto/v30/KFOmCnqEu92Fr1Mu4mxK.woff2');
    expect(fonts.faces[3]?.src[0]?.format).toBe('truetype');
    expect(fonts.faces[5]?.unicodeRange).toBe('U+0000-007F');
    expect(fonts.faces.every((f) => f.readable && f.sheet === 'inline' && !f.variable)).toBe(true);
  });

  it('counts the text per family stack, weight and style with samples', () => {
    const fonts = collect({}, env());
    const usage = (stack: string, weight: number) => fonts.usages.find((u) => u.stack === stack && u.weight === weight);
    expect(usage('Inter, Arial, sans-serif', 600)?.count).toBe(3);
    expect(usage('Roboto, Arial, sans-serif', 400)?.family).toBe('Roboto');
    expect(usage('"Legacy Serif", Georgia, serif', 400)?.family).toBe('Legacy Serif');
    const subset = usage('"Inter ASCII", Arial, sans-serif', 400);
    expect(subset?.samples[0]?.nonAscii).toBe('\u00f6\u00df\u00dc\u00e4\u00c4');
    expect(subset?.samples[0]?.path).toBe('html > body > main > p:nth-of-type(4)');
    expect(fonts.usages.every((u) => u.aboveFold)).toBe(true);
  });

  it('gives the size range of each text role', () => {
    const roles = collect({}, env()).roles;
    expect(roles.find((r) => r.role === 'h1')).toMatchObject({ count: 1, min: 32, max: 32, first: { path: 'html > body > header > h1' } });
    expect(roles.find((r) => r.role === 'input')).toMatchObject({ count: 1, min: 14, max: 14 });
  });

  it('picks the readability candidates by the request thresholds', () => {
    const fonts = collect({ minTextPx: 12, minInputPx: 16, minLineHeight: 1.2 }, env());
    expect(fonts.smallText.map((c) => `${c.tag} ${c.size}`)).toEqual(['p 11']);
    expect(fonts.smallInputs.map((c) => `${c.tag} ${c.size}`)).toEqual(['input 14']);
    expect(fonts.tightLines.map((c) => `${c.path} ${c.lineHeight}`)).toEqual(['html > body > main > p:nth-of-type(5) 1']);
  });

  it('lists the preloads and only the font files of the loaded resources', () => {
    const fonts = collect({}, env());
    expect(fonts.preloads).toEqual(['http://localhost:8080/fonts/inter-400.woff2']);
    expect(fonts.resources.map((r) => `${r.url} ${r.size}`)).toEqual([
      'http://localhost:8080/fonts/inter-400.woff2 17156',
      'https://fonts.gstatic.com/s/roboto/v30/KFOmCnqEu92Fr1Mu4mxK.woff2 0',
    ]);
  });

  it('skips text that is not shown and the plugins own elements', () => {
    document.body.insertAdjacentHTML('beforeend', '<rbrowser-fonts><p class="fineprint">mark</p></rbrowser-fonts>');
    const hidden = document.querySelector('.lead');
    const fonts = collect({}, env({ shown: (element) => element !== hidden }));
    expect(fonts.usages.some((u) => u.family === 'Roboto')).toBe(false);
    expect(fonts.smallText).toHaveLength(1);
  });

  it('never collects what was typed into a field: placeholder or nothing', () => {
    document.body.insertAdjacentHTML(
      'beforeend',
      '<form>' +
        '<input id="pw" type="password" value="hunter2-secret" placeholder="Password">' +
        '<input id="card" autocomplete="cc-number" value="4111111111111111" placeholder="Card">' +
        '<input id="otp" autocomplete="one-time-code" value="123456" placeholder="Code">' +
        '<input id="name" type="text" value="typed-name" placeholder="Your name">' +
        '<input id="plain" type="email" value="typed@example.com">' +
        '<textarea id="note" placeholder="Note">typed-note</textarea>' +
        '<select id="pick"><option value="opt-secret" selected>chosen-option</option></select>' +
        '</form>',
    );
    (document.querySelector('#plain') as HTMLInputElement).value = 'typed-later@example.com';
    const fonts = collect({ minInputPx: 99 }, env());
    const seen = JSON.stringify(fonts);
    for (const secret of ['hunter2', '4111', '123456', 'typed-', 'opt-secret', 'chosen-option']) {
      expect(seen).not.toContain(secret);
    }
    const inputs = new Map(fonts.smallInputs.map((c) => [c.selector, c.text]));
    expect(inputs.get('#name')).toBe('Your name');
    expect(inputs.get('#note')).toBe('Note');
    expect(inputs.get('#pw')).toBe('');
    expect(inputs.get('#card')).toBe('');
    expect(inputs.get('#otp')).toBe('');
    expect(inputs.get('#plain')).toBe('');
    expect(inputs.get('#pick')).toBe('');
  });

  it('caps the candidates', () => {
    document.body.insertAdjacentHTML('beforeend', '<p class="fineprint">x</p>'.repeat(MAX_CANDIDATES + 5));
    const fonts = collect({}, env());
    expect(fonts.smallText).toHaveLength(MAX_CANDIDATES);
    expect(fonts.truncated).toBe(true);
  });
});

describe('a face only document.fonts knows', () => {
  afterEach(() => {
    document.head.innerHTML = '';
  });

  it('is kept unreadable, attributed to the one stylesheet the page may not read', () => {
    document.head.innerHTML = '';
    const fonts = collect(
      {},
      env({ fontFaces: () => [{ family: 'Roboto', weight: '400', style: 'normal', display: 'auto', unicodeRange: 'U+0-FF', status: 'loaded' }] }),
      ['https://fonts.googleapis.com/css2?family=Roboto'],
    );
    expect(fonts.faces).toEqual([
      {
        family: 'Roboto',
        weight: '400',
        style: 'normal',
        display: 'auto',
        unicodeRange: 'U+0-FF',
        status: 'loaded',
        src: [],
        sheet: 'https://fonts.googleapis.com/css2?family=Roboto',
        readable: false,
        variable: false,
      },
    ]);
  });
});

describe('the helpers', () => {
  it('splits a src list into urls, formats and local fonts', () => {
    expect(parseSrc('local("Inter"), url("a b.woff2") format("woff2"), url(x.ttf)', 'http://h/c/s.css')).toEqual([
      { url: 'Inter', format: '', local: true },
      { url: 'http://h/c/a%20b.woff2', format: 'woff2', local: false },
      { url: 'http://h/c/x.ttf', format: 'truetype', local: false },
    ]);
  });

  it('reads the first family, the weight and the line height', () => {
    expect(firstFamily('"Inter ASCII", Arial')).toBe('Inter ASCII');
    expect(firstFamily("'Roboto'")).toBe('Roboto');
    expect(weightOf('bold')).toBe(700);
    expect(weightOf('normal')).toBe(400);
    expect(weightOf('550')).toBe(550);
    expect(lineHeightRatio('24px', 16)).toBe(1.5);
    expect(lineHeightRatio('normal', 16)).toBe(1.2);
    expect(lineHeightRatio('1.25', 16)).toBe(1.25);
    expect(lineHeightRatio('150%', 16)).toBe(1.5);
  });

  it('run takes a malformed request as the defaults', () => {
    document.body.innerHTML = '<p>text</p>';
    // The real page's geometry: happy-dom has no layout, so nothing counts as shown.
    expect(run('nonsense')).toMatchObject({ url: location.href, faces: [], elements: 0 });
    expect(run({ minTextPx: 'x' }).smallText).toEqual([]);
    document.body.innerHTML = '';
  });
});
