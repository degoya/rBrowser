// The PWA panel in panel-ui's e2e (RB-080-85): a page with a finding, a manifest and two icons,
// one of them loaded as a data: URI through the tool `icon`.
const PNG = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==';
const PAGE = {
  view: 1,
  views: [1, 2],
  url: 'https://example.test/',
  manifest: {
    url: 'https://example.test/manifest.json', status: 200, contentType: 'application/manifest+json', error: null,
    table: [['name', 'Demo'], ['display', 'standalone']], startUrl: '/', scope: '/', startUrlStatus: '200',
  },
  icons: [
    { index: 0, src: 'icons/192.png', url: 'https://example.test/icons/192.png', sizes: '192x192', type: 'image/png', purpose: ['any'], state: 'ok', status: 200, format: 'image/png', width: 192, height: 192, bytes: 70 },
    { index: 1, src: 'icons/512.png', url: null, sizes: '512x512', type: 'image/png', purpose: ['maskable'], state: 'unreachable', status: 404, format: null, width: null, height: null, bytes: null },
  ],
  serviceWorker: { secure: true, supported: true, registered: true, active: true, controlled: true, scope: '/', script: '/sw.js', state: 'activated', fetchHandler: true },
  offline: { tested: true, url: '/', cached: true },
  installability: { checked: true, errors: [], failure: null },
  themeColor: true,
  appleTouchIcon: null,
};

export default {
  title: 'PWA',
  tooltip: '#report',
  answer: (call) => {
    if (call.tool === 'icon') return { dataUri: PNG, tooLarge: false, bytes: 70 };
    if (call.tool !== 'check') return {};
    return {
      pages: [PAGE],
      summary: { error: 1, warning: 0, info: 0 },
      skipped: [],
      cut: false,
      findings: [{ rule: 'manifest.icon_unreachable', severity: 'error', message: 'plugin.pwa-check.findings.unknown', params: {}, view: 1, url: null, path: 'pwa:manifest/icons/1', data: { panel: 'pwa', section: 'icons', anchor: 'icon-1' } }],
    };
  },
  /** A finding opens the Icons tab at its icon; the loaded icon shows from its data: URI. */
  check: async (frame, expect) => {
    await frame.locator('#findings [data-finding]').first().click();
    await frame.locator('#icon-0 img').waitFor({ timeout: 5000 });
    const focused = await frame.locator('body').evaluate(() => document.activeElement?.id ?? '');
    expect('pwa-check: a finding opens its tab at its place', focused === 'icon-1', focused);
  },
};
