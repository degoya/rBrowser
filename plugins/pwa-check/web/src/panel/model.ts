// The PWA panel without its DOM (RB-080-08; its components on Nuxt UI since RB-080-85): the shapes of the `check` and `icon` answers
// (src/tool.rs, src/audit/), the texts, and the rows and cards each section shows. Texts are
// catalogue messages (plurals included) and a failure is a sentence, through the shared texts.js
// (RB-080-90).

// The shared texts.js is the panel's prelude (panel-ui); imported here too for the tests.
import '../../../../web-common/panel/texts.js';

export type Strings = Record<string, string>;
export type Severity = 'error' | 'warning' | 'info';
export type Section = 'findings' | 'manifest' | 'icons' | 'sw' | 'install';

export const SECTIONS: Section[] = ['findings', 'manifest', 'icons', 'sw', 'install'];
const PREFIX = 'plugin.pwa-check.';

export interface Finding {
  rule: string;
  severity: Severity;
  message: string;
  params: Record<string, string>;
  view: number | null;
  url: string | null;
  path: string | null;
  data?: { panel?: string; section?: string; anchor?: string } | null;
}

export interface IconReport {
  index: number;
  src: string;
  url: string | null;
  sizes: string;
  type: string | null;
  purpose: string[];
  state: string;
  status: number | null;
  format: string | null;
  width: number | null;
  height: number | null;
  bytes: number | null;
}

export interface ManifestReport {
  url: string;
  status: number | null;
  contentType: string | null;
  error: string | null;
  table: [string, string][];
  startUrl: string | null;
  scope: string | null;
  startUrlStatus: string | null;
}

export interface WorkerReport {
  secure: boolean;
  supported: boolean;
  registered: boolean;
  active: boolean;
  controlled: boolean;
  scope: string | null;
  script: string | null;
  state: string | null;
  fetchHandler: boolean | null;
}

export interface PageReport {
  view: number;
  views: number[];
  url: string;
  manifest: ManifestReport | null;
  icons: IconReport[];
  serviceWorker: WorkerReport;
  offline: { tested: boolean; url: string | null; cached: boolean | null };
  installability: { checked: boolean; errors: { id: string; arguments: [string, string][] }[]; failure: string | null };
  themeColor: boolean;
  appleTouchIcon: string | null;
}

export interface Answer {
  pages: PageReport[];
  findings: Finding[];
  summary: Record<Severity, number>;
  skipped: { view: number; reason: string }[];
  cut: boolean;
}

export interface IconPreview {
  dataUri: string | null;
  tooLarge: boolean;
  bytes: number;
}

/** The placeholders and plurals of `template` filled from `params`. */
export function format(template: string, params: Record<string, unknown> = {}): string {
  return PanelTexts.format(template, params);
}

/** The plugin's text `plugin.pwa-check.<key>`, its key when the catalogue lacks it. */
export function text(strings: Strings, key: string, params: Record<string, unknown> = {}): string {
  return format(strings[PREFIX + key] ?? key, params);
}

/** A failed call as one sentence: its code's text (the plugin's or the host's) and its reason's,
 * never the code (RB-080-90). */
export function errorText(strings: Strings, code: string, params: Record<string, string> = {}): string {
  return PanelTexts.errorText(strings, code, params);
}

/** A finding's message in English, its rule id when the catalogue lacks it. */
export function findingText(strings: Strings, finding: Finding): string {
  const template = strings[finding.message];
  return template === undefined ? finding.rule : format(template, finding.params);
}

/** Where a finding's place is in the panel: the anchor its `data` names, else its section. */
export function anchorOf(finding: Finding): string {
  return finding.data?.anchor ?? finding.data?.section ?? 'findings';
}

/** The tab that shows a finding's place: the section its `data` names (RB-080-85). */
export function sectionOf(finding: Finding): Section {
  const section = finding.data?.section;
  return (SECTIONS as string[]).includes(section ?? '') ? (section as Section) : 'findings';
}

/** The findings of `page` (those its view reported), errors first. */
export function findingsOf(answer: Answer, page: PageReport): Finding[] {
  const order: Record<Severity, number> = { error: 0, warning: 1, info: 2 };
  return answer.findings
    .filter((finding) => finding.view === page.view)
    .sort((a, b) => order[a.severity] - order[b.severity]);
}

function yesNo(strings: Strings, value: boolean | null): string {
  if (value === null) return text(strings, 'panel.unknown');
  return text(strings, value ? 'panel.yes' : 'panel.no');
}

/** The manifest's head rows (URL, status, type, start URL, scope) before its member table. */
export function manifestRows(strings: Strings, manifest: ManifestReport): [string, string][] {
  const rows: [string, string | null][] = [
    ['url', manifest.url],
    ['status', manifest.status === null ? null : String(manifest.status)],
    ['contentType', manifest.contentType],
    ['error', manifest.error],
    ['startUrl', manifest.startUrl],
    ['startUrlStatus', manifest.startUrlStatus],
    ['scope', manifest.scope],
  ];
  return rows
    .filter((row): row is [string, string] => row[1] !== null && row[1] !== '')
    .map(([key, value]) => [text(strings, `panel.manifest.${key}`), value]);
}

/** The service worker's rows. */
export function workerRows(strings: Strings, page: PageReport): [string, string][] {
  const sw = page.serviceWorker;
  const rows: [string, string][] = [
    [text(strings, 'panel.sw.secure'), yesNo(strings, sw.secure)],
    [text(strings, 'panel.sw.registered'), yesNo(strings, sw.registered)],
    [text(strings, 'panel.sw.active'), yesNo(strings, sw.active)],
    [text(strings, 'panel.sw.controlled'), yesNo(strings, sw.controlled)],
  ];
  if (sw.state) rows.push([text(strings, 'panel.sw.state'), sw.state]);
  if (sw.scope) rows.push([text(strings, 'panel.sw.scope'), sw.scope]);
  if (sw.script) rows.push([text(strings, 'panel.sw.script'), sw.script]);
  if (sw.registered) rows.push([text(strings, 'panel.sw.fetchHandler'), yesNo(strings, sw.fetchHandler)]);
  if (page.offline.tested) rows.push([text(strings, 'panel.sw.offline'), yesNo(strings, page.offline.cached)]);
  rows.push([text(strings, 'panel.meta.themeColor'), yesNo(strings, page.themeColor)]);
  rows.push([text(strings, 'panel.meta.appleTouchIcon'), page.appleTouchIcon ?? text(strings, 'panel.no')]);
  return rows;
}

/** One icon card: its anchor, its lines of text, its load state as a badge's text and tone,
 * whether it is maskable and may be previewed. */
export interface IconCard {
  id: string;
  src: string;
  lines: string[];
  state: { text: string; tone: 'ok' | 'error' | 'neutral' };
  maskable: boolean;
  previewUrl: string | null;
  ok: boolean;
}

export function iconCard(strings: Strings, icon: IconReport, preview?: IconPreview): IconCard {
  const lines = [
    text(strings, 'panel.icons.declared', { sizes: icon.sizes || '-', type: icon.type ?? '' }).trim(),
  ];
  if (icon.format) {
    lines.push(
      icon.width === null
        ? text(strings, 'panel.icons.scalable', { format: icon.format })
        : text(strings, 'panel.icons.actual', { width: icon.width, height: icon.height, format: icon.format }),
    );
  }
  lines.push(text(strings, 'panel.icons.purpose', { purpose: icon.purpose.join(' ') }));
  const state = text(strings, `panel.icons.state.${icon.state}`) + (icon.status === null ? '' : ` (${icon.status})`);
  if (preview?.tooLarge) lines.push(text(strings, 'panel.icons.tooLarge', { bytes: preview.bytes }));
  const ok = icon.state === 'ok';
  const broken = icon.state === 'unreachable' || icon.state === 'not_image';
  return {
    id: `icon-${icon.index}`,
    src: icon.src,
    lines,
    state: { text: state, tone: ok ? 'ok' : broken ? 'error' : 'neutral' },
    maskable: icon.purpose.includes('maskable'),
    previewUrl: ok ? icon.url : null,
    ok,
  };
}

/** The icon URLs to ask `icon` for: loaded ones, each once. */
export function previewUrls(page: PageReport): string[] {
  return [...new Set(page.icons.filter((icon) => icon.state === 'ok' && icon.url).map((icon) => icon.url as string))];
}

/** The installability section's lines. */
export function installLines(strings: Strings, page: PageReport): string[] {
  const verdict = page.installability;
  if (!verdict.checked) {
    return [text(strings, 'panel.install.notChecked', { reason: verdict.failure ? errorText(strings, verdict.failure) : '-' })];
  }
  if (verdict.errors.length === 0) return [text(strings, 'panel.install.none')];
  return verdict.errors.map((error) =>
    [error.id, ...error.arguments.map(([name, value]) => `${name}: ${value}`)].join(' - '),
  );
}

/** The summary line of the answer. */
export function summaryText(strings: Strings, answer: Answer): string {
  return text(strings, 'panel.summary', answer.summary);
}
