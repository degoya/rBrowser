// html-validate's browser build (what dist/runner.js bundles) on the fixture pages: the
// duplicate id and the misnested elements of invalid.html are messages with line and column,
// valid.html has none, a rule set to off stays silent, and the bounds hold.
import { describe, expect, it } from 'vitest';
import acceptance from '../../tests/fixtures/acceptance.html?raw';
import invalid from '../../tests/fixtures/invalid.html?raw';
import valid from '../../tests/fixtures/valid.html?raw';
import rulesFile from '../dist/rules.json';
import { type BuiltinOptions, extractAt, validateBuiltin } from './builtin';

const RULES: BuiltinOptions['rules'] = {
  'no-dup-id': 'error',
  'element-permitted-content': 'error',
  'close-order': 'error',
  'doctype-html': 'error',
  'missing-doctype': 'error',
  'no-raw-characters': ['error', { relaxed: true }],
};

function options(source: string, rules = RULES): BuiltinOptions {
  return { source, rules, maxMessages: 100, maxText: 200 };
}

describe('validateBuiltin', () => {
  it('finds the duplicate id with its line and column', async () => {
    const answer = await validateBuiltin(options(invalid));
    expect(answer.engine).toBe('builtin');
    expect(answer.version).toMatch(/^\d+\.\d+\.\d+/);
    const dup = answer.messages.filter((message) => message.rule === 'no-dup-id');
    expect(dup).toHaveLength(1);
    expect(dup[0]).toMatchObject({ severity: 'error', line: 10 });
    expect(dup[0]?.column).toBeGreaterThan(0);
    expect(dup[0]?.message).toContain('intro');
    expect(dup[0]?.extract).toBe('<p id="intro">Second</p>');
  });

  it('finds the misnested elements', async () => {
    const answer = await validateBuiltin(options(invalid));
    const lines = answer.messages
      .filter((message) => message.rule === 'element-permitted-content')
      .map((message) => message.line);
    expect(lines).toContain(12);
    const rules = new Set(answer.messages.map((message) => message.rule));
    // `<p><div>` closes the paragraph early: either html-validate's content or its close rule.
    expect(
      answer.messages.some(
        (message) =>
          message.line === 11 &&
          (message.rule === 'close-order' || message.rule === 'element-permitted-content'),
      ),
    ).toBe(true);
    for (const rule of rules) expect(Object.keys(RULES)).toContain(rule);
  });

  it('has nothing to say about a valid page', async () => {
    const answer = await validateBuiltin(options(valid));
    expect(answer.messages).toEqual([]);
    expect(answer.truncated).toBe(false);
  });

  it('runs only the rules it is given', async () => {
    const answer = await validateBuiltin(options(invalid, { 'element-permitted-content': 'error' }));
    expect(answer.messages.every((message) => message.rule === 'element-permitted-content')).toBe(
      true,
    );
  });

  it('reports a rule set to warn as a warning', async () => {
    const answer = await validateBuiltin(options(invalid, { 'no-dup-id': 'warn' }));
    expect(answer.messages.map((message) => message.severity)).toEqual(['warning']);
  });

  it('keeps to maxMessages and says so', async () => {
    const answer = await validateBuiltin({ ...options(invalid), maxMessages: 1 });
    expect(answer.messages).toHaveLength(1);
    expect(answer.truncated).toBe(true);
  });

  it('takes a rule with its options', async () => {
    const answer = await validateBuiltin(
      options('<!DOCTYPE html><title>a</title><p>a & b</p>', {
        'no-raw-characters': ['error', { relaxed: true }],
      }),
    );
    expect(answer.messages).toEqual([]);
    const strict = await validateBuiltin(
      options('<!DOCTYPE html><title>a</title><p>a & b</p>', { 'no-raw-characters': 'error' }),
    );
    expect(strict.messages.map((message) => message.rule)).toEqual(['no-raw-characters']);
  });

  it('ignores a rule html-validate does not know instead of failing', async () => {
    const answer = await validateBuiltin(
      options(invalid, { 'no-dup-id': 'error', 'no-such-rule': 'error' }),
    );
    expect(answer.messages.map((message) => message.rule)).toEqual(['no-dup-id']);
  });
});

// RB-094-06's acceptance: a duplicate id, a `<div>` in a `<p>` and an unknown attribute are
// exactly three messages with the plugin's rule table and `no-unknown-attributes` switched on (as
// a project's `.rbrowser/htmlvalidate.json` does: it is off by default, see scripts/build.mjs);
// the valid page has none; and the offline engine sends no request at all.
describe('the acceptance fixture', () => {
  const table = Object.fromEntries(
    rulesFile.rules
      .filter((rule) => !rule.id.startsWith('nu-'))
      .map((rule) => [rule.id, rule.config]),
  ) as BuiltinOptions['rules'];

  it('has three messages with unknown attributes switched on', async () => {
    const answer = await validateBuiltin(
      options(acceptance, { ...table, 'no-unknown-attributes': 'error' }),
    );
    expect(answer.messages.map((message) => [message.rule, message.line])).toEqual([
      ['no-dup-id', 10],
      ['close-order', 11],
      ['no-unknown-attributes', 12],
    ]);
  });

  it('leaves unknown attributes out by default', async () => {
    const answer = await validateBuiltin(options(acceptance, table));
    expect(answer.messages.map((message) => message.rule)).toEqual(['no-dup-id', 'close-order']);
  });

  it('has nothing to say about the valid page with every rule on', async () => {
    const answer = await validateBuiltin(
      options(valid, { ...table, 'no-unknown-attributes': 'error' }),
    );
    expect(answer.messages).toEqual([]);
  });

  it('gives the stray end tag the selector of the element of its line', async () => {
    const answer = await validateBuiltin(options(acceptance, table));
    const stray = answer.messages.find((message) => message.rule === 'close-order');
    expect(stray?.selector).toBe('html > body > main > div');
  });

  it('sends no request', async () => {
    const calls: unknown[] = [];
    const saved = globalThis.fetch;
    globalThis.fetch = (async (...args: unknown[]) => {
      calls.push(args);
      throw new Error('no request expected');
    }) as typeof fetch;
    try {
      await validateBuiltin(options(acceptance, { ...table, 'no-unknown-attributes': 'error' }));
    } finally {
      globalThis.fetch = saved;
    }
    expect(calls).toEqual([]);
  });
});

describe('extractAt', () => {
  it('takes the whole line the message is on', () => {
    expect(extractAt('one\n<p id="a">two\r\nthree', 7, 200)).toBe('<p id="a">two');
    expect(extractAt('only', 0, 200)).toBe('only');
  });

  it('cuts a long line to a window around the place', () => {
    const line = `${'a'.repeat(100)}<p id="x">${'b'.repeat(100)}`;
    const extract = extractAt(line, 100, 30);
    expect(extract.startsWith('…')).toBe(true);
    expect(extract.endsWith('…')).toBe(true);
    expect(extract).toContain('<p id="x">');
    expect(extract.length).toBeLessThanOrEqual(30);
  });
});
