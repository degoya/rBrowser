import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { FRAMEWORK_EVENT, HOOK_MARKER } from '../protocol';
import { loadPage } from '../test-page';
import { MAX_SCANNED_ELEMENTS, detectFrameworks, installProbe } from './probe';

type Win = Window & Record<string, unknown>;

const win = (): Win => window as unknown as Win;

beforeEach(() => {
  loadPage('<!doctype html><html><head></head><body><div id="app"><p>plain</p></div></body></html>');
  const scope = win();
  delete scope.__VUE_DEVTOOLS_GLOBAL_HOOK__;
  delete scope.__REACT_DEVTOOLS_GLOBAL_HOOK__;
  delete scope.__VUE__;
});

afterEach(() => {
  vi.useRealTimers();
});

describe('detectFrameworks', () => {
  it('finds nothing in a plain page', () => {
    expect(detectFrameworks(win())).toEqual({});
  });

  it('finds a Vue app by its container and version, without any hook', () => {
    const app = document.getElementById('app') as HTMLElement & { __vue_app__?: unknown };
    app.setAttribute('data-v-app', '');
    app.__vue_app__ = { version: '3.5.22' };
    expect(detectFrameworks(win())).toEqual({ vue: { version: '3.5.22', hook: false, devtools: true } });
  });

  it('tells a Vue production build without devtools from one with them', () => {
    document.getElementById('app')?.setAttribute('data-v-app', '');
    expect(detectFrameworks(win())).toEqual({ vue: { hook: false, devtools: false } });
  });

  it("reads Vue's apps from rBrowser's hook", () => {
    win().__VUE_DEVTOOLS_GLOBAL_HOOK__ = {
      id: 'vue-devtools-next',
      [HOOK_MARKER]: true,
      apps: [{ version: '3.4.0' }],
    };
    expect(detectFrameworks(win())).toEqual({ vue: { version: '3.4.0', hook: true, devtools: true } });
  });

  it("reports rBrowser's Vue hook without an app as no Vue", () => {
    win().__VUE_DEVTOOLS_GLOBAL_HOOK__ = { id: 'vue-devtools-next', [HOOK_MARKER]: true, apps: [] };
    expect(detectFrameworks(win())).toEqual({});
  });

  it("finds React by a renderer registered in rBrowser's hook", () => {
    win().__REACT_DEVTOOLS_GLOBAL_HOOK__ = {
      [HOOK_MARKER]: true,
      renderers: new Map([[1, { version: '19.3.1', rendererPackageName: 'react-dom' }]]),
    };
    expect(detectFrameworks(win())).toEqual({ react: { version: '19.3.1', hook: true, devtools: true } });
  });

  it("finds React by a renderer registered in another hook (the extension's)", () => {
    win().__REACT_DEVTOOLS_GLOBAL_HOOK__ = { renderers: new Map([[1, { version: '18.3.1' }]]) };
    expect(detectFrameworks(win())).toEqual({ react: { version: '18.3.1', hook: false, devtools: true } });
  });

  it('finds React loaded without any hook by the fiber keys on its elements', () => {
    const p = document.querySelector('p') as unknown as HTMLElement & Record<string, unknown>;
    p.__reactFiber$abc123 = {};
    expect(detectFrameworks(win())).toEqual({ react: { hook: false, devtools: false } });
  });

  it('finds the React root container key too', () => {
    const app = document.getElementById('app') as HTMLElement & Record<string, unknown>;
    app.__reactContainer$x1 = {};
    expect(detectFrameworks(win())).toEqual({ react: { hook: false, devtools: false } });
  });

  it('scans a bounded number of elements only', () => {
    const body = document.body;
    for (let i = 0; i < MAX_SCANNED_ELEMENTS + 5; i += 1) body.append(document.createElement('i'));
    const last = body.lastElementChild as HTMLElement & Record<string, unknown>;
    last.__reactFiber$late = {};
    expect(detectFrameworks(win())).toEqual({});
  });

  it('reports both frameworks of a page', () => {
    document.getElementById('app')?.setAttribute('data-v-app', '');
    win().__REACT_DEVTOOLS_GLOBAL_HOOK__ = { [HOOK_MARKER]: true, renderers: new Map([[1, {}]]) };
    expect(detectFrameworks(win())).toEqual({
      vue: { hook: false, devtools: false },
      react: { hook: true, devtools: true },
    });
  });

  it('survives a hook that throws', () => {
    Object.defineProperty(window, '__REACT_DEVTOOLS_GLOBAL_HOOK__', {
      get: () => {
        throw new Error('no');
      },
      configurable: true,
    });
    expect(detectFrameworks(win())).toEqual({});
  });
});

describe('installProbe', () => {
  /** The reports dispatched on `document`, parsed. */
  function reports(): unknown[] {
    const seen: unknown[] = [];
    document.addEventListener(FRAMEWORK_EVENT, (event) => {
      seen.push(JSON.parse((event as CustomEvent<string>).detail));
    });
    return seen;
  }

  it('reports at once in a complete document, then only when the finding changes', () => {
    vi.useFakeTimers();
    const seen = reports();
    const stop = installProbe(win());
    expect(seen).toEqual([{}]);
    window.dispatchEvent(new Event('load'));
    expect(seen).toEqual([{}]);
    document.getElementById('app')?.setAttribute('data-v-app', '');
    vi.advanceTimersByTime(1000);
    expect(seen).toEqual([{}, { vue: { hook: false, devtools: false } }]);
    vi.advanceTimersByTime(10_000);
    expect(seen).toHaveLength(2);
    stop();
  });

  it('defines no global and stops cleanly', () => {
    vi.useFakeTimers();
    const before = new Set(Object.getOwnPropertyNames(window));
    const seen = reports();
    const stop = installProbe(win());
    stop();
    document.getElementById('app')?.setAttribute('data-v-app', '');
    window.dispatchEvent(new Event('load'));
    vi.advanceTimersByTime(10_000);
    expect(seen).toEqual([{}]);
    const added = Object.getOwnPropertyNames(window).filter((name) => !before.has(name));
    expect(added).toEqual([]);
  });

  it('does nothing in a child frame', () => {
    const frame = { top: {} } as unknown as Win;
    expect(installProbe(frame)).toBeTypeOf('function');
  });
});
