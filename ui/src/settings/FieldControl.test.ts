import { afterEach, describe, expect, it, vi } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import UInputMenu from '@nuxt/ui/components/InputMenu.vue';
import FieldControl from './FieldControl.vue';
import type { JsonSchema } from './schema';
import type { JsonValue } from '../ipc/protocol';
import { mountPage, settle } from './test/mountPage';

let current: VueWrapper | null = null;

afterEach(() => {
  current?.unmount();
  current = null;
});

async function control(
  schema: JsonSchema,
  value: JsonValue,
  pickPath?: (kind: string) => Promise<string | null>,
  name = 'engines.firefox.profileDir',
) {
  const { wrapper } = await mountPage(FieldControl, {
    schema,
    value,
    name,
    label: 'Profile folder',
    inputId: 'f',
    pickPath,
  });
  current = wrapper;
  return wrapper;
}

const CATEGORIES: JsonSchema = {
  type: 'array',
  items: { type: 'string', enum: ['performance', 'accessibility', 'best-practices', 'seo'] },
  default: [],
};
const LIGHTHOUSE: JsonSchema = { type: 'object', properties: { categories: CATEGORIES } };

describe('the field controls (RB-080-20)', () => {
  it('opens the folder dialog for a folder path and the file dialog for a file', async () => {
    const pick = vi.fn(async (kind: string) => (kind === 'folder' ? 'C:\\Profiles\\dev' : 'C:\\firefox.exe'));
    let wrapper = await control({ type: ['string', 'null'], 'x-input': 'folder' }, null, pick);
    const browse = wrapper.get('[data-testid="browse"]');
    expect(browse.attributes('data-path-kind')).toBe('folder');
    await browse.trigger('click');
    await settle();
    expect(pick).toHaveBeenCalledWith('folder');
    expect(wrapper.emitted('update')).toEqual([['C:\\Profiles\\dev']]);
    wrapper.unmount();

    wrapper = await control({ type: ['string', 'null'], 'x-input': 'file' }, null, pick);
    await wrapper.get('[data-testid="browse"]').trigger('click');
    await settle();
    expect(pick).toHaveBeenLastCalledWith('file');
    expect(wrapper.emitted('update')).toEqual([['C:\\firefox.exe']]);
  });

  it('keeps the value when the dialog is cancelled', async () => {
    const wrapper = await control({ type: 'string', 'x-input': 'file' }, 'C:\\old.exe', async () => null);
    await wrapper.get('[data-testid="browse"]').trigger('click');
    await settle();
    expect(wrapper.emitted('update')).toBeUndefined();
    expect((wrapper.get('#f').element as HTMLInputElement).value).toBe('C:\\old.exe');
  });

  it('hands the dialog down to a path inside a list of objects', async () => {
    const pick = vi.fn(async () => 'C:\\ext');
    const schema: JsonSchema = {
      type: 'array',
      items: { type: 'object', properties: { path: { type: 'string', 'x-input': 'folder' } }, required: ['path'] },
    };
    const wrapper = await control(schema, [{ path: '' }], pick);
    await wrapper.get('[data-testid="list-toggle"]').trigger('click');
    await settle();
    const browse = wrapper.get('[data-testid="browse"]');
    expect(browse.attributes('data-path-kind')).toBe('folder');
    await browse.trigger('click');
    await settle();
    expect(wrapper.emitted('update')).toEqual([[[{ path: 'C:\\ext' }]]]);
  });

  // RB-080-88: a Nuxt UI `UCheckboxGroup`, each box named by its label.
  it('offers the Lighthouse categories as checkboxes and keeps their order', async () => {
    const wrapper = await control(LIGHTHOUSE, { categories: ['seo'] });
    const boxes = wrapper.findAll('[data-testid="multi-select"] [role="checkbox"]');
    expect(boxes.map((box) => wrapper.get(`label[for="${box.attributes('id')}"] [title]`).attributes('title'))).toEqual([
      'performance',
      'accessibility',
      'best-practices',
      'seo',
    ]);
    expect(boxes.map((box) => box.attributes('aria-checked'))).toEqual(['false', 'false', 'false', 'true']);
    await boxes[0]?.trigger('click');
    await settle();
    expect(wrapper.emitted('update')?.at(-1)).toEqual([{ categories: ['performance', 'seo'] }]);
  });

  it('keeps a value the schema does not know as its own box', async () => {
    const wrapper = await control(CATEGORIES, ['pwa', 'seo']);
    const boxes = wrapper.findAll('[role="checkbox"]');
    const titles = boxes.map((box) => wrapper.get(`label[for="${box.attributes('id')}"] [title]`).attributes('title'));
    expect(titles).toContain('pwa');
    await boxes[titles.indexOf('seo')]?.trigger('click');
    await settle();
    expect(wrapper.emitted('update')?.at(-1)).toEqual([['pwa']]);
  });

  it('is disabled as a whole with its setting', async () => {
    const { wrapper } = await mountPage(FieldControl, {
      schema: CATEGORIES,
      value: ['seo'],
      name: 'checks.lighthouse.categories',
      label: 'Categories',
      inputId: 'f',
      disabled: true,
    });
    current = wrapper;
    const boxes = wrapper.findAll('[role="checkbox"]');
    expect(boxes.every((box) => box.attributes('disabled') !== undefined)).toBe(true);
    expect(wrapper.get('[data-testid="multi-select"] legend').text()).toBe('Categories');
  });

  it('names a value the schema does not know, an empty string included', async () => {
    const wrapper = await control(CATEGORIES, ['performance', '']);
    const labels = wrapper.findAll('[data-testid="multi-select"] label').map((label) => label.text());
    expect(labels).toContain('performance');
    expect(labels).toContain('"" (not an allowed value)');
  });

  it('suggests the values of x-options while the text stays free', async () => {
    const wrapper = await control({ type: ['string', 'null'], 'x-options': ['Win32', 'MacIntel'] }, null);
    const menu = wrapper.findComponent(UInputMenu);
    const items = (menu.props() as Record<string, unknown>).items as { value: string }[];
    expect(items.map((item) => item.value)).toEqual(['Win32', 'MacIntel']);
    expect(wrapper.get('#f').attributes('aria-label')).toBe('Profile folder');
    menu.vm.$emit('update:modelValue', 'Linux armv81');
    menu.vm.$emit('blur', new FocusEvent('blur'));
    await settle();
    expect(wrapper.emitted('update')).toEqual([['Linux armv81']]);
  });
});

// RB-080-88: a number is a `UInputNumber`, its unit joined to it in a `UFieldGroup`; it commits on
// Enter or on leaving the field, as the native field did, and a refused value still reaches the
// core, which says why.
describe('a number field (RB-080-88)', () => {
  const THROTTLE: JsonSchema = { type: 'integer', minimum: 0, maximum: 1000 };

  it('shows the unit beside the field and no plus or minus on a measure', async () => {
    const wrapper = await control(THROTTLE, 50, undefined, 'sync.hoverThrottleMs');
    const input = wrapper.get('#f');
    expect(input.attributes('role')).toBe('spinbutton');
    expect(input.attributes('aria-label')).toBe('Profile folder');
    expect((input.element as HTMLInputElement).value).toBe('50');
    expect(wrapper.get('[data-testid="unit"]').text()).toBe('ms');
    expect(wrapper.find('[data-slot="increment"]').exists()).toBe(false);
  });

  it('commits on leaving the field, not while typing', async () => {
    const wrapper = await control(THROTTLE, 50, undefined, 'sync.hoverThrottleMs');
    const input = wrapper.get('#f');
    (input.element as HTMLInputElement).value = '120';
    await input.trigger('input');
    expect(wrapper.emitted('update')).toBeUndefined();
    await input.trigger('blur');
    await settle();
    expect(wrapper.emitted('update')).toEqual([[120]]);
  });

  it('lets a value outside the bounds through for the core to refuse', async () => {
    const wrapper = await control(THROTTLE, 50, undefined, 'sync.hoverThrottleMs');
    const input = wrapper.get('#f');
    (input.element as HTMLInputElement).value = '-3';
    await input.trigger('keydown', { key: 'Enter' });
    await settle();
    expect(wrapper.emitted('update')).toEqual([[-3]]);
  });

  it('offers plus and minus on a small count and has no unit there', async () => {
    const wrapper = await control({ type: 'integer', minimum: 1, maximum: 50 }, 5, undefined, 'logs.maxFiles');
    expect(wrapper.find('[data-testid="unit"]').exists()).toBe(false);
    // Plus steps on the press (and repeats while held), so a press and its release.
    await wrapper.get('[data-slot="increment"] button').trigger('pointerdown', { button: 0, pointerId: 1, pointerType: 'mouse' });
    window.dispatchEvent(new PointerEvent('pointerup', { pointerId: 1 }));
    await settle();
    expect(wrapper.emitted('update')).toEqual([[6]]);
  });

  it('writes no thousands separator into a port', async () => {
    const wrapper = await control({ type: 'integer', minimum: 1024, maximum: 65535 }, 9222, undefined, 'cdp.port');
    expect((wrapper.get('#f').element as HTMLInputElement).value).toBe('9222');
  });

  it('clears a nullable number to null and keeps a required one', async () => {
    let wrapper = await control({ type: ['number', 'null'] }, 1.5, undefined, 'emulation.dpr');
    expect((wrapper.get('#f').element as HTMLInputElement).value).toBe('1.5');
    let input = wrapper.get('#f');
    (input.element as HTMLInputElement).value = '';
    await input.trigger('blur');
    await settle();
    expect(wrapper.emitted('update')).toEqual([[null]]);
    wrapper.unmount();

    wrapper = await control({ type: 'number' }, 1.5, undefined, 'emulation.dpr');
    input = wrapper.get('#f');
    (input.element as HTMLInputElement).value = '';
    await input.trigger('blur');
    await settle();
    expect(wrapper.emitted('update')).toBeUndefined();
  });
});
