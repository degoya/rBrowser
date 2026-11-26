// RB-090-16: every choice of the settings shows its name, not its id - a core key's from the
// shell's catalogue, a plugin key's from the plugin's own (Checks > SEO showed `imessage`,
// `teams`); the id only with `settings.showKeys`. The rule of the keys is the settings lint's
// (scripts/lib/settings_options.py), held here against the real schemas.
import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import USelect from '@nuxt/ui/components/Select.vue';
import seoCatalogue from '../../../plugins/seo/locales/en.json';
import seoSchema from '../../../plugins/seo/settings.schema.json';
import { shellMessages } from '../i18n';
import { PAGES_CATALOGUE } from '../i18n/pages';
import { takePluginTexts } from '../i18n/pluginTexts';
import type { JsonValue } from '../ipc/protocol';
import { enumOptions, itemsOf, kindOf, mapValuesOf, propertiesOf } from './kinds';
import { optionKey } from './optionNames';
import type { JsonSchema, SettingDef } from './schema';
import SettingField from './SettingField.vue';
import { realDefs } from './test/fakeCore';
import { mountPage, settle } from './test/mountPage';

let current: VueWrapper | null = null;

afterEach(() => {
  current?.unmount();
  current = null;
});

/** A nested catalogue as dotted keys. */
function flatten(tree: unknown, prefix = ''): Record<string, string> {
  if (typeof tree === 'string') {
    return { [prefix]: tree };
  }
  if (typeof tree !== 'object' || tree === null) {
    return {};
  }
  return Object.assign({}, ...Object.entries(tree).map(([name, value]) => flatten(value, prefix === '' ? name : `${prefix}.${name}`)));
}

function realDef(key: string): SettingDef {
  const found = realDefs().find((candidate) => candidate.key === key);
  if (found === undefined) {
    throw new Error(`${key} is no key of the schemas`);
  }
  return found;
}

/** `seo.social.platforms` as the core sends it: the plugin's schema, its texts beside it. */
function platformsDef(): SettingDef {
  const social = (seoSchema.properties as Record<string, JsonSchema>).social as JsonSchema;
  const schema = (social.properties as Record<string, JsonSchema>).platforms as JsonSchema;
  return {
    key: 'seo.social.platforms',
    category: 'checks',
    section: 'seo',
    order: 17,
    scopeAllowed: ['project', 'local', 'user'],
    localOnly: false,
    protectedFromMcp: false,
    labelKey: 'plugin.seo.settings.social.platforms.label',
    descriptionKey: 'plugin.seo.settings.social.platforms.description',
    helpKey: 'plugin.seo.settings.social.platforms.help',
    schema,
    plugin: 'seo',
  };
}

/** The setting's row; the SEO plugin's settings texts arrive as the core sends them, beside the
 * schema, and the row is drawn again with its value. */
async function field(def: SettingDef, value: JsonValue, showKeys = false) {
  const props = { def, value: null, origin: 'default', issues: [], hasProject: false, showKeys };
  const { wrapper } = await mountPage(SettingField, props);
  current = wrapper;
  const texts = Object.fromEntries(
    Object.entries(flatten(seoCatalogue)).filter(([key]) => key.startsWith('plugin.seo.settings.')),
  );
  takePluginTexts({ type: 'settings', values: [], schema: { version: 1, texts } });
  await wrapper.setProps({ value });
  await settle();
  return wrapper;
}

function boxes(wrapper: VueWrapper): string[] {
  return wrapper.findAll('[data-testid="multi-select"] label').map((label) => label.text());
}

describe('the names of the choices', () => {
  it('reads a core option, a nested one and a plugin option from their catalogues', () => {
    expect(optionKey('ui.theme', 'dark')).toBe('settings.option.ui.theme.dark');
    expect(optionKey('breakpoints.media.contrast', 'no-preference')).toBe(
      'settings.option.breakpoints.media.contrast.no-preference',
    );
    expect(optionKey('emulation.paper', 'A4')).toBe('settings.option.emulation.paper.a4');
    expect(optionKey('seo.social.platforms', 'teams', 'seo')).toBe('plugin.seo.settings.option.social.platforms.teams');
  });

  it('shows the social platforms of the SEO check by name', async () => {
    const wrapper = await field(platformsDef(), ['facebook', 'imessage']);
    const names = boxes(wrapper);
    expect(names).toContain('iMessage');
    expect(names).toContain('Microsoft Teams');
    expect(names).not.toContain('teams');
    expect(names).not.toContain('imessage');
  });

  it('adds the id to the name with settings.showKeys', async () => {
    const wrapper = await field(platformsDef(), [], true);
    expect(boxes(wrapper)).toContain('Microsoft Teams (teams)');
  });

  it('names the options of a core select', async () => {
    const wrapper = await field(realDef('ui.theme'), 'dark');
    // A Nuxt UI `USelect` (RB-080-88): its items carry the names.
    const select = wrapper.findComponent(USelect) as unknown as VueWrapper;
    const items = (select.props() as Record<string, unknown>).items as { label: string }[];
    expect(items.map((item) => item.label)).toEqual(['System', 'Light', 'Dark']);
  });

  it('has a name for every option a core key shows', () => {
    const messages = flatten(shellMessages([PAGES_CATALOGUE]));
    const missing: string[] = [];
    const walk = (schema: JsonSchema, name: string): void => {
      const options = enumOptions(schema);
      if (options !== null) {
        for (const { value } of options) {
          if (typeof value === 'string' && messages[optionKey(name, value)] === undefined) {
            missing.push(optionKey(name, value));
          }
        }
        return;
      }
      const kind = kindOf(schema);
      if (kind === 'multiEnum' || kind === 'objectList' || kind === 'stringList') {
        walk(itemsOf(schema), name);
      } else if (kind === 'choiceMap' || kind === 'objectMap') {
        walk(mapValuesOf(schema), name);
      } else if (kind === 'object') {
        for (const [child, childSchema] of propertiesOf(schema)) {
          walk(childSchema, `${name}.${child}`);
        }
      }
    };
    for (const def of realDefs()) {
      walk(def.schema, def.key);
    }
    expect(missing).toEqual([]);
  });
});
