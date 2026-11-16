import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import type { CoreToShell, StateSnapshot } from '../ipc/protocol';
import { sampleState } from '../ipc/mockState';
import type { MockOptions } from '../ipc/mockTransport';
import { mountWithShell, settle } from '../test/mountShell';
import SavePresetBar from './SavePresetBar.vue';
import { presetName, suggestedName } from './savePreset';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

async function mounted(mock: MockOptions = {}) {
  let push: (message: CoreToShell) => void = () => undefined;
  const subscribe = (listener: (message: CoreToShell) => void) => {
    push = listener;
    return () => undefined;
  };
  const result = await mountWithShell(SavePresetBar, { mock, mount: { props: { subscribe } } });
  current = result.wrapper;
  const open = async () => {
    push({ type: 'uiCommand', id: 'layout.save_preset' });
    await settle();
  };
  return { ...result, open, push: (message: CoreToShell) => push(message) };
}

function canvasState(): StateSnapshot {
  const state = sampleState();
  state.layout = { preset: null, presets: [], mode: 'canvas', focus: null };
  return state;
}

describe('the save-preset bar', () => {
  it('opens only on its command, with the selected preset as the name', async () => {
    const { wrapper, open, push } = await mounted();
    push({ type: 'uiCommand', id: 'view.set_size' });
    await settle();
    expect(wrapper.find('[data-testid="save-preset"]').exists()).toBe(false);
    await open();
    const field = wrapper.get<HTMLInputElement>('[data-testid="save-preset-name"]');
    expect(field.element.value).toBe('grid-main');
    expect(document.activeElement).toBe(field.element);
    expect(field.element.selectionStart).toBe(0);
    expect(field.element.selectionEnd).toBe('grid-main'.length);
    expect(wrapper.get('[data-testid="save-preset-share"]').attributes('aria-checked')).toBe('false');
  });

  it('is drawn with Nuxt UI: a field, a switch named for sharing and buttons (RB-080-88)', async () => {
    const { wrapper, open } = await mounted();
    await open();
    expect(wrapper.get('[data-testid="save-preset-name"]').attributes('data-slot')).toBe('base');
    const share = wrapper.get('[data-testid="save-preset-share"]');
    expect(share.attributes('role')).toBe('switch');
    const label = wrapper.get(`label[for="${share.attributes('id')}"]`);
    expect(label.text()).toBe('Share with the team (config.json)');
    for (const id of ['save-preset-save', 'save-preset-cancel']) {
      expect(wrapper.get(`[data-testid="${id}"]`).attributes('data-slot')).toBe('base');
    }
    expect(wrapper.get('[data-testid="save-preset-save"]').attributes('type')).toBe('submit');
  });

  it('names the mode without a preset and shares as the setting says', async () => {
    const { wrapper, open } = await mounted({
      state: canvasState(),
      settings: { 'layoutModes.savePresetTo': 'project' },
    });
    await open();
    expect(wrapper.get<HTMLInputElement>('[data-testid="save-preset-name"]').element.value).toBe('canvas');
    expect(wrapper.get('[data-testid="save-preset-share"]').attributes('aria-checked')).toBe('true');
  });

  it('sends the command with the name and the file on Enter', async () => {
    const { wrapper, mock, open } = await mounted();
    await open();
    await wrapper.get('[data-testid="save-preset-name"]').setValue('  my board ');
    await wrapper.get('[data-testid="save-preset-share"]').trigger('click');
    await wrapper.get('form').trigger('submit');
    await settle();
    expect(mock.sent.at(-1)).toEqual({
      type: 'runCommand',
      id: 'layout.save_preset',
      args: { id: 'my board', file: 'project' },
    });
    // The mock core refuses the command: the message reaches the status bar, the bar stays.
    expect(mock.reports.length).toBeGreaterThan(0);
    expect(wrapper.find('[data-testid="save-preset"]').exists()).toBe(true);
  });

  it('marks a name the core would refuse and sends nothing', async () => {
    const { wrapper, mock, open } = await mounted();
    await open();
    const before = mock.sent.length;
    await wrapper.get('[data-testid="save-preset-name"]').setValue('a/b');
    await wrapper.get('form').trigger('submit');
    await settle();
    expect(mock.sent.length).toBe(before);
    expect(wrapper.get('[data-testid="save-preset-name"]').attributes('aria-invalid')).toBe('true');
    expect(wrapper.get('[data-testid="save-preset-hint"]').text()).toContain('1 to 40');
  });

  it('closes on Escape and on Cancel', async () => {
    const { wrapper, open } = await mounted();
    await open();
    await wrapper.get('[data-testid="save-preset-name"]').trigger('keydown', { key: 'Escape' });
    await settle();
    expect(wrapper.find('[data-testid="save-preset"]').exists()).toBe(false);
    await open();
    await wrapper.get('[data-testid="save-preset-cancel"]').trigger('click');
    await settle();
    expect(wrapper.find('[data-testid="save-preset"]').exists()).toBe(false);
  });
});

describe('the preset name rules', () => {
  it('trims and takes letters, digits, spaces, dots, dashes and underscores', () => {
    expect(presetName(' Board 2.1_x-y ')).toBe('Board 2.1_x-y');
    expect(presetName('Ωmega 2')).toBe('Ωmega 2');
    expect(presetName('x'.repeat(40))).toBe('x'.repeat(40));
  });

  it('refuses an empty, long or odd name', () => {
    for (const bad of ['', '   ', 'x'.repeat(41), 'a/b', 'quote"d', 'tab\there']) {
      expect(presetName(bad)).toBeNull();
    }
  });

  it('suggests the preset, else the mode', () => {
    expect(suggestedName({ preset: 'pair', presets: ['pair'], mode: 'split', focus: null })).toBe('pair');
    expect(suggestedName({ preset: null, presets: [], mode: 'split', focus: null })).toBe('split');
    expect(suggestedName(undefined)).toBe('grid');
  });
});
