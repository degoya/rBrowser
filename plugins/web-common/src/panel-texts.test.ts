/// <reference path="../panel/texts.d.ts" />
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { runClassicScripts } from './harness';

// RB-080-59 and its review: the one source of the first-party panels' texts handling.

const PLUGINS = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const source = readFileSync(join(PLUGINS, 'web-common', 'panel', 'texts.js'), 'utf8');
let asked = 0;
let stop: () => void = () => undefined;

const make = (): PanelTextsOf => PanelTexts.create('plugin.demo.', 'demo panel');
const notice = (): HTMLElement | null => document.querySelector('[data-texts-notice]');

beforeEach(() => {
  vi.useFakeTimers();
  asked = 0;
  vi.spyOn(window, 'postMessage').mockImplementation((message: unknown) => {
    if ((message as { type?: string }).type === 'settings') asked += 1;
  });
  document.head.innerHTML = '';
  document.body.innerHTML = '<main><h1>x</h1></main>';
  delete document.body.dataset.texts;
  stop = runClassicScripts([source]);
});

afterEach(() => {
  stop();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe('a panel\'s texts', () => {
  it('hide the panel until they arrived', () => {
    const texts = make();
    expect(document.body.dataset.texts).toBe('waiting');
    expect(document.head.querySelector('style')?.textContent).toContain('body[data-texts="waiting"] > :not([data-texts-notice])');
    expect(texts.ready).toBe(false);
    expect(texts.take({ 'plugin.demo.title': 'Demo' })).toBe(true);
    expect(document.body.dataset.texts).toBeUndefined();
    expect(texts.take({ 'plugin.demo.title': 'Demo' })).toBe(false);
    expect(texts.text('title')).toBe('Demo');
  });

  it('are asked for again every second until they arrive', async () => {
    const texts = make();
    texts.ask();
    expect(asked).toBe(1);
    await vi.advanceTimersByTimeAsync(2000);
    expect(asked).toBe(3);
    texts.take({});
    await vi.advanceTimersByTimeAsync(20_000);
    expect(asked).toBe(3);
    expect(notice()).toBeNull();
  });

  // Review of RB-080-59: after ten requests the panel stayed blank for good.
  it('say after ten requests that they did not arrive, with a Retry button', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    const texts = make();
    texts.ask();
    await vi.advanceTimersByTimeAsync(30_000);
    expect(asked).toBe(10);
    expect(warn).toHaveBeenCalledWith('demo panel: no texts from the plugin panels window');
    expect(notice()?.getAttribute('role')).toBe('alert');
    expect(notice()?.textContent).toContain("The panel's texts did not arrive.");
    expect(notice()?.parentElement).toBe(document.body);
    notice()?.querySelector('button')?.click();
    expect(notice()).toBeNull();
    expect(asked).toBe(11);
    await vi.advanceTimersByTimeAsync(30_000);
    expect(notice()).not.toBeNull();
    texts.take({});
    expect(notice()).toBeNull();
  });

  it('fill their parameters and log a missing key once, only after they arrived', () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    const texts = make();
    expect(texts.text('gone')).toBe('gone');
    expect(warn).not.toHaveBeenCalled();
    texts.take({ 'plugin.demo.hello': 'Hello {name}', 'plugins.error.busy': 'Busy' });
    expect(texts.text('hello', { name: 'Ada' })).toBe('Hello Ada');
    expect(texts.full('plugins.error.busy')).toBe('Busy');
    expect(texts.text('gone')).toBe('gone');
    expect(texts.text('gone')).toBe('gone');
    texts.missing('plugin.demo.other');
    expect(warn.mock.calls.map(([line]) => line)).toEqual(['demo panel: no text for plugin.demo.gone', 'demo panel: no text for plugin.demo.other']);
  });
});

describe('the copies', () => {
  // RB-080-86: the last copies (rulers-guides, screenshot-editor) and the simulators' import went
  // with their panels' move to Nuxt UI; panel-ui's prelude bundles this file into every such panel.
  it('no panel ships a copy; panel-ui bundles this file', () => {
    const copies = readdirSync(PLUGINS).filter((plugin) => plugin !== 'web-common' && existsSync(join(PLUGINS, plugin, 'panel', 'texts.js')));
    expect(copies).toEqual([]);
    const prelude = readFileSync(join(PLUGINS, 'web-common', 'panel-ui', 'src', 'prelude.ts'), 'utf8');
    expect(prelude).toContain("import '../../panel/texts.js';");
  });
});

// RB-080-90: the panels showed "Failed: plugins.error.host_failed" (the code, its reason lost)
// and "Marked 152 elements in 1 views". A failure is a sentence with its reason; a count picks
// its plural form, the catalogue's ICU form as the core's and the shell's (ui/src/i18n/format.ts).
describe('a message', () => {
  it('fills its placeholders and picks the plural form of a count', () => {
    const marked = 'Marked {count, plural, one {# element} other {# elements}} in {views, plural, one {# view} other {# views}}.';
    expect(PanelTexts.format(marked, { count: 1, views: 1 })).toBe('Marked 1 element in 1 view.');
    expect(PanelTexts.format(marked, { count: 152, views: 3 })).toBe('Marked 152 elements in 3 views.');
    expect(PanelTexts.format('{n, plural, =0 {No findings} one {# finding} other {# findings}}', { n: 0 })).toBe('No findings');
    expect(PanelTexts.format('{n, plural, one {# x} other {# xs}}', { n: '1' })).toBe('1 x');
    expect(PanelTexts.format('Hello {name}, {missing}.', { name: 'Ada' })).toBe('Hello Ada, .');
    // A message that does not parse is shown as written (the lint refuses it long before).
    expect(PanelTexts.format('{n, plural, one {# x}', { n: 1 })).toBe('{n, plural, one {# x}');
  });

  it('is plural in a panel\'s own texts too', () => {
    const texts = make();
    texts.take({ 'plugin.demo.views': '{count, plural, one {# view} other {# views}}' });
    expect(texts.text('views', { count: 1 })).toBe('1 view');
    expect(texts.text('views', { count: 2 })).toBe('2 views');
  });
});

describe('a failure', () => {
  const strings = {
    'plugins.error.host_failed': 'rBrowser could not run the plugin call. The log has the details.',
    'plugins.reason.timeout': 'The view did not answer in time; a running check may hold it.',
    'plugins.reason.exception': 'The page script threw: {message}',
    'plugins.panel.call_failed': 'The plugin could not do that.',
    'plugin.demo.errors.no_view': 'No view {view}.',
  };

  it('is the code\'s text and its reason\'s, never the code', () => {
    expect(PanelTexts.errorText(strings, 'plugins.error.host_failed', { reason: 'timeout' })).toBe(
      'rBrowser could not run the plugin call. The log has the details. The view did not answer in time; a running check may hold it.',
    );
    expect(PanelTexts.errorText(strings, 'plugins.error.host_failed', { reason: 'exception', message: 'TypeError: x' })).toBe(
      'rBrowser could not run the plugin call. The log has the details. The page script threw: TypeError: x',
    );
    expect(PanelTexts.errorText(strings, 'plugin.demo.errors.no_view', { view: '3' })).toBe('No view 3.');
    expect(PanelTexts.errorText(strings, 'plugins.error.unheard_of')).toBe('The plugin could not do that.');
    expect(PanelTexts.errorText({}, 'plugins.error.host_failed')).toBe('The plugin could not do that.');
  });

  it('names a reason without a text of its own only when it is words, not a code', () => {
    expect(PanelTexts.errorText(strings, 'plugins.error.host_failed', { reason: 'frame' })).toBe(
      'rBrowser could not run the plugin call. The log has the details.',
    );
    expect(PanelTexts.errorText(strings, 'plugins.error.host_failed', { reason: 'the target closed' })).toBe(
      'rBrowser could not run the plugin call. The log has the details. the target closed',
    );
  });

  it('is said through the texts a panel got', () => {
    const texts = make();
    texts.take(strings);
    expect(texts.error('plugins.error.host_failed', { reason: 'timeout' })).toContain('did not answer in time');
  });
});
