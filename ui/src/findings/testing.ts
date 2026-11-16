// Test helpers of the findings panel: a run of three findings over three views, and a channel
// that records requests and pushes events.

import type { CheckJob, ChecksProgress, FindingsChannel, FindingsEvent, FindingsRequest, FindingsRun, ProgressEvent } from './types';

export function sampleRun(): FindingsRun {
  return {
    version: 1,
    runId: 'run-0000000000001',
    startedAt: 1,
    target: 'all',
    plugins: ['seo', 'a11y-axe'],
    views: [
      { id: 1, breakpoint: 'mobile' },
      { id: 2, breakpoint: 'tablet' },
      { id: 3, breakpoint: 'desktop' },
    ],
    findings: [
      {
        id: 'a',
        ruleId: 'seo.images',
        plugins: ['seo', 'a11y-axe'],
        severity: 'error',
        message: { key: 'plugin.seo.findings.alt', params: { count: '2' } },
        views: [1, 2, 3],
        breakpoints: ['mobile', 'tablet', 'desktop'],
        element: { selector: 'img.hero', path: 'html>body>img' },
        helpUrl: 'https://help.test/alt',
        sharedId: 'img-alt-missing',
      },
      {
        id: 'b',
        ruleId: 'seo.images',
        plugins: ['seo'],
        severity: 'warning',
        message: { key: 'plugin.seo.findings.alt', params: { count: '1' } },
        views: [1, 3],
        breakpoints: ['mobile', 'desktop'],
        element: { selector: 'img.logo' },
      },
      {
        id: 'c',
        ruleId: 'seo.title',
        plugins: ['seo'],
        severity: 'info',
        message: { key: 'plugin.seo.findings.title' },
        views: [2],
        breakpoints: ['tablet'],
        element: { path: 'html>head>title' },
      },
    ],
    summary: { total: 3, error: 1, warning: 1, info: 1, byPlugin: { seo: 3, 'a11y-axe': 1 } },
  };
}

export const sampleMessages: Record<string, string> = {
  'plugin.seo.findings.alt': '{count} images without alt text',
  'plugin.seo.findings.title': 'The title is short',
};

/** A run of two plugins over two views, halfway: seo through, a11y-axe running in view 1. */
export function sampleProgress(overrides: Partial<ChecksProgress> = {}): ProgressEvent {
  return {
    type: 'checksProgress',
    runId: 'run-0000000000002',
    elapsedMs: 12_400,
    jobs: [
      { plugin: 'seo', view: 1, breakpoint: 'mobile', state: 'done', findings: 1, elapsedMs: 900 },
      { plugin: 'seo', view: 2, breakpoint: 'tablet', state: 'done', findings: 0, elapsedMs: 800 },
      { plugin: 'a11y-axe', view: 1, breakpoint: 'mobile', state: 'running' },
      { plugin: 'a11y-axe', view: 2, breakpoint: 'tablet', state: 'queued' },
    ],
    finished: false,
    ...overrides,
  };
}

/** The owner's grid (RB-080-98): nine views, `mobile` a WebKit view. */
export const GRID_VIEWS = ['mobile', 'md', 'lg', 'xl', 'laptop', 'w1600', 'w1920', 'w2000', 'desktop'];
/** Eleven check plugins, in plan order. */
export const GRID_PLUGINS = [
  'error-report',
  'fonts',
  'lighthouse',
  'outline',
  'pwa-check',
  'seo',
  'visual-regression',
  'a11y-axe',
  'console',
  'coverage',
  'links',
];

const SKIP_ENGINE: Partial<CheckJob> = {
  state: 'skipped',
  reason: 'checks.skip.engine',
  reasonParams: { engine: 'WebKit' },
};

/** How each plugin stands in `breakpoint` halfway through a run over the grid. */
function gridJob(plugin: string, breakpoint: string, index: number): Partial<CheckJob> {
  switch (plugin) {
    case 'error-report':
      return breakpoint === 'mobile' ? { state: 'running' } : { state: 'done', findings: 0 };
    case 'fonts':
      if (breakpoint === 'mobile') return SKIP_ENGINE;
      return breakpoint === 'w2000' ? { state: 'running' } : { state: 'done', findings: 1 };
    case 'lighthouse':
      if (breakpoint === 'mobile') return SKIP_ENGINE;
      if (breakpoint === 'laptop' || breakpoint === 'w1600') return { state: 'running' };
      return index < 4 ? { state: 'done', findings: 20 + index } : { state: 'queued' };
    case 'a11y-axe':
      return breakpoint === 'mobile'
        ? { state: 'failed', reason: 'plugins.error.timed_out' }
        : { state: 'done', findings: 10 };
    case 'console':
      return { state: 'done', findings: 0 };
    case 'coverage':
      return breakpoint === 'mobile' ? SKIP_ENGINE : { state: 'done', findings: 17 * index };
    default:
      return { state: 'queued' };
  }
}

/** Eleven plugins over nine views, as on the owner's screen: three running, five waiting. */
export function gridProgress(overrides: Partial<ChecksProgress> = {}): ProgressEvent {
  const jobs: CheckJob[] = GRID_PLUGINS.flatMap((plugin) =>
    GRID_VIEWS.map((breakpoint, index) => ({
      plugin,
      view: index + 1,
      breakpoint,
      state: 'queued' as const,
      ...gridJob(plugin, breakpoint, index),
    })),
  );
  return {
    type: 'checksProgress',
    runId: 'run-0000000000003',
    elapsedMs: 35_000,
    estimateMs: 61_000,
    jobs,
    finished: false,
    ...overrides,
  };
}

export function fakeChannel() {
  const listeners = new Set<(event: FindingsEvent | ProgressEvent) => void>();
  const sent: FindingsRequest[] = [];
  const channel: FindingsChannel = {
    async request(message) {
      sent.push(message);
      return { type: 'ack' };
    },
    on(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    async last() {
      return null;
    },
    async progress() {
      return null;
    },
  };
  const emit = (event: FindingsEvent | ProgressEvent) => listeners.forEach((listener) => listener(event));
  return { channel, sent, emit, listeners };
}
