import { describe, expect, it } from 'vitest';
import en from '../../../locales/en.json';
import {
  type Answer,
  anchorOf,
  errorText,
  type Finding,
  findingsOf,
  findingText,
  iconCard,
  type IconReport,
  installLines,
  manifestRows,
  type PageReport,
  previewUrls,
  sectionOf,
  type Strings,
  summaryText,
  text,
  workerRows,
} from './model';

/** The locale flattened to `plugin.pwa-check.*` keys, as the host's `settings` message sends it. */
function flatten(value: unknown, prefix = ''): Strings {
  if (typeof value === 'string') return { [prefix]: value };
  return Object.entries(value as Record<string, unknown>).reduce<Strings>(
    (all, [key, child]) => ({ ...all, ...flatten(child, prefix ? `${prefix}.${key}` : key) }),
    {},
  );
}
const strings = flatten(en);

function icon(overrides: Partial<IconReport>): IconReport {
  return {
    index: 0, src: 'icons/192.png', url: 'http://localhost:8080/pwa-ok/icons/192.png', sizes: '192x192',
    type: 'image/png', purpose: ['any'], state: 'ok', status: 200, format: 'image/png', width: 192, height: 192,
    bytes: 412, ...overrides,
  };
}

function page(overrides: Partial<PageReport> = {}): PageReport {
  return {
    view: 1, views: [1, 2], url: 'http://localhost:8080/pwa-ok/', manifest: null, icons: [],
    serviceWorker: { secure: true, supported: true, registered: true, active: true, controlled: true, scope: null,
      script: 'http://localhost:8080/pwa-ok/sw.js', state: 'activated', fetchHandler: true },
    offline: { tested: true, url: 'http://localhost:8080/pwa-ok/', cached: true },
    installability: { checked: true, errors: [], failure: null },
    themeColor: true, appleTouchIcon: null, ...overrides,
  };
}

const finding = (overrides: Partial<Finding>): Finding => ({
  rule: 'sw.missing', severity: 'error', message: 'plugin.pwa-check.findings.sw.missing', params: {}, view: 1,
  url: null, path: 'pwa:service-worker', data: { panel: 'pwa', section: 'sw', anchor: 'sw' }, ...overrides,
});

describe('texts', () => {
  it('fills placeholders and falls back to the key', () => {
    expect(text(strings, 'panel.views', { views: '1, 2' })).toBe('Views: 1, 2');
    expect(text(strings, 'panel.nope')).toBe('panel.nope');
    expect(errorText(strings, 'plugins.error.busy')).toBe('The plugin could not do that.'); // RB-080-90: never the code
  });

  it('renders a finding message with its parameters', () => {
    const size = finding({ rule: 'manifest.icon_size', message: 'plugin.pwa-check.findings.manifest.icon_size', params: { size: '512' } });
    expect(findingText(strings, size)).toBe('The manifest declares no icon of 512x512 px.');
    expect(findingText(strings, finding({ message: 'plugin.pwa-check.findings.unknown' }))).toBe('sw.missing');
  });
});

describe('findings', () => {
  it('lists the page view findings errors first and knows their place', () => {
    const answer: Answer = {
      pages: [page()], summary: { error: 1, warning: 0, info: 1 }, skipped: [], cut: false,
      findings: [
        finding({ severity: 'info', rule: 'meta.theme_color', data: { section: 'manifest' } }),
        finding({ view: 2 }),
        finding({}),
      ],
    };
    const list = findingsOf(answer, page());
    expect(list.map((f) => f.severity)).toEqual(['error', 'info']);
    expect(anchorOf(list[0] as Finding)).toBe('sw');
    expect(anchorOf(list[1] as Finding)).toBe('manifest');
    expect(anchorOf(finding({ data: null }))).toBe('findings');
    // RB-080-85: a finding opens the tab of its section.
    expect(sectionOf(list[0] as Finding)).toBe('sw');
    expect(sectionOf(finding({ data: { section: 'icons', anchor: 'icon-3' } }))).toBe('icons');
    expect(sectionOf(finding({ data: { section: 'elsewhere' } }))).toBe('findings');
    expect(sectionOf(finding({ data: null }))).toBe('findings');
    expect(summaryText(strings, answer)).toBe('1 error, 0 warnings, 1 note') // RB-080-90: plural forms;
  });
});

describe('sections', () => {
  it('shows the manifest head rows that have a value', () => {
    const rows = manifestRows(strings, {
      url: 'http://x/m.json', status: 200, contentType: null, error: null, table: [], startUrl: 'http://x/',
      scope: 'http://x/', startUrlStatus: '200',
    });
    expect(rows.map(([label]) => label)).toEqual(['URL', 'Status', 'Start URL', 'Start URL answers', 'Scope']);
  });

  it('describes an icon with its declared and actual size and its preview', () => {
    const card = iconCard(strings, icon({ index: 2, purpose: ['maskable'] }));
    expect(card.id).toBe('icon-2');
    expect(card.maskable).toBe(true);
    expect(card.lines).toContain('Actual: 192x192 image/png');
    // RB-080-62: the state is a badge of its own, not a line.
    expect(card.state).toEqual({ text: 'Loaded (200)', tone: 'ok' });
    expect(card.lines).not.toContain('Loaded (200)');
    const broken = iconCard(strings, icon({ state: 'unreachable', status: 404, format: null }));
    expect(broken.previewUrl).toBeNull();
    expect(broken.state).toEqual({ text: 'Did not load (404)', tone: 'error' });
    expect(iconCard(strings, icon({ state: 'other_origin', status: null })).state.tone).toBe('neutral');
    const large = iconCard(strings, icon({}), { dataUri: null, tooLarge: true, bytes: 90000 });
    expect(large.lines.at(-1)).toBe('Larger than the panel can show (90000 bytes).');
  });

  it('asks for each loaded icon once', () => {
    const shared = icon({});
    const urls = previewUrls(page({ icons: [shared, { ...shared, index: 1 }, icon({ index: 2, state: 'unreachable' })] }));
    expect(urls).toEqual(['http://localhost:8080/pwa-ok/icons/192.png']);
  });

  it('shows the service worker and the installability verdict', () => {
    const rows = Object.fromEntries(workerRows(strings, page()));
    expect(rows['Handles fetch']).toBe('yes');
    expect(rows['Start URL cached (offline)']).toBe('yes');
    expect(rows['apple-touch-icon']).toBe('no');
    expect(installLines(strings, page())).toEqual(['Chromium reports no installability error.']);
    const failing = page({ installability: { checked: true, failure: null,
      errors: [{ id: 'no-manifest', arguments: [['a', 'b']] }] } });
    expect(installLines(strings, failing)).toEqual(['no-manifest - a: b']);
    const unasked = page({ installability: { checked: false, errors: [], failure: 'plugins.error.not_available' } });
    expect(installLines(strings, unasked)[0]).toBe('Chromium was not asked: The plugin could not do that.');
  });
});
