// The built files the component embeds are current: dist/axe.min.js is the pinned axe-core byte
// for byte, dist/rules.json and the locale's rule titles are axe's rules (scripts/build.mjs).
import axe from 'axe-core';
import pinned from 'axe-core/axe.min.js?raw';
import { describe, expect, it } from 'vitest';
import locale from '../../locales/en.json';
import bundled from '../dist/axe.min.js?raw';
import rulesFile from '../dist/rules.json';
import runner from '../dist/runner.js?raw';

interface RuleEntry {
  id: string;
  impact: string | null;
  tags: string[];
  help: string;
  helpUrl: string;
}

const rules: RuleEntry[] = rulesFile.rules;

describe('dist/', () => {
  it('holds the pinned axe-core byte for byte', () => {
    expect(bundled.length).toBeGreaterThan(100_000);
    expect(bundled === pinned).toBe(true);
  });

  it('describes every rule of that axe-core', () => {
    expect(rulesFile.axeVersion).toBe(axe.version);
    const ids = axe.getRules().map((rule) => rule.ruleId).sort();
    expect(rules.map((rule) => rule.id)).toEqual(ids);
    for (const rule of rules) {
      expect(rule.id).toMatch(/^[a-z0-9][a-z0-9-]{0,63}$/);
      expect(['critical', 'serious', 'moderate', 'minor', null]).toContain(rule.impact);
    }
  });

  it('gives every rule its title in the plugin locale', () => {
    const titles: Record<string, string> = locale.plugin['a11y-axe'].rules;
    expect(Object.keys(titles).sort()).toEqual(rules.map((rule) => rule.id));
    for (const rule of rules) expect(titles[rule.id]).toBe(rule.help);
  });

  it('bundles the runner as one IIFE without imports', () => {
    expect(runner).toMatch(/^("use strict";)?var rbA11yAxe=/);
    expect(runner).not.toMatch(/\bimport\b|\brequire\(/);
  });
});
