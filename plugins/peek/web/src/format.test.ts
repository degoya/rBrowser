import { describe, expect, it } from 'vitest';
import { inspection } from './fixtures';
import { copyText, fontNote, rows, SHOWN_CLASSES, title } from './format';
import { t } from './labels';

function row(label: string) {
  return rows(inspection()).find((entry) => entry.label === t(`tooltip.${label}`));
}

describe('the tooltip text', () => {
  it('titles the element with id and cut classes', () => {
    expect(title(inspection())).toBe('button#send.btn.btn-primary');
    const many = Array.from({ length: SHOWN_CLASSES + 2 }, (_, index) => `c${index}`);
    expect(title(inspection({ classes: many, id: '' }))).toBe('button.c0.c1.c2.c3…');
  });

  it('shows size, device pixels, contrast with its level and the rendered font', () => {
    expect(row('size')).toMatchObject({ value: '120.5 × 40', note: '241 × 80 device px' });
    expect(row('contrast')?.value).toBe('4.48:1 · AA large');
    expect(row('colors')).toMatchObject({ value: '#777777 / #ffffff', swatches: ['#777777', '#ffffff'] });
    expect(row('font')?.value).toBe('16px · 400 · "Inter", sans-serif');
    expect(row('font')?.note).toBe('Line height 24px · Letter spacing normal');
    expect(row('rendered')?.value).toBe('Segoe UI');
    expect(row('box')?.value).toBe('Margin 4 8 4 8 · Border 1 · Padding 6 12 6 12');
    expect(row('box')?.note).toBe('Content 94.5 × 26');
    expect(row('role')?.value).toBe('button');
    expect(row('name')?.value).toBe('"Send"');
  });

  it('says n/a without CDP and uncertain over an image', () => {
    const data = inspection({
      font: { ...inspection().font, rendered: null },
      contrast: null,
      colors: { foreground: '#000000', background: '#ffffff', backgroundUncertain: true },
      size: { width: 1, height: 1, dpr: 1, deviceWidth: 1, deviceHeight: 1 },
    });
    const list = rows(data);
    const find = (label: string) => list.find((entry) => entry.label === t(`tooltip.${label}`));
    expect(find('rendered')?.value).toBe('n/a');
    expect(find('contrast')?.value).toBe('n/a');
    expect(find('colors')?.note).toBe(t('tooltip.uncertain'));
    expect(find('size')?.note).toBeUndefined();
  });

  it('notes a declared font that is not rendered, or only partly (RB-070-13)', () => {
    expect(row('rendered')?.note).toBe('Inter is not rendered (fallback)');
    const font = inspection().font;
    const drawn = (list: [string, boolean, number][]) =>
      list.map(([family, custom, glyphs]) => ({ family, postScriptName: family, custom, glyphs }));
    expect(fontNote({ ...font, rendered: drawn([['Inter', true, 9]]) })).toBeUndefined();
    // A web font's file may carry another name than its alias.
    expect(fontNote({ ...font, rendered: drawn([['Inter Display', true, 9]]) })).toBeUndefined();
    expect(fontNote({ ...font, rendered: drawn([['Inter', true, 9], ['Arial', false, 2], ['Unused', false, 0]]) })).toBe(
      'Some characters from Arial',
    );
    expect(fontNote({ ...font, declared: 'sans-serif', rendered: drawn([['Arial', false, 9]]) })).toBeUndefined();
    expect(fontNote({ ...font, rendered: null })).toBeUndefined();
  });

  it('copies every row as text', () => {
    const text = copyText(inspection());
    expect(text.split('\n')[0]).toBe('button#send.btn.btn-primary');
    expect(text).toContain('Contrast: 4.48:1 · AA large');
  });

  it('labels come from the catalogue, placeholders filled', () => {
    expect(t('level.aaa')).toBe('AAA');
    expect(t('tooltip.device', { width: 2, height: 3 })).toBe('2 × 3 device px');
    expect(t('tooltip.unknown_key')).toBe('tooltip.unknown_key');
  });
});
