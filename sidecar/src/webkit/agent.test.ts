import { createContext, runInContext } from 'node:vm';

import { describe, expect, it } from 'vitest';

import { MAX_REPORT_BYTES, agentScript, randomNames, reportPayload, runAgentCommand } from './agent.ts';

/** A stand-in for agent/dist/rb-agent.js: boots like agent/src/boot.ts does. */
const BUNDLE = `
(() => {
  const scope = globalThis;
  const emit = scope.__rbEmit;
  const config = scope.__rbAgentBoot;
  delete scope.__rbAgentBoot;
  Object.defineProperty(scope, '__rbAgent', {
    value: Object.freeze({ command: (input) => ({ status: 'done', echo: input, config }) }),
    enumerable: false,
  });
  emit(JSON.stringify({ kind: 'ready' }));
})();
`;

const NAMES = { binding: '__rbBINDING', agent: '__rbAGENT' };

function page() {
  const reports: string[] = [];
  const window: Record<string, unknown> = { [NAMES.binding]: (payload: string) => reports.push(payload) };
  const context = createContext(window);
  return { reports, window, run: (script: string) => runInContext(script, context) };
}

describe('agentScript', () => {
  it('boots the agent in a private scope and reports through the binding', () => {
    const { reports, window, run } = page();
    run(agentScript(BUNDLE, { active: true }, NAMES));
    expect(reports).toEqual(['{"kind":"ready"}']);
    for (const name of ['__rbEmit', '__rbAgentBoot', '__rbAgent']) {
      expect(name in window).toBe(false);
    }
    const handle = window[NAMES.agent] as { command(input: unknown): unknown };
    expect(handle.command({ cmd: 'ping' })).toEqual({ status: 'done', echo: { cmd: 'ping' }, config: { active: true } });
    expect(Object.keys(window)).not.toContain(NAMES.agent);
  });

  it('does nothing without the binding', () => {
    const { window, run } = page();
    delete window[NAMES.binding];
    run(agentScript(BUNDLE, null, NAMES));
    expect(window[NAMES.agent]).toBeUndefined();
  });

  it('keeps a hostile config inside its JSON literal', () => {
    const { reports, run } = page();
    run(agentScript(BUNDLE, { label: '"}; globalThis.pwned = 1; ({"' }, NAMES));
    expect(reports).toHaveLength(1);
    expect(run('globalThis.pwned')).toBeUndefined();
  });
});

describe('runAgentCommand', () => {
  it('answers an error when the page has no agent', () => {
    expect(runAgentCommand(['__rbMissing', { cmd: 'ping' }])).toEqual({ status: 'error', message: 'no agent in this page' });
  });
});

describe('randomNames', () => {
  it('makes two different names of 16 random bytes', () => {
    let next = 0;
    const names = randomNames((bytes) => Uint8Array.from({ length: bytes }, () => next++));
    expect(names.binding).toBe(`__rb${Buffer.from(Array.from({ length: 16 }, (_, i) => i)).toString('hex')}`);
    expect(names.agent).not.toBe(names.binding);
  });
});

describe('reportPayload', () => {
  it('passes strings within the limit only', () => {
    expect(reportPayload('{}')).toBe('{}');
    expect(reportPayload({ kind: 'ready' })).toBeNull();
    expect(reportPayload('x'.repeat(MAX_REPORT_BYTES + 1))).toBeNull();
  });
});
