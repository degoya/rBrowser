import { describe, expect, it } from 'vitest';

import { FlowSyntaxError, GLOBALS, PREFIX, compileFlow, fileColumn } from './compile.ts';
import { createSteps, flowArguments } from './globals.ts';
import { locate, redact, screenshotName, secretsOf } from './result.ts';

const FILE = '/home/dev/site/.rbrowser/flows/login.ts';
const VIEW = { id: 2, label: 'Mobile', width: 375, height: 812, engine: 'chromium', breakpoint: 'mobile' };

describe('compileFlow', () => {
  it('runs TypeScript with top-level await and the injected globals, no import', async () => {
    const source = [
      'const width: number = view.width;',
      'await step("open", async () => { await page.goto(env + "/login"); });',
      'page.seen = { width, label: view.label, hasExpect: typeof expect };',
    ].join('\n');
    const visited: string[] = [];
    const page: Record<string, unknown> = { goto: async (url: string) => visited.push(url) };
    const steps = createSteps(() => 0);
    const fn = compileFlow(source, FILE);
    await fn(...flowArguments(page, VIEW, () => undefined, 'https://a.test', steps));
    expect(page.seen).toEqual({ width: 375, label: 'Mobile', hasExpect: 'function' });
    expect(visited).toEqual(['https://a.test/login']);
    expect(steps.done).toEqual([{ name: 'open', status: 'passed', durationMs: 0 }]);
  });

  it('names the flow file and its line in a stack', async () => {
    const fn = compileFlow('const a = 1;\n\nthrow new Error(`boom ${a}`);\n', FILE);
    const error = await fn(...GLOBALS.map(() => undefined)).catch((e: unknown) => e as Error);
    expect(error).toBeInstanceOf(Error);
    expect(locate((error as Error).stack ?? '', FILE)).toEqual({ line: 3, column: 7 });
  });

  it('reports a syntax error with its line', () => {
    const lineOf = (source: string): number | null => {
      try {
        compileFlow(source, FILE);
      } catch (error) {
        expect(error).toBeInstanceOf(FlowSyntaxError);
        return (error as FlowSyntaxError).line;
      }
      return -1;
    };
    expect(lineOf('const a: number = 1;\nconst = ;\nconst b = 2;\n')).toBe(2);
    expect(lineOf('const a = 1;\nif (a) {\n  a +;\n}\n')).toBe(3);
  });

  it('refuses imports and import()', async () => {
    expect(() => compileFlow("import fs from 'node:fs';\n", FILE)).toThrow(SyntaxError);
    const fn = compileFlow("await import('node:fs');\n", FILE);
    await expect(fn()).rejects.toMatchObject({ code: 'ERR_VM_DYNAMIC_IMPORT_CALLBACK_MISSING' });
  });

  it('runs in strict mode, so views share no accidental global', async () => {
    const fn = compileFlow('leaked = 1;\n', FILE);
    await expect(fn()).rejects.toBeInstanceOf(ReferenceError);
  });

  it('keeps file columns on the first line', () => {
    expect(fileColumn(1, PREFIX.length + 5)).toBe(5);
    expect(fileColumn(2, 5)).toBe(5);
  });
});

describe('steps', () => {
  it('names the failing step by its error, and runs it no longer', async () => {
    let t = 0;
    const steps = createSteps(() => (t += 10));
    await steps.step('outer', async () => {
      await steps.step('inner', () => 1);
    });
    const error = new Error('x');
    await expect(steps.step('broken', () => Promise.reject(error))).rejects.toThrow('x');
    expect(steps.stepOf(error)).toBe('broken');
    // RB-090-76: a step whose error the flow caught is not running.
    expect(steps.current()).toBeNull();
    expect(steps.done.map((s) => [s.name, s.status])).toEqual([
      ['inner', 'passed'],
      ['outer', 'passed'],
      ['broken', 'failed'],
    ]);
  });

  it('gives the flow a frozen view', () => {
    const args = flowArguments({}, VIEW, null, null, createSteps(() => 0));
    expect(Object.isFrozen(args[1])).toBe(true);
  });
});

describe('result helpers', () => {
  it('finds a Windows path with spaces in a stack', () => {
    const file = 'C:\\My Site\\.rbrowser\\flows\\a.ts';
    const stack = `Error: x\n    at Object.toBe (C:\\sidecar\\expect.js:1:1)\n    at ${file}:12:5\n`;
    expect(locate(stack, file)).toEqual({ line: 12, column: 5 });
    expect(locate('Error: x', file)).toBeNull();
  });

  it('redacts the token of the endpoint, also without its Bearer prefix', () => {
    const token = 'a1'.repeat(32);
    const secrets = secretsOf({
      views: [{ ...VIEW, endpoint: { url: 'http://127.0.0.1:1', headers: { Authorization: `Bearer ${token}` } } }],
    });
    expect(redact(`connect failed: ${token}`, secrets)).toBe('connect failed: <redacted>');
  });

  it('names screenshots by label and id', () => {
    expect(screenshotName({ id: 3, label: 'Desktop XL (1440)' })).toBe('desktop-xl-1440-3.png');
    expect(screenshotName({ id: 4, label: '...' })).toBe('view-4.png');
  });
});
