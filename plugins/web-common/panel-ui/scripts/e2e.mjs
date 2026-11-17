// The Nuxt UI panels in a real headless Chromium, the way rBrowser runs them (RB-080-83): each
// built panel in an `<iframe sandbox="allow-scripts">` of a stand-in panels window, its files
// served with the panel CSP as a header (with `sandbox allow-scripts`) and as the document's first
// element, exactly as crates/rbrowser/src/plugins/panels.rs serves them. Every request outside the
// panel's files is aborted and counted. The stand-in answers `settings` with the plugin's English
// texts and the theme, and every `call` with a canned answer. A panel passes when it draws its
// title, no request left the frame, no console message and no page error came, a tooltip opens,
// and dark switches the colours. Opt-in, not part of `npm test`: `npm run test:e2e` (Playwright's
// Chromium headless shell must be installed: `npx playwright-core install chromium-headless-shell`).
// With RB_PANEL_SHOTS=<folder> it saves each panel light and dark as `<plugin>-<theme>.png` there.
// Beside the two below, every `e2e-panels/<plugin>.mjs` adds a panel (RB-080-87): its default
// export is a spec as here - `title`, `tooltip` (an element with a tooltip), `answer(call)` and an
// optional `check(frame, expect)`. With RB_PANELS=<id>,<id> only those panels run.

import { chromium } from 'playwright-core';
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { dirname, join, normalize, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const PLUGINS = resolve(here, '..', '..', '..');
// crates/rbrowser/src/plugins/panels.rs PANEL_CSP; a test there holds the header form.
const CSP =
  "default-src 'none'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; " +
  "font-src 'self' data:; media-src 'self' data:; connect-src 'none'; frame-src 'none'; worker-src 'none'; " +
  "form-action 'none'; base-uri 'none'";
const TYPES = { html: 'text/html', js: 'text/javascript', css: 'text/css', txt: 'text/plain' };
// A real web font, sent to the Fonts panel part by part as the tool `preview` sends it.
const INTER = readFileSync(join(PLUGINS, '..', 'tests', 'fixtures', 'site', 'fonts', 'inter-400.woff2'));
const PART = 40 * 1024;
const INTER_SRC = 'https://site.test/fonts/inter-400.woff2';
const view = (id, label, engine = 'chromium') => ({ id, label, width: id * 400, engine });

/** The canned answers of each panel's calls. */
const PANELS = {
  lighthouse: {
    title: 'Lighthouse',
    tooltip: '#cancel',
    answer: (call) =>
      call.tool === 'status'
        ? {
            sidecar: { available: true, running: null, queued: 0 },
            latest: {
              runId: 'r1',
              url: 'https://example.test/',
              categories: ['performance', 'seo'],
              breakpoints: [
                { breakpoint: 'mobile', view: 1, width: 375, height: 812, formFactor: 'mobile', url: 'https://example.test/', scores: { performance: 48, seo: 92 }, report: '.rbrowser/out/lighthouse/r1/mobile.html', json: '', findings: 2 },
              ],
              previous: null,
            },
          }
        : {},
  },
  fonts: {
    title: 'Fonts',
    tooltip: '#refresh',
    answer: (call) => {
      if (call.tool === 'preview') {
        const offset = call.args.offset ?? 0;
        const data = INTER.subarray(offset, offset + PART).toString('base64');
        return { src: call.args.src, offset, total: INTER.length, data, done: offset + PART >= INTER.length };
      }
      if (call.tool !== 'list') return {};
      const usage = [{ view: 1, elements: 12, weights: [400], styles: ['normal'], sizes: [16] }];
      return {
        views: [view(1, 'md'), view(2, 'mobile', 'firefox')],
        families: [
          {
            family: 'Inter', provider: 'own', host: 'site.test', webFont: true, formats: ['woff2'], variable: false, fileKb: Math.round(INTER.length / 1024),
            faces: [{ weight: '400', style: 'normal', display: 'swap', unicodeRange: '', src: INTER_SRC, format: 'woff2', kb: 1, preloaded: false, status: [{ view: 1, state: 'loaded' }] }],
            usage, rendered: [],
          },
          { family: 'ui-monospace', provider: 'generic', host: '', webFont: false, formats: [], variable: false, fileKb: null, faces: [], usage, rendered: [] },
        ],
        roles: [{ role: 'h1', views: [{ view: 1, width: 400, min: 32, max: 32 }], growth: null, scaled: null, jump: false }],
        findings: [],
      };
    },
    /** The web font came through the tool as bytes and loaded as a FontFace in the frame. */
    check: async (frame) => {
      await frame.locator('#families [role="option"]:first-child [data-preview]').waitFor({ timeout: 5000 });
      const loaded = await frame.locator('body').evaluate(() => [...document.fonts].filter((face) => face.status === 'loaded').map((face) => face.family));
      expect('fonts: the web font loads from its bytes, no request', loaded.includes('rb-preview-1'), loaded);
    },
  },
};

const more = join(here, 'e2e-panels');
for (const file of existsSync(more) ? readdirSync(more).filter((name) => name.endsWith('.mjs')).sort() : []) {
  PANELS[file.slice(0, -'.mjs'.length)] = (await import(pathToFileURL(join(more, file)).href)).default;
}
const only = process.env.RB_PANELS?.split(',').filter(Boolean);

function flatten(value, prefix, into) {
  if (typeof value === 'string') into[prefix] = value;
  else if (value && typeof value === 'object') for (const [key, child] of Object.entries(value)) flatten(child, prefix ? `${prefix}.${key}` : key, into);
  return into;
}

const parent = (plugin) => `<!doctype html><html><body style="margin:0">
<iframe id="panel" sandbox="allow-scripts" style="width:900px;height:700px;border:0" src="http://plugin.test/${plugin}/panel/index.html"></iframe>
<script>
  window.posted = [];
  const frame = document.getElementById('panel');
  window.send = (message) => frame.contentWindow.postMessage(message, '*');
  addEventListener('message', (event) => {
    if (event.source !== frame.contentWindow) return;
    window.posted.push(event.data);
    window.onPanelMessage(event.data);
  });
</script></body></html>`;

const failures = [];
function expect(what, ok, actual) {
  if (!ok) failures.push(`${what}: got ${JSON.stringify(actual)}`);
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${what}`);
}

const browser = await chromium.launch();
for (const [plugin, spec] of Object.entries(PANELS)) {
  if (only && !only.includes(plugin)) continue;
  const strings = flatten(JSON.parse(readFileSync(join(PLUGINS, plugin, 'locales', 'en.json'), 'utf8')), '', {});
  const context = await browser.newContext();
  const blocked = [];
  await context.route('**/*', async (route) => {
    const url = new URL(route.request().url());
    if (url.host === 'shell.test') return route.fulfill({ contentType: 'text/html', body: parent(plugin) });
    const path = normalize(decodeURIComponent(url.pathname)).replace(/^\/+/, '');
    if (url.host === 'plugin.test' && path.startsWith(`${plugin}/panel/`)) {
      let body = readFileSync(join(PLUGINS, path));
      const type = TYPES[path.split('.').pop()] ?? 'application/octet-stream';
      if (type === 'text/html') body = `<meta http-equiv="Content-Security-Policy" content="${CSP}">\n${body}`;
      return route.fulfill({ contentType: type, body, headers: { 'Content-Security-Policy': `${CSP}; sandbox allow-scripts` } });
    }
    blocked.push(url.href);
    return route.abort();
  });
  const page = await context.newPage();
  const logs = [];
  page.on('console', (message) => logs.push(`${message.type()}: ${message.text()}`));
  page.on('pageerror', (error) => logs.push(`pageerror: ${error.message}`));
  const settings = (theme) => ({ type: 'settings', strings, settings: { 'ui.theme': theme } });
  await page.exposeFunction('onPanelMessage', async (message) => {
    if (message?.type === 'settings') await page.evaluate((m) => window.send(m), settings('light'));
    else if (message?.type === 'call') {
      await page.evaluate((m) => window.send(m), { type: 'result', id: message.id, ok: true, value: spec.answer(message) });
    }
  });
  const started = Date.now();
  await page.goto('http://shell.test/');
  const frame = page.frameLocator('#panel');
  try {
    await frame.locator('#title').filter({ hasText: spec.title }).waitFor({ timeout: 15_000 });
    console.log(`     ${plugin}: drawn ${Date.now() - started} ms after the window's navigation`);
    expect(`${plugin}: draws its title`, true);
    await frame.locator(spec.tooltip).hover();
    const name = await frame.locator(spec.tooltip).getAttribute('aria-label');
    await frame.getByRole('tooltip').filter({ hasText: name ?? '' }).first().waitFor({ timeout: 3000 });
    expect(`${plugin}: a tooltip opens in the frame`, true);
    const background = () => frame.locator('body').evaluate((body) => getComputedStyle(body).backgroundColor);
    const light = await background();
    const shots = process.env.RB_PANEL_SHOTS;
    if (shots) await page.screenshot({ path: join(shots, `${plugin}-light.png`) });
    await page.evaluate((m) => window.send(m), settings('dark'));
    await frame.locator('html.dark').waitFor({ timeout: 3000 });
    const dark = await background();
    if (shots) await page.screenshot({ path: join(shots, `${plugin}-dark.png`) });
    expect(`${plugin}: dark switches the colours`, light !== dark, { light, dark });
    if (spec.check) await spec.check(frame, expect);
  } catch (error) {
    expect(`${plugin}: runs`, false, String(error));
  }
  const origin = await frame.locator('body').evaluate(() => origin).catch(() => '?');
  expect(`${plugin}: runs with an opaque origin`, origin === 'null', origin);
  expect(`${plugin}: no request leaves the frame`, blocked.length === 0, blocked);
  expect(`${plugin}: no console message or page error`, logs.length === 0, logs);
  await context.close();
}
await browser.close();

if (failures.length > 0) {
  console.error(`\n${failures.length} failed:\n${failures.join('\n')}`);
  process.exit(1);
}
console.log('\nall panels passed');
