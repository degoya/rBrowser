import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { mountWithShell, settle } from '../test/mountShell';
import ShellToolbar from './ShellToolbar.vue';

let current: VueWrapper | null = null;

async function setup() {
  const mounted = await mountWithShell(ShellToolbar);
  current = mounted.wrapper;
  await settle();
  return mounted;
}

afterEach(() => {
  current?.unmount();
  current = null;
});

type Mounted = Awaited<ReturnType<typeof setup>>;

async function enterFocusMode({ mock }: Mounted, focus = 'tablet'): Promise<void> {
  mock.state.layout = { ...mock.state.layout, mode: 'focus', focus };
  mock.emit({ type: 'state', ...structuredClone(mock.state) });
  await settle();
}

describe('the layout switcher', () => {
  it('shows the preset select only when the project has presets', async () => {
    const mounted = await setup();
    const { wrapper, mock } = mounted;
    expect(wrapper.find('[data-testid="layout"]').exists()).toBe(true);
    mock.state.layout = { ...mock.state.layout, presets: [], preset: null };
    mock.emit({ type: 'state', ...structuredClone(mock.state) });
    await settle();
    expect(wrapper.find('[data-testid="layout"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="layout-modes"]').exists()).toBe(true);
  });

  it('names the preset in effect and lets it be chosen again, the first one included', async () => {
    const { wrapper, mock } = await setup();
    const select = wrapper.get<HTMLSelectElement>('[data-testid="layout"] select');
    // The list shows the preset in effect in a placeholder of its own, so choosing an entry -
    // the shown one too - always changes the select's value (RB-080-16).
    expect(select.element.value).toBe('');
    expect(select.element.selectedOptions[0]?.text.trim()).toBe('grid-main');
    await select.setValue('grid-main');
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'selectLayoutPreset', id: 'grid-main' });
    expect(select.element.value).toBe('');
    await select.setValue('focus');
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'selectLayoutPreset', id: 'focus' });
    expect(select.element.selectedOptions[0]?.text.trim()).toBe('focus');
    expect(select.findAll('option').map((o) => o.attributes('value'))).toEqual(['', 'grid-main', 'focus']);
  });

  it('lets the only preset be chosen and says when none is in effect', async () => {
    const mounted = await setup();
    const { wrapper, mock } = mounted;
    mock.state.layout = { ...mock.state.layout, presets: ['only'], preset: null };
    mock.emit({ type: 'state', ...structuredClone(mock.state) });
    await settle();
    const select = wrapper.get<HTMLSelectElement>('[data-testid="layout"] select');
    expect(select.element.selectedOptions[0]?.text.trim()).toBe('No preset');
    await select.setValue('only');
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'selectLayoutPreset', id: 'only' });
    expect(select.element.selectedOptions[0]?.text.trim()).toBe('only');
    await select.setValue('only');
    await settle();
    expect(mock.sent.filter((m) => m.type === 'selectLayoutPreset')).toHaveLength(2);
  });

  it('asks for a preset name through the save-preset command', async () => {
    const { wrapper, mock } = await setup();
    const button = wrapper.get('[data-testid="save-preset-open"]');
    expect(button.attributes('aria-label')).toBe('Save layout as preset');
    await button.trigger('click');
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'runCommand', id: 'layout.save_preset' });
  });

  it('toggles focus mode and zooms to fit through their commands', async () => {
    const mounted = await setup();
    const { wrapper, mock } = mounted;
    const toggle = wrapper.get('[data-testid="focus-mode"]');
    expect(toggle.attributes('aria-pressed')).toBe('false');
    await toggle.trigger('click');
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'runCommand', id: 'layout.focus_mode' });
    await wrapper.get('[data-testid="zoom-to-fit"]').trigger('click');
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'runCommand', id: 'layout.zoom_to_fit' });
    expect(wrapper.find('[data-testid="focus-next"]').exists()).toBe(false);
  });

  it('shows the quick switcher in focus mode', async () => {
    const mounted = await setup();
    await enterFocusMode(mounted);
    const { wrapper, mock } = mounted;
    expect(wrapper.get('[data-testid="focus-mode"]').attributes('aria-pressed')).toBe('true');
    expect(wrapper.get('[data-testid="focus-current"]').text()).toBe('Showing Tablet');
    await wrapper.get('[data-testid="focus-next"]').trigger('click');
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'runCommand', id: 'layout.focus_next' });
    await wrapper.get('[data-testid="focus-prev"]').trigger('click');
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'runCommand', id: 'layout.focus_prev' });
  });

  it('steps with the arrow keys in focus mode, but not in the address bar', async () => {
    const mounted = await setup();
    const { wrapper, mock } = mounted;
    const before = mock.sent.length;
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight' }));
    await settle();
    expect(mock.sent.length).toBe(before);
    await enterFocusMode(mounted);
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight' }));
    await settle();
    expect(mock.sent.at(-1)).toEqual({
      type: 'runCommand',
      id: 'layout.focus_next',
      args: { source: 'arrowKey' },
    });
    const count = mock.sent.length;
    const input = wrapper.get('input[type="text"]').element;
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }));
    await settle();
    expect(mock.sent.length).toBe(count);
  });
});

describe('the environment switcher', () => {
  it('shows the colour of an environment that has one', async () => {
    const { wrapper, mock } = await setup();
    expect(wrapper.find('[data-testid="environment-color"]').exists()).toBe(false);
    await wrapper.get('[data-testid="environment"] select').setValue('staging');
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'selectEnvironment', id: 'staging' });
    const swatch = wrapper.get('[data-testid="environment-color"]');
    expect(swatch.attributes('aria-label')).toBe('Environment colour #f59e0b');
  });

  it('names the environment in effect and lets the first one be chosen back', async () => {
    const { wrapper, mock } = await setup();
    const select = wrapper.get<HTMLSelectElement>('[data-testid="environment"] select');
    expect(select.element.value).toBe('');
    expect(select.element.selectedOptions[0]?.text.trim()).toBe('local');
    await select.setValue('staging');
    await settle();
    expect(select.element.selectedOptions[0]?.text.trim()).toBe('staging');
    await select.setValue('local');
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'selectEnvironment', id: 'local' });
    expect(select.element.value).toBe('');
  });
});
