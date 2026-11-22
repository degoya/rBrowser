import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick, reactive, ref } from 'vue';
import '../../panel/texts.js';
import { createPanel } from './panel';

type Posted = Record<string, unknown>;
let posted: Posted[] = [];

const fromHost = (data: unknown, source: MessageEventSource | null = window): void => {
  window.dispatchEvent(new MessageEvent('message', { data, source }));
};

beforeEach(() => {
  posted = [];
  document.body.innerHTML = '<div id="app"></div>';
  vi.spyOn(window, 'postMessage').mockImplementation((message: unknown) => {
    posted.push(message as Posted);
  });
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('a Nuxt UI panel\'s link', () => {
  it('asks for its texts and takes them with the theme', async () => {
    const themes: unknown[] = [];
    const panel = createPanel({ prefix: 'plugin.demo.', name: 'demo', onTheme: (settings) => themes.push(settings['ui.theme']) });
    panel.start();
    expect(posted[0]).toEqual({ type: 'settings' });
    expect(panel.ready.value).toBe(false);
    expect(document.body.dataset.texts).toBe('waiting');
    fromHost({ type: 'settings', strings: { 'plugin.demo.panel.count': '{n, plural, one {# view} other {# views}}' }, settings: { 'ui.theme': 'dark' } });
    await nextTick();
    expect(panel.ready.value).toBe(true);
    expect(document.body.dataset.texts).toBeUndefined();
    expect(themes).toEqual(['dark']);
    expect(panel.t('panel.count', { n: 1 })).toBe('1 view');
    expect(panel.t('panel.count', { n: 3 })).toBe('3 views');
    expect(panel.t('panel.none')).toBe('panel.none');
  });

  it('calls a tool and a command, and answers each by its id', async () => {
    const panel = createPanel({ prefix: 'plugin.demo.', name: 'demo' });
    panel.start();
    const one = panel.call({ tool: 'status' });
    const two = panel.call({ command: 'run' }, { a: 1 });
    expect(posted.filter((m) => m.type === 'call')).toEqual([
      { type: 'call', id: 1, tool: 'status', args: {} },
      { type: 'call', id: 2, command: 'run', args: { a: 1 } },
    ]);
    fromHost({ type: 'result', id: 2, ok: false, error: 'plugins.error.busy', params: { reason: 'timeout' } });
    fromHost({ type: 'result', id: 1, ok: true, value: { x: 1 } });
    await expect(one).resolves.toEqual({ x: 1 });
    const failure = await two.catch((error: unknown) => error);
    fromHost({ type: 'settings', strings: { 'plugins.error.busy': 'Busy.', 'plugins.reason.timeout': 'Too long.' } });
    expect(panel.error(failure)).toBe('Busy. Too long.');
  });

  it('shows the host\'s waiting until the call answers', () => {
    const panel = createPanel({ prefix: 'plugin.demo.', name: 'demo' });
    panel.start();
    void panel.call({ command: 'check' });
    fromHost({ type: 'event', message: { host: 'waiting', plugin: 'demo' } });
    expect(panel.waiting.value).toBe(true);
    fromHost({ type: 'result', id: 1, ok: true, value: null });
    expect(panel.waiting.value).toBe(false);
  });

  it('hears only the panels window', () => {
    const panel = createPanel({ prefix: 'plugin.demo.', name: 'demo' });
    panel.start();
    fromHost({ type: 'settings', strings: {} }, null);
    expect(panel.ready.value).toBe(false);
  });

  it('posts a host action as it is', () => {
    const panel = createPanel({ prefix: 'plugin.demo.', name: 'demo' });
    panel.post({ target: 'host', action: 'open_link', url: 'https://example.test/' });
    expect(posted).toEqual([{ target: 'host', action: 'open_link', url: 'https://example.test/' }]);
  });

  it('asks the host to open the settings at a key', () => {
    const panel = createPanel({ prefix: 'plugin.demo.', name: 'demo' });
    panel.openSettings('api-client.benchRepeats');
    expect(posted).toEqual([{ target: 'host', action: 'open_settings', key: 'api-client.benchRepeats' }]);
  });

  // RB-096-13: a frame's postMessage clones its message as structured data, and a Vue reactive
  // value is a Proxy, which the clone refuses (DataCloneError). The panel's calls with the
  // editor's request in them (Save, Save as new, Copy as code) never left the frame and said only
  // "The plugin could not do that."; the call now posts plain data.
  it('posts a reactive argument as plain data, as the frame\'s clone needs', async () => {
    vi.mocked(window.postMessage).mockImplementation((message: unknown) => {
      posted.push(structuredClone(message) as Posted);
    });
    const panel = createPanel({ prefix: 'plugin.demo.', name: 'demo' });
    panel.start();
    const request = ref({ name: 'Me', headers: [{ name: 'a', value: 'b' }] });
    const answered = panel.call({ tool: 'panel' }, { op: { op: 'save', request: request.value, extra: reactive({ n: 1 }) } });
    expect(posted.filter((m) => m.type === 'call')).toEqual([
      { type: 'call', id: 1, tool: 'panel', args: { op: { op: 'save', request: { name: 'Me', headers: [{ name: 'a', value: 'b' }] }, extra: { n: 1 } } } },
    ]);
    fromHost({ type: 'result', id: 1, ok: true, value: { id: 'me.http' } });
    await expect(answered).resolves.toEqual({ id: 'me.http' });
  });

  it('says a call it could not post with its code and reason, and logs it', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    vi.mocked(window.postMessage).mockImplementation((message: unknown) => {
      if ((message as Posted).type === 'call') throw new DOMException('#<Object> could not be cloned.', 'DataCloneError');
    });
    const panel = createPanel({ prefix: 'plugin.demo.', name: 'demo' });
    panel.start();
    const failure = await panel.call({ tool: 'panel' }, { a: 1 }).catch((error: unknown) => error);
    expect((failure as Error).message).toBe('plugins.panel.not_sent');
    fromHost({ type: 'settings', strings: { 'plugins.panel.not_sent': 'The panel could not send this to rBrowser.' } });
    expect(panel.error(failure)).toBe('The panel could not send this to rBrowser. DataCloneError: #<Object> could not be cloned.');
    expect(warn.mock.calls.flat().join(' ')).toContain('plugins.panel.not_sent');
  });

  it('says an exception of the panel itself with its reason, never only "could not do that"', () => {
    vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    const panel = createPanel({ prefix: 'plugin.demo.', name: 'demo' });
    panel.start();
    fromHost({ type: 'settings', strings: { 'plugins.panel.call_failed': 'The plugin could not do that.' } });
    expect(panel.error(new TypeError("Cannot read properties of undefined (reading 'id')"))).toBe(
      "The plugin could not do that. TypeError: Cannot read properties of undefined (reading 'id')",
    );
  });
});
