// The panels page docked and folded to its icon rail, and its side bar's modes (RB-080-61,
// RB-080-93).
import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { IpcClient } from '../ipc/client';
import type { CoreToShell, ShellToCore } from '../ipc/protocol';
import { mountWithShell, settle } from '../test/mountShell';
import PanelsPage from './PanelsPage.vue';
import type { PanelInfo } from './protocol';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

const FONTS: PanelInfo = {
  plugin: 'fonts',
  panel: 'fonts',
  title: 'Fonts',
  placement: 'sidebar',
  icon: 'type',
  url: 'rbrowser://plugin/fonts/panel/index.html',
};
const NET: PanelInfo = {
  ...FONTS,
  plugin: 'network-panel',
  panel: 'network',
  title: 'Network',
  icon: 'network',
  url: 'rbrowser://plugin/network-panel/panel/index.html',
};

/** Mounted with `settings`; open unless they say otherwise (the core's default is the rail). */
async function mounted(settings: Record<string, unknown>, pageWidth = 1000) {
  const asked: ShellToCore[] = [];
  let values: Record<string, unknown> = { 'panels.dockState': 'open', ...settings };
  const client = new IpcClient({
    send: async (message): Promise<CoreToShell> => {
      asked.push(message);
      if (message.type === 'getSettings') {
        return {
          type: 'settings',
          values: Object.entries(values).map(([key, value]) => ({ key, value: value as never, origin: 'user' })),
        };
      }
      if (message.type === 'pluginPanel' && message.request.op === 'list') {
        return { type: 'pluginPanel', message: { panels: [FONTS, NET] } };
      }
      return { type: 'ack' };
    },
    subscribe: () => () => undefined,
  });
  const listeners: ((message: CoreToShell) => void)[] = [];
  const subscribe = (listener: (message: CoreToShell) => void) => {
    listeners.push(listener);
    return () => undefined;
  };
  const result = await mountWithShell(PanelsPage, {
    mount: { props: { client, subscribe, messages: new EventTarget(), pageWidth } },
  });
  current = result.wrapper;
  await settle();
  const change = async (next: Record<string, unknown>, keys: string[]): Promise<void> => {
    values = next;
    listeners.forEach((listener) => listener({ type: 'settingsChanged', keys }));
    await settle();
  };
  const of = (type: ShellToCore['type']) => asked.filter((message) => message.type === type);
  return { wrapper: result.wrapper, asked, of, change };
}

const visible = (wrapper: VueWrapper, id: string): boolean =>
  (wrapper.get(`[data-testid="${id}"]`).element as HTMLElement).style.display !== 'none';

describe('the docked panels page', () => {
  it('tells the core which panel the page shows', async () => {
    const { of } = await mounted({});
    expect(of('pluginPanel')).toContainEqual({
      type: 'pluginPanel',
      request: { op: 'selected', plugin: 'fonts', panel: 'fonts' },
    });
  });

  it('open: header, side bar with icons and titles, and the panel', async () => {
    const { wrapper } = await mounted({ 'panels.dock': 'right' });
    expect(wrapper.find('[data-testid="panel-dockbar"]').exists()).toBe(true);
    expect(visible(wrapper, 'panels-page') && visible(wrapper, 'panels-content')).toBe(true);
    expect(wrapper.get('[data-testid="panel-tab-fonts.fonts"]').text()).toBe('Fonts');
  });

  it('folded below the views: the icon rail as a row, no header, no panel loaded', async () => {
    const { wrapper } = await mounted({ 'panels.dock': 'bottom', 'panels.dockState': 'rail' });
    expect(wrapper.find('[data-testid="panel-dockbar"]').exists()).toBe(false);
    expect(visible(wrapper, 'panels-page')).toBe(true);
    expect(visible(wrapper, 'panels-content')).toBe(false);
    expect(wrapper.get('[data-testid="panel-sidebar-menu"]').attributes('data-orientation')).toBe('horizontal');
    // A rail that never opened loads no panel (RB-080-93).
    expect(wrapper.find('[data-testid="panel-frame"]').exists()).toBe(false);
  });

  it('the rail of a start opens at Findings, a panel shown before stays in the page', async () => {
    const { wrapper, change } = await mounted({ 'panels.dock': 'right', 'panels.dockState': 'rail' });
    expect(wrapper.find('[aria-current="page"]').exists()).toBe(false);
    await change({ 'panels.dock': 'right', 'panels.dockState': 'open' }, ['panels.dockState']);
    expect(wrapper.get('[data-testid="panel-tab-findings"]').attributes('aria-current')).toBe('page');
    await wrapper.get('[data-testid="panel-tab-fonts.fonts"]').trigger('click');
    await settle();
    await change({ 'panels.dock': 'right', 'panels.dockState': 'rail' }, ['panels.dockState']);
    // Folding loses nothing: the frame stays, hidden.
    expect(wrapper.find('[data-testid="panel-frame"]').exists()).toBe(true);
    expect(visible(wrapper, 'panels-content')).toBe(false);
  });

  it('folded beside the views: the icon column alone; an icon opens the dock at its panel', async () => {
    const { wrapper, of } = await mounted({ 'panels.dock': 'left', 'panels.dockState': 'rail' });
    expect(wrapper.find('[data-testid="panel-dockbar"]').exists()).toBe(false);
    expect(visible(wrapper, 'panels-page')).toBe(true);
    expect(visible(wrapper, 'panels-content')).toBe(false);
    expect(wrapper.get('[data-testid="panel-tab-fonts.fonts"]').text()).toBe('');
    await wrapper.get('[data-testid="panel-tab-network-panel.network"]').trigger('click');
    await wrapper.get('[data-testid="panel-strip-expand"]').trigger('click');
    expect(of('runCommand')).toEqual([
      {
        type: 'runCommand',
        id: 'panels.dock.toggle',
        args: { collapsed: false, plugin: 'network-panel', panel: 'network' },
      },
      { type: 'runCommand', id: 'panels.dock.toggle', args: { collapsed: false } },
    ]);
    expect(wrapper.get('[data-testid="panel-tab-network-panel.network"]').attributes('aria-current')).toBe('page');
  });

  it('a floating window never folds', async () => {
    const { wrapper } = await mounted({ 'panels.dock': 'floating', 'panels.dockState': 'rail' });
    expect(visible(wrapper, 'panels-page') && visible(wrapper, 'panels-content')).toBe(true);
  });

  it('follows the settings when they change', async () => {
    const { wrapper, change } = await mounted({ 'panels.dock': 'bottom' });
    await change({ 'panels.dock': 'bottom', 'panels.dockState': 'rail' }, ['panels.dockState']);
    expect(visible(wrapper, 'panels-content')).toBe(false);
    expect(wrapper.find('[data-testid="panel-dockbar"]').exists()).toBe(false);
    await change(
      { 'panels.dock': 'bottom', 'panels.dockState': 'open', 'panels.sidebar': 'icons' },
      ['panels.sidebar', 'panels.dockState'],
    );
    expect(visible(wrapper, 'panels-content')).toBe(true);
    expect(wrapper.get('[data-testid="panel-tab-fonts.fonts"]').text()).toBe('');
  });
});

describe('the panels side bar mode', () => {
  it('the mode button writes panels.sidebar and shows it at once', async () => {
    const { wrapper, of } = await mounted({});
    const toggle = wrapper.get('[data-testid="panel-sidebar-toggle"]');
    (toggle.element as HTMLElement).focus();
    await toggle.trigger('click');
    await settle();
    expect(wrapper.get('[data-testid="panel-tab-fonts.fonts"]').text()).toBe('');
    // The switch stays where it was and keeps the focus (docs/design.md, rule 11).
    expect(document.activeElement).toBe(wrapper.get('[data-testid="panel-sidebar-toggle"]').element);
    expect(wrapper.get('[data-testid="panel-sidebar-toggle"]').attributes('aria-label')).toBe('Expand side bar');
    await wrapper.get('[data-testid="panel-sidebar-toggle"]').trigger('click');
    await settle();
    expect(wrapper.get('[data-testid="panel-tab-fonts.fonts"]').text()).toBe('Fonts');
    expect(of('setSetting')).toEqual([
      { type: 'setSetting', key: 'panels.sidebar', value: 'icons', scope: 'user' },
      { type: 'setSetting', key: 'panels.sidebar', value: 'expanded', scope: 'user' },
    ]);
  });

  it('a narrow page shows icons only, whatever the setting', async () => {
    const { wrapper } = await mounted({ 'panels.sidebar': 'expanded' }, 300);
    expect(wrapper.get('[data-testid="panel-tab-fonts.fonts"]').text()).toBe('');
    expect(wrapper.find('[data-testid="panel-sidebar-toggle"]').exists()).toBe(false);
  });
});

// Review point 5: every `panels.*` change started a load and the last answer won, stale or not.
describe('the dock state loads in order', () => {
  it('an older answer never overwrites a newer one or the page\'s own change', async () => {
    const answers: ((values: Record<string, unknown>) => void)[] = [];
    const listeners: ((message: CoreToShell) => void)[] = [];
    const client = new IpcClient({
      send: (message): Promise<CoreToShell> => {
        if (message.type === 'getSettings') {
          return new Promise((resolve) => {
            answers.push((values) =>
              resolve({
                type: 'settings',
                values: Object.entries(values).map(([key, value]) => ({ key, value: value as never, origin: 'user' })),
              }),
            );
          });
        }
        if (message.type === 'pluginPanel' && message.request.op === 'list') {
          return Promise.resolve({ type: 'pluginPanel', message: { panels: [FONTS] } });
        }
        return Promise.resolve({ type: 'ack' });
      },
      subscribe: () => () => undefined,
    });
    const result = await mountWithShell(PanelsPage, {
      mount: {
        props: {
          client,
          subscribe: (listener: (message: CoreToShell) => void) => {
            listeners.push(listener);
            return () => undefined;
          },
          messages: new EventTarget(),
          pageWidth: 1000,
        },
      },
    });
    current = result.wrapper;
    await settle();
    const changed = async (): Promise<void> => {
      listeners.forEach((listener) => listener({ type: 'settingsChanged', keys: ['panels.dockState'] }));
      await settle();
    };
    await changed();
    // The newer load answers first (folded below), the first one late (open).
    answers[1]?.({ 'panels.dock': 'bottom', 'panels.dockState': 'rail' });
    await settle();
    answers[0]?.({ 'panels.dock': 'bottom', 'panels.dockState': 'open' });
    await settle();
    expect(visible(result.wrapper, 'panels-content')).toBe(false);
    // The page's own change of the side bar is not undone by a load still on its way.
    await changed();
    answers[2]?.({ 'panels.dock': 'floating' });
    await settle();
    expect(visible(result.wrapper, 'panels-content')).toBe(true);
    await changed();
    await result.wrapper.get('[data-testid="panel-sidebar-toggle"]').trigger('click');
    await settle();
    answers[3]?.({ 'panels.dock': 'floating', 'panels.sidebar': 'expanded' });
    await settle();
    expect(result.wrapper.get('[data-testid="panel-tab-fonts.fonts"]').text()).toBe('');
  });
});
