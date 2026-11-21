import { describe, expect, it } from 'vitest';
import { breadcrumbText, devicesShown, estimateMeasure, faviconNote, fit, isSerp, safeFavicon, type Favicon, type Measure, type Serp } from './serp';

/** Every character 10 px wide at any size: the cut is easy to follow. */
const ten: Measure = (text) => [...text].length * 10;

const SERP: Serp = {
  title: { text: 'Red shoes', source: 'title' },
  description: null,
  siteName: { text: 'shop.test', source: 'domain' },
  url: 'https://shop.test/p/',
  breadcrumb: { host: 'shop.test', segments: ['shoes', 'red'] },
  noindex: false,
  limits: {
    confidence: 'unverified',
    checked: '2026-11-20',
    source: 'https://developers.google.com/search/docs/appearance/title-link',
    desktop: { width: 600, titleFontPx: 20, titleLines: 1, descriptionFontPx: 14, descriptionLines: 2 },
    mobile: { width: 328, titleFontPx: 20, titleLines: 2, descriptionFontPx: 14, descriptionLines: 3 },
  },
  favicon: null,
};

describe('fit', () => {
  it('keeps a text that fits whole and says how wide it is', () => {
    const fitted = fit('Red shoes', 100, 1, 20, ten);
    expect(fitted).toMatchObject({ lines: ['Red shoes'], text: 'Red shoes', truncated: false, chars: 9, widthPx: 90, limitPx: 100 });
  });

  it('wraps at word ends onto the lines it may use', () => {
    expect(fit('aaa bbb ccc', 70, 2, 14, ten).lines).toEqual(['aaa bbb', 'ccc']);
  });

  it('cuts at a word end with an ellipsis when the lines run out', () => {
    const fitted = fit('aaa bbb ccc ddd', 70, 1, 20, ten);
    expect(fitted.truncated).toBe(true);
    expect(fitted.lines).toEqual(['aaa…']);
    expect(fitted.text).toBe('aaa…');
    expect(fitted.full).toBe('aaa bbb ccc ddd');
    expect(fitted.shownChars).toBe(3);
  });

  it('cuts the last of several lines and keeps the ones before', () => {
    const fitted = fit('aaa bbb ccc ddd eee', 70, 2, 14, ten);
    expect(fitted.lines).toEqual(['aaa bbb', 'ccc…']);
  });

  it('breaks a word longer than a line by characters', () => {
    expect(fit('abcdefghij', 50, 2, 14, ten).lines).toEqual(['abcde', 'fghij']);
    expect(fit('abcdefghijkl', 50, 2, 14, ten).lines).toEqual(['abcde', 'fghi…']);
  });

  it('collapses white space and counts characters, not UTF-16 units', () => {
    const fitted = fit('  a \n b  \u{1F600} ', 200, 1, 14, ten);
    expect(fitted.full).toBe('a b \u{1F600}');
    expect(fitted.chars).toBe(5);
  });

  it('passes the font size to the measure', () => {
    const sizes: number[] = [];
    fit('a b', 100, 1, 17, (text, px) => {
      sizes.push(px);
      return text.length;
    });
    expect(new Set(sizes)).toEqual(new Set([17]));
  });
});

describe('the estimate without a canvas', () => {
  it('grows with the text and the font size', () => {
    expect(estimateMeasure('abcd', 20)).toBeGreaterThan(estimateMeasure('abc', 20));
    expect(estimateMeasure('abc', 20)).toBeGreaterThan(estimateMeasure('abc', 14));
  });
});

describe('the breadcrumb', () => {
  it('joins the host and the segments with the separator of the search', () => {
    expect(breadcrumbText(SERP.breadcrumb)).toBe('shop.test › shoes › red');
    expect(breadcrumbText({ host: 'shop.test', segments: [] })).toBe('shop.test');
  });
});

describe('the setting serpPreview', () => {
  it('shows both devices by default and on a value it does not know', () => {
    expect(devicesShown(undefined)).toEqual(['desktop', 'mobile']);
    expect(devicesShown('both')).toEqual(['desktop', 'mobile']);
    expect(devicesShown(42)).toEqual(['desktop', 'mobile']);
  });

  it('shows one device or none', () => {
    expect(devicesShown('desktop')).toEqual(['desktop']);
    expect(devicesShown('mobile')).toEqual(['mobile']);
    expect(devicesShown('off')).toEqual([]);
  });
});

describe('the answer', () => {
  it('reads a whole serp and refuses a broken one', () => {
    expect(isSerp(SERP)).toBe(true);
    expect(isSerp({ ...SERP, title: { text: 1 } })).toBe(false);
    expect(isSerp({ ...SERP, limits: { ...SERP.limits, desktop: { width: '600' } } })).toBe(false);
    expect(isSerp({ ...SERP, breadcrumb: { host: 'a', segments: [1] } })).toBe(false);
    expect(isSerp(null)).toBe(false);
  });

  it('takes only a raster or ICO data: URI as a favicon', () => {
    const favicon = (dataUri: string | null) => ({ url: 'u', source: 'default', status: 200, error: null, format: null, width: null, height: null, bytes: 1, dataUri, tooLarge: false });
    expect(safeFavicon(favicon('data:image/x-icon;base64,AAAB'))).toBe('data:image/x-icon;base64,AAAB');
    expect(safeFavicon(favicon('data:image/png;base64,iVBO'))).toBe('data:image/png;base64,iVBO');
    expect(safeFavicon(favicon('data:image/svg+xml;base64,PHN2Zz4='))).toBeNull();
    expect(safeFavicon(favicon('https://shop.test/favicon.ico'))).toBeNull();
    expect(safeFavicon(null)).toBeNull();
  });
});

describe('the favicon note', () => {
  const t = (key: string, params?: Record<string, unknown>): string => `${key}${params ? ` ${JSON.stringify(params)}` : ''}`;
  const favicon = (over: Partial<Favicon>): Favicon => ({ url: 'u', source: 'default', status: 200, error: null, format: null, width: null, height: null, bytes: 10, dataUri: null, tooLarge: false, ...over });

  it('is empty while the favicon shows', () => {
    expect(faviconNote(t, favicon({ dataUri: 'data:image/png;base64,iVBO' }))).toBe('');
  });

  it('names what kept it away', () => {
    expect(faviconNote(t, null)).toBe('panel.serp.favicon_none');
    expect(faviconNote(t, favicon({ status: 404 }))).toBe('panel.serp.favicon_failed {"reason":404}');
    expect(faviconNote(t, favicon({ status: null, error: 'plugins.http.denied' }), () => 'Refused.')).toBe('panel.serp.favicon_failed {"reason":"Refused."}');
    expect(faviconNote(t, favicon({ tooLarge: true, bytes: 200000 }))).toBe('panel.serp.favicon_large {"bytes":200000}');
    expect(faviconNote(t, favicon({}))).toBe('panel.serp.favicon_format');
  });
});
