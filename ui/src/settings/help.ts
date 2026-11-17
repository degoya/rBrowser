// The help of a setting (RB-080-20): the catalogue's text - what the key is for, its effect,
// an example - followed by the default the schema carries, so the two never drift.

import type { JsonValue } from '../ipc/protocol';
import type { SettingDef } from './schema';

export type Translate = (key: string, named?: Record<string, unknown>) => string;

/** The default of `schema` as the help shows it: a string as it is, other values as JSON. */
export function defaultTextOf(schema: { default?: JsonValue }, t: Translate): string {
  const value = schema.default;
  if (value === undefined || value === null) {
    return t('settings.value.unset');
  }
  if (typeof value === 'string') {
    return value === '' ? t('settings.value.empty') : value;
  }
  if (Array.isArray(value) && value.length === 0) {
    return t('settings.value.none');
  }
  return JSON.stringify(value);
}

/** The help text of `def` with its default. */
export function helpTextOf(def: SettingDef, t: Translate): string {
  return `${t(def.helpKey)} ${t('settings.help.default', { value: defaultTextOf(def.schema, t) })}`;
}
