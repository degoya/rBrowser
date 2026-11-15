import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { mountWithShell, settle } from '../test/mountShell';
import ShellToolbar from '../toolbar/ShellToolbar.vue';
import type { LayoutState } from '../ipc/protocol';

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

async function enter({ mock }: Mounted, layout: Partial<LayoutState>): Promise<void> {
  mock.state.layout = { ...mock.state.layout, ...layout };
  mock.emit({ type: 'state', ...structuredClone(mock.state) });
  await settle();
}

describe('the layout modes', () => {
  it('switches the mode through its command and shows which one is on', async () => {
    const mounted = await setup();
    const { wrapper, mock } = mounted;
    expect(wrapper.get('[data-testid="mode-grid"]').attributes('aria-pressed')).toBe('true');
    expect(wrapper.get('[data-testid="layout-modes"]').attributes('aria-label')).toBe('Layout mode');
    for (const mode of ['horizontal', 'split', 'canvas', 'grid']) {
      await wrapper.get(`[data-testid="mode-${mode}"]`).trigger('click');
      await settle();
      expect(mock.sent.at(-1)).toEqual({ type: 'runCommand', id: `layout.mode.${mode}` });
    }
    await enter(mounted, { mode: 'canvas' });
    expect(wrapper.get('[data-testid="mode-canvas"]').attributes('aria-pressed')).toBe('true');
    expect(wrapper.get('[data-testid="mode-grid"]').attributes('aria-pressed')).toBe('false');
  });

  it('moves the split divider on release and swaps the sides', async () => {
    const mounted = await setup();
    await enter(mounted, {
      mode: 'split',
      modes: { split: { ratioPermille: 600, vertical: true, stepPermille: 50 } },
    });
    const { wrapper, mock } = mounted;
    const divider = wrapper.get('[data-testid="split-divider"]');
    expect((divider.element as HTMLInputElement).value).toBe('60');
    expect(divider.attributes('step')).toBe('5');
    expect(divider.attributes('aria-label')).toBe('Split divider (primary side left)');
    expect(divider.attributes('aria-valuetext')).toBe('60 % for the primary side');
    await divider.setValue('35');
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'runCommand', id: 'layout.mode.split', args: { ratio: 0.35 } });
    await wrapper.get('[data-testid="split-swap"]').trigger('click');
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'runCommand', id: 'layout.split.swap' });
  });

  it('scrolls a strip wider than the window and hides the controls when it fits', async () => {
    const mounted = await setup();
    await enter(mounted, { mode: 'horizontal', modes: { strip: { first: 0, lastFirst: 0, count: 3 } } });
    const { wrapper, mock } = mounted;
    expect(wrapper.find('[data-testid="strip-next"]').exists()).toBe(false);
    await enter(mounted, { mode: 'horizontal', modes: { strip: { first: 2, lastFirst: 5, count: 8 } } });
    expect(wrapper.get('[data-testid="strip-position"]').text()).toBe('View 3 of 8');
    await wrapper.get('[data-testid="strip-next"]').trigger('click');
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'runCommand', id: 'layout.focus_next' });
    await enter(mounted, { mode: 'horizontal', modes: { strip: { first: 0, lastFirst: 5, count: 8 } } });
    expect(wrapper.get('[data-testid="strip-prev"]').attributes('disabled')).toBeDefined();
  });

  it('hands the focus to the other arrow when one disables itself at either end (RB-090-23)', async () => {
    const mounted = await setup();
    await enter(mounted, { mode: 'horizontal', modes: { strip: { first: 1, lastFirst: 5, count: 8 } } });
    const { wrapper } = mounted;
    const element = (id: string) => wrapper.get(`[data-testid="${id}"]`).element as HTMLElement;
    element('strip-prev').focus();
    await enter(mounted, { mode: 'horizontal', modes: { strip: { first: 0, lastFirst: 5, count: 8 } } });
    expect(element('strip-prev').matches(':disabled')).toBe(true);
    expect(document.activeElement).toBe(element('strip-next'));

    await enter(mounted, { mode: 'horizontal', modes: { strip: { first: 4, lastFirst: 5, count: 8 } } });
    expect(document.activeElement).toBe(element('strip-next'));
    await enter(mounted, { mode: 'horizontal', modes: { strip: { first: 5, lastFirst: 5, count: 8 } } });
    expect(document.activeElement).toBe(element('strip-prev'));
  });
});
