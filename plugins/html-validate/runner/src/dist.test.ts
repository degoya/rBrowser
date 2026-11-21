// The built files the component embeds are current and work where they run: dist/runner.js is
// one IIFE that validates in a context with only what a view's isolated world has (no Node
// globals), dist/rules.json is html-validate's preset as scripts/build.mjs selects it, and the
// locale has a title for every rule.
// @ts-expect-error node:vm has no types here (the runner is checked against DOM types only).
import { runInNewContext } from 'node:vm';
import * as presets from 'html-validate/presets';
import { version } from 'html-validate/browser';
import { describe, expect, it } from 'vitest';
import locale from '../../locales/en.json';
import invalid from '../../tests/fixtures/invalid.html?raw';
import rulesFile from '../dist/rules.json';
import runner from '../dist/runner.js?raw';
import type { Answer } from './shape';

interface RuleEntry {
  id: string;
  severity: string;
  config: unknown;
  url: string;
}

const rules: RuleEntry[] = rulesFile.rules;

describe('dist/', () => {
  it('bundles the runner as one IIFE without imports', () => {
    expect(runner).toMatch(/^("use strict";)?var rbHtmlValidate=/);
    expect(runner).not.toMatch(/\bimport\s*\(|^import\b/m);
  });

  it('validates in a context without Node globals', async () => {
    // What every window has, the isolated world included.
    const context = {
      console,
      performance,
      setTimeout,
      clearTimeout,
      AbortController,
      URL,
      TextEncoder,
      TextDecoder,
    };
    const script = `${runner}\n;rbHtmlValidate.start(options)`;
    const answer = (await runInNewContext(script, {
      ...context,
      options: {
        engine: 'builtin',
        source: invalid,
        rules: { 'no-dup-id': 'error' },
        maxMessages: 10,
        maxText: 100,
      },
    })) as Answer;
    expect(answer.engine).toBe('builtin');
    expect(answer.messages.map((message) => [message.rule, message.line])).toEqual([
      ['no-dup-id', 10],
    ]);
  });

  it('holds the standard preset with the two document rules and the opt-in rule, without the directive rules', () => {
    expect(rulesFile.htmlValidateVersion).toBe(version);
    const builtin = rules.filter((rule) => !rule.id.startsWith('nu-'));
    const expected = Object.keys(presets.standard.rules ?? {})
      .filter((id) => id !== 'deprecated-rule' && id !== 'no-unused-disable')
      .concat(['missing-doctype', 'no-missing-references', 'no-unknown-attributes'])
      .sort();
    expect(builtin.map((rule) => rule.id)).toEqual(expected);
    // The opt-in rule is off until a project switches it on (RB-094-06).
    expect(builtin.filter((rule) => rule.config === 'off').map((rule) => rule.id)).toEqual([
      'no-unknown-attributes',
    ]);
    for (const rule of builtin) {
      expect(rule.id).toMatch(/^[a-z0-9][a-z0-9-]{0,63}$/);
      expect(['error', 'warning']).toContain(rule.severity);
      expect(rule.url).toBe(`https://html-validate.org/rules/${rule.id}.html`);
    }
    expect(rules.filter((rule) => rule.id.startsWith('nu-')).map((rule) => rule.id)).toEqual([
      'nu-error',
      'nu-warning',
      'nu-info',
    ]);
  });

  it('gives every rule its title in the plugin locale', () => {
    const titles: Record<string, string> = locale.plugin['html-validate'].rules;
    expect(Object.keys(titles).sort()).toEqual(rules.map((rule) => rule.id).sort());
    for (const title of Object.values(titles)) expect(title.length).toBeGreaterThan(0);
  });
});
