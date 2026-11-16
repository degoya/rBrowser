// The tooltip's rows from the component's inspection, and the same as plain text for the copy
// button. Long class lists and names are cut here; the tool answers them whole.

import { t } from './labels';
import type { Inspection, SidesObject } from './types';

/** Classes shown in the title; the rest is "…". */
export const SHOWN_CLASSES = 4;
/** The longest class, name or font family shown. */
export const SHOWN_CHARS = 48;

export interface Row {
  label: string;
  value: string;
  /** Colour swatches before the value (text colour, background). */
  swatches?: string[];
  /** A second, quieter line. */
  note?: string;
}

function cut(text: string, max = SHOWN_CHARS): string {
  return text.length > max ? `${text.slice(0, max - 1)}…` : text;
}

/** The generic families: nothing to compare with what was rendered. */
const GENERIC = new Set(['serif', 'sans-serif', 'monospace', 'cursive', 'fantasy', 'system-ui', 'ui-serif', 'ui-sans-serif', 'ui-monospace', 'ui-rounded', 'math', 'emoji', 'fangsong', '-apple-system', 'blinkmacsystemfont']);

function families(stack: string): string[] {
  return stack
    .split(',')
    .map((family) => family.trim().replace(/^(["'])(.*)\1$/, '$2'))
    .filter(Boolean);
}

/** RB-070-13 (fonts): a note when the declared font is not what Chromium drew - all of the text
 * from a fallback, or some characters (a subset without umlauts). A web font may carry another
 * name in its file than its `@font-face` alias, so a custom font no other family of the stack
 * names counts as the declared one. */
export function fontNote(font: Inspection['font']): string | undefined {
  const declared = families(font.declared);
  const first = declared[0];
  const drawn = (font.rendered ?? []).filter((entry) => entry.glyphs > 0);
  if (!first || GENERIC.has(first.toLowerCase()) || drawn.length === 0) return undefined;
  const others = declared.slice(1).map((family) => family.toLowerCase());
  const own = (entry: { family: string; custom: boolean }) =>
    entry.family.toLowerCase() === first.toLowerCase() || (entry.custom && !others.includes(entry.family.toLowerCase()));
  const foreign = drawn.filter((entry) => !own(entry));
  if (foreign.length === 0) return undefined;
  if (foreign.length === drawn.length) return t('tooltip.fallback', { declared: cut(first) });
  return t('tooltip.glyphs', { fonts: cut(foreign.map((entry) => entry.family).join(', ')) });
}

function num(value: number): string {
  return String(Math.round(value * 100) / 100);
}

function sides(value: SidesObject): string {
  const all = [value.top, value.right, value.bottom, value.left];
  return all.every((side) => side === all[0]) ? num(value.top) : all.map(num).join(' ');
}

/** `button#send.btn.btn-primary…` */
export function title(inspection: Pick<Inspection, 'tag' | 'id' | 'classes'>): string {
  const id = inspection.id ? `#${cut(inspection.id)}` : '';
  const classes = inspection.classes.slice(0, SHOWN_CLASSES).map((name) => `.${cut(name, 24)}`);
  const more = inspection.classes.length > SHOWN_CLASSES ? '…' : '';
  return `${inspection.tag}${id}${classes.join('')}${more}`;
}

export function rows(inspection: Inspection): Row[] {
  const { size, box, colors, contrast, font } = inspection;
  const list: Row[] = [];
  list.push({
    label: t('tooltip.size'),
    value: `${num(size.width)} × ${num(size.height)}`,
    note:
      size.dpr === 1
        ? undefined
        : t('tooltip.device', { width: num(size.deviceWidth), height: num(size.deviceHeight) }),
  });
  if (colors.foreground) {
    list.push({
      label: t('tooltip.colors'),
      value: `${colors.foreground} / ${colors.background ?? t('tooltip.notAvailable')}`,
      swatches: [colors.foreground, colors.background ?? 'transparent'],
      note: colors.backgroundUncertain ? t('tooltip.uncertain') : undefined,
    });
  }
  list.push({
    label: t('tooltip.contrast'),
    value: contrast ? `${contrast.ratio.toFixed(2)}:1 · ${t(`level.${contrast.level}`)}` : t('tooltip.notAvailable'),
  });
  const rendered = font.rendered?.map((entry) => entry.family).filter(Boolean) ?? [];
  list.push({
    label: t('tooltip.font'),
    value: `${num(font.size)}px · ${font.weight} · ${cut(font.declared)}`,
    note: `${t('tooltip.lineHeight')} ${font.lineHeight} · ${t('tooltip.letterSpacing')} ${font.letterSpacing}`,
  });
  list.push({
    label: t('tooltip.rendered'),
    value: rendered.length > 0 ? cut(rendered.join(', ')) : t('tooltip.notAvailable'),
    note: fontNote(font),
  });
  list.push({
    label: t('tooltip.box'),
    value: `${t('tooltip.margin')} ${sides(box.margin)} · ${t('tooltip.border')} ${sides(box.border)} · ${t('tooltip.padding')} ${sides(box.padding)}`,
    note: `${t('tooltip.content')} ${num(box.content.width)} × ${num(box.content.height)}`,
  });
  list.push({ label: t('tooltip.role'), value: inspection.role || t('tooltip.none') });
  list.push({ label: t('tooltip.name'), value: inspection.name ? `"${cut(inspection.name)}"` : t('tooltip.none') });
  return list;
}

/** The tooltip as text, for the clipboard. */
export function copyText(inspection: Inspection): string {
  const lines = [title(inspection)];
  for (const row of rows(inspection)) {
    lines.push(`${row.label}: ${row.value}${row.note ? ` (${row.note})` : ''}`);
  }
  return lines.join('\n');
}
