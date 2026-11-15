import { afterEach, describe, expect, it, vi } from 'vitest';
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
  vi.restoreAllMocks();
});

const MAIN: PanelInfo = {
  plugin: 'hello-panel',
  panel: 'main',
  title: 'Hello',
  placement: 'sidebar',
  url: 'rbrowser://plugin/hello-panel/panel/index.html',
};
const LOG: PanelInfo = { ...MAIN, panel: 'log', title: 'Log', url: 'rbrowser://plugin/hello-panel/panel/log.html' };
const ASK: PanelInfo = {
  ...MAIN,
  panel: 'ask',
  title: 'Ask',
  placement: 'dialog',
  url: 'rbrowser://plugin/hello-panel/panel/ask.html',
};

async function mounted(panels: PanelInfo[], routeKey: string | null = null, listed?: Promise<void>) {
  const asked: ShellToCore[] = [];
  const client = new IpcClient({
    send: async (message): Promise<CoreToShell> => {
      asked.push(message);
      await listed;
      if (message.type === 'pluginPanel' && message.request.op === 'settings') {
        return { type: 'pluginPanel', message: { strings: { 'plugin.hello-panel.title': 'Hello' }, settings: {} } };
      }
      return { type: 'pluginPanel', message: { panels } };
    },
    subscribe: () => () => undefined,
  });
  const listeners: ((message: CoreToShell) => void)[] = [];
  const subscribe = (listener: (message: CoreToShell) => void) => {
    listeners.push(listener);
    return () => undefined;
  };
  const messages = new EventTarget();
  const result = await mountWithShell(PanelsPage, {
    mount: { props: { client, routeKey, subscribe, messages } },
  });
  current = result.wrapper;
  await settle();
  const emit = async (message: CoreToShell): Promise<void> => {
    listeners.forEach((listener) => listener(message));
    await settle();
  };
  /** A message of `frame` (the shown panel's by default) to the window, as the browser hands it over. */
  const fromFrame = async (data: unknown, frame?: HTMLIFrameElement): Promise<void> => {
    const from = frame ?? (result.wrapper.get('[data-testid="panel-frame"]').element as HTMLIFrameElement);
    messages.dispatchEvent(Object.assign(new Event('message'), { source: from.contentWindow, origin: 'null', data }));
    await settle();
  };
  const click = async (panel: PanelInfo): Promise<void> => {
    await result.wrapper.get(`[data-testid="panel-tab-${panel.plugin}.${panel.panel}"]`).trigger('click');
    await settle();
  };
  const shown = (): HTMLIFrameElement => result.wrapper.get('[data-testid="panel-frame"]').element as HTMLIFrameElement;
  return { wrapper: result.wrapper, asked, emit, fromFrame, click, shown };
}

/** As in the browser (happy-dom loads no frame): a frame has a window once it is in the page;
 * the windows a test's frames got, with what each was sent. */
function browserFrames(): Map<HTMLIFrameElement, { got: unknown[] }> {
  const windows = new Map<HTMLIFrameElement, { got: unknown[]; postMessage: (message: unknown) => void }>();
  vi.spyOn(HTMLIFrameElement.prototype, 'contentWindow', 'get').mockImplementation(function (this: HTMLIFrameElement) {
    if (!this.isConnected) {
      return null;
    }
    let made = windows.get(this);
    if (made === undefined) {
      const got: unknown[] = [];
      made = { got, postMessage: (message: unknown) => got.push(message) };
      windows.set(this, made);
    }
    return made as unknown as Window;
  });
  return windows;
}

const settingsRequests = (asked: ShellToCore[]): number =>
  asked.filter((message) => message.type === 'pluginPanel' && message.request.op === 'settings').length;

const settingsAsked = (asked: ShellToCore[]): boolean =>
  asked.some((message) => message.type === 'pluginPanel' && message.request.op === 'settings');

const select = (plugin: string, panel: string): CoreToShell => ({
  type: 'pluginPanel',
  message: { select: { plugin, panel } },
});

describe('the plugin panels window', () => {
  it('asks the core for the panels and shows the first in a sandboxed frame', async () => {
    const { wrapper, asked } = await mounted([MAIN, LOG]);
    expect(asked).toContainEqual({ type: 'pluginPanel', request: { op: 'list' } });
    const frame = wrapper.get('[data-testid="panel-frame"]');
    expect(frame.attributes('src')).toBe(MAIN.url);
    expect(frame.attributes('sandbox')).toBe('allow-scripts');
    // The one permission a panel gets beyond scripts: writing to the clipboard (a copy button);
    // never reading it.
    expect(frame.attributes('allow')).toBe('clipboard-write');
    expect(frame.attributes('referrerpolicy')).toBe('no-referrer');
    expect(wrapper.get('[data-testid="panel-tab-hello-panel.main"]').attributes('aria-current')).toBe('page');
  });

  it('switches to another panel and opens the one the route names', async () => {
    const first = await mounted([MAIN, LOG]);
    await first.wrapper.get('[data-testid="panel-tab-hello-panel.log"]').trigger('click');
    expect(first.wrapper.get('[data-testid="panel-frame"]').attributes('src')).toBe(LOG.url);
    first.wrapper.unmount();
    current = null;
    const routed = await mounted([MAIN, LOG], 'hello-panel.log');
    expect(routed.wrapper.get('[data-testid="panel-frame"]').attributes('src')).toBe(LOG.url);
  });

  // RB-080-59: a panel the manifest calls a dialog lay as a box over the panel chosen before
  // (Storage stayed marked); every panel now opens in the content area, the mark follows.
  it('opens a dialog panel in the content area like any other, the mark following', async () => {
    const { wrapper } = await mounted([MAIN, ASK]);
    const tab = wrapper.get('[data-testid="panel-tab-hello-panel.ask"]');
    expect(tab.classes()).not.toContain('italic');
    await tab.trigger('click');
    expect(wrapper.find('[data-testid="panel-dialog"]').exists()).toBe(false);
    const frame = wrapper.get('[data-testid="panel-frame"]');
    expect(frame.attributes('src')).toBe(ASK.url);
    expect(frame.attributes('sandbox')).toBe('allow-scripts');
    expect(frame.attributes('allow')).toBe('clipboard-write');
    expect(wrapper.findAll('[data-testid="panel-frame"]')).toHaveLength(1);
    expect(tab.attributes('aria-current')).toBe('page');
    expect(wrapper.get('[data-testid="panel-tab-hello-panel.main"]').attributes('aria-current')).toBeUndefined();
  });

  // RB-080-59: the frame of a panel opened in a new box was handed to the bridge while that box
  // was not yet in the page (no `contentWindow`), so its `settings` request was dropped and it
  // showed its raw keys. Every panel opened, whichever way, reaches the bridge.
  it('answers the settings request of every panel it opens', async () => {
    browserFrames();
    for (const panel of [ASK, MAIN, LOG]) {
      const { wrapper, asked, fromFrame } = await mounted([MAIN, LOG, ASK]);
      if (panel !== MAIN) {
        await wrapper.get(`[data-testid="panel-tab-hello-panel.${panel.panel}"]`).trigger('click');
      }
      await fromFrame({ type: 'settings' });
      expect(settingsAsked(asked), panel.panel).toBe(true);
      wrapper.unmount();
      current = null;
    }
  });

  // Review of RB-080-59: a frame taken out of the page stayed in the bridge - every switch left a
  // dead window that got each settings refresh (an IPC round trip) and every event.
  it('keeps only the frames in the page in the bridge, however often the panels switch', async () => {
    const windows = browserFrames();
    const { asked, emit, click } = await mounted([MAIN, LOG, ASK]);
    for (let round = 0; round < 5; round += 1) {
      await click(LOG);
      await click(ASK);
      await click(MAIN);
    }
    const before = settingsRequests(asked);
    await emit({ type: 'settingsChanged', keys: ['plugins.panelsHidden'] });
    expect(settingsRequests(asked) - before).toBe(1);
    await emit({ type: 'pluginPanel', message: { event: { plugin: 'hello-panel', message: { n: 1 } } } });
    const reached = [...windows.entries()].filter(([, view]) => view.got.some((message) => (message as { type?: string }).type === 'event'));
    expect(reached.map(([frame]) => frame.isConnected)).toEqual([true]);
  });

  // Review of RB-080-59: with every panel in the content area, a former dialog listed first
  // (the screenshot editor) would open by itself with the window and start its calls.
  it('opens the first panel that is not a dialog when no panel is asked for', async () => {
    const { wrapper } = await mounted([ASK, MAIN, LOG]);
    expect(wrapper.get('[data-testid="panel-frame"]').attributes('src')).toBe(MAIN.url);
    const only = await mounted([ASK]);
    expect(only.wrapper.get('[data-testid="panel-frame"]').attributes('src')).toBe(ASK.url);
  });

  // Review of RB-080-59: switching away destroyed the frame, and the screenshot editor's unsaved
  // annotations with it. A panel that reports unsaved work stays alive, hidden, until it is clean.
  it('keeps a panel with unsaved work alive while another is shown', async () => {
    browserFrames();
    const { wrapper, fromFrame, click, shown } = await mounted([MAIN, ASK]);
    await click(ASK);
    const editor = shown();
    await fromFrame({ type: 'dirty', dirty: true });
    await click(MAIN);
    expect(shown().getAttribute('src')).toBe(MAIN.url);
    const kept = wrapper.get('[data-testid="panel-frame-kept"]').element as HTMLIFrameElement;
    expect(kept).toBe(editor);
    expect(kept.isConnected).toBe(true);
    await click(ASK);
    expect(shown()).toBe(editor);
    expect(wrapper.find('[data-testid="panel-frame-kept"]').exists()).toBe(false);
    await click(MAIN);
    expect(wrapper.get('[data-testid="panel-frame-kept"]').element).toBe(editor);
    await fromFrame({ type: 'dirty', dirty: false }, editor);
    expect(wrapper.find('[data-testid="panel-frame-kept"]').exists()).toBe(false);
    expect(editor.isConnected).toBe(false);
  });

  it('takes away the frame of a panel without unsaved work when another is shown', async () => {
    browserFrames();
    const { wrapper, click, shown } = await mounted([MAIN, ASK]);
    const first = shown();
    await click(ASK);
    expect(first.isConnected).toBe(false);
    expect(wrapper.findAll('iframe')).toHaveLength(1);
  });

  it('says so when no plugin offers a panel', async () => {
    const { wrapper } = await mounted([]);
    expect(wrapper.find('[data-testid="panel-frame"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="panels-empty"]').exists()).toBe(true);
  });

  it('selects the panel a command asks for in the open window', async () => {
    const { wrapper, emit } = await mounted([MAIN, LOG, ASK]);
    await emit(select('hello-panel', 'log'));
    expect(wrapper.get('[data-testid="panel-frame"]').attributes('src')).toBe(LOG.url);
    await emit(select('hello-panel', 'ask'));
    expect(wrapper.get('[data-testid="panel-frame"]').attributes('src')).toBe(ASK.url);
    expect(wrapper.get('[data-testid="panel-tab-hello-panel.ask"]').attributes('aria-current')).toBe('page');
  });

  it('ignores a select for a panel it does not offer', async () => {
    const { wrapper, emit } = await mounted([MAIN, LOG]);
    await emit(select('other', 'log'));
    await emit(select('hello-panel', 'nope'));
    expect(wrapper.get('[data-testid="panel-frame"]').attributes('src')).toBe(MAIN.url);
  });

  it('keeps a select that arrives before the list for when it is there', async () => {
    let release = (): void => undefined;
    const listed = new Promise<void>((resolve) => {
      release = resolve;
    });
    const { wrapper, emit } = await mounted([MAIN, LOG], null, listed);
    await emit(select('hello-panel', 'log'));
    release();
    await settle();
    expect(wrapper.get('[data-testid="panel-frame"]').attributes('src')).toBe(LOG.url);
  });
});
