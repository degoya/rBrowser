// The schema the settings window renders from: the core's `getSettings { includeSchema: true }`
// payload (`crates/rbrowser/src/settings_window/values.rs`, version 1) - the categories with
// their sections in order, every key's definition with its own JSON schema, the open project.
// Nothing here is maintained by hand: a key added to the Rust schema shows up by itself.

import type { JsonValue, Scope } from '../ipc/protocol';
import { isObject } from '../../../plugins/web-common/src/is-object';

export type JsonSchema = { [key: string]: JsonValue };

/** One choice the core found for a field (`"x-options": "dynamic"`, RB-080-32, or a source
 * such as `"plugins"`, RB-080-72). */
export interface DynamicOption {
  /** What the setting takes (a path). */
  value: string;
  /** What the list shows (`<edition> <version> - <path>`). */
  label: string;
}

/** What the core adds to a dynamic field at runtime: its choices in search order (the first
 * is what an empty field means), and the configured value when it names nothing any more. */
export interface DynamicData {
  options: DynamicOption[];
  missing?: string;
  /** The values are usually known but open: a value not offered may be typed (a rule of a check
   * plugin not loaded, RB-080-72). Without it a pick list takes only the values offered. */
  open?: boolean;
  /** The core still enumerates the values (the check rules, RB-080-72); they follow. */
  loading?: boolean;
}

export interface SettingDef {
  key: string;
  category: string;
  section: string;
  order: number;
  scopeAllowed: Scope[];
  localOnly: boolean;
  protectedFromMcp: boolean;
  labelKey: string;
  descriptionKey: string;
  /** The help text: what the key is for, its effect, an example (RB-080-20). */
  helpKey: string;
  schema: JsonSchema;
  /** The plugin that contributed the key; absent for a core key. */
  plugin?: string;
  /**
   * The i18n key of why the key is locked in this build (self-update without its feature,
   * RB-090-02): shown under the field, which takes no change.
   */
  lockedKey?: string;
  /** The choices the core found at runtime, for a schema with `"x-options": "dynamic"`. */
  dynamic?: DynamicData;
}

export interface CategoryInfo {
  id: string;
  sections: string[];
}

export interface SettingsSchema {
  version: number;
  categories: CategoryInfo[];
  defs: SettingDef[];
  project: { root: string; hasLocal: boolean } | null;
}

/** The key the shortcut editor lives at. */
export const SHORTCUTS_KEY = 'shortcuts.bindings';
/** The section id of a plugin's own page under Plugins. */
export const PLUGIN_SECTION_PREFIX = 'plugin:';


function isDef(value: unknown): value is SettingDef {
  return (
    isObject(value) &&
    typeof value.key === 'string' &&
    typeof value.category === 'string' &&
    typeof value.section === 'string' &&
    typeof value.order === 'number' &&
    Array.isArray(value.scopeAllowed) &&
    typeof value.labelKey === 'string' &&
    typeof value.descriptionKey === 'string' &&
    typeof value.helpKey === 'string' &&
    isObject(value.schema)
  );
}

/** Reads the payload; throws when it is not the shape the window knows. */
export function parseSchema(value: JsonValue | undefined): SettingsSchema {
  if (!isObject(value) || value.version !== 1) {
    throw new Error('the settings schema is missing or of an unknown version');
  }
  const { categories, defs, project } = value;
  if (!Array.isArray(categories) || !Array.isArray(defs) || !defs.every(isDef)) {
    throw new Error('the settings schema is malformed');
  }
  const parsedCategories: CategoryInfo[] = [];
  for (const category of categories as unknown[]) {
    if (isObject(category) && typeof category.id === 'string' && Array.isArray(category.sections)) {
      parsedCategories.push({
        id: category.id,
        sections: category.sections.filter((section): section is string => typeof section === 'string'),
      });
    }
  }
  const parsedProject =
    isObject(project) && typeof project.root === 'string'
      ? { root: project.root, hasLocal: project.hasLocal === true }
      : null;
  return {
    version: 1,
    categories: parsedCategories,
    defs: defs as unknown as SettingDef[],
    project: parsedProject,
  };
}

export interface SectionView {
  /** The section id; `plugin:<id>` for a plugin's own page under Plugins. */
  id: string;
  /** The i18n key of the title; for a plugin page the plugin id itself. */
  titleKey: string;
  defs: SettingDef[];
  /** A plugin page repeats keys shown in their own category. */
  mirror: boolean;
}

export interface CategoryView {
  id: string;
  titleKey: string;
  sections: SectionView[];
}

function byOrder(a: SettingDef, b: SettingDef): number {
  return a.order - b.order || a.key.localeCompare(b.key);
}

/**
 * The categories in navigation order, each with its sections in order and the keys of each by
 * `x-order`. A plugin's keys show in their own category and again under Plugins > `<plugin>`.
 */
export function layoutOf(schema: SettingsSchema, include: (def: SettingDef) => boolean = () => true): CategoryView[] {
  const defs = schema.defs.filter(include);
  return schema.categories.map((category) => {
    const sections: SectionView[] = category.sections.map((section) => ({
      id: section,
      titleKey: `settings.section.${category.id}.${section}`,
      defs: defs
        .filter((def) => def.category === category.id && def.section === section)
        .sort(byOrder),
      mirror: false,
    }));
    if (category.id === 'plugins') {
      const plugins = [...new Set(defs.flatMap((def) => (def.plugin === undefined ? [] : [def.plugin])))];
      for (const plugin of plugins.sort()) {
        sections.push({
          id: `${PLUGIN_SECTION_PREFIX}${plugin}`,
          titleKey: plugin,
          defs: defs.filter((def) => def.plugin === plugin).sort(byOrder),
          mirror: true,
        });
      }
    }
    return {
      id: category.id,
      titleKey: `settings.category.${category.id}`,
      sections: sections.filter((section) => section.defs.length > 0),
    };
  });
}

/** The definition of `key`. */
export function findDef(schema: SettingsSchema, key: string): SettingDef | undefined {
  return schema.defs.find((def) => def.key === key);
}
