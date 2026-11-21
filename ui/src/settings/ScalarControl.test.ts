import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import UInputMenu from '@nuxt/ui/components/InputMenu.vue';
import USelect from '@nuxt/ui/components/Select.vue';
import ScalarControl from './ScalarControl.vue';
import type { DynamicData, JsonSchema } from './schema';
import { mountPage, settle } from './test/mountPage';

let current: VueWrapper | null = null;

afterEach(() => {
  current?.unmount();
  current = null;
});

const RELEASE = 'C:\\Program Files\\Mozilla Firefox\\firefox.exe';
const DEVELOPER = 'C:\\Program Files\\Firefox Developer Edition\\firefox.exe';
const FOUND: DynamicData = {
  options: [
    { value: RELEASE, label: `Firefox 134.0 - ${RELEASE}` },
    { value: DEVELOPER, label: `Firefox Developer Edition 135.0b3 - ${DEVELOPER}` },
  ],
};

async function control(value: string | null, dynamic?: DynamicData) {
  const { wrapper } = await mountPage(ScalarControl, {
    schema: { type: ['string', 'null'], 'x-input': 'file', 'x-options': 'dynamic' },
    value,
    kind: 'path',
    label: 'Firefox path',
    inputId: 'f',
    dynamic,
  });
  current = wrapper;
  return wrapper;
}

// RB-080-32: the installed Firefox editions the core found, offered at engines.firefox.path.
describe('a path with the choices the core found', () => {
  // RB-080-88: a `UInputMenu` with free entry in place of the datalist.
  it('offers every installation as a labelled suggestion and takes the chosen path', async () => {
    const wrapper = await control(null, FOUND);
    const menu = wrapper.findComponent(UInputMenu) as unknown as VueWrapper;
    const items = (menu.props() as Record<string, unknown>).items as { value: string; label: string }[];
    expect(items.map((item) => item.value)).toEqual([RELEASE, DEVELOPER]);
    expect(items[1]?.label).toContain('Developer Edition');
    expect(wrapper.get('#f').attributes('aria-label')).toBe('Firefox path');
    menu.vm.$emit('update:modelValue', DEVELOPER);
    menu.vm.$emit('blur', new FocusEvent('blur'));
    await settle();
    expect(wrapper.emitted('update')).toEqual([[DEVELOPER]]);
  });

  it('shows the first installation as the placeholder of an empty field', async () => {
    const wrapper = await control(null, FOUND);
    expect(wrapper.get('#f').attributes('placeholder')).toBe(`Found: ${RELEASE}`);
  });

  it('says when nothing was found and leaves Browse', async () => {
    const wrapper = await control(null, { options: [] });
    expect(wrapper.get('#f').attributes('placeholder')).toBe('Not found - use Browse to set the path');
    expect(wrapper.find('[data-testid="suggestions"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="browse"]').exists()).toBe(true);
  });

  it('marks a configured path that no longer exists, until the value changes', async () => {
    const wrapper = await control('D:\\gone\\firefox.exe', { ...FOUND, missing: 'D:\\gone\\firefox.exe' });
    expect(wrapper.get('[data-testid="path-missing"]').text()).toBe(
      'This file does not exist; choose one of the found installations or Browse.',
    );
    expect(wrapper.get('#f').attributes('placeholder')).toBeUndefined();
    await wrapper.setProps({ value: RELEASE });
    expect(wrapper.find('[data-testid="path-missing"]').exists()).toBe(false);
  });

  it('is a plain path field without the data, committed on change', async () => {
    const wrapper = await control(null);
    const input = wrapper.get('#f');
    expect(input.attributes('placeholder')).toBeUndefined();
    expect(wrapper.find('[data-testid="suggestions"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="path-missing"]').exists()).toBe(false);
    (input.element as HTMLInputElement).value = 'C:\\ff.exe';
    await input.trigger('input');
    expect(wrapper.emitted('update')).toBeUndefined();
    await input.trigger('change');
    expect(wrapper.emitted('update')).toEqual([['C:\\ff.exe']]);
  });

  it('flags the missing path on the field itself', async () => {
    const wrapper = await control('D:\\gone\\firefox.exe', { ...FOUND, missing: 'D:\\gone\\firefox.exe' });
    expect(wrapper.get('#f').attributes('aria-invalid')).toBe('true');
  });
});

async function scalar(schema: JsonSchema, value: string | null, kind: string, name = 'x.y') {
  const { wrapper } = await mountPage(ScalarControl, { schema, value, kind, label: 'Field', inputId: 'f', name });
  current = wrapper;
  return wrapper;
}

// RB-080-88: the other string kinds on Nuxt UI, still committed on change.
describe('the other string kinds', () => {
  it('chooses an enum in a USelect by its names, with (not set) where the value may be empty', async () => {
    const schema: JsonSchema = { type: ['string', 'null'], enum: ['manual', 'onStart', 'onNavigate'] };
    const wrapper = await scalar(schema, 'onStart', 'enum', 'engines.cookies.copy');
    const select = wrapper.findComponent(USelect);
    const items = (select.props() as Record<string, unknown>).items as { label: string; value: string }[];
    expect(items.map((item) => item.label)).toEqual([
      '(not set)',
      'By hand',
      'When a view starts',
      'After every page load',
    ]);
    expect((select.props() as Record<string, unknown>).modelValue).toBe('1');
    expect(wrapper.get('#f').attributes('aria-label')).toBe('Field');
    select.vm.$emit('update:modelValue', '2');
    select.vm.$emit('update:modelValue', '-1');
    expect(wrapper.emitted('update')).toEqual([['onNavigate'], [null]]);
  });

  // RB-090-58: the picker in a popover behind a swatch button; the hex field types the value.
  it('picks the colour behind a swatch button and types the hex beside it', async () => {
    const wrapper = await scalar({ type: 'string' }, '#336699', 'color');
    expect(wrapper.find('input[type="color"]').exists()).toBe(false);
    expect(wrapper.get('[data-testid="color-pick"]').attributes('aria-label')).toBeTruthy();
    const hex = wrapper.get('#f');
    expect((hex.element as HTMLInputElement).value).toBe('#336699');
    (hex.element as HTMLInputElement).value = '#000000';
    await hex.trigger('change');
    expect(wrapper.emitted('update')).toEqual([['#000000']]);
  });

  it('commits a plain text on change and an emptied nullable text as null', async () => {
    const wrapper = await scalar({ type: ['string', 'null'] }, 'a', 'string');
    const input = wrapper.get('#f');
    (input.element as HTMLInputElement).value = '';
    await input.trigger('change');
    expect(wrapper.emitted('update')).toEqual([[null]]);
  });
});

// RB-090-84: `automation.flows.runLayout` - a string whose choices come from the `layouts` source.
describe('a string with the values of a source', () => {
  it('offers the source values as suggestions', async () => {
    const { wrapper } = await mountPage(ScalarControl, {
      schema: { type: 'string', 'x-options': 'layouts' },
      value: 'grid',
      kind: 'string',
      label: 'Run layout',
      inputId: 'l',
      dynamic: { options: [{ value: 'grid', label: 'grid' }, { value: 'review', label: 'review' }] },
    });
    current = wrapper;
    const menu = wrapper.findComponent(UInputMenu) as unknown as VueWrapper;
    const items = (menu.props() as Record<string, unknown>).items as { value: string }[];
    expect(items.map((item) => item.value)).toEqual(['grid', 'review']);
  });
});
