// The structured data panel in its frame (RB-090-96): mounted as the panel frame mounts it; the
// panels window is played by spying on postMessage and dispatching `message` events from
// window.parent (the test window itself).
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { mountPanel } from '../../../../web-common/panel-ui/src/mount';
import App from './App.vue';
import { ANSWER, STRINGS as TEXTS } from './fixtures';

type Posted = { type?: string; id?: number; tool?: string; command?: string; args?: Record<string, unknown> };

const STRINGS = { ...TEXTS, 'plugins.panel.call_failed': 'The plugin could not do that.' };

let posted: Posted[] = [];
let unmount: () => void = () => undefined;
const answered = new Set<number>();

const $ = (selector: string): HTMLElement | null => document.querySelector<HTMLElement>(selector);
const settle = async (): Promise<void> => {
  await new Promise((resolve) => setTimeout(resolve, 0));
  await nextTick();
};

async function fromHost(data: unknown): Promise<void> {
  window.dispatchEvent(new MessageEvent('message', { data, source: window }));
  await settle();
}

/** Answers the calls of `name` (a tool or a command) not answered yet. */
async function reply(name: string, value: unknown, ok = true): Promise<void> {
  for (const message of posted.filter((m) => m.type === 'call' && (m.tool ?? m.command) === name && !answered.has(m.id ?? 0))) {
    answered.add(message.id ?? 0);
    await fromHost(ok ? { type: 'result', id: message.id, ok: true, value } : { type: 'result', id: message.id, ok: false, error: value });
  }
}

const rows = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>('#tree [role="treeitem"]')];
const tabButton = (index: number): HTMLElement => document.querySelectorAll<HTMLElement>('#tabs [role="tab"]')[index] as HTMLElement;
const openTab = async (index: number): Promise<void> => {
  tabButton(index).dispatchEvent(new MouseEvent('mousedown', { bubbles: true, button: 0 }));
  await settle();
};

beforeEach(async () => {
  posted = [];
  answered.clear();
  document.body.innerHTML = '<div id="app"></div>';
  vi.spyOn(window, 'postMessage').mockImplementation((message: unknown) => {
    posted.push(message as Posted);
  });
  unmount = mountPanel(App, { prefix: 'plugin.structured-data.', name: 'structured data panel' }).unmount;
  await fromHost({ type: 'settings', strings: STRINGS, settings: { 'plugins.panelMessageMaxBytes': 1024 * 1024 } });
});

afterEach(() => {
  unmount();
  vi.restoreAllMocks();
});

describe('the structured data panel', () => {
  it('reads the active view at once, within its message limit, and says so', () => {
    expect($('#status')?.textContent).toBe('Reading the page...');
    expect(posted.find((m) => m.tool === 'inspect')?.args).toEqual({ budget: 1024 * 1024 - 5536 });
  });

  it('shows the tree, the findings and the raw JSON-LD as tabs', async () => {
    await reply('inspect', ANSWER);
    const tabs = [...document.querySelectorAll<HTMLElement>('#tabs [role="tab"]')];
    expect(tabs.map((one) => one.textContent?.trim())).toEqual(['Tree', 'Findings3', 'JSON-LD']);
    expect($('#vocabulary')?.textContent).toBe('schema.org 30.1');
  });

  it('draws every node with its types and the worst finding at it', async () => {
    await reply('inspect', ANSWER);
    const labels = rows().map((row) => row.querySelector('[data-label]')?.textContent);
    expect(labels).toContain('Organization');
    expect(labels).toContain('Article');
    expect(labels).toContain('headlinee: typo');
    const article = rows().find((row) => row.querySelector('[data-node="n1"]'));
    expect(article?.querySelector('[data-severity]')?.textContent?.trim()).toBe('Error 2');
  });

  it('shows the node of a finding in the tree', async () => {
    await reply('inspect', ANSWER);
    await openTab(1);
    expect($('#summary')?.textContent).toBe('1 error, 2 warnings, 0 notes');
    const first = $('#findings-table [data-finding]') as HTMLButtonElement;
    expect(first.textContent).toBe('Article: schema.org has no property headlinee.');
    first.click();
    await settle();
    expect(tabButton(0).getAttribute('aria-selected')).toBe('true');
    expect($('#node-n1')).not.toBeNull();
  });

  it('shows the raw blocks', async () => {
    await reply('inspect', { ...ANSWER, blocks: [...ANSWER.blocks, { index: 1, path: 'p', text: '{broken', textCut: true, error: 'EOF' }] });
    await openTab(2);
    const blocks = [...document.querySelectorAll<HTMLElement>('[data-block]')];
    expect(blocks).toHaveLength(2);
    expect(blocks[1]?.querySelector('[data-error]')?.textContent).toContain('Not JSON: EOF');
    expect(blocks[1]?.querySelector('[data-cut]')).not.toBeNull();
  });

  it('reads the view chosen, and sends the findings to the findings panel', async () => {
    await reply('inspect', ANSWER);
    expect(posted.filter((m) => m.tool === 'inspect')).toHaveLength(1);
    ($('#report') as HTMLButtonElement).click();
    await settle();
    expect(posted.some((m) => m.command === 'check')).toBe(true);
    await reply('check', { findings: 3 });
    expect($('#status')?.textContent).toBe('3 findings sent to the findings panel.');
  });

  it('says an empty page and a failed call in one sentence', async () => {
    await reply('inspect', { ...ANSWER, items: [], blocks: [], findings: [] });
    expect($('#empty')?.textContent).toContain('The page has no structured data.');
    ($('#refresh') as HTMLButtonElement).click();
    await settle();
    await reply('inspect', 'plugins.error.busy', false);
    expect($('#status')?.textContent).toBe('The plugin could not do that.');
  });
});
