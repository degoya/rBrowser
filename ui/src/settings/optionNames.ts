// The names of the choices of the settings (RB-090-16): an option of a closed choice (`enum`, a
// `oneOf` of constants) shows its name from the catalogue, never its id; the id joins it only with
// `settings.showKeys`, and stays in the tooltip. The keys, held by the settings lint
// (scripts/lib/settings_options.py):
// - a core key's option is `settings.option.<key>.<value>`;
// - a plugin key's is `plugin.<id>.settings.option.<path>.<value>`, in the plugin's own catalogue,
//   which the core sends with the schema (`plugin.<id>.settings.` texts, RB-080-82);
// - a nested field adds its name (`breakpoints.media.contrast`), a list's items and a map's values
//   add nothing; the value's first letter is in lower case (`A4` -> `a4`), as a key segment starts.
// A value without a text - a number, a value the catalogue does not know - reads as itself.

import { inject, type InjectionKey } from 'vue';
import { useI18n } from 'vue-i18n';
import type { JsonValue } from '../ipc/protocol';

/** What names the choices of one setting: the plugin that contributed it and whether the ids
 * show (`settings.showKeys`). `SettingField` provides it to every control under it. */
export interface OptionNaming {
  plugin(): string | undefined;
  showKeys(): boolean;
}

export const OPTION_NAMING: InjectionKey<OptionNaming> = Symbol('optionNaming');

/** The catalogue segment of an option: the value, its first letter in lower case. */
function segment(value: JsonValue): string {
  const text = String(value);
  return text.slice(0, 1).toLowerCase() + text.slice(1);
}

/** The i18n key of `value` of the field `name` (the key, a nested field's with its path);
 * `plugin` is the plugin that contributed the key. */
export function optionKey(name: string, value: JsonValue, plugin?: string): string {
  if (plugin === undefined || !name.startsWith(`${plugin}.`)) {
    return `settings.option.${name}.${segment(value)}`;
  }
  return `plugin.${plugin}.settings.option.${name.slice(plugin.length + 1)}.${segment(value)}`;
}

/** The name of an option: `(name, value)` -> its text, `value` itself where the catalogue has
 * none or `name` is not given; with `settings.showKeys` the id follows in brackets. */
export function useOptionName(): (name: string | undefined, value: JsonValue) => string {
  const { t, te } = useI18n();
  const naming = inject(OPTION_NAMING, null);
  return (name, value) => {
    const id = String(value);
    const key = name === undefined ? '' : optionKey(name, value, naming?.plugin());
    if (key === '' || !te(key)) {
      return id;
    }
    return naming?.showKeys() === true ? `${t(key)} (${id})` : t(key);
  };
}
