// Findings in the plugin panels page (RB-080-69): before, `panels.findings` opened a window of
// its own (`crates/rbrowser/src/control/handlers.rs`, `PageWindow::Findings`); now the page lists
// Findings first and shows the findings page itself in its content area, floating or docked.
import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { IpcClient } from '../ipc/client';
import type { CoreToShell, ShellToCore } from '../ipc/protocol';
import { sampleMessages, sampleRun } from '../findings/testing';
import { mountWithShell, settle } from '../test/mountShell';
import PanelsPage from './PanelsPage.vue';
import type { PanelInfo } from './protocol';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

const NET: PanelInfo = {
  plugin: 'network-panel',
  panel: 'network',
  title: 'Network',
  placement: 'sidebar',
  icon: 'network',
  url: 'rbrowser://plugin/network-panel/panel/index.html',
};

/** Mounted with `settings`; open unless they say otherwise (the core's default is the rail). */
async function mounted(routeKey: string | null, given: Record<string, unknown> = {}) {
  const settings: Record<string, unknown> = { 'panels.dockState': 'open', ...given };
  const asked: ShellToCore[] = [];
  const client = new IpcClient({
    send: async (message): Promise<CoreToShell> => {
      asked.push(message);
      if (message.type === 'getSettings') {
        return {
          type: 'settings',
          values: Object.entries(settings).map(([key, value]) => ({ key, value: value as never, origin: 'user' })),
        };
      }
      if (message.type === 'getFindings') {
        return { type: 'findings', run: sampleRun(), messages: sampleMessages };
      }
      if (message.type === 'pluginPanel' && message.request.op === 'list') {
        return { type: 'pluginPanel', message: { panels: [NET] } };
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
    mount: { props: { client, routeKey, subscribe, messages: new EventTarget(), pageWidth: 1000 } },
  });
  current = result.wrapper;
  await settle();
  const emit = async (message: CoreToShell): Promise<void> => {
    listeners.forEach((listener) => listener(message));
    await settle();
  };
  return { wrapper: result.wrapper, asked, emit };
}

const shown = (wrapper: VueWrapper, id: string): boolean => {
  const found = wrapper.find(`[data-testid="${id}"]`);
  return found.exists() && (found.element as HTMLElement).style.display !== 'none';
};

describe('Findings in the panels page', () => {
  it('stands first in the side bar with its own icon', async () => {
    const { wrapper } = await mounted(null);
    const tabs = wrapper.findAll('[data-testid^="panel-tab-"]').map((tab) => tab.attributes('data-testid'));
    expect(tabs).toEqual(['panel-tab-findings', 'panel-tab-videos', 'panel-tab-network-panel.network']);
    expect(wrapper.get('[data-testid="panel-tab-findings"]').text()).toBe('Findings');
    // Unasked, the page still opens the first plugin panel.
    expect(shown(wrapper, 'panel-frame')).toBe(true);
    expect(wrapper.find('[data-testid="findings-page"]').exists()).toBe(false);
  });

  it('the route opens Findings in the content area, marked, with the last run', async () => {
    const { wrapper, asked } = await mounted('findings');
    expect(shown(wrapper, 'findings-page')).toBe(true);
    expect(wrapper.get('[data-testid="panel-tab-findings"]').attributes('aria-current')).toBe('page');
    expect(asked).toContainEqual({ type: 'getFindings' });
    expect(wrapper.find('[data-testid="finding-a"]').text()).toContain('2 images without alt text');
    expect(wrapper.find('[data-testid="panels-empty"]').exists()).toBe(false);
    // No plugin frame for a core panel; the core hears which panel the page shows.
    expect(wrapper.find('[data-testid="panel-frame"]').exists()).toBe(false);
    expect(asked).toContainEqual({ type: 'pluginPanel', request: { op: 'selectedCore', panel: 'findings' } });
  });

  it('a select event of the core opens Findings in an open page, and the findings stay', async () => {
    const { wrapper, emit } = await mounted(null);
    await emit({ type: 'pluginPanel', message: { select: { core: 'findings' } } });
    expect(shown(wrapper, 'findings-page')).toBe(true);
    expect(shown(wrapper, 'panel-frame')).toBe(false);
    await wrapper.get('[data-testid="panel-tab-network-panel.network"]').trigger('click');
    await settle();
    expect(shown(wrapper, 'findings-page')).toBe(false);
    expect(wrapper.find('[data-testid="findings-page"]').exists()).toBe(true);
    expect(shown(wrapper, 'panel-frame')).toBe(true);
  });

  it('docks with the rest: folded beside the views its icon opens the dock at Findings', async () => {
    const { wrapper, asked } = await mounted(null, { 'panels.dock': 'right', 'panels.dockState': 'rail' });
    await wrapper.get('[data-testid="panel-tab-findings"]').trigger('click');
    await settle();
    expect(asked.filter((message) => message.type === 'runCommand')).toEqual([
      { type: 'runCommand', id: 'panels.dock.toggle', args: { collapsed: false } },
    ]);
    expect(asked).toContainEqual({ type: 'pluginPanel', request: { op: 'selectedCore', panel: 'findings' } });
  });
});

describe('requests before the list and while folded', () => {
  /** A page whose list answers only when `release` runs. */
  async function waiting() {
    let release = (): void => undefined;
    const listed = new Promise<void>((resolve) => {
      release = resolve;
    });
    const listeners: ((message: CoreToShell) => void)[] = [];
    const client = new IpcClient({
      send: async (message): Promise<CoreToShell> => {
        if (message.type === 'pluginPanel' && message.request.op === 'list') {
          await listed;
          return { type: 'pluginPanel', message: { panels: [NET] } };
        }
        return message.type === 'getSettings'
          ? { type: 'settings', values: [{ key: 'panels.dockState', value: 'open', origin: 'user' }] }
          : { type: 'ack' };
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
    const emit = async (message: CoreToShell): Promise<void> => {
      listeners.forEach((listener) => listener(message));
      await settle();
    };
    return { wrapper: result.wrapper, emit, release };
  }

  // Review point 4: a core panel asked for before the list was there won over a plugin panel
  // asked for after it. The last request wins.
  it('the last request before the list wins, core or plugin', async () => {
    const first = await waiting();
    await first.emit({ type: 'pluginPanel', message: { select: { core: 'findings' } } });
    await first.emit({ type: 'pluginPanel', message: { select: { plugin: 'network-panel', panel: 'network' } } });
    first.release();
    await settle();
    expect(shown(first.wrapper, 'panel-frame')).toBe(true);
    expect(first.wrapper.find('[data-testid="findings-page"]').exists()).toBe(false);
    first.wrapper.unmount();
    const second = await waiting();
    await second.emit({ type: 'pluginPanel', message: { select: { plugin: 'network-panel', panel: 'network' } } });
    await second.emit({ type: 'pluginPanel', message: { select: { core: 'findings' } } });
    second.release();
    await settle();
    expect(shown(second.wrapper, 'findings-page')).toBe(true);
  });

  // Review point 2: a command's open keeps the fold; the strip marks the entry.
  it('folded, a selected panel is marked in the strip until the person opens it', async () => {
    const { wrapper, emit } = await mounted(null, { 'panels.dock': 'left', 'panels.dockState': 'rail' });
    await emit({ type: 'pluginPanel', message: { select: { core: 'findings' } } });
    // Nuxt UI's chip on the entry's icon (RB-080-83).
    const mark = wrapper.find('[data-testid="panel-tab-findings"] [data-slot="base"]');
    expect(mark.exists()).toBe(true);
    expect(wrapper.get('[data-testid="panel-tab-findings"]').attributes('aria-label')).toBe(
      'Findings, opened while the panels were collapsed',
    );
    await wrapper.get('[data-testid="panel-tab-findings"]').trigger('click');
    expect(wrapper.find('[data-slot="link"] [data-slot="base"]').exists()).toBe(false);
  });
});
