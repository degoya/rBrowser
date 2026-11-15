import { describe, expect, it } from 'vitest';
import { HOST_TAG, Viewer } from './viewer';

const SITEMAP =
  '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">' +
  '<url><loc>https://a.test/</loc><lastmod>2026-11-01</lastmod></url>' +
  '<url><loc>https://a.test/de</loc></url>' +
  '</urlset>';

const bridge = {
  send: () => Promise.resolve({ config: { expandDepth: 2 } }),
  on: () => () => undefined,
  onStop: () => () => undefined,
};

/** A view of an XML document: the document parsed, the window's other members borrowed. */
function xmlWindow(text: string): Window {
  const doc = new DOMParser().parseFromString(text, 'application/xml');
  return new Proxy(window, {
    get: (target, name) => (name === 'document' ? doc : Reflect.get(target, name)),
  });
}

function rows(viewer: Viewer): string[] {
  return [...(viewer.shadow?.querySelectorAll('.row:not([hidden])') ?? [])].map((row) => row.textContent ?? '');
}

describe('the viewer on an XML document', () => {
  it('shows the elements as a tree and filters with XPath', async () => {
    const win = xmlWindow(SITEMAP);
    const viewer = new Viewer(win, bridge, { contentType: 'application/xml', firefox: false });
    await viewer.start();
    expect(win.document.documentElement.lastElementChild?.localName).toBe(HOST_TAG);
    expect(rows(viewer)).toEqual([
      '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">',
      '<url>',
      '<loc>https://a.test/</loc>',
      '<lastmod>2026-11-01</lastmod>',
      '<url>',
      '<loc>https://a.test/de</loc>',
    ]);
    const filter = viewer.shadow?.querySelector('input') as HTMLInputElement;
    filter.value = '//url/loc';
    viewer.runFilter();
    expect(rows(viewer)).toEqual([
      '/urlset/url[1]/loc<loc>https://a.test/</loc>',
      '/urlset/url[2]/loc<loc>https://a.test/de</loc>',
    ]);
    viewer.setMode('pretty');
    expect(viewer.shadow?.querySelector('.raw')?.textContent).toContain('\n  <url>\n    <loc>https://a.test/</loc>');
    viewer.stop();
    expect(win.document.querySelector(HOST_TAG)).toBeNull();
  });
});
