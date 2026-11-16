import { afterEach, describe, expect, it } from 'vitest';
import ui from '@nuxt/ui/vue-plugin';
import { mount, type VueWrapper } from '@vue/test-utils';
import { createShellI18n } from '../i18n';
import type { JsonValue } from '../ipc/protocol';
import MapControl from './MapControl.vue';

let current: VueWrapper | null = null;

afterEach(() => {
  current?.unmount();
  current = null;
});

const PRESETS = {
  review: { mode: 'grid', layout: { columns: 2, rows: 1 } },
  phone: { mode: 'focus', focus: 'mobile' },
};

function open(value: { [name: string]: JsonValue } = PRESETS) {
  const wrapper = mount(MapControl, {
    props: { value, label: 'Presets', inputId: 'setting-layouts-presets', fresh: () => ({}) },
    attachTo: document.body,
    global: { plugins: [ui, createShellI18n()] },
  });
  current = wrapper;
  return wrapper;
}

function last(wrapper: VueWrapper): { [name: string]: JsonValue } | undefined {
  const all = wrapper.emitted('update') ?? [];
  return all[all.length - 1]?.[0] as { [name: string]: JsonValue } | undefined;
}

describe('a map of named objects as one-line entries (RB-080-13)', () => {
  it('shows each preset as its name and summary', () => {
    const wrapper = open();
    expect(wrapper.findAll('[data-testid="list-toggle"]').map((toggle) => toggle.text())).toEqual([
      'review · grid · 2 × 1 grid',
      'phone · focus · mobile',
    ]);
  });

  it('adds a preset under a free name and keeps the order when one moves', async () => {
    const wrapper = open();
    await wrapper.get('[data-testid="list-add"]').trigger('click');
    expect(Object.keys(last(wrapper) ?? {})).toEqual(['review', 'phone', 'new']);
    await wrapper.findAll('[data-testid="list-down"]')[0]!.trigger('click');
    expect(Object.keys(last(wrapper) ?? {})).toEqual(['phone', 'review']);
  });

  it('renames a preset in place and refuses an empty or taken name', async () => {
    const wrapper = open();
    await wrapper.findAll('[data-testid="list-toggle"]')[0]!.trigger('click');
    const name = wrapper.get('[data-testid="map-name"]');
    await name.setValue('phone');
    expect(wrapper.emitted('update')).toBeUndefined();
    expect(wrapper.get('[data-testid="map-name-refused"]').attributes('role')).toBe('alert');
    expect((name.element as HTMLInputElement).value).toBe('review');
    await name.setValue('wide');
    expect(last(wrapper)).toEqual({ wide: PRESETS.review, phone: PRESETS.phone });
  });
});
