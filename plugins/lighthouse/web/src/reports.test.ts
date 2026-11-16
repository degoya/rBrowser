// The panel's reports (RB-091-02): "Open report" per breakpoint asks the command `open_report`,
// the history lists the runs of the tool `reports` with a delete that asks first, and two runs
// compare through the tool. The panels window is played as in panel.test.ts.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { mountPanel } from '../../../web-common/panel-ui/src/mount';
import App from './panel/App.vue';
import type { HistoryRun } from './panel/history';
import type { LighthouseStatus } from './panel/model';

type Posted = { type?: string; id?: number; tool?: string; command?: string; args?: Record<string, unknown> };

let posted: Posted[] = [];
let unmount: () => void = () => undefined;

const STRINGS: Record<string, string> = {
  'plugin.lighthouse.panel.openReport': 'Open report',
  'plugin.lighthouse.panel.openReportOf': 'Open the report of {breakpoint}',
  'plugin.lighthouse.panel.history.title': 'History',
  'plugin.lighthouse.panel.history.delete': 'Delete the run of {run}',
  'plugin.lighthouse.panel.history.confirm': 'Delete the run of {run} with all its reports?',
  'plugin.lighthouse.panel.history.deleteYes': 'Delete',
  'plugin.lighthouse.panel.history.keep': 'Keep',
  'plugin.lighthouse.panel.history.open': 'Open the report of {breakpoint} from {run}',
  'plugin.lighthouse.panel.reportRefused.report_untrusted': 'Reports open only in a trusted workspace.',
};

const NEW = '20261112-100000';
const OLD = '20261101-090000';

const STATUS: LighthouseStatus = {
  sidecar: { available: true, running: null, queued: 0 },
  latest: {
    runId: NEW,
    url: 'http://x.test/',
    categories: ['performance'],
    breakpoints: [
      { breakpoint: 'mobile', view: 1, width: 375, height: 812, formFactor: 'mobile', url: 'http://x.test/', scores: { performance: 80 }, report: `.rbrowser/out/lighthouse/${NEW}/mobile.html`, json: '', findings: 0 },
    ],
  },
};

const RUNS: HistoryRun[] = [
  { runId: NEW, startedAt: 0, url: null, cancelled: false, breakpoints: [{ breakpoint: 'mobile', scores: { performance: 80 }, report: `${NEW}/mobile.html` }] },
  { runId: OLD, startedAt: 0, url: null, cancelled: false, breakpoints: [{ breakpoint: 'mobile', scores: { performance: 40 }, report: `${OLD}/mobile.html` }] },
];

const $ = (selector: string): HTMLElement | null => document.querySelector<HTMLElement>(selector);
const settle = async (): Promise<void> => {
  await new Promise((resolve) => setTimeout(resolve, 0));
  await nextTick();
};

async function fromHost(data: unknown): Promise<void> {
  window.dispatchEvent(new MessageEvent('message', { data, source: window }));
  await settle();
}

const answered = new Set<number>();

/** Answers every call not answered yet: the status, the history, and `extra` for the rest. */
async function answerCalls(extra: (message: Posted) => unknown = () => ({})): Promise<void> {
  for (const message of posted.filter((m) => m.type === 'call' && !answered.has(m.id ?? -1))) {
    answered.add(message.id ?? -1);
    let value: unknown = extra(message);
    if (message.tool === 'status') value = STATUS;
    else if (message.tool === 'reports' && message.args?.action === 'list') value = { runs: RUNS };
    window.dispatchEvent(new MessageEvent('message', { data: { type: 'result', id: message.id, ok: true, value }, source: window }));
  }
  await settle();
}

const calls = (target: Partial<Posted>): Posted[] =>
  posted.filter((m) => m.type === 'call' && Object.entries(target).every(([key, value]) => m[key as keyof Posted] === value));

beforeEach(async () => {
  posted = [];
  answered.clear();
  document.body.innerHTML = '<div id="app"></div>';
  vi.spyOn(window, 'postMessage').mockImplementation((message: unknown) => {
    posted.push(message as Posted);
  });
  unmount = mountPanel(App, { prefix: 'plugin.lighthouse.', name: 'lighthouse panel' }).unmount;
  await fromHost({ type: 'settings', strings: STRINGS, settings: {} });
  await answerCalls();
  await answerCalls();
});

afterEach(() => {
  unmount();
  vi.restoreAllMocks();
});

describe("the Lighthouse panel's reports", () => {
  it('opens the report of a breakpoint of the latest run through the command', async () => {
    const button = $('#reports [data-open-report]') as HTMLButtonElement;
    expect(button.textContent).toContain('Open report');
    expect(button.getAttribute('aria-label')).toBe('Open the report of mobile');
    button.click();
    expect(calls({ command: 'open_report' }).map((m) => m.args)).toEqual([{ run: NEW, breakpoint: 'mobile' }]);
  });

  it('says why the host did not open a report', async () => {
    ($('#reports [data-open-report]') as HTMLButtonElement).click();
    await answerCalls((m) => (m.command === 'open_report' ? { window: { opened: false, reason: 'report_untrusted' } } : {}));
    expect(document.body.textContent).toContain('Reports open only in a trusted workspace.');
  });

  it('lists the runs on disk, newest first, each report to open', async () => {
    expect(calls({ tool: 'reports' }).some((m) => m.args?.action === 'list')).toBe(true);
    const runs = [...document.querySelectorAll<HTMLElement>('#history [data-run]')];
    expect(runs.map((run) => run.dataset.runId)).toEqual([NEW, OLD]);
    expect(runs[1]?.querySelector('[data-run-label]')?.textContent).toBe('2026-11-01 09:00:00 UTC');
    (runs[1]?.querySelector('[data-open]') as HTMLButtonElement).click();
    expect(calls({ command: 'open_report' }).map((m) => m.args)).toEqual([{ run: OLD, breakpoint: 'mobile' }]);
  });

  it('deletes a run only after it asked, and Keep deletes nothing', async () => {
    const old = (): HTMLElement => $(`#history [data-run-id="${OLD}"]`) as HTMLElement;
    (old().querySelector('[data-delete]') as HTMLButtonElement).click();
    await settle();
    expect(calls({ command: 'delete_run' })).toEqual([]);
    expect(old().querySelector('[data-confirm]')?.textContent).toContain('Delete the run of 2026-11-01 09:00:00 UTC with all its reports?');
    (old().querySelector('[data-confirm-keep]') as HTMLButtonElement).click();
    await settle();
    expect(old().querySelector('[data-confirm]')).toBeNull();
    (old().querySelector('[data-delete]') as HTMLButtonElement).click();
    await settle();
    (old().querySelector('[data-confirm-delete]') as HTMLButtonElement).click();
    expect(calls({ command: 'delete_run' }).map((m) => m.args)).toEqual([{ run: OLD }]);
  });

  it('compares the two newest runs of a breakpoint through the tool', async () => {
    ($('#compare') as HTMLButtonElement).click();
    const compare = calls({ tool: 'reports' }).filter((m) => m.args?.action === 'compare');
    expect(compare.map((m) => m.args)).toEqual([{ action: 'compare', breakpoint: 'mobile', runs: [OLD, NEW] }]);
    await answerCalls((m) =>
      m.args?.action === 'compare'
        ? {
            breakpoint: 'mobile',
            before: OLD,
            after: NEW,
            scores: [{ category: 'performance', before: 40, after: 80, delta: 40 }],
            metrics: [{ metric: 'largest-contentful-paint', unit: 'ms', before: 2400, after: 1800, delta: -600 }],
          }
        : {},
    );
    const score = $('#comparison [data-category="performance"] [data-change]') as HTMLElement;
    expect(score.textContent).toBe('+40');
    expect(score.className).toContain('text-success');
    const metric = $('#comparison [data-metric="largest-contentful-paint"] [data-change]') as HTMLElement;
    expect(metric.textContent).toBe('-600 ms');
    expect(metric.className).toContain('text-success');
  });
});
