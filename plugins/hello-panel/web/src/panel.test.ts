// The hello-panel panel in a window of its own (RB-080-87): the bridge messages it sends and how
// it shows the host's events. The panels window is played by spying on postMessage and
// dispatching `message` events from window.parent (the test window itself).
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { mountPanel } from '../../../web-common/panel-ui/src/mount';
import App from './panel/App.vue';
import { readEvent } from './panel/model';

type Posted = { type?: string; id?: number; tool?: string; args?: Record<string, unknown> };

let posted: Posted[] = [];
let unmount: () => void = () => undefined;

const STRINGS: Record<string, string> = {
  'plugin.hello-panel.panels.main': 'Hello',
  'plugin.hello-panel.panel.greet': 'Greet every view',
  'plugin.hello-panel.panel.greeted': 'Greeted: {name}',
  'plugin.hello-panel.panel.empty': 'No page has said hello yet.',
  'plugin.hello-panel.panel.pages': '{n, plural, one {# page said hello} other {# pages said hello}}',
  'plugin.hello-panel.panel.columns.view': 'View',
  'plugin.hello-panel.panel.columns.page': 'Page',
  'plugins.error.busy': 'The plugin is busy.',
  'plugins.reason.timeout': 'It took too long.',
};

const $ = (selector: string): HTMLElement | null => document.querySelector<HTMLElement>(selector);
const settle = async (): Promise<void> => {
  await new Promise((resolve) => setTimeout(resolve, 0));
  await nextTick();
};

async function fromHost(data: unknown): Promise<void> {
  window.dispatchEvent(new MessageEvent('message', { data, source: window }));
  await settle();
}

beforeEach(async () => {
  posted = [];
  document.body.innerHTML = '<div id="app"></div>';
  vi.spyOn(window, 'postMessage').mockImplementation((message: unknown) => {
    posted.push(message as Posted);
  });
  unmount = mountPanel(App, { prefix: 'plugin.hello-panel.', name: 'hello-panel panel' }).unmount;
  await fromHost({ type: 'settings', strings: STRINGS, settings: { 'ui.theme': 'light' } });
});

afterEach(() => {
  unmount();
  vi.restoreAllMocks();
});

describe('the hello-panel panel', () => {
  it('shows its title, the greet action and the empty state', () => {
    expect($('#title')?.textContent).toBe('Hello');
    expect($('#greet')?.getAttribute('aria-label')).toBe('Greet every view');
    expect($('#empty')?.textContent).toContain('No page has said hello yet.');
    expect($('#pages')).toBeNull();
  });

  it('calls the tool greet and shows whom the component greeted', async () => {
    $('#greet')?.click();
    await settle();
    const call = posted.find((message) => message.type === 'call');
    expect(call).toEqual({ type: 'call', id: 1, tool: 'greet', args: {} });
    await fromHost({ type: 'result', id: call?.id, ok: true, value: { greeting: 'Hello, rBrowser!' } });
    await fromHost({ type: 'event', message: { greeted: 'rBrowser' } });
    expect($('#status')?.dataset.tone).toBe('ok');
    expect($('#status')?.textContent).toContain('Greeted: rBrowser');
  });

  it('says a failed call as one sentence, never the code', async () => {
    $('#greet')?.click();
    await settle();
    const call = posted.find((message) => message.type === 'call');
    await fromHost({ type: 'result', id: call?.id, ok: false, error: 'plugins.error.busy', params: { reason: 'timeout' } });
    expect($('#status')?.dataset.tone).toBe('error');
    expect($('#status')?.textContent).toContain('The plugin is busy. It took too long.');
    expect($('#status')?.textContent).not.toContain('plugins.error');
  });

  it("draws its text at the shell's text size and follows a change (RB-090-39)", async () => {
    const root = document.documentElement;
    expect(root.style.fontSize).toBe('');
    await fromHost({ type: 'settings', strings: STRINGS, settings: { 'ui.theme': 'light', 'ui.textScale': 200 } });
    expect(root.style.fontSize).toBe('200%');
    await fromHost({ type: 'settings', strings: STRINGS, settings: { 'ui.theme': 'light', 'ui.textScale': 125 } });
    expect(root.style.fontSize).toBe('125%');
    await fromHost({ type: 'settings', strings: STRINGS, settings: { 'ui.theme': 'light', 'ui.textScale': 100 } });
    expect(root.style.fontSize).toBe('');
  });

  it('lists the pages that said hello, with their view', async () => {
    await fromHost({ type: 'event', message: { loaded: 'https://a.test/', view: 1 }, view: 1 });
    await fromHost({ type: 'event', message: { page: 'https://b.test/', title: 'B', view: 2 } });
    expect($('#empty')).toBeNull();
    const rows = [...document.querySelectorAll('#pages tbody tr')].map((row) => row.textContent);
    expect(rows).toEqual(['1https://a.test/', '2https://b.test/']);
    expect($('#count')?.textContent).toBe('2 pages said hello');
  });
});

describe('an event of hello-panel', () => {
  it('is a greeting, a page or nothing', () => {
    expect(readEvent({ greeted: 'Ada' }, undefined)).toEqual({ greeted: 'Ada' });
    expect(readEvent({ loaded: 'https://a.test/', view: 3 }, undefined)).toEqual({ page: { view: 3, url: 'https://a.test/' } });
    expect(readEvent({ page: 'https://b.test/', view: 3 }, 4)).toEqual({ page: { view: 4, url: 'https://b.test/' } });
    expect(readEvent({ page: 'https://c.test/' }, undefined)).toEqual({ page: { view: null, url: 'https://c.test/' } });
    expect(readEvent({ host: 'waiting' }, undefined)).toBeNull();
    expect(readEvent('text', 1)).toBeNull();
    expect(readEvent(null, 1)).toBeNull();
    expect(readEvent({ greeted: 7 }, undefined)).toBeNull();
  });
});
