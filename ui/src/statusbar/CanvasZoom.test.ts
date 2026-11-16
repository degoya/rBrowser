import { afterEach, describe, expect, it, vi } from 'vitest';
import { mount, type VueWrapper } from '@vue/test-utils';
import { nextTick } from 'vue';
import ui from '@nuxt/ui/vue-plugin';
import { createShellI18n } from '../i18n';
import { IpcClient } from '../ipc/client';
import type { BoardState } from '../ipc/layout';
import { createMockTransport, type MockTransport } from '../ipc/mockTransport';
import { sampleState } from '../ipc/mockState';
import { MIN_PERCENT, clampPercent, percentOf } from './canvasZoom';
import CanvasZoom from './CanvasZoom.vue';

let current: VueWrapper | null = null;

async function flush(): Promise<void> {
  for (let i = 0; i < 6; i += 1) {
    await Promise.resolve();
    await nextTick();
  }
}

function board(levelPermille: number): BoardState {
  return { spacePan: false, items: [], camera: { x: 0, y: 0, zoomPermille: 500 }, levelPermille };
}

async function setup(level: number | null, settings: Record<string, unknown> = {}) {
  const state = sampleState();
  if (level !== null) {
    state.layout = { ...state.layout, mode: 'canvas', modes: { board: board(level) } };
  }
  const mock: MockTransport = createMockTransport({ state, settings });
  vi.spyOn(console, 'warn').mockImplementation(() => undefined);
  const wrapper = mount(CanvasZoom, {
    props: { client: new IpcClient(mock) },
    attachTo: document.body,
    global: { plugins: [ui, createShellI18n()] },
  });
  current = wrapper;
  await flush();
  return { wrapper, mock };
}

function commands(mock: MockTransport): unknown[] {
  return mock.sent.filter((message) => message.type === 'runCommand');
}

afterEach(() => {
  current?.unmount();
  current = null;
  vi.restoreAllMocks();
});

describe('the canvas zoom of the status bar', () => {
  it('reads the level in whole per cent and holds the slider to its range', () => {
    expect(percentOf(board(250))).toBe(25);
    expect(percentOf(board(1000))).toBe(100);
    expect(percentOf(board(0))).toBeNull();
    expect(percentOf(null)).toBeNull();
    expect(percentOf({ spacePan: false, items: [] })).toBeNull();
    expect(clampPercent(0.1)).toBe(MIN_PERCENT);
    expect(clampPercent(500)).toBe(200);
  });

  it('shows a level below 5 % as it is and the slider agrees with it', async () => {
    // Review finding: 0.4 % read as 0 % and the slider sat at 5 %, so touching it zoomed in.
    expect(percentOf(board(5))).toBe(0.5);
    expect(percentOf(board(25))).toBe(2.5);
    expect(percentOf(board(4))).toBe(0.4);
    expect(MIN_PERCENT).toBe(0.5);
    expect(clampPercent(2.5)).toBe(2.5);
    const { wrapper } = await setup(25);
    expect(wrapper.get('[data-testid="canvas-zoom-level"]').text()).toBe('2.5 %');
  });

  it('shows the level only on the canvas', async () => {
    const grid = await setup(null);
    expect(grid.wrapper.find('[data-testid="canvas-zoom"]').exists()).toBe(false);
    grid.wrapper.unmount();
    const { wrapper } = await setup(750);
    const level = wrapper.get('[data-testid="canvas-zoom-level"]');
    expect(level.text()).toBe('75 %');
    expect(level.attributes('title')).toBe('Canvas zoom 75 %: choose a zoom');
    expect(wrapper.get('[data-testid="canvas-zoom"]').attributes('aria-label')).toBe('Canvas zoom');
  });

  it('runs the commands of the buttons', async () => {
    const { wrapper, mock } = await setup(1000);
    await wrapper.get('[data-testid="canvas-zoom-in"]').trigger('click');
    await wrapper.get('[data-testid="canvas-zoom-out"]').trigger('click');
    await flush();
    expect(commands(mock)).toEqual([
      { type: 'runCommand', id: 'canvas.zoom.in' },
      { type: 'runCommand', id: 'canvas.zoom.out' },
    ]);
  });

  // RB-090-18: the presets were a native `<select>`, whose list the page placed below the field -
  // off the screen with the status bar at its bottom (fullscreen). The level is a button now that
  // asks the core for a native menu above its top edge, centred on it.
  it('opens the presets as the core native menu above the level button', async () => {
    const { wrapper, mock } = await setup(1000);
    expect(wrapper.find('[data-testid="canvas-zoom"] select').exists()).toBe(false);
    const level = wrapper.get('[data-testid="canvas-zoom-level"]');
    expect(level.attributes('aria-haspopup')).toBe('menu');
    vi.spyOn(level.element, 'getBoundingClientRect').mockReturnValue(new DOMRect(400.4, 2, 64, 20));
    await level.trigger('click');
    await flush();
    expect(mock.sent.filter((message) => message.type === 'canvasZoomMenu')).toEqual([
      { type: 'canvasZoomMenu', x: 432, y: 2 },
    ]);
    expect(commands(mock)).toEqual([]);
  });

  it('opens the presets from the keyboard as the select did', async () => {
    const { wrapper, mock } = await setup(1000);
    const level = wrapper.get('[data-testid="canvas-zoom-level"]');
    for (const key of ['ArrowDown', 'ArrowUp', 'F4']) {
      await level.trigger('keydown', { key });
    }
    await level.trigger('keydown', { key: 'a' });
    await flush();
    expect(mock.sent.filter((message) => message.type === 'canvasZoomMenu')).toHaveLength(3);
  });

  // RB-090-45: reka commits a key step (`change`) before the new value reaches the model, and
  // Nuxt UI's `change` carries no value - the slider sent the level it had before the step.
  it('sends the level an arrow key steps to, not the one before', async () => {
    const { wrapper, mock } = await setup(1000);
    await wrapper.get('[role="slider"]').trigger('keydown', { key: 'ArrowRight' });
    await flush();
    expect(commands(mock)).toEqual([{ type: 'runCommand', id: 'canvas.zoom.reset', args: { level: 1.005 } }]);
  });

  it('follows the board a gesture tries out and the next state', async () => {
    const { wrapper, mock } = await setup(500);
    mock.emit({ type: 'canvasLive', board: board(1250), seq: 1 });
    await flush();
    expect(wrapper.get('[data-testid="canvas-zoom-level"]').text()).toBe('125 %');
    mock.emit({ type: 'state', ...sampleState() });
    await flush();
    expect(wrapper.find('[data-testid="canvas-zoom"]').exists()).toBe(false);
  });

  it('stays away when the setting says so', async () => {
    const { wrapper } = await setup(1000, { 'ui.statusBar.canvasZoom': false });
    expect(wrapper.find('[data-testid="canvas-zoom"]').exists()).toBe(false);
  });
});
