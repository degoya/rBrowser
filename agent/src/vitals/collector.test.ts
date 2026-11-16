import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Agent } from '../agent';
import type { AgentMessage, VitalsData } from '../protocol';
import { loadPage } from '../test-page';
import { installVitals, type ObserverClass, REPORT_MS } from './collector';

/** A PerformanceObserver stand-in: the test fires entries by type. */
function fakeObserver(supported: string[]): {
  Observer: ObserverClass;
  fire: (type: string, entries: object[]) => void;
  observed: () => string[];
} {
  const live = new Map<string, (list: { getEntries(): PerformanceEntry[] }) => void>();
  class Fake {
    static supportedEntryTypes = supported;
    private types: string[] = [];
    constructor(private readonly callback: (list: { getEntries(): PerformanceEntry[] }) => void) {}
    observe(init: PerformanceObserverInit): void {
      if (init.type) {
        this.types.push(init.type);
        live.set(init.type, this.callback);
      }
    }
    disconnect(): void {
      for (const type of this.types) live.delete(type);
    }
  }
  return {
    Observer: Fake,
    fire: (type, entries) =>
      live.get(type)?.({ getEntries: () => entries as unknown as PerformanceEntry[] }),
    observed: () => [...live.keys()].sort(),
  };
}

const ALL = ['largest-contentful-paint', 'layout-shift', 'event', 'first-input', 'paint', 'navigation'];

let reports: VitalsData[] = [];
let stop: (() => void) | null = null;

beforeEach(() => {
  vi.useFakeTimers();
  reports = [];
  window.history.replaceState(null, '', '/home');
  loadPage(
    '<!doctype html><html><head></head><body><div id="banner">ad</div><img id="hero" alt=""><button id="buy">Buy</button></body></html>',
  );
});

afterEach(() => {
  stop?.();
  stop = null;
  vi.useRealTimers();
});

function install(supported = ALL): ReturnType<typeof fakeObserver> {
  const fake = fakeObserver(supported);
  stop = installVitals({ win: window, emit: (data) => reports.push(data), Observer: fake.Observer });
  return fake;
}

describe('installVitals', () => {
  it('observes every supported type and names the rest unsupported', () => {
    const fake = install(['paint', 'navigation']);
    expect(fake.observed()).toEqual(['navigation', 'paint']);
    vi.advanceTimersByTime(REPORT_MS);
    expect(reports).toHaveLength(1);
    expect(reports[0]?.unsupported).toEqual(['lcp', 'cls', 'inp']);
    expect(reports[0]?.segment).toBe(0);
    expect(reports[0]?.soft).toBe(false);
  });

  it('reports LCP with its element, CLS with its sources, FCP and TTFB', () => {
    const fake = install();
    fake.fire('navigation', [{ responseStart: 212.4, activationStart: 0 }]);
    fake.fire('paint', [{ name: 'first-paint', startTime: 300 }, { name: 'first-contentful-paint', startTime: 412.7 }]);
    fake.fire('largest-contentful-paint', [{ startTime: 1300, element: document.getElementById('hero') }]);
    const banner = document.getElementById('banner');
    fake.fire('layout-shift', [{ value: 0.18, startTime: 900, hadRecentInput: false, sources: [{ node: banner }] }]);
    vi.advanceTimersByTime(REPORT_MS);
    const last = reports.at(-1);
    expect(last).toMatchObject({
      ttfb: 212,
      fcp: 413,
      lcp: { value: 1300, element: '#hero' },
      cls: { value: 0.18, sources: ['#banner'] },
      unsupported: [],
    });
  });

  it('reports INP after an interaction with its target', () => {
    const fake = install();
    vi.advanceTimersByTime(REPORT_MS);
    expect(reports.at(-1)?.inp).toBeUndefined();
    fake.fire('event', [{ interactionId: 3, duration: 336, startTime: 2000, target: document.getElementById('buy') }]);
    vi.advanceTimersByTime(REPORT_MS);
    expect(reports.at(-1)?.inp).toEqual({ value: 336, target: '#buy', interactions: 1 });
  });

  it('reports at most once per interval and nothing while nothing changed', () => {
    const fake = install();
    vi.advanceTimersByTime(REPORT_MS);
    const before = reports.length;
    for (let index = 1; index <= 5; index += 1) {
      fake.fire('layout-shift', [{ value: 0.01, startTime: index * 100, hadRecentInput: false }]);
    }
    vi.advanceTimersByTime(REPORT_MS - 1);
    expect(reports).toHaveLength(before);
    vi.advanceTimersByTime(1);
    expect(reports).toHaveLength(before + 1);
    vi.advanceTimersByTime(REPORT_MS * 5);
    expect(reports).toHaveLength(before + 1);
  });

  it('starts a soft segment on a new route: CLS and INP anew, load metrics gone', () => {
    const fake = install();
    fake.fire('paint', [{ name: 'first-contentful-paint', startTime: 400 }]);
    fake.fire('layout-shift', [{ value: 0.3, startTime: 500, hadRecentInput: false }]);
    vi.advanceTimersByTime(REPORT_MS);
    window.history.pushState(null, '', '/cart');
    window.dispatchEvent(new PopStateEvent('popstate'));
    vi.advanceTimersByTime(REPORT_MS);
    const last = reports.at(-1);
    expect(last?.segment).toBe(1);
    expect(last?.soft).toBe(true);
    expect(last?.cls).toEqual({ value: 0, sources: [] });
    expect(last?.fcp).toBeUndefined();
    expect(last?.url).toContain('/cart');
  });

  it('a hash change is no new segment', () => {
    install();
    window.location.hash = '#reviews';
    window.dispatchEvent(new PopStateEvent('popstate'));
    vi.advanceTimersByTime(REPORT_MS);
    expect(reports.at(-1)?.segment).toBe(0);
  });

  it('stops observing on uninstall', () => {
    const fake = install();
    stop?.();
    stop = null;
    expect(fake.observed()).toEqual([]);
  });
});

describe('the agent', () => {
  it('posts the vitals of the top frame over the binding', () => {
    const sent: AgentMessage[] = [];
    const agent = new Agent({ win: window, send: (payload) => sent.push(JSON.parse(payload) as AgentMessage) });
    agent.start();
    vi.advanceTimersByTime(REPORT_MS);
    agent.stop();
    const vitals = sent.find((message) => message.kind === 'vitals');
    expect(vitals).toMatchObject({ kind: 'vitals', v: 1, top: true, segment: 0 });
  });
});
