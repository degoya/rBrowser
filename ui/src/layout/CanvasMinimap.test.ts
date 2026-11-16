import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { mountWithShell, settle } from '../test/mountShell';
import ShellToolbar from '../toolbar/ShellToolbar.vue';
import type { MinimapState } from '../ipc/layout';

let current: VueWrapper | null = null;

const MAP: MinimapState = {
  position: 'end',
  items: [
    { view: 1, breakpoint: 'mobile', rect: { x: 0, y: 0, width: 200, height: 420 }, visible: true },
    { view: 2, breakpoint: 'desktop', rect: { x: 216, y: 0, width: 720, height: 470 }, visible: false },
  ],
  viewport: { x: 0, y: 0, width: 936, height: 470 },
};

async function setup(map: MinimapState | null = MAP) {
  const mounted = await mountWithShell(ShellToolbar);
  current = mounted.wrapper;
  mounted.mock.state.layout = { ...mounted.mock.state.layout, mode: 'canvas', modes: map ? { canvas: map } : {} };
  mounted.mock.emit({ type: 'state', ...structuredClone(mounted.mock.state) });
  await settle();
  return mounted;
}

afterEach(() => {
  current?.unmount();
  current = null;
});

function pointer(type: string, x: number, y: number): PointerEvent {
  // jsdom has no PointerEvent constructor everywhere; a MouseEvent carries the coordinates.
  return new MouseEvent(type, { clientX: x, clientY: y, bubbles: true }) as PointerEvent;
}

describe('the canvas minimap', () => {
  it('is shown once, at its position, with every view and the visible area', async () => {
    const { wrapper } = await setup();
    const maps = wrapper.findAll('[data-testid="minimap"]');
    expect(maps).toHaveLength(1);
    const map = maps[0]!;
    expect(map.attributes('tabindex')).toBe('0');
    expect(map.attributes('aria-label')).toBe('Canvas minimap');
    expect(wrapper.findAll('[data-testid^="minimap-item-"]')).toHaveLength(2);
    expect(wrapper.get('[data-testid="minimap-item-2"] title').text()).toBe('desktop');
    expect(wrapper.find('[data-testid="minimap-viewport"]').exists()).toBe(true);
  });

  it('is gone while the minimap is off', async () => {
    const { wrapper } = await setup(null);
    expect(wrapper.find('[data-testid="minimap"]').exists()).toBe(false);
  });

  it('centres the canvas on a clicked point', async () => {
    const { wrapper, mock } = await setup();
    const svg = wrapper.get('[data-testid="minimap"]').element;
    svg.dispatchEvent(pointer('pointerup', 72, 16));
    await settle();
    const sent = mock.sent.at(-1) as unknown as { id: string; args: { x: number; y: number } };
    expect(sent.id).toBe('layout.center');
    expect(Number.isInteger(sent.args.x) && Number.isInteger(sent.args.y)).toBe(true);
  });

  it('moves a dragged view on release, its label band above the point', async () => {
    const { wrapper, mock } = await setup();
    const item = wrapper.get('[data-testid="minimap-item-1"]').element;
    const svg = wrapper.get('[data-testid="minimap"]').element;
    item.dispatchEvent(pointer('pointerdown', 10, 10));
    svg.dispatchEvent(pointer('pointermove', 30, 10));
    svg.dispatchEvent(pointer('pointerup', 30, 10));
    await settle();
    const sent = mock.sent.at(-1) as unknown as { id: string; args: { view: number; x: number; y: number } };
    expect(sent.id).toBe('layout.canvas.place');
    expect(sent.args.view).toBe(1);
    expect(sent.args.x).toBeGreaterThan(0);
    expect(sent.args.y).toBe(20);
  });

  it('pans with the arrow keys and centres on the active view with Enter', async () => {
    const { wrapper, mock } = await setup();
    const map = wrapper.get('[data-testid="minimap"]');
    await map.trigger('keydown', { key: 'ArrowRight' });
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'runCommand', id: 'layout.center', args: { x: 702, y: 235 } });
    await map.trigger('keydown', { key: 'Enter' });
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'runCommand', id: 'layout.center' });
  });
});
