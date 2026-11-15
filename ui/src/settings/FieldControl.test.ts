import { afterEach, describe, expect, it, vi } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import FieldControl from './FieldControl.vue';
import type { JsonSchema } from './schema';
import type { JsonValue } from '../ipc/protocol';
import { mountPage, settle } from './test/mountPage';

let current: VueWrapper | null = null;

afterEach(() => {
  current?.unmount();
  current = null;
});

async function control(schema: JsonSchema, value: JsonValue, pickPath?: (kind: string) => Promise<string | null>) {
  const { wrapper } = await mountPage(FieldControl, {
    schema,
    value,
    name: 'engines.firefox.profileDir',
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

  it('offers the Lighthouse categories as checkboxes and keeps their order', async () => {
    const wrapper = await control(LIGHTHOUSE, { categories: ['seo'] });
    const boxes = wrapper.findAll('[data-testid="multi-select"] input[type="checkbox"]');
    expect(boxes.map((box) => box.attributes('value'))).toEqual([
      'performance',
      'accessibility',
      'best-practices',
      'seo',
    ]);
    expect(boxes.map((box) => (box.element as HTMLInputElement).checked)).toEqual([false, false, false, true]);
    await boxes[0]?.setValue(true);
    expect(wrapper.emitted('update')?.at(-1)).toEqual([{ categories: ['performance', 'seo'] }]);
  });

  it('keeps a value the schema does not know as its own box', async () => {
    const wrapper = await control(CATEGORIES, ['pwa', 'seo']);
    const boxes = wrapper.findAll('input[type="checkbox"]');
    expect(boxes.map((box) => box.attributes('value'))).toContain('pwa');
    await boxes.find((box) => box.attributes('value') === 'seo')?.setValue(false);
    expect(wrapper.emitted('update')?.at(-1)).toEqual([['pwa']]);
  });

  it('names a value the schema does not know, an empty string included', async () => {
    const wrapper = await control(CATEGORIES, ['performance', '']);
    const labels = wrapper.findAll('[data-testid="multi-select"] label').map((label) => label.text());
    expect(labels).toContain('performance');
    expect(labels).toContain('"" (not an allowed value)');
  });

  it('suggests the values of x-options while the text stays free', async () => {
    const wrapper = await control({ type: ['string', 'null'], 'x-options': ['Win32', 'MacIntel'] }, null);
    const input = wrapper.get('#f');
    expect(input.attributes('list')).toBe('f-options');
    expect(wrapper.findAll('[data-testid="suggestions"] option').map((o) => o.attributes('value'))).toEqual([
      'Win32',
      'MacIntel',
    ]);
    await input.setValue('Linux armv81');
    expect(wrapper.emitted('update')).toEqual([['Linux armv81']]);
  });
});
