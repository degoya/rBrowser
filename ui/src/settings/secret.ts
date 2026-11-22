// Secret settings (RB-094-26, RB-094-27): a key annotated `x-secret: true` (`api-client.secrets`)
// holds credentials. It is no setting of the window - its values are edited where its plugin
// edits them (the API client's Variables tab) - so the page shows it nowhere: the core leaves it
// out of what it sends, and the page drops one should it come through anyway.

import type { SettingValue } from '../ipc/protocol';
import type { SettingDef } from './schema';

/** Whether `def`'s values are secrets. */
export function isSecret(def: Pick<SettingDef, 'schema'>): boolean {
  return def.schema['x-secret'] === true;
}

/** The values of the keys `defs` shows: a value of a key without a definition (a secret key's)
 * stays out of the JSON view. */
export function shownValues(values: Iterable<SettingValue>, defs: readonly SettingDef[]): SettingValue[] {
  const shown = new Set(defs.map((def) => def.key));
  return [...values].filter((value) => shown.has(value.key));
}
