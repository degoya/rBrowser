// RB-080-97: the panels page of the start - since RB-080-93 the docked icon rail - asked for the
// list once, while the core still loaded its plugins; the core refused it
// (`plugins.error.not_available`), the page kept no panels for good and the dock showed Findings
// alone. A page mounted after the load (the dock floated and docked again) listed every panel.
// The core now tells the pages when the panels offered change (`{ panelsChanged: true }`).
import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { IpcClient } from '../ipc/client';
import { IpcErrorCode, type CoreToShell, type ShellToCore } from '../ipc/protocol';
import { IpcError } from '../ipc/transport';
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

/** The core's refusal while it still loads its plugins (`plugins/ipc.rs`, `runtime_of`). */
const loading = (): IpcError => new IpcError(IpcErrorCode.NotFound, 'plugins.error.not_available');
const CHANGED: CoreToShell = { type: 'pluginPanel', message: { panelsChanged: true } };

/** A `list` answer the test hands out when it likes: the panels, or a failure. */
interface Pending {
  give: (panels: PanelInfo[] | IpcError) => void;
}

/** Mounted with the dock `settings`; each `list` waits in `lists` until the test answers it. */
async function mounted(settings: Record<string, unknown>) {
  const asked: ShellToCore[] = [];
  const lists: Pending[] = [];
  const client = new IpcClient({
    send: async (message): Promise<CoreToShell> => {
      asked.push(message);
      if (message.type === 'getSettings') {
        return {
          type: 'settings',
          values: Object.entries(settings).map(([key, value]) => ({ key, value: value as never, origin: 'user' })),
        };
      }
      if (message.type === 'pluginPanel' && message.request.op === 'list') {
        return new Promise<CoreToShell>((resolve, reject) => {
          lists.push({
            give: (panels) =>
              panels instanceof IpcError ? reject(panels) : resolve({ type: 'pluginPanel', message: { panels } }),
          });
        });
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
    mount: { props: { client, subscribe, messages: new EventTarget(), pageWidth: 1000 } },
  });
  current = result.wrapper;
  await settle();
  const emit = async (message: CoreToShell): Promise<void> => {
    listeners.forEach((listener) => listener(message));
    await settle();
  };
  /** Answers list request `index` (the first by default). */
  const answer = async (panels: PanelInfo[] | IpcError, index = 0): Promise<void> => {
    lists[index]?.give(panels);
    await settle();
  };
  const listsAsked = (): number =>
    asked.filter((message) => message.type === 'pluginPanel' && message.request.op === 'list').length;
  return { wrapper: result.wrapper, emit, answer, listsAsked };
}

const tabs = (wrapper: VueWrapper): string[] =>
  wrapper.findAll('[data-testid^="panel-tab-"]').map((tab) => tab.attributes('data-testid') ?? '');

describe('the panels list while the plugins load', () => {
  it('the rail of the start shows every plugin panel once the core says they are there', async () => {
    const { wrapper, emit, answer, listsAsked } = await mounted({ 'panels.dock': 'right', 'panels.dockState': 'rail' });
    await answer(loading());
    expect(tabs(wrapper)).toEqual(['panel-tab-findings']);
    await emit(CHANGED);
    expect(listsAsked()).toBe(2);
    await answer([FONTS, NET], 1);
    expect(tabs(wrapper)).toEqual(['panel-tab-findings', 'panel-tab-fonts.fonts', 'panel-tab-network-panel.network']);
    // Still the rail of a start: no panel loaded before the person opens one (RB-080-93).
    expect(wrapper.find('iframe').exists()).toBe(false);
  });

  // The owner's evidence (diag owner-1115b/3.png): floated and docked again, the new page asked
  // after the load and listed every panel - only the page of the start stayed empty.
  it('a page mounted after the load lists every panel with its first request', async () => {
    const { wrapper, answer, listsAsked } = await mounted({ 'panels.dock': 'right', 'panels.dockState': 'rail' });
    await answer([FONTS, NET]);
    expect(listsAsked()).toBe(1);
    expect(tabs(wrapper)).toEqual(['panel-tab-findings', 'panel-tab-fonts.fonts', 'panel-tab-network-panel.network']);
  });

  it('a page the plugins are not ready for says they load, not that none offers a panel', async () => {
    const { wrapper, emit, answer } = await mounted({ 'panels.dock': 'right', 'panels.dockState': 'open' });
    await answer(loading());
    expect(wrapper.find('[data-testid="panels-empty"]').exists()).toBe(false);
    expect(wrapper.get('[data-testid="panels-loading"]').text()).toBe('The plugins are still loading; their panels follow.');
    await emit(CHANGED);
    await answer([FONTS, NET], 1);
    expect(wrapper.find('[data-testid="panels-loading"]').exists()).toBe(false);
    // Unasked, the first panel opens - as with a list that came at once.
    expect(wrapper.get('[data-testid="panel-frame"]').attributes('src')).toBe(FONTS.url);
  });

  // Review of RB-080-97: the sentence carried the core's raw text - a code such as
  // `plugins.error.timed_out`, or the transport's "no answer to pluginPanel in 5000 ms".
  it('another failure is said as a sentence: the code and the reason translated, never raw', async () => {
    const cases: [IpcError, string][] = [
      [
        new IpcError(IpcErrorCode.Internal, 'plugins.error.timed_out'),
        'The plugin panels could not be listed. The plugin took too long and was stopped.',
      ],
      [
        new IpcError(IpcErrorCode.Internal, 'no answer to pluginPanel in 5000 ms'),
        'The plugin panels could not be listed. rBrowser did not answer in time.',
      ],
      [new IpcError(IpcErrorCode.Internal, 'the window is closed'), 'The plugin panels could not be listed.'],
    ];
    for (const [error, said] of cases) {
      const { wrapper, answer } = await mounted({ 'panels.dock': 'right', 'panels.dockState': 'open' });
      await answer(error);
      expect(wrapper.find('[data-testid="panels-empty"]').exists()).toBe(false);
      expect(wrapper.get('[data-testid="panels-failed"]').text()).toBe(said);
      // Said at once, not found later (RB-090-22).
      expect(wrapper.get('[data-testid="panels-failed"]').attributes('role')).toBe('alert');
      wrapper.unmount();
      current = null;
    }
  });

  it('after a refusal Findings, which needs no list, opens at once and stays', async () => {
    const { wrapper, emit, answer } = await mounted({ 'panels.dock': 'right', 'panels.dockState': 'open' });
    await answer(loading());
    await emit({ type: 'pluginPanel', message: { select: { core: 'findings' } } });
    expect(wrapper.find('[data-testid="findings-page"]').exists()).toBe(true);
    await emit(CHANGED);
    await answer([FONTS, NET], 1);
    expect(wrapper.find('[data-testid="panel-frame"]').exists()).toBe(false);
    expect(wrapper.get('[data-testid="panel-tab-findings"]').attributes('aria-current')).toBe('page');
  });

  it('a new list keeps the panel shown, its frame and the rail', async () => {
    const { wrapper, emit, answer } = await mounted({ 'panels.dock': 'right', 'panels.dockState': 'open' });
    await answer([FONTS, NET]);
    await wrapper.get('[data-testid="panel-tab-network-panel.network"]').trigger('click');
    await settle();
    const frame = wrapper.get('[data-testid="panel-frame"]').element;
    await emit(CHANGED);
    await answer([NET], 1);
    expect(wrapper.get('[data-testid="panel-frame"]').element).toBe(frame);
    expect(tabs(wrapper)).toEqual(['panel-tab-findings', 'panel-tab-network-panel.network']);
    expect(wrapper.get('[data-testid="panel-tab-network-panel.network"]').attributes('aria-current')).toBe('page');
  });

  it('a panel shown and no longer offered (hidden, trust revoked) gives way to Findings', async () => {
    const { wrapper, emit, answer } = await mounted({ 'panels.dock': 'right', 'panels.dockState': 'open' });
    await answer([FONTS, NET]);
    await emit(CHANGED);
    await answer([NET], 1);
    expect(wrapper.find('[data-testid="panel-frame"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="panels-empty"]').exists()).toBe(false);
    expect(wrapper.get('[data-testid="panel-tab-findings"]').attributes('aria-current')).toBe('page');
  });

  it('a refusal answered after a newer list changes nothing', async () => {
    const { wrapper, emit, answer } = await mounted({ 'panels.dock': 'right', 'panels.dockState': 'open' });
    await emit(CHANGED);
    await answer([FONTS, NET], 1);
    await answer(loading(), 0);
    expect(tabs(wrapper)).toEqual(['panel-tab-findings', 'panel-tab-fonts.fonts', 'panel-tab-network-panel.network']);
    expect(wrapper.find('[data-testid="panels-loading"]').exists()).toBe(false);
    expect(wrapper.get('[data-testid="panel-frame"]').attributes('src')).toBe(FONTS.url);
  });
});
