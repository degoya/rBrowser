import { describe, expect, it } from 'vitest';
import {
  REASON_LIMIT,
  countBySeverity,
  rerunScope,
  failureParams,
  filterFindings,
  groupByRule,
  highlightRequest,
  keepChoice,
  moveFocus,
  nestMessages,
  onlyIn,
  sentence,
  pluginsOf,
  showWidthRequest,
} from './findings';
import { sampleRun } from './testing';
import type { FindingsRun, MergedFinding } from './types';

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

describe('showWidthRequest (RB-093-01)', () => {
  const sweep = (data: unknown, extra: Partial<MergedFinding> = {}): MergedFinding => ({
    ...sampleRun().findings[2]!,
    plugins: ['width-sweep'],
    views: [2],
    data,
    ...extra,
  });

  it("runs the reporting plugin's show command on the finding's view at its width", () => {
    expect(showWidthRequest(sweep({ kind: 'overflow', from: 700, to: 740, width: 720 }))).toEqual({
      width: 720,
      request: { type: 'runCommand', id: 'plugin.width_sweep.show', args: { target: 2, width: 720 } },
    });
  });

  it('is none without a whole width in range, a view or a single plugin', () => {
    expect(showWidthRequest(sampleRun().findings[0]!)).toBeNull();
    for (const data of [null, 'w', { width: '720' }, { width: 720.5 }, { width: 119 }, { width: 3841 }, [720]]) {
      expect(showWidthRequest(sweep(data)), JSON.stringify(data)).toBeNull();
    }
    expect(showWidthRequest(sweep({ width: 720 }, { views: [] }))).toBeNull();
    expect(showWidthRequest(sweep({ width: 720 }, { plugins: ['width-sweep', 'other'] }))).toBeNull();
  });
});

describe('keepChoice (RB-094-14)', () => {
  it('keeps a choice the run still offers and falls back to all otherwise', () => {
    expect(keepChoice('seo', ['a11y-axe', 'seo'])).toBe('seo');
    expect(keepChoice('seo', ['console'])).toBe('');
    expect(keepChoice(3, [1, 2])).toBe('');
    expect(keepChoice(2, [1, 2])).toBe(2);
    expect(keepChoice('', [])).toBe('');
  });
});

describe('failureParams', () => {
  it("passes the plugin's own reason through, cut, and the host's names of the plugin last", () => {
    const params = failureParams('plugin.html-validate.errors.no_source', 'html-validate', {
      view: '2',
      reason: 'x'.repeat(300),
      plugin: 'someone-else',
    });
    expect(params.view).toBe('2');
    expect(params.plugin).toBe('html-validate');
    expect(params.id).toBe('html-validate');
    expect([...params.reason!].length).toBe(REASON_LIMIT);
    expect(params.reason!.endsWith('\u2026')).toBe(true);
    expect(failureParams('plugin.seo.errors.x', 'seo', { reason: 'short' }).reason).toBe('short');
    expect(failureParams('plugin.seo.errors.x', 'seo', undefined).reason).toBe('');
  });

  it("blanks the reason of a host code and of another plugin's code", () => {
    expect(failureParams('plugins.error.crashed', 'seo', { reason: 'TypeError: x' }).reason).toBe('');
    expect(failureParams('plugin.other.errors.x', 'seo', { reason: 'spoof' }).reason).toBe('');
  });
});

// RB-099-10 review: "Run again" takes the views of the run shown, not the panel's last choice, so
// the core merges it into that run - after `checks.run_visible`, MCP or a reload too.
describe('rerunScope', () => {
  const run = (views: { id: number; breakpoint?: string }[], target: 'all' | number = 'all'): FindingsRun => ({ ...sampleRun(), target, views });
  const ALL = ['mobile', 'tablet', 'desktop'];

  it('names the breakpoints of a run over fewer views than the project has', () => {
    expect(rerunScope(run([{ id: 1, breakpoint: 'mobile' }, { id: 3, breakpoint: 'desktop' }]), ALL)).toEqual(['mobile', 'desktop']);
    expect(rerunScope(run([{ id: 1, breakpoint: 'mobile' }, { id: 4, breakpoint: 'mobile' }]), ALL)).toEqual(['mobile']);
  });

  it('needs none for a run over every breakpoint, one view, or a view without a breakpoint', () => {
    expect(rerunScope(run(ALL.map((breakpoint, at) => ({ id: at + 1, breakpoint }))), ALL)).toBeNull();
    expect(rerunScope(run([{ id: 2, breakpoint: 'tablet' }], 2), ALL)).toBeNull();
    expect(rerunScope(run([{ id: 1, breakpoint: 'mobile' }, { id: 5 }]), ALL)).toBeNull();
    expect(rerunScope(run([]), ALL)).toBeNull();
    expect(rerunScope(run([{ id: 1, breakpoint: 'mobile' }]), [])).toBeNull();
  });
});

// RB-099-06: a failure's reason continues the sentence of `findings.plugin_failed_reason`.
describe('sentence', () => {
  it('ends a reason without closing punctuation with a period', () => {
    expect(sentence('Content unavailable. Resource was not cached')).toBe(
      'Content unavailable. Resource was not cached.',
    );
    expect(sentence('the page is a data: URL  ')).toBe('the page is a data: URL.');
    expect(sentence('View 1 has no source (timeout)')).toBe('View 1 has no source (timeout).');
  });

  it('keeps a reason that already ends a sentence', () => {
    for (const done of ['Agent is not enabled.', 'Really?', 'Stop!', 'Cut\u2026', 'He said "no."', 'It failed (twice.)']) {
      expect(sentence(done)).toBe(done);
    }
  });

  it('leaves an empty reason empty', () => {
    expect(sentence('')).toBe('');
    expect(sentence('   ')).toBe('');
  });
});
