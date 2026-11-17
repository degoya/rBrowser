// The English catalogue is `locales/en.json`, shared with the Rust side (rb-i18n). During a wave
// each job delivers `locales/parts/<job-id>.json` and the coordinator merges them into it; the
// shell reads both, the catalogue winning, so it shows its texts before and after that merge.

import { isObject } from '../../../plugins/web-common/src/is-object';

export type Catalogue = { [key: string]: string | Catalogue };


function mergeInto(target: Catalogue, source: Record<string, unknown>): void {
  for (const [key, value] of Object.entries(source)) {
    if (typeof value === 'string') {
      target[key] = value;
    } else if (isObject(value)) {
      const existing = target[key];
      const child: Catalogue = isObject(existing) ? existing : {};
      mergeInto(child, value);
      target[key] = child;
    }
  }
}

export function mergeCatalogues(catalogue: unknown, parts: unknown[]): Catalogue {
  const merged: Catalogue = {};
  for (const part of parts) {
    if (isObject(part)) {
      mergeInto(merged, part);
    }
  }
  if (isObject(catalogue)) {
    mergeInto(merged, catalogue);
  }
  return merged;
}
