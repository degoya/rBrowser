import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import ScalarControl from './ScalarControl.vue';
import type { DynamicData } from './schema';
import { mountPage } from './test/mountPage';

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
  it('offers every installation as a labelled suggestion and takes the chosen path', async () => {
    const wrapper = await control(null, FOUND);
    const input = wrapper.get('#f');
    expect(input.attributes('list')).toBe('f-options');
    const options = wrapper.findAll('[data-testid="suggestions"] option');
    expect(options.map((option) => option.attributes('value'))).toEqual([RELEASE, DEVELOPER]);
    expect(options[1]?.text()).toContain('Developer Edition');
    await input.setValue(DEVELOPER);
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

  it('is a plain path field without the data', async () => {
    const wrapper = await control(null);
    expect(wrapper.get('#f').attributes('placeholder')).toBeUndefined();
    expect(wrapper.get('#f').attributes('list')).toBeUndefined();
    expect(wrapper.find('[data-testid="path-missing"]').exists()).toBe(false);
  });
});
