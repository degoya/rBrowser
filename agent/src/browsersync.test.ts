import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import browsersyncPage from '../../tests/fixtures/site/browsersync.html?raw';
import { Agent } from './agent';
import { hasBrowsersync, watchBrowsersync } from './browsersync';
import type { AgentMessage } from './protocol';
import { loadPage } from './test-page';

/** A performance stand-in with the given resource URLs. */
function perf(...urls: string[]): Pick<Performance, 'getEntriesByType'> {
  return {
    getEntriesByType: () => urls.map((name) => ({ name }) as PerformanceEntry),
  };
}

beforeEach(() => {
  loadPage('<!doctype html><html><head></head><body><main>plain</main></body></html>');
});

describe('hasBrowsersync', () => {
  it('finds the snippet Browsersync injects', () => {
    loadPage(browsersyncPage);
    expect(hasBrowsersync(document)).toBe(true);
  });

  it('finds the client script by its path', () => {
    const script = document.createElement('script');
    script.src = 'http://localhost:3000/browser-sync/browser-sync-client.js?v=3.0.3';
    document.body.append(script);
    expect(hasBrowsersync(document)).toBe(true);
  });

  it('finds a request to its socket', () => {
    expect(hasBrowsersync(document, perf('http://localhost:3000/app.js'))).toBe(false);
    expect(
      hasBrowsersync(document, perf('http://localhost:3000/browser-sync/socket.io/?EIO=4')),
    ).toBe(true);
  });

  it('finds nothing in a plain page, nor in a script that only names it', () => {
    const script = document.createElement('script');
    script.textContent = '// we do not use browser-sync here';
    document.body.append(script);
    expect(hasBrowsersync(document, perf())).toBe(false);
  });

  it('survives a performance object that throws', () => {
    const broken = {
      getEntriesByType: () => {
        throw new Error('no');
      },
    };
    expect(hasBrowsersync(document, broken)).toBe(false);
  });
});

describe('watchBrowsersync', () => {
  it('reports at once in a parsed document, and once only', () => {
    loadPage(browsersyncPage);
    let found = 0;
    const stop = watchBrowsersync(window, () => (found += 1));
    window.dispatchEvent(new Event('load'));
    expect(found).toBe(1);
    stop();
  });

  it('reports when the client arrives with the load', () => {
    let found = 0;
    const stop = watchBrowsersync(window, () => (found += 1));
    expect(found).toBe(0);
    loadPage(browsersyncPage);
    window.dispatchEvent(new Event('load'));
    window.dispatchEvent(new Event('load'));
    expect(found).toBe(1);
    stop();
  });

  it('stops listening when stopped', () => {
    let found = 0;
    const stop = watchBrowsersync(window, () => (found += 1));
    stop();
    loadPage(browsersyncPage);
    window.dispatchEvent(new Event('load'));
    expect(found).toBe(0);
  });
});

describe('the agent', () => {
  let agent: Agent | null = null;

  afterEach(() => {
    agent?.stop();
    agent = null;
  });

  it('says so once when Browsersync is in its top frame', () => {
    loadPage(browsersyncPage);
    const sent: AgentMessage[] = [];
    agent = new Agent({ win: window, send: (payload) => sent.push(JSON.parse(payload) as AgentMessage) });
    agent.start();
    window.dispatchEvent(new Event('load'));
    const detected = sent.filter((message) => message.kind === 'detected');
    expect(detected).toEqual([{ kind: 'detected', v: 1, top: true, tool: 'browsersync' }]);
  });

  it('says nothing in a plain page', () => {
    const sent: AgentMessage[] = [];
    agent = new Agent({ win: window, send: (payload) => sent.push(JSON.parse(payload) as AgentMessage) });
    agent.start();
    window.dispatchEvent(new Event('load'));
    expect(sent.map((message) => message.kind)).toEqual(['ready']);
  });
});
