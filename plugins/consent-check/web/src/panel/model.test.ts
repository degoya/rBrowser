// The consent check panel's model (RB-094-05): texts of the findings, the banner, the caveats,
// lifetimes and the differences, the status line.
import { describe, expect, it } from 'vitest';
import { ANSWER, STRINGS, VIEW_ANSWER } from './fixtures';
import { bannerText, caveatTexts, diffGroups, findingText, hostLabel, lifetime, sortedFindings, statusOf, text } from './model';

describe('the consent panel model', () => {
  it('writes a finding with its parameters and plurals', () => {
    expect(findingText(STRINGS, ANSWER.findings[0]!)).toBe('Before any consent the page reaches cdn.tracker.test (script, 1 request).');
    expect(findingText(STRINGS, ANSWER.findings[1]!)).toBe('The cookie _track (.shop.example.test) is set before any consent and kept 365 days.');
    expect(findingText(STRINGS, ANSWER.findings[3]!)).toContain('for the browser session');
  });

  it('lists errors before warnings', () => {
    expect(sortedFindings(ANSWER).map((finding) => finding.severity)).toEqual(['error', 'error', 'warning', 'warning']);
  });

  it('names the banner and whether it was rejected', () => {
    expect(bannerText(STRINGS, ANSWER)).toBe('"Reject all" was clicked: OneTrust');
    expect(bannerText(STRINGS, { ...ANSWER, banner: { ...ANSWER.banner, rejected: false } })).toBe('Consent banner shown: OneTrust');
    expect(bannerText(STRINGS, { ...ANSWER, banner: { ...ANSWER.banner, rejected: false, visible: false } })).toBe('Consent banner found, not shown: OneTrust');
    expect(bannerText(STRINGS, VIEW_ANSWER)).toBe('No known consent banner found.');
  });

  it('writes the caveats of the run', () => {
    expect(caveatTexts(STRINGS, ANSWER)).toEqual([]);
    const caveats = caveatTexts(STRINGS, VIEW_ANSWER);
    expect(caveats).toHaveLength(3);
    expect(caveats[0]).toContain('a consent given earlier');
  });

  it('writes a lifetime in days or as the session', () => {
    expect(lifetime(STRINGS, 365)).toBe('365 days');
    expect(lifetime(STRINGS, 1)).toBe('1 day');
    expect(lifetime(STRINGS, null)).toBe('Session');
  });

  it('names a host row with its kind and count', () => {
    expect(hostLabel(STRINGS, ANSWER.before.hosts[0]!)).toBe('script, 1 request');
  });

  it('groups the differences after reject, empty groups left out', () => {
    const groups = diffGroups(STRINGS, ANSWER);
    expect(groups.map((group) => group.title)).toEqual([
      'Loaded only after "Reject all"',
      'Not loaded again after "Reject all"',
      'Cookies set after "Reject all"',
    ]);
    expect(groups[0]!.items).toEqual(['pixel.ads.test (pixel)']);
    expect(diffGroups(STRINGS, VIEW_ANSWER)).toEqual([]);
  });

  it('shows failures, the run and an export in the status line', () => {
    expect(statusOf(STRINGS, { failure: 'Broken', loading: false, answer: null, exported: null, reported: null })).toEqual({ tone: 'error', text: 'Broken' });
    expect(statusOf(STRINGS, { failure: null, loading: true, answer: null, exported: null, reported: null })?.tone).toBe('busy');
    expect(statusOf(STRINGS, { failure: null, loading: false, answer: ANSWER, exported: '.rbrowser/out/consent-check/consent-check.json', reported: null })).toEqual({
      tone: 'ok',
      text: 'Written to .rbrowser/out/consent-check/consent-check.json.',
    });
    expect(statusOf(STRINGS, { failure: null, loading: false, answer: ANSWER, exported: null, reported: 4 })?.text).toBe('4 findings sent to the findings panel.');
    expect(statusOf(STRINGS, { failure: null, loading: false, answer: null, exported: null, reported: null })).toBeNull();
  });

  it('falls back to the key for a text the catalogue lacks', () => {
    expect(text(STRINGS, 'panel.nothing')).toBe('panel.nothing');
  });
});
