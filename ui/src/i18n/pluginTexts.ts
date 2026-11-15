// The plugins' texts in the shell's catalogue (RB-080-82). A plugin's command titles and setting
// labels live in the plugin's own catalogue (`plugins/<id>/locales/en.json`), which the core
// registers (rb-i18n) - the shell's catalogue is `locales/en.json` only, so `t()` on a plugin
// key showed the key itself. The core sends the texts a window needs beside the message that
// names them: the command list (`commands.texts`) and the settings schema (`schema.texts`). The
// IPC client hands every message here, so the palette, the which-key bar, the toolbar tooltips
// and the settings window all read them. Only `plugin.` keys are taken: a message can never
// replace a core text; a key with a `__proto__`, `constructor` or `prototype` segment is dropped.

import { isObject } from '../../../plugins/web-common/src/is-object';
import type { CoreToShell } from '../ipc/protocol';
import type { Catalogue } from './messages';

const PLUGIN_PREFIX = 'plugin.';
/** Segments that would walk a nested catalogue into an object's prototype; rb-i18n refuses
 * them in a plugin's catalogue too (security review c20). */
const UNSAFE_SEGMENTS = new Set(['__proto__', 'constructor', 'prototype']);

function safeKey(key: string): boolean {
  return key.startsWith(PLUGIN_PREFIX) && !key.split('.').some((segment) => UNSAFE_SEGMENTS.has(segment));
}

type Merge = (messages: Catalogue) => void;
/** Where the texts go: the catalogue of this window's i18n (`createShellI18n`). */
let target: Merge | null = null;

export function attachPluginTexts(merge: Merge): void {
  target = merge;
}

function onlyPluginTexts(value: unknown): Record<string, string> | null {
  if (!isObject(value)) {
    return null;
  }
  const texts = Object.fromEntries(
    Object.entries(value).filter(
      (entry): entry is [string, string] => safeKey(entry[0]) && typeof entry[1] === 'string',
    ),
  );
  return Object.keys(texts).length === 0 ? null : texts;
}

/** The plugin texts `message` carries, by full key; `null` when it carries none. */
export function pluginTextsOf(message: CoreToShell): Record<string, string> | null {
  if (message.type === 'commands') {
    return onlyPluginTexts(message.texts);
  }
  if (message.type === 'settings' && isObject(message.schema)) {
    return onlyPluginTexts(message.schema.texts);
  }
  return null;
}

/** `{"plugin.a.b": "x"}` as the nested catalogue vue-i18n reads (`{plugin: {a: {b: "x"}}}`). */
function nest(texts: Record<string, string>): Catalogue {
  const root: Catalogue = {};
  for (const [key, text] of Object.entries(texts)) {
    const parts = key.split('.');
    const last = parts.pop();
    let node: Catalogue | null = root;
    for (const part of parts) {
      // Own properties only: nothing is ever read from or written through a prototype.
      const next: string | Catalogue | undefined = Object.hasOwn(node, part) ? node[part] : undefined;
      if (typeof next === 'string') {
        node = null;
        break;
      }
      node = next ?? (node[part] = {});
    }
    if (node !== null && last !== undefined && !(Object.hasOwn(node, last) && isObject(node[last]))) {
      node[last] = text;
    }
  }
  return root;
}

/** Puts the plugin texts of `message`, if any, into this window's catalogue. */
export function takePluginTexts(message: CoreToShell): void {
  const texts = pluginTextsOf(message);
  if (texts !== null && target !== null) {
    target(nest(texts));
  }
}
