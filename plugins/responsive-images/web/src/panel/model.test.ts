// The responsive images panel's model (RB-093-02): tabs per view, rows, texts, the marker.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ANSWER, STRINGS } from './fixtures';
import { createMarker, fileName, issueText, markTargets, rowsOf, statusOf, tabsOf, unmeasured, type PanelImage, type ViewAnswer } from './model';

const mobile = ANSWER.views[0] as ViewAnswer;

describe('tabs and rows', () => {
  it('has a tab per view with its density and count of problems', () => {
    expect(tabsOf(STRINGS, ANSWER).map((tab) => [tab.label, tab.badge, tab.icon])).toEqual([
      ['Mobile · 3x', '4', 'i-lucide-triangle-alert'],
      ['Desktop · 1x', '4', 'i-lucide-triangle-alert'],
      ['Firefox · 1x', undefined, 'i-lucide-circle-slash'],
    ]);
    expect(tabsOf(STRINGS, null)).toEqual([]);
  });

  it('has a row per image with what it needs and has at the view density', () => {
    const rows = rowsOf(STRINGS, mobile, false);
    expect(rows).toHaveLength(5);
    const large = rows[1];
    expect(large).toMatchObject({ rendered: '300 × 200', needed: '900 × 600', file: '1800 × 1200', ratio: '2×', note: null });
    expect(large?.name).toBe('large-1800.jpg');
    expect(rows[0]?.flags).toEqual(['LCP', 'lazy', 'high priority']);
    expect(rows[0]?.origin).toBe('from srcset');
    expect(rows[1]?.origin).toBe('from src');
    expect(rowsOf(STRINGS, mobile, true).map((row) => row.image.selector)).toEqual(['#hero', '#large', '#small', '#no-sizes']);
  });

  it('says why an image is not measured instead of empty sizes', () => {
    const base = mobile.images?.[4] as PanelImage;
    expect(unmeasured(base)).toBeNull();
    expect(unmeasured({ ...base, visible: false })).toBe('hidden');
    expect(unmeasured({ ...base, vector: true })).toBe('vector');
    expect(unmeasured({ ...base, loaded: false, file: null })).toBe('pending');
    expect(unmeasured({ ...base, needed: null })).toBe('tiny');
    const [row] = rowsOf(STRINGS, { ...mobile, images: [{ ...base, loaded: false, file: null, needed: null, ratio: null }] }, false);
    expect(row).toMatchObject({ needed: '', file: '', ratio: '', note: 'not loaded yet' });
  });

  it('names the picture source a file came from', () => {
    const base = mobile.images?.[4] as PanelImage;
    const source = { media: '(min-width: 600px)', type: 'image/webp', srcset: 'a.webp 600w', sizes: '' };
    expect(rowsOf(STRINGS, { ...mobile, images: [{ ...base, picture: true, source }] }, false)[0]?.origin).toBe('from <source media="(min-width: 600px)">');
  });

  it('writes each issue as its sentence with the view density', () => {
    const [issue] = mobile.images?.[1]?.issues ?? [];
    expect(issueText(STRINGS, issue as NonNullable<typeof issue>)).toBe(
      'The file is 1800 × 1200 px, 2× the 900 × 600 px that 300 × 200 CSS px need at 3x. Above 1.5×, the view loads more than it shows.',
    );
  });

  it('names a file by its last path segment', () => {
    expect(fileName('https://example.test/img/a%20b.jpg?w=2')).toBe('a b.jpg');
    expect(fileName('data:image/png;base64,AAAA')).toBe('data:image/png');
    expect(fileName('not a url/x.png')).toBe('x.png');
  });
});

describe('marks and status', () => {
  it('marks a selector in every view that was measured', () => {
    expect(markTargets(ANSWER, '#hero')).toEqual([
      { view: 1, selector: '#hero' },
      { view: 2, selector: '#hero' },
    ]);
  });

  it('sums the problems, says a shortened answer and a failure', () => {
    expect(statusOf(STRINGS, { failure: null, loading: false, answer: ANSWER })).toEqual({ tone: 'warn', text: '0 errors, 8 warnings, 0 notes' });
    expect(statusOf(STRINGS, { failure: null, loading: false, answer: { ...ANSWER, cut: true } })?.tone).toBe('warn');
    expect(statusOf(STRINGS, { failure: 'Broken.', loading: false, answer: ANSWER })).toEqual({ tone: 'error', text: 'Broken.' });
    expect(statusOf(STRINGS, { failure: null, loading: true, answer: null })?.tone).toBe('busy');
    expect(statusOf(STRINGS, { failure: null, loading: false, answer: { ...ANSWER, summary: { error: 0, warning: 0, info: 0 } } })?.tone).toBe('ok');
  });
});

describe('createMarker', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('marks the last hover after the delay, one call at a time, and clears on leave', async () => {
    const sent: (string | null)[] = [];
    let finish: () => void = () => undefined;
    const marker = createMarker((selector) => {
      sent.push(selector);
      return new Promise<void>((resolve) => {
        finish = resolve;
      });
    }, 100);
    marker.hover('#a');
    marker.hover('#b');
    await vi.advanceTimersByTimeAsync(100);
    expect(sent).toEqual(['#b']);
    marker.hover('#c');
    await vi.advanceTimersByTimeAsync(100);
    expect(sent).toEqual(['#b']);
    finish();
    await vi.advanceTimersByTimeAsync(0);
    expect(sent).toEqual(['#b', '#c']);
    finish();
    marker.hover('#c');
    await vi.advanceTimersByTimeAsync(100);
    expect(sent).toEqual(['#b', '#c']);
    marker.hover(null);
    await vi.advanceTimersByTimeAsync(100);
    expect(sent).toEqual(['#b', '#c', null]);
    marker.stop();
  });
});
