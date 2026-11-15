// The JSON view of the settings (read-only, copyable): what one scope contributes - the keys
// whose value comes from it - or every value in effect, nested by the dots of the keys.

import type { JsonValue, Origin, SettingValue } from '../ipc/protocol';
import { booleanSetting } from './booleanSetting';

export type JsonViewScope = 'effective' | Exclude<Origin, 'default'>;

export const JSON_VIEW_SCOPES: JsonViewScope[] = ['effective', 'user', 'project', 'local'];

type JsonObject = { [key: string]: JsonValue };

function insert(target: JsonObject, key: string, value: JsonValue): void {
  const segments = key.split('.');
  const last = segments.pop();
  if (last === undefined) {
    return;
  }
  let node = target;
  for (const segment of segments) {
    const child = node[segment];
    if (typeof child === 'object' && child !== null && !Array.isArray(child)) {
      node = child;
    } else {
      const created: JsonObject = {};
      node[segment] = created;
      node = created;
    }
  }
  node[last] = value;
}

/** The document of `scope`, nested by key. */
export function scopeDocument(values: Iterable<SettingValue>, scope: JsonViewScope): JsonObject {
  const document: JsonObject = {};
  for (const value of values) {
    if (scope === 'effective' || value.origin === scope) {
      insert(document, value.key, value.value);
    }
  }
  return document;
}

/** The text shown and copied: two-space JSON, like the files. */
export function scopeText(values: Iterable<SettingValue>, scope: JsonViewScope): string {
  return `${JSON.stringify(scopeDocument(values, scope), null, 2)}\n`;
}

/** The key of the user setting that switches the syntax colours (RB-080-42). */
export const JSON_HIGHLIGHT_KEY = 'settings.jsonHighlight';

/** The key of the user setting that switches syntax colours everywhere (RB-080-65). */
export const SYNTAX_HIGHLIGHT_KEY = 'ui.syntaxHighlight';

/**
 * Whether the view colours its tokens: on unless `settings.jsonHighlight` or
 * `ui.syntaxHighlight` is `false`.
 */
export function jsonHighlightOn(values: Iterable<SettingValue>): boolean {
  for (const value of values) {
    if ((value.key === JSON_HIGHLIGHT_KEY || value.key === SYNTAX_HIGHLIGHT_KEY) && !booleanSetting(value.value, true)) {
      return false;
    }
  }
  return true;
}
