import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { mountWithShell, settle } from '../test/mountShell';
import { closeNativeMenu, lastNativeMenu } from '../test/nativeMenu';
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

/** Opens the pulldown `selector` and chooses `id` in the core's native menu (RB-090-45). */
async function choose({ wrapper, mock }: Mounted, selector: string, id: string): Promise<void> {
  await wrapper.get(`${selector} button`).trigger('click');
  await settle();
  await closeNativeMenu(mock.sent, id);
}

/** The rows of the native menu asked for last: id and check. */
function rows({ mock }: Mounted): [string, boolean | undefined][] {
  return lastNativeMenu(mock.sent).rows.map((row) => (row.kind === 'item' ? [row.id, row.checked] : [row.kind, undefined]));
}

async function enterFocusMode({ mock }: Mounted, focus = 'tablet'): Promise<void> {
  mock.state.layout = { ...mock.state.layout, mode: 'focus', focus };
  mock.emit({ type: 'state', ...structuredClone(mock.state) });
  await settle();
}

describe('the layout switcher', () => {
  it('shows the preset pulldown only when the project has presets', async () => {
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
    const mounted = await setup();
    const { wrapper, mock } = mounted;
    const button = () => wrapper.get('[data-testid="layout"] button');
    expect(button().text()).toBe('grid-main');
    expect(button().attributes('aria-label')).toBe('Layout preset: grid-main');
    expect(wrapper.find('[data-testid="layout"] select').exists()).toBe(false);
    // The menu checks the preset in effect; choosing it again runs it again (RB-080-16).
    await choose(mounted, '[data-testid="layout"]', 'choice:0');
    expect(rows(mounted)).toEqual([
      ['choice:0', true],
      ['choice:1', false],
    ]);
    expect(mock.sent.at(-1)).toEqual({ type: 'selectLayoutPreset', id: 'grid-main' });
    await choose(mounted, '[data-testid="layout"]', 'choice:1');
    expect(mock.sent.at(-1)).toEqual({ type: 'selectLayoutPreset', id: 'focus' });
    expect(button().text()).toBe('focus');
  });

  it('lets the only preset be chosen and says when none is in effect', async () => {
    const mounted = await setup();
    const { wrapper, mock } = mounted;
    mock.state.layout = { ...mock.state.layout, presets: ['only'], preset: null };
    mock.emit({ type: 'state', ...structuredClone(mock.state) });
    await settle();
    const button = () => wrapper.get('[data-testid="layout"] button');
    expect(button().text()).toBe('No preset');
    await choose(mounted, '[data-testid="layout"]', 'choice:0');
    expect(rows(mounted)).toEqual([['choice:0', false]]);
    expect(mock.sent.at(-1)).toEqual({ type: 'selectLayoutPreset', id: 'only' });
    expect(button().text()).toBe('only');
    await choose(mounted, '[data-testid="layout"]', 'choice:0');
    expect(mock.sent.filter((m) => m.type === 'selectLayoutPreset')).toHaveLength(2);
  });

  it('offers a preset with a name past the row id limit and ignores an unknown row (RB-090-57)', async () => {
    const mounted = await setup();
    const { mock } = mounted;
    const long = 'p'.repeat(300);
    mock.state.layout = { ...mock.state.layout, presets: ['grid-main', long], preset: 'grid-main' };
    mock.emit({ type: 'state', ...structuredClone(mock.state) });
    await settle();
    await choose(mounted, '[data-testid="layout"]', 'choice:1');
    expect(rows(mounted)).toEqual([
      ['choice:0', true],
      ['choice:1', false],
    ]);
    expect(mock.sent.at(-1)).toEqual({ type: 'selectLayoutPreset', id: long });
    const sent = mock.sent.length;
    await choose(mounted, '[data-testid="layout"]', long);
    await choose(mounted, '[data-testid="layout"]', 'choice:7');
    expect(mock.sent.slice(sent).filter((m) => m.type === 'selectLayoutPreset')).toEqual([]);
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
    const mounted = await setup();
    const { wrapper, mock } = mounted;
    expect(wrapper.find('[data-testid="environment-color"]').exists()).toBe(false);
    await choose(mounted, '[data-testid="environment"]', 'choice:1');
    expect(mock.sent.at(-1)).toEqual({ type: 'selectEnvironment', id: 'staging' });
    const swatch = wrapper.get('[data-testid="environment-color"]');
    expect(swatch.attributes('aria-label')).toBe('Environment colour #f59e0b');
  });

  it('names the environment in effect and lets the first one be chosen back', async () => {
    const mounted = await setup();
    const { wrapper, mock } = mounted;
    const button = () => wrapper.get('[data-testid="environment"] button');
    expect(button().text()).toBe('local');
    expect(button().attributes('aria-label')).toBe('Environment: local');
    await choose(mounted, '[data-testid="environment"]', 'choice:1');
    expect(rows(mounted)).toEqual([
      ['choice:0', true],
      ['choice:1', false],
    ]);
    expect(button().text()).toBe('staging');
    await choose(mounted, '[data-testid="environment"]', 'choice:0');
    expect(rows(mounted)).toEqual([
      ['choice:0', false],
      ['choice:1', true],
    ]);
    expect(mock.sent.at(-1)).toEqual({ type: 'selectEnvironment', id: 'local' });
  });

  it('offers an environment with a name past the row id limit (RB-090-57)', async () => {
    const mounted = await setup();
    const { mock } = mounted;
    const long = 'e'.repeat(300);
    mock.state.environments = [...mock.state.environments, { ...mock.state.environments[0]!, id: long }];
    mock.emit({ type: 'state', ...structuredClone(mock.state) });
    await settle();
    await choose(mounted, '[data-testid="environment"]', 'choice:2');
    expect(rows(mounted).map(([id]) => id)).toEqual(['choice:0', 'choice:1', 'choice:2']);
    expect(mock.sent.at(-1)).toEqual({ type: 'selectEnvironment', id: long });
  });
});
