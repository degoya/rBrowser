// The page-script prologue of the host (crates/rb-plugin-host/src/bridge/prologue.js) in a
// context of its own, as the plugin's isolated world is: RB-080-66 sends what a plugin's script
// logs - `rbPlugin.log` and the world's `console` - to rBrowser over the binding (`l:`), never
// into the page's console.
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { runInNewContext } from 'node:vm';
import { describe, expect, it } from 'vitest';
import { describeLayers, type LayersImpl } from './layers-spec';

const PROLOGUE = readFileSync(resolve(import.meta.dirname, '../../../crates/rb-plugin-host/src/bridge/prologue.js'), 'utf8');

interface World {
  calls: string[];
  printed: unknown[][];
  rbPlugin: { log(level: string, ...values: unknown[]): void };
  console: Record<string, (...values: unknown[]) => void>;
}

function world(timeout = 30000, scripts = '', id = 'demo'): World {
  const calls: string[] = [];
  const printed: unknown[][] = [];
  const console = Object.fromEntries(
    ['error', 'warn', 'info', 'log', 'debug'].map((name) => [name, (...values: unknown[]) => printed.push([name, ...values])]),
  );
  const context: Record<string, unknown> = {
    console,
    document: { readyState: 'complete', addEventListener() {} },
    setTimeout,
    clearTimeout,
    TextEncoder,
    __bind: (payload: string) => calls.push(payload),
  };
  context.globalThis = context;
  const config = JSON.stringify({ id, view: 1, binding: '__bind', receiver: '__recv', max: 65536, timeout });
  runInNewContext(`(() => {\n"use strict";\nconst C = ${config};\n${PROLOGUE}\n${scripts}\n})();`, context);
  return { calls, printed, rbPlugin: context.rbPlugin as World['rbPlugin'], console };
}

function lines(calls: string[]): unknown[] {
  return calls.filter((call) => call.startsWith('l:')).map((call) => JSON.parse(call.slice(2)));
}

describe('the prologue', () => {
  it("sends rbPlugin.log to rBrowser with the error's message", () => {
    const { calls, printed, rbPlugin } = world();
    rbPlugin.log('warning', new Error('plugins.error.busy'), { n: 1 });
    rbPlugin.log('warn', 'x');
    rbPlugin.log('shout', 'y');
    expect(lines(calls)).toEqual([
      { level: 'warning', values: ['plugins.error.busy', '{"n":1}'] },
      { level: 'warning', values: ['x'] },
      { level: 'log', values: ['y'] },
    ]);
    expect(printed).toEqual([]);
  });

  it("sends the world's console to rBrowser instead of the page's console", () => {
    const { calls, printed, console } = world();
    console.warn?.('[demo]', 'plugins.error.timed_out');
    console.error?.('boom');
    expect(lines(calls)).toEqual([
      { level: 'warning', values: ['[demo]', 'plugins.error.timed_out'] },
      { level: 'error', values: ['boom'] },
    ]);
    expect(printed).toEqual([]);
  });

  it('rejects a request without an answer as no_answer, not as a stopped plugin', async () => {
    const { rbPlugin } = world(5);
    const send = (rbPlugin as unknown as { send(message: unknown): Promise<unknown> }).send({ op: 'x' });
    await expect(send).rejects.toThrow('plugins.error.no_answer');
  });

  it('keeps a line within its bounds', () => {
    const { calls, rbPlugin } = world();
    rbPlugin.log('info', ...Array.from({ length: 20 }, () => 'z'.repeat(2000)));
    const [line] = lines(calls) as Array<{ values: string[] }>;
    expect(line?.values).toHaveLength(16);
    expect(line?.values[0]).toHaveLength(1000);
  });

  // Review of RB-080-66, 1: a cut through a surrogate pair left a lone half; the host's JSON
  // refuses it and the whole line was lost.
  it('cuts a text on a character, never through one, and mends lone halves', () => {
    const { calls, rbPlugin } = world();
    rbPlugin.log('info', 'x'.repeat(999) + '\u{1F600}', 'a\uD800b');
    const [line] = lines(calls) as Array<{ values: string[] }>;
    expect(line?.values[0]).toBe('x'.repeat(999));
    expect(line?.values[1]).toBe('a\uFFFDb');
    // No lone surrogate half anywhere.
    const lone = /[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/;
    expect(line?.values.some((value) => lone.test(value))).toBe(false);
  });

  // Review of RB-080-66, 4: a page script that throws is rBrowser's failure, with its stack.
  it("reports a page script's throw with its stack", () => {
    const { calls, printed } = world(30000, 'run("document-start", function (rbPlugin) { throw new Error("boom"); });');
    const [line] = lines(calls) as Array<{ level: string; values: string[]; stack?: string }>;
    expect(line?.level).toBe('error');
    expect(line?.values).toEqual(['[demo]', 'boom']);
    expect(line?.stack).toContain('boom');
    expect(printed).toEqual([]);
  });
});


// RB-090-51: `rbPlugin.layers` behaves as web-common's layers.ts, which the bundled page scripts
// use.
describeLayers('rbPlugin.layers', (owner) => (world(30000, '', owner).rbPlugin as unknown as { layers: LayersImpl }).layers);
