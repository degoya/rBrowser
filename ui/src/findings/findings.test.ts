import { describe, expect, it } from 'vitest';
import {
  countBySeverity,
  filterFindings,
  groupByRule,
  highlightRequest,
  moveFocus,
  nestMessages,
  onlyIn,
  pluginsOf,
} from './findings';
import { sampleRun } from './testing';

describe('findings', () => {
  it('filters by severity, plugin and view', () => {
    const run = sampleRun();
    const all = { severities: ['error', 'warning', 'info'] as const, plugin: '', view: null };
    expect(filterFindings(run.findings, { ...all, severities: [...all.severities] })).toHaveLength(3);
    expect(
      filterFindings(run.findings, { ...all, severities: ['error'] }).map((f) => f.id),
    ).toEqual(['a']);
    expect(
      filterFindings(run.findings, { ...all, severities: [...all.severities], plugin: 'a11y-axe' }).map(
        (f) => f.id,
      ),
    ).toEqual(['a']);
    expect(
      filterFindings(run.findings, { ...all, severities: [...all.severities], view: 2 }).map((f) => f.id),
    ).toEqual(['a', 'c']);
  });

  it('groups by rule in the order of the list', () => {
    const groups = groupByRule(sampleRun().findings);
    expect(groups.map((group) => [group.ruleId, group.findings.length])).toEqual([
      ['seo.images', 2],
      ['seo.title', 1],
    ]);
    expect(groups[0]?.severity).toBe('error');
  });

  it('names the breakpoints of a finding that is not in every view', () => {
    const run = sampleRun();
    expect(onlyIn(run.findings[0]!, run)).toBeNull();
    expect(onlyIn(run.findings[1]!, run)).toEqual([
      { id: 1, breakpoint: 'mobile' },
      { id: 3, breakpoint: 'desktop' },
    ]);
    expect(onlyIn(run.findings[1]!, { ...run, views: [] })).toBeNull();
  });

  it('counts per severity and lists the plugins', () => {
    const run = sampleRun();
    expect(countBySeverity(run.findings)).toEqual({ error: 1, warning: 1, info: 1 });
    expect(pluginsOf(run)).toEqual(['a11y-axe', 'seo']);
  });

  it('asks to highlight the element in the views it was found in', () => {
    const run = sampleRun();
    expect(highlightRequest(run.findings[1]!)).toEqual({
      type: 'highlightFinding',
      findingId: 'b',
      selector: 'img.logo',
      views: [1, 3],
    });
    expect(highlightRequest({ ...run.findings[1]!, element: undefined })).toBeNull();
  });

  it('nests dotted keys for the i18n catalogue', () => {
    expect(
      nestMessages({ 'plugin.seo.findings.alt': 'No alt', 'plugin.seo.name': 'SEO' }),
    ).toEqual({ plugin: { seo: { findings: { alt: 'No alt' }, name: 'SEO' } } });
    expect(nestMessages({ 'plugin.seo': 'leaf', 'plugin.seo.name': 'nested' })).toEqual({
      plugin: { seo: 'leaf' },
    });
  });

  it('moves the focus through a list and stops at its ends', () => {
    expect(moveFocus(0, 3, 1)).toBe(1);
    expect(moveFocus(2, 3, 1)).toBe(2);
    expect(moveFocus(0, 3, -1)).toBe(0);
    expect(moveFocus(-1, 3, 1)).toBe(0);
    expect(moveFocus(0, 0, 1)).toBe(-1);
  });
});
