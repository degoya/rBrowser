// The rulers-guides panel's pure logic (RB-060-05, RB-080-66; on Nuxt UI since RB-080-86): the
// state the tool `set` answers, the column setup's fields per breakpoint, the change a group of
// fields shows and a failure as a sentence.

export type Axis = 'x' | 'y';
export type Unit = 'px' | 'px-right' | 'percent';

export interface Guide {
  id: number;
  axis: Axis;
  unit: Unit;
  value: number;
}

export type Layer = 'rulers' | 'columns' | 'baseline';

export interface RulersState {
  visible: Record<Layer, boolean>;
  guides: Guide[];
  columns: Record<string, Record<string, unknown>>;
  baseline: { lineHeight: number; offset: number };
}

/** A template's values, as the component answers them (`''` is no template). */
export interface Template {
  columns: number;
  gutter: number;
  margin: number;
  maxWidth: { min: number; max: number } | null;
}

export interface SetAnswer {
  state?: RulersState;
  breakpoints?: unknown[];
  templates?: Record<string, Template>;
  saved?: boolean;
}

export const LAYERS: readonly Layer[] = ['rulers', 'columns', 'baseline'];
export const PRESETS = ['bootstrap', 'tailwind'] as const;
export const ALIGNS = ['left', 'center', 'right'] as const;
/** The number fields of a column setup, by the key they set. */
export const SPEC_FIELDS = ['columns', 'gutter', 'margin', 'maxWidth'] as const;
export type SpecField = (typeof SPEC_FIELDS)[number];

/** A choice's value for "None": a select item's value cannot be empty. */
export const NONE = '@none';

/** The units a guide of `axis` can be measured in: a horizontal one only from the top. */
export const unitsOf = (axis: Axis): Unit[] => (axis === 'x' ? ['px', 'px-right', 'percent'] : ['px']);

/** The unit's text key; a horizontal guide's px count from the top. */
export const unitKey = (axis: Axis, unit: Unit): string => (axis === 'y' ? 'panel.unit_px_top' : `panel.unit_${unit}`);

/** The guides as `set` takes them. */
export const guidesArg = (guides: readonly Guide[]): { guides: Omit<Guide, 'id'>[] } => ({
  guides: guides.map(({ axis, unit, value }) => ({ axis, unit, value })),
});

/** The setups one can edit: the view's breakpoints, those with a setup of their own, then `*`. */
export const setupKeys = (breakpoints: readonly string[], state: RulersState): string[] => [
  ...new Set([...breakpoints, ...Object.keys(state.columns).filter((key) => key !== '*'), '*']),
];

/** The fields of a column setup as the panel shows them: text, `''` without a value. */
export interface ColumnFields {
  preset: string;
  align: string;
  columns: string;
  gutter: string;
  margin: string;
  maxWidth: string;
}

export function columnFields(spec: Record<string, unknown> | undefined): ColumnFields {
  const text = (value: unknown): string => (value === undefined || value === null ? '' : String(value));
  return {
    preset: text(spec?.preset),
    align: text(spec?.align),
    columns: text(spec?.columns),
    gutter: text(spec?.gutter),
    margin: text(spec?.margin),
    maxWidth: text(spec?.maxWidth),
  };
}

/** The column setup the fields show for `breakpoint`: only what is set. */
export function columnsArg(breakpoint: string, fields: ColumnFields): { columns: Record<string, Record<string, unknown>> } {
  const spec: Record<string, unknown> = {};
  if (fields.preset) spec.preset = fields.preset;
  if (fields.align) spec.align = fields.align;
  for (const field of SPEC_FIELDS) {
    if (fields[field] !== '') spec[field] = Number(fields[field]);
  }
  return { columns: { [breakpoint]: spec } };
}

/** The baseline the fields show; an empty offset is 0. */
export const baselineArg = (lineHeight: string, offset: string): { baseline: { lineHeight: number; offset: number } } => ({
  baseline: { lineHeight: Number(lineHeight), offset: Number(offset || 0) },
});

/** A layer set up is a layer shown: its switch goes on with the change, unless it is on. */
export const shownArg = (state: RulersState | null, layer: Layer): Record<string, unknown> =>
  state && !state.visible[layer] ? { visible: { [layer]: true } } : {};

/** What applies where a field is empty: the chosen template's values (RB-080-66). */
export function placeholders(
  t: (key: string, params?: Record<string, unknown>) => string,
  templates: Record<string, Template>,
  preset: string,
): Partial<Record<SpecField, string>> {
  const template = templates[preset] ?? templates[''];
  if (!template) return {};
  return {
    columns: String(template.columns),
    gutter: String(template.gutter),
    margin: String(template.margin),
    maxWidth: template.maxWidth ? t('panel.max_width_range', { min: template.maxWidth.min, max: template.maxWidth.max }) : t('panel.max_width_full'),
  };
}

const PREFIX = 'plugin.rulers-guides.';

/** Host codes as sentences (RB-080-66): a code is never the message. */
const HOST_ERRORS: Record<string, string> = {
  'plugins.error.busy': 'panel.error_busy',
  'plugins.error.timed_out': 'panel.error_timed_out',
  'plugins.error.host_failed': 'panel.error_no_answer',
  'plugins.error.suspended': 'panel.error_suspended',
};

/** A failure's code as a sentence: the plugin's own text, a host code's, else a general one. */
export function failure(
  t: (key: string, params?: Record<string, unknown>) => string,
  strings: Record<string, string>,
  code: string,
  params: Record<string, unknown> = {},
): string {
  if (code.startsWith(PREFIX) && strings[code] !== undefined) {
    // Without its reason a sentence ending in the reason ends there.
    const own = t(code.slice(PREFIX.length), params).trim().replace(/:\s*$/, '');
    if (own) return own.endsWith('.') ? own : `${own}.`;
  }
  return t(HOST_ERRORS[code] ?? 'panel.error_other');
}
