// The snippet wrapper (src/wrapper.js) as a view runs it: the value as JSON a person reads -
// cyclic objects marked, DOM nodes described, errors with their stack - and the timeout.
import { afterEach, describe, expect, it, vi } from 'vitest';

import { wrap } from './support';

type Outcome = {
  ok: boolean;
  type: string;
  text: string;
  value?: unknown;
  stack?: string;
  timedOut?: boolean;
  truncated?: boolean;
};

/** Runs `code` as Runtime.evaluate would: the expression's value, awaited. */
async function run(code: string, timeoutMs = 1000, maxValue?: number): Promise<Outcome> {
  return (await (0, eval)(wrap(code, timeoutMs, maxValue))) as Outcome;
}

afterEach(() => {
  vi.useRealTimers();
  document.body.innerHTML = '';
});

describe('the snippet wrapper', () => {
  it('answers a returned value with its type and text', async () => {
    document.title = 'Fixture';
    const outcome = await run('return document.title');
    expect(outcome).toEqual({ ok: true, type: 'string', text: '"Fixture"', value: 'Fixture' });
  });

  it('awaits a promise and reads undefined', async () => {
    expect(await run('await new Promise((r) => setTimeout(r, 5)); return 2 + 3')).toMatchObject({ ok: true, type: 'number', value: 5 });
    expect(await run('const x = 1;')).toMatchObject({ ok: true, type: 'undefined', text: 'undefined', value: null });
  });

  it('marks cyclic objects instead of failing', async () => {
    const outcome = await run('const a = { name: "a" }; a.self = a; a.list = [a]; return a');
    expect(outcome.ok).toBe(true);
    expect(outcome.value).toEqual({ name: 'a', self: '[Circular]', list: ['[Circular]'] });
  });

  it('describes DOM nodes', async () => {
    document.body.innerHTML = '<main id="app" class="page wide"><p>hi</p></main>';
    const outcome = await run('return [document.getElementById("app"), document.querySelector("p").firstChild, document]');
    expect(outcome.value).toEqual(['<main#app.page.wide>', '#text "hi"', '#document']);
    expect((await run('return document.body')).type).toBe('node');
  });

  it('turns maps, sets, dates, bigints and functions into text', async () => {
    const outcome = await run(
      'return { m: new Map([["k", 1]]), s: new Set([1]), d: new Date(0), b: 10n, f: function named() {}, n: NaN }',
    );
    expect(outcome.value).toEqual({
      m: [['k', 1]],
      s: [1],
      d: '1970-01-01T00:00:00.000Z',
      b: '10n',
      f: 'function named()',
      n: 'NaN',
    });
  });

  it('answers a throw with its message and stack', async () => {
    const outcome = await run('throw new TypeError("broken")');
    expect(outcome).toMatchObject({ ok: false, type: 'error', text: 'TypeError: broken', timedOut: false });
    expect(outcome.stack).toContain('broken');
  });

  it('stops waiting at the timeout', async () => {
    vi.useFakeTimers();
    const pending = run('await new Promise(() => {}); return 1', 200);
    await vi.advanceTimersByTimeAsync(250);
    expect(await pending).toMatchObject({ ok: false, timedOut: true, text: 'Error: The snippet did not finish within 200 ms.' });
  });

  it('keeps only the text of a value too big to send', async () => {
    const outcome = await run('return "x".repeat(5000)', 1000, 1000);
    expect(outcome.truncated).toBe(true);
    expect(outcome.value).toBeUndefined();
    expect(outcome.text.length).toBe(2000);
  });

  it('caps long lists and deep objects', async () => {
    const outcome = await run('return { list: Array.from({ length: 150 }, (_, i) => i), deep: { a: { b: { c: { d: { e: { f: { g: 1 } } } } } } } }');
    const value = outcome.value as { list: unknown[]; deep: unknown };
    expect(value.list).toHaveLength(101);
    expect(value.list[100]).toBe('... 50 more');
    expect(JSON.stringify(value.deep)).toContain('[Object]');
  });

  it('does not let a snippet see the helpers', async () => {
    expect(await run('return typeof plain + typeof LIMIT + typeof done')).toMatchObject({ value: 'undefinedundefinedundefined' });
  });
});
