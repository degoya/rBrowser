// The overlays panel in a window of its own (RB-080-86): the bridge messages it sends and how it
// draws the plugin's answers and the views' events. The panels window is played by spying on
// postMessage and dispatching `message` events from window.parent (the test window itself).
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { mountPanel } from '../../../web-common/panel-ui/src/mount';
import en from '../../locales/en.json';
import App from './panel/App.vue';
import { PREFIX } from './panel/model';

type Posted = { type?: string; id?: number; tool?: string; command?: string; args?: Record<string, unknown> };

let posted: Posted[] = [];
let unmount: () => void = () => undefined;

function flatten(value: unknown, prefix: string, into: Record<string, string>): Record<string, string> {
  if (typeof value === 'string') into[prefix] = value;
  else if (value && typeof value === 'object') for (const [key, child] of Object.entries(value)) flatten(child, prefix ? `${prefix}.${key}` : key, into);
  return into;
}
const STRINGS = { ...flatten(en, '', {}), 'plugins.panel.call_failed': 'The plugin could not do that.' };

const $ = (selector: string): HTMLElement | null => document.querySelector<HTMLElement>(selector);
const settle = async (): Promise<void> => {
  await new Promise((resolve) => setTimeout(resolve, 0));
  await nextTick();
};
async function fromHost(data: unknown): Promise<void> {
  window.dispatchEvent(new MessageEvent('message', { data, source: window }));
  await settle();
}
const calls = (): Posted[] => posted.filter((message) => message.type === 'call');
const last = (): Posted | undefined => calls().at(-1);
const answer = (call: Posted | undefined, value: unknown): Promise<void> => fromHost({ type: 'result', id: call?.id, ok: true, value });
const status = (): HTMLElement => $('#status') as HTMLElement;

async function click(selector: string, shiftKey = false): Promise<void> {
  ($(selector) as HTMLElement).dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true, shiftKey }));
  await settle();
}
async function typeSelector(value: string): Promise<HTMLInputElement> {
  const input = $('#selector') as HTMLInputElement;
  input.value = value;
  input.dispatchEvent(new Event('input', { bubbles: true }));
  await settle();
  return input;
}
async function key(target: HTMLElement, name: string): Promise<void> {
  target.dispatchEvent(new KeyboardEvent('keydown', { key: name, bubbles: true, cancelable: true }));
  await settle();
}

const STATE = { all: { overflow: { enabled: true }, highlight: { enabled: false, options: { selector: '.card' } } }, views: {} };

/** The texts, then the answer to the first `state` call. */
async function ready(overlays: unknown = STATE.all): Promise<void> {
  await fromHost({ type: 'settings', strings: STRINGS, settings: {} });
  await answer(calls().find((m) => m.tool === 'state'), { overlays });
}

beforeEach(() => {
  posted = [];
  document.body.innerHTML = '<div id="app"></div>';
  vi.spyOn(window, 'postMessage').mockImplementation((message: unknown) => {
    posted.push(message as Posted);
  });
  unmount = mountPanel(App, { prefix: PREFIX, name: 'overlays panel' }).unmount;
});

afterEach(() => {
  unmount();
  vi.restoreAllMocks();
});

describe('the overlays panel', () => {
  it('asks for its texts, then for the overlays every view shows', async () => {
    expect(posted[0]).toEqual({ type: 'settings' });
    expect(calls()).toHaveLength(0);
    await fromHost({ type: 'settings', strings: STRINGS, settings: {} });
    expect(calls()).toEqual([expect.objectContaining({ tool: 'state', args: {} })]);
    await answer(calls()[0], { overlays: STATE.all });
    expect($('#title')?.textContent).toBe('Overlays');
    expect(($('#toggle-overflow') as HTMLElement).getAttribute('aria-checked')).toBe('true');
    expect(($('#toggle-highlight') as HTMLElement).getAttribute('aria-checked')).toBe('false');
    expect(($('#selector') as HTMLInputElement).value).toBe('.card');
  });

  it('groups the switches by kind under named headings, each with its name', async () => {
    await ready();
    const groups = [...document.querySelectorAll<HTMLElement>('[data-group]')];
    expect(groups.map((group) => group.querySelector('h2')?.textContent)).toEqual(['Layout', 'Elements by selector', 'Accessibility', 'Page content']);
    expect(groups.map((group) => group.getAttribute('aria-labelledby'))).toEqual(groups.map((group) => group.querySelector('h2')?.id));
    expect([...groups[1]!.querySelectorAll('[data-toggle]')].map((item) => item.textContent)).toEqual(['Element highlight', 'Force state on the selector']);
    expect(document.querySelectorAll('[role="switch"]')).toHaveLength(16);
    expect($('#shift-hint')?.textContent).toBe('Shift-click a switch to change the active view only.');
  });

  it('has All off as a header icon button with its name', async () => {
    await ready();
    const off = $('#off') as HTMLButtonElement;
    expect(off.getAttribute('aria-label')).toBe('All overlays off');
    expect(off.textContent).toBe('');
    await click('#off');
    expect(last()).toMatchObject({ command: 'off', args: {} });
  });

  it('switches an overlay in every view, Shift-click in the active view only', async () => {
    await ready();
    await click('#toggle-outlines');
    expect(last()).toMatchObject({ tool: 'set', args: { overlay: 'outlines', enabled: true, options: {} } });
    expect(last()?.args).not.toHaveProperty('target');
    expect(($('#toggle-outlines') as HTMLElement).getAttribute('aria-checked')).toBe('true');
    await click('#toggle-overflow', true);
    expect(last()).toMatchObject({ tool: 'set', args: { overlay: 'overflow', enabled: false, options: {}, target: 'active' } });
    // The next state is the component's word: the active view's change leaves every view's on.
    await fromHost({ type: 'event', view: 1, message: { type: 'state', state: STATE } });
    expect(($('#toggle-overflow') as HTMLElement).getAttribute('aria-checked')).toBe('true');
    expect(($('#toggle-outlines') as HTMLElement).getAttribute('aria-checked')).toBe('false');
  });

  it('gives the highlight and force-state the selector and the forced state', async () => {
    await ready({});
    await typeSelector(' nav a ');
    await click('#toggle-highlight');
    expect(last()?.args).toEqual({ overlay: 'highlight', enabled: true, options: { selector: 'nav a' } });
    await click('#toggle-force-state');
    expect(last()?.args).toEqual({ overlay: 'force-state', enabled: true, options: { selector: 'nav a', states: ['hover'] } });
  });

  it('finds the selector with Enter and steps through the matches with the arrow keys and buttons', async () => {
    await ready({});
    const input = await typeSelector('.card');
    await key(input, 'Enter');
    expect(last()).toMatchObject({ command: 'highlight', args: { selector: '.card' } });
    await answer(last(), { matches: [{ count: 2 }, { count: 1 }] });
    expect(status().textContent).toBe('3 matches in 2 views.');
    expect(status().dataset.tone).toBe('info');
    await key(input, 'ArrowDown');
    expect(last()).toMatchObject({ command: 'highlight', args: { step: 1 } });
    await key(input, 'ArrowUp');
    expect(last()).toMatchObject({ command: 'highlight', args: { step: -1 } });
    await click('#next');
    expect(last()).toMatchObject({ command: 'highlight', args: { step: 1 } });
    await click('#previous');
    expect(last()).toMatchObject({ command: 'highlight', args: { step: -1 } });
    expect(($('#previous') as HTMLElement).getAttribute('aria-label')).toBe('Previous match');
    await click('#find');
    expect(last()).toMatchObject({ command: 'highlight', args: { selector: '.card' } });
    await answer(last(), { matches: [{ count: 1 }] });
    expect(status().textContent).toBe('1 match in 1 view.');
  });

  it('asks for a selector before finding, in a sentence', async () => {
    await ready({});
    const before = calls().length;
    await typeSelector('  ');
    await click('#find');
    expect(calls()).toHaveLength(before);
    expect(status().textContent).toBe('Enter a CSS selector in the Overlays panel first.');
    expect(status().dataset.tone).toBe('error');
  });

  it('says a failed call in one sentence, never the code', async () => {
    await ready();
    await click('#off');
    await fromHost({ type: 'result', id: last()?.id, ok: false, error: 'plugin.overlays.errors.unknown_overlay', params: { overlay: 'x' } });
    expect(status().textContent).toBe('There is no overlay "x".');
    await click('#off');
    await fromHost({ type: 'result', id: last()?.id, ok: false, error: 'plugins.error.busy' });
    expect(status().textContent).toBe('The plugin could not do that.');
    await click('#off');
    await answer(last(), {});
    expect(status().textContent).toBe('');
  });

  it('lists what each view reports for the overlays that are on', async () => {
    await ready({ overflow: { enabled: true }, highlight: { enabled: true }, 'target-size': { enabled: true } });
    expect($('#empty')?.textContent).toBe('No view has reported yet.');
    await fromHost({ type: 'event', view: 2, message: { type: 'matches', count: 3, index: 0 } });
    await fromHost({ type: 'event', view: 1, message: { type: 'hits', overlay: 'target-size', count: 4 } });
    await fromHost({ type: 'event', view: 1, message: { type: 'overflow', report: { hasHorizontalScroll: false, scrollWidth: 375, clientWidth: 375, innerWidth: 375, causes: [], vw100: [] } } });
    expect([...document.querySelectorAll('#views li')].map((item) => item.textContent)).toEqual([
      'View 1: no horizontal scroll; Target size: 4',
      'View 2: match 1 of 3',
    ]);
    expect($('#empty')).toBeNull();
  });

  it('compares the tab order across the views, from the answer and from the event', async () => {
    await ready();
    expect($('#tab-title')?.textContent).toBe('Tab order across views');
    await click('#tab-compare');
    expect(last()).toMatchObject({ tool: 'tab_order', args: {} });
    await answer(last(), { differences: { views: [1, 2], onlyIn: [], onlyInCount: 0, moved: [], movedCount: 0 } });
    expect($('#tab-diffs')?.textContent).toBe('Every view tabs through the same elements in the same order.');
    await fromHost({ type: 'event', view: 1, message: { type: 'tab-order', differences: { views: [1], onlyIn: [], moved: [] } } });
    expect($('#tab-diffs')?.textContent).toBe('Comparing needs at least two views that answered.');
  });

  it('ignores messages from anyone but the panels window', async () => {
    window.dispatchEvent(new MessageEvent('message', { data: { type: 'settings', strings: STRINGS }, source: null }));
    await settle();
    expect(document.body.dataset.texts).toBe('waiting');
    expect(calls()).toHaveLength(0);
    await ready();
    window.dispatchEvent(new MessageEvent('message', { data: { type: 'event', view: 1, message: { type: 'state', state: { all: {}, views: {} } } }, source: null }));
    await settle();
    expect(($('#toggle-overflow') as HTMLElement).getAttribute('aria-checked')).toBe('true');
  });
});
