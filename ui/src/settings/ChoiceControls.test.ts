// RB-080-72: known values are a choice with names, not free text - the plugin lists, the rule
// overrides of the checks, the footer's fields; a list never saves an empty entry, and a value
// the field does not allow can be removed with one click.
import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import USelect from '@nuxt/ui/components/Select.vue';
import USelectMenu from '@nuxt/ui/components/SelectMenu.vue';
import FieldControl from './FieldControl.vue';
import { kindOf } from './kinds';
import type { DynamicData, JsonSchema } from './schema';
import type { JsonValue } from '../ipc/protocol';
import { mountPage, settle } from './test/mountPage';

let current: VueWrapper | null = null;

afterEach(() => {
  current?.unmount();
  current = null;
});

async function control(schema: JsonSchema, value: JsonValue, name: string, dynamic?: DynamicData) {
  const { wrapper } = await mountPage(FieldControl, {
    schema,
    value,
    name,
    label: 'Field',
    inputId: 'f',
    ...(dynamic === undefined ? {} : { dynamic }),
  });
  current = wrapper;
  return wrapper;
}

function updates(wrapper: VueWrapper): unknown[] {
  return (wrapper.emitted('update') ?? []).map((args) => args[0]);
}

function menu(wrapper: VueWrapper) {
  return wrapper.findComponent(USelectMenu) as unknown as VueWrapper;
}

/** A prop of the select (its generic props type reads as `never` to `props()`). */
function prop(wrapper: VueWrapper, name: string): unknown {
  return (menu(wrapper).props() as Record<string, unknown>)[name];
}

/** Chooses `value` in the searchable select, as a click on an entry does. */
async function add(wrapper: VueWrapper, value: string): Promise<void> {
  menu(wrapper).vm.$emit('update:modelValue', value);
  await settle();
}

/** Types `text` and takes it as a new entry ("Create ..."), which an open list offers. */
async function create(wrapper: VueWrapper, text: string): Promise<void> {
  menu(wrapper).vm.$emit('create', text);
  await settle();
}

function offered(wrapper: VueWrapper): unknown {
  return prop(wrapper, 'items');
}

/** The labels of a `USelect`'s items. */
function selectLabels(select: VueWrapper): string[] {
  return ((select.props() as Record<string, unknown>).items as { label: string }[]).map((item) => item.label);
}

/** The visible labels of a multi-select's boxes, and the value each names in its tooltip. */
function boxLabels(wrapper: VueWrapper): { text: string; title: string | undefined }[] {
  return wrapper
    .findAll('[data-testid="multi-select"] label [title]')
    .map((label) => ({ text: label.text(), title: label.attributes('title') }));
}

const PANELS: JsonSchema = {
  type: 'array',
  items: { type: 'string', pattern: '^[a-z][a-z0-9]*(-[a-z0-9]+)*$' },
  uniqueItems: true,
  default: [],
  'x-options': 'plugin-panels',
};
const PANEL_PLUGINS: DynamicData = {
  options: [
    { value: 'console', label: 'Console' },
    { value: 'network-panel', label: 'Network' },
  ],
};

describe('a list of known values (x-options source)', () => {
  it('is a pick list, not free text', () => {
    expect(kindOf(PANELS)).toBe('pickList');
    expect(kindOf({ ...PANELS, 'x-options': undefined } as unknown as JsonSchema)).toBe('stringList');
  });

  it('names each chosen value, the id beside it, and marks one that no longer exists', async () => {
    const wrapper = await control(PANELS, ['console', 'removed-plugin'], 'plugins.panelsHidden', PANEL_PLUGINS);
    const rows = wrapper.findAll('[data-testid="pick-item"]');
    expect(rows.map((row) => row.get('[data-testid="pick-name"]').text())).toEqual(['Console', 'removed-plugin']);
    expect(rows[0]!.get('[data-testid="pick-id"]').text()).toBe('console');
    expect(rows[0]!.find('[data-testid="pick-missing"]').exists()).toBe(false);
    expect(rows[1]!.get('[data-testid="pick-missing"]').text()).toBe('Not found');
  });

  // Review 7: a searchable Nuxt UI select, searched by name and id; a choice adds at once.
  it('offers the values not chosen yet in a searchable select, by name and id, and adds one', async () => {
    const wrapper = await control(PANELS, ['console'], 'plugins.panelsHidden', PANEL_PLUGINS);
    expect(offered(wrapper)).toEqual([{ label: 'Network', value: 'network-panel', description: 'network-panel' }]);
    expect(prop(wrapper, 'filterFields')).toEqual(['label', 'value']);
    expect(prop(wrapper, 'searchInput')).toBeTruthy();
    await add(wrapper, 'network-panel');
    expect(updates(wrapper)).toEqual([['console', 'network-panel']]);
  });

  it('takes nothing but the values offered, and nothing empty', async () => {
    const wrapper = await control(PANELS, [], 'plugins.panelsHidden', PANEL_PLUGINS);
    expect(prop(wrapper, 'createItem')).toBe(false);
    await add(wrapper, '');
    expect(updates(wrapper)).toEqual([]);
  });

  // Review 8: an entry that is no text is kept and shown until removed by hand.
  it('keeps an entry that is no text, marked not allowed, and removes it on request', async () => {
    const wrapper = await control(PANELS, ['console', 3], 'plugins.panelsHidden', PANEL_PLUGINS);
    const rows = wrapper.findAll('[data-testid="pick-item"]');
    expect(rows[1]!.get('[data-testid="pick-name"]').text()).toBe('3');
    expect(rows[1]!.get('[data-testid="pick-not-allowed"]').text()).toBe('Not allowed');
    await add(wrapper, 'network-panel');
    expect(updates(wrapper)[0]).toEqual(['console', 3, 'network-panel']);
    await wrapper.findAll('[data-testid="pick-remove"]')[1]!.trigger('click');
    expect(updates(wrapper)[1]).toEqual(['console']);
  });

  it('says that the values are still being read', async () => {
    const wrapper = await control(PANELS, ['console'], 'plugins.panelsHidden', { options: [], loading: true });
    expect(prop(wrapper, 'loading')).toBe(true);
    expect(prop(wrapper, 'placeholder')).toBe('Loading…');
    expect(wrapper.find('[data-testid="pick-missing"]').exists()).toBe(false);
  });

  it('removes a chosen value', async () => {
    const wrapper = await control(PANELS, ['console', 'network-panel'], 'plugins.panelsHidden', PANEL_PLUGINS);
    await wrapper.findAll('[data-testid="pick-remove"]')[0]!.trigger('click');
    expect(updates(wrapper)).toEqual([['network-panel']]);
  });

  it('says when nothing is chosen', async () => {
    const wrapper = await control(PANELS, [], 'plugins.panelsHidden', PANEL_PLUGINS);
    expect(wrapper.get('[data-testid="pick-empty"]').text()).toBe('None');
  });
});

const RULES: JsonSchema = {
  type: 'object',
  additionalProperties: {
    oneOf: [
      { type: 'string', const: 'off' },
      { type: 'string', const: 'info' },
      { type: 'string', const: 'warning' },
      { type: 'string', const: 'error' },
    ],
  },
  'x-options': 'check-rules',
};
const RULE_OPTIONS: DynamicData = {
  open: true,
  options: [
    { value: 'seo.images.alt', label: 'Images have alt text' },
    { value: 'a11y-axe.color-contrast', label: 'Colour contrast' },
  ],
};

describe('the rule overrides (checks.rules)', () => {
  it('are a list of rule and choice, not a JSON box with null', async () => {
    expect(kindOf(RULES)).toBe('choiceMap');
    const wrapper = await control(RULES, null, 'checks.rules', RULE_OPTIONS);
    expect(wrapper.find('textarea').exists()).toBe(false);
    expect(wrapper.text()).not.toContain('null');
    expect(wrapper.get('[data-testid="pick-empty"]').text()).toBe('No rule changed');
    expect(wrapper.get('[data-testid="pick-add-label"]').text()).toBe('Add rule');
    expect(prop(wrapper, 'createItem')).toBe('always');
  });

  it('names each rule and offers off and the severities', async () => {
    const wrapper = await control(RULES, { 'seo.images.alt': 'warning' }, 'checks.rules', RULE_OPTIONS);
    const row = wrapper.get('[data-testid="pick-item"]');
    expect(row.get('[data-testid="pick-name"]').text()).toBe('Images have alt text');
    expect(row.get('[data-testid="pick-id"]').text()).toBe('seo.images.alt');
    // RB-080-88: a Nuxt UI `USelect`, named for its rule.
    const select = wrapper.findComponent(USelect) as VueWrapper;
    expect(selectLabels(select)).toEqual(['Off', 'Info', 'Warning', 'Error']);
    expect((select.props() as Record<string, unknown>).modelValue).toBe('2');
    expect(row.get('[data-testid="pick-choice"]').attributes('aria-label')).toBe('Setting for Images have alt text');
    select.vm.$emit('update:modelValue', '3');
    expect(updates(wrapper)).toEqual([{ 'seo.images.alt': 'error' }]);
  });

  it('adds a rule switched off, takes a rule of a plugin not loaded, and removes one', async () => {
    const wrapper = await control(RULES, { 'seo.images.alt': 'warning' }, 'checks.rules', RULE_OPTIONS);
    await add(wrapper, 'a11y-axe.color-contrast');
    expect(updates(wrapper)[0]).toEqual({ 'seo.images.alt': 'warning', 'a11y-axe.color-contrast': 'off' });
    await create(wrapper, 'other.rule');
    expect(updates(wrapper)[1]).toEqual({ 'seo.images.alt': 'warning', 'other.rule': 'off' });
    await wrapper.get('[data-testid="pick-remove"]').trigger('click');
    expect(updates(wrapper)[2]).toEqual({});
  });
});

describe('a list of the allowed values (enum items)', () => {
  const FIELDS: JsonSchema = {
    type: 'array',
    uniqueItems: true,
    items: {
      oneOf: [
        { type: 'string', const: 'breakpoint' },
        { type: 'string', const: 'range' },
        { type: 'string', const: 'size' },
      ],
    },
  };

  it('names the values', async () => {
    const wrapper = await control(FIELDS, ['breakpoint'], 'ui.viewFooter.fields');
    const labels = boxLabels(wrapper);
    expect(labels.map((label) => label.text)).toEqual(['Breakpoint', 'Width range', 'Size']);
    expect(labels[0]?.title).toBe('breakpoint');
  });

  // Review 1: a nested field is named by its full key, so its values find their names.
  it('names the values of a nested field by its full key', async () => {
    const lighthouse: JsonSchema = {
      type: 'object',
      properties: {
        categories: {
          type: 'array',
          items: { type: 'string', enum: ['performance', 'accessibility', 'best-practices', 'seo'] },
        },
      },
    };
    const wrapper = await control(lighthouse, { categories: ['seo'] }, 'checks.lighthouse');
    expect(boxLabels(wrapper).map((label) => label.text)).toEqual(['Performance', 'Accessibility', 'Best practices', 'SEO']);
  });

  // The owner's project held `"categories": [""]`, from the free-text list of before RB-080-20.
  it('removes a value it does not allow with one click', async () => {
    const lighthouse: JsonSchema = {
      type: 'array',
      items: { type: 'string', enum: ['performance', 'accessibility', 'best-practices', 'seo'] },
    };
    const wrapper = await control(lighthouse, ['seo', ''], 'checks.lighthouse.categories');
    const remove = wrapper.get('[data-testid="remove-invalid"]');
    expect(remove.text()).toBe('Remove invalid value');
    await remove.trigger('click');
    expect(updates(wrapper)).toEqual([['seo']]);
  });
});

describe('a list of free strings', () => {
  const URLS: JsonSchema = { type: 'array', items: { type: 'string' }, default: [] };

  it('saves no empty entry: Add opens a line that is saved once it holds text', async () => {
    const wrapper = await control(URLS, ['a'], 'reload.watch');
    await wrapper.get('[data-testid="list-add"]').trigger('click');
    await settle();
    expect(updates(wrapper)).toEqual([]);
    const fresh = wrapper.get('#f-1');
    // `setValue` fires input and change, as leaving the field does.
    await fresh.setValue('b');
    await settle();
    expect(updates(wrapper)).toEqual([['a', 'b']]);
  });

  it('drops an entry emptied by hand instead of saving it empty', async () => {
    const wrapper = await control(URLS, ['a', 'b'], 'reload.watch');
    const first = wrapper.get('#f-0');
    await first.setValue('  ');
    await settle();
    expect(updates(wrapper)).toEqual([['b']]);
  });
});

describe('a value the window has no control for', () => {
  it('shows an empty JSON box for an unset value, never null', async () => {
    const wrapper = await control({ type: ['object', 'null'] }, null, 'x.y');
    expect((wrapper.get('textarea').element as HTMLTextAreaElement).value).toBe('');
  });
});

describe('a string of a few fixed values', () => {
  it('is a select of their names', async () => {
    const copy: JsonSchema = { type: 'string', enum: ['manual', 'onStart', 'onNavigate'], default: 'manual' };
    const wrapper = await control(copy, 'onStart', 'engines.cookies.copy');
    expect(selectLabels(wrapper.findComponent(USelect) as VueWrapper)).toEqual([
      'By hand',
      'When a view starts',
      'After every page load',
    ]);
  });
});
