import { beforeEach, describe, expect, it } from 'vitest';
import { Agent } from './agent';
import { watchFrameworks } from './framework';
import { isFrameworkReport } from './framework/report';
import type { AgentMessage } from './protocol';
import { FRAMEWORK_EVENT } from './protocol';
import { loadPage } from './test-page';

function emit(detail: unknown): void {
  document.dispatchEvent(new CustomEvent(FRAMEWORK_EVENT, { detail }));
}

beforeEach(() => {
  loadPage('<!doctype html><html><head></head><body><main>plain</main></body></html>');
});

describe('isFrameworkReport', () => {
  it('accepts an empty report and complete entries', () => {
    expect(isFrameworkReport({})).toBe(true);
    expect(isFrameworkReport({ vue: { hook: true, devtools: true, version: '3.5.0' } })).toBe(true);
    expect(isFrameworkReport({ react: { hook: false, devtools: false } })).toBe(true);
  });

  it('refuses anything else', () => {
    expect(isFrameworkReport(null)).toBe(false);
    expect(isFrameworkReport([])).toBe(false);
    expect(isFrameworkReport({ vue: { hook: 'yes', devtools: true } })).toBe(false);
    expect(isFrameworkReport({ react: { hook: true } })).toBe(false);
    expect(isFrameworkReport({ vue: { hook: true, devtools: true, version: 5 } })).toBe(false);
    expect(isFrameworkReport({ svelte: { hook: true, devtools: true } })).toBe(false);
    expect(isFrameworkReport({ vue: { hook: true, devtools: true, version: 'x'.repeat(65) } })).toBe(false);
  });
});

describe('watchFrameworks', () => {
  it('hands every valid report over and ignores the rest', () => {
    const seen: unknown[] = [];
    const stop = watchFrameworks(window, (report) => seen.push(report));
    emit(JSON.stringify({ vue: { hook: true, devtools: true } }));
    emit('not json');
    emit({ vue: { hook: true, devtools: true } });
    emit(JSON.stringify({ react: { hook: 1, devtools: true } }));
    emit(JSON.stringify({}));
    expect(seen).toEqual([{ vue: { hook: true, devtools: true } }, {}]);
    stop();
    emit(JSON.stringify({ react: { hook: false, devtools: false } }));
    expect(seen).toHaveLength(2);
  });

  it('is reported by the agent of the top frame', () => {
    const sent: AgentMessage[] = [];
    const agent = new Agent({ win: window, send: (payload) => sent.push(JSON.parse(payload)) });
    agent.start();
    emit(JSON.stringify({ react: { version: '19.3.1', hook: true, devtools: true } }));
    const report = sent.find((message) => message.kind === 'framework');
    expect(report).toEqual({
      kind: 'framework', v: 1, top: true, react: { version: '19.3.1', hook: true, devtools: true },
    });
    agent.stop();
    emit(JSON.stringify({}));
    expect(sent.filter((message) => message.kind === 'framework')).toHaveLength(1);
  });
});
