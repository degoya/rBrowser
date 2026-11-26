import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { IpcClient } from '../../ipc/client';
import { IpcErrorCode, type CoreToShell, type ShellToCore, type SitemapDiscovery } from '../../ipc/protocol';
import { IpcError } from '../../ipc/transport';
import { mountWithShell, settle } from '../../test/mountShell';
import SitemapStartPage from './SitemapStartPage.vue';
import { CHOICE_KEY } from './start';

let current: VueWrapper | null = null;

beforeEach(() => window.localStorage.removeItem(CHOICE_KEY));
afterEach(() => {
  current?.unmount();
  current = null;
  document.body.replaceChildren();
});

const PAGES = ['/', '/about', '/de', '/de/produkte/a', '/de/produkte/b'].map((path) => `https://a.test${path}`);

function discovery(): SitemapDiscovery {
  const leaf = (name: string, first: number) => ({ name, first, count: 1 });
  return {
    discoveryId: '1795000000-1',
    source: 'https://a.test/',
    site: 'https://a.test',
    pages: PAGES,
    tree: [
      {
        name: 'https://a.test',
        first: 0,
        count: 5,
        children: [
          leaf('/', 0),
          leaf('about', 1),
          {
            name: 'de',
            first: 2,
            count: 3,
            children: [leaf('/de', 2), { name: 'produkte', first: 3, count: 2, children: [leaf('a', 3), leaf('b', 4)] }],
          },
        ],
      },
    ],
    truncated: false,
    maxPages: 100_000,
    views: { all: 3, visible: 2 },
  };
}

/** The page over a client whose discovery answers `answer` (or never, or fails). */
async function mounted(answer: 'found' | 'never' | IpcError = 'found', start?: IpcError) {
  const asked: ShellToCore[] = [];
  const client = new IpcClient({
    send: (message): Promise<CoreToShell> => {
      asked.push(message);
      if (message.type === 'sitemapStart' && message.op === 'discover') {
        if (answer === 'never') return new Promise(() => undefined);
        if (answer instanceof IpcError) return Promise.reject(answer);
        return Promise.resolve({ type: 'sitemapDiscovery', ...discovery() });
      }
      if (message.type === 'runCommand' && start !== undefined) return Promise.reject(start);
      return Promise.resolve({ type: 'ack' });
    },
    subscribe: () => () => undefined,
  });
  const result = await mountWithShell(SitemapStartPage, { mount: { props: { client } } });
  current = result.wrapper;
  await settle();
  return { wrapper: result.wrapper, asked };
}

const rowNames = (wrapper: VueWrapper): string[] =>
  wrapper.findAll('[role="treeitem"]').map((row) => row.get('[data-name]').text());

function nodeId(wrapper: VueWrapper, name: string): string {
  const row = wrapper.findAll('[role="treeitem"]').find((candidate) => candidate.get('[data-name]').text() === name);
  if (row === undefined) throw new Error(`no row ${name}`);
  return row.attributes('data-testid') ?? '';
}

async function check(wrapper: VueWrapper, name: string): Promise<void> {
  await wrapper.get(`[data-testid="${nodeId(wrapper, name)}-check"]`).trigger('click');
  await settle();
}

const counter = (wrapper: VueWrapper): string => wrapper.get('[data-testid="sitemap-start-counter"]').text();
const startButton = (wrapper: VueWrapper) => wrapper.get('[data-testid="sitemap-start-start"]');

describe('the sitemap start page', () => {
  it('finds the pages first and can stop waiting for them', async () => {
    const { wrapper, asked } = await mounted('never');
    expect(asked).toContainEqual({ type: 'sitemapStart', op: 'discover' });
    expect(wrapper.find('[data-testid="sitemap-start-finding"]').exists()).toBe(true);
    await wrapper.get('[data-testid="sitemap-start-cancel"]').trigger('click');
    await settle();
    expect(wrapper.find('[data-testid="sitemap-start-cancelled"]').exists()).toBe(true);
    // RB-099-21: Cancel is gone, so the focus moves on to the way back, never to the body.
    expect(document.activeElement).toBe(wrapper.get('[data-testid="sitemap-start-again"]').element);
  });

  it('shows the pages as a tree with the sites open and the counts of each folder', async () => {
    const { wrapper } = await mounted();
    expect(rowNames(wrapper)).toEqual(['https://a.test', '/', 'about', 'de']);
    const site = wrapper.get(`[data-testid="${nodeId(wrapper, 'https://a.test')}"]`);
    expect(site.attributes('aria-expanded')).toBe('true');
    expect(site.text()).toContain('5 pages');
    expect(wrapper.get(`[data-testid="${nodeId(wrapper, 'de')}"]`).text()).toContain('3 pages');
    expect(startButton(wrapper).attributes('disabled')).toBeDefined();
    expect(wrapper.get('[data-testid="sitemap-start-blocked"]').text()).not.toBe('');
  });

  it('chooses a subtree, shows the site partly chosen and counts the shots', async () => {
    const { wrapper } = await mounted();
    await check(wrapper, 'de');
    expect(wrapper.get(`[data-testid="${nodeId(wrapper, 'de')}"]`).attributes('aria-checked')).toBe('true');
    expect(wrapper.get(`[data-testid="${nodeId(wrapper, 'https://a.test')}"]`).attributes('aria-checked')).toBe('mixed');
    expect(counter(wrapper)).toBe('3 pages × 3 views = 9 shots');
    await check(wrapper, 'about');
    expect(counter(wrapper)).toBe('4 pages × 3 views = 12 shots');
    expect(startButton(wrapper).attributes('disabled')).toBeUndefined();
  });

  it('starts sitemap.capture with the discovery and exactly the chosen pages', async () => {
    const { wrapper, asked } = await mounted();
    await check(wrapper, 'about');
    await wrapper.get(`[data-testid="${nodeId(wrapper, 'de')}-flip"]`).trigger('click');
    await settle();
    await check(wrapper, 'produkte');
    await startButton(wrapper).trigger('click');
    await settle();
    expect(asked).toContainEqual({
      type: 'runCommand',
      id: 'sitemap.capture',
      args: { discovery: '1795000000-1', urls: ['https://a.test/about', 'https://a.test/de/produkte/a', 'https://a.test/de/produkte/b'] },
    });
    expect(wrapper.find('[data-testid="sitemap-start-starting"]').exists()).toBe(true);
  });

  it('all and none follow the search', async () => {
    const { wrapper } = await mounted();
    await wrapper.get('[data-testid="sitemap-start-search"]').setValue('produkte');
    await settle();
    expect(rowNames(wrapper)).toEqual(['https://a.test', 'de', 'produkte', 'a', 'b']);
    await wrapper.get('[data-testid="sitemap-start-all"]').trigger('click');
    await settle();
    expect(counter(wrapper)).toBe('2 pages × 3 views = 6 shots');
    await wrapper.get('[data-testid="sitemap-start-search"]').setValue('');
    await settle();
    await wrapper.get('[data-testid="sitemap-start-all"]').trigger('click');
    await settle();
    expect(counter(wrapper)).toBe('5 pages × 3 views = 15 shots');
    await wrapper.get('[data-testid="sitemap-start-none"]').trigger('click');
    await settle();
    expect(counter(wrapper)).toBe('0 pages × 3 views = 0 shots');
  });

  it('is worked with the keyboard: arrows move and open, Space chooses', async () => {
    const { wrapper } = await mounted();
    const tree = wrapper.get('[data-testid="sitemap-tree"]');
    expect(document.activeElement).toBe(tree.element);
    // RB-099-21: a row is chosen by its check (aria-checked); the active row is the
    // activedescendant, not a selection.
    expect(tree.attributes('aria-multiselectable')).toBeUndefined();
    expect(wrapper.findAll('[role="treeitem"]').filter((row) => row.attributes('aria-selected') !== undefined)).toEqual([]);
    const press = async (key: string) => {
      await tree.trigger('keydown', { key });
      await settle();
    };
    await press('End');
    expect(tree.attributes('aria-activedescendant')).toBe(nodeIdOf(wrapper, 'de'));
    await press('ArrowRight');
    expect(rowNames(wrapper)).toEqual(['https://a.test', '/', 'about', 'de', '/de', 'produkte']);
    await press('ArrowRight');
    await press(' ');
    expect(counter(wrapper)).toBe('1 page × 3 views = 3 shots');
    await press('ArrowLeft');
    expect(tree.attributes('aria-activedescendant')).toBe(nodeIdOf(wrapper, 'de'));
  });

  it('takes only the visible views when chosen and remembers the choice', async () => {
    const { wrapper, asked } = await mounted();
    await check(wrapper, '/');
    await wrapper.get('[data-testid="sitemap-start-views"] [role="radio"][value="visible"]').trigger('click');
    await settle();
    expect(counter(wrapper)).toBe('1 page × 2 views = 2 shots');
    await startButton(wrapper).trigger('click');
    await settle();
    const run = asked.find((message) => message.type === 'runCommand');
    expect(run).toMatchObject({ args: { views: 'visible', urls: ['https://a.test/'] } });
    expect(window.localStorage.getItem(CHOICE_KEY)).toContain('visible');
  });

  it('says what to do when the pages could not be found, and finds them again', async () => {
    const failure = new IpcError(IpcErrorCode.NotFound, 'sitemap.error.no_pages: the sitemap gave no page to capture');
    const { wrapper, asked } = await mounted(failure);
    const alert = wrapper.get('[data-testid="sitemap-start-failed"]');
    expect(alert.text()).toContain('The sitemap names no page to capture');
    expect(alert.text()).not.toContain('sitemap.error');
    // RB-099-21: the failure is said at once, and the focus waits on the way to try again.
    expect(alert.get('[role="alert"]').text()).toContain('The sitemap names no page to capture');
    expect(document.activeElement).toBe(wrapper.get('[data-testid="sitemap-start-again"]').element);
    await wrapper.get('[data-testid="sitemap-start-again"]').trigger('click');
    await settle();
    expect(asked.filter((message) => message.type === 'sitemapStart')).toHaveLength(2);
  });

  it('shows a refused start on the page and lets the person try again', async () => {
    const busy = new IpcError(IpcErrorCode.Internal, 'command failed: sitemap.error.busy: a sitemap capture is already running');
    const { wrapper } = await mounted('found', busy);
    await check(wrapper, '/');
    await startButton(wrapper).trigger('click');
    await settle();
    const refused = wrapper.get('[data-testid="sitemap-start-refused"]');
    expect(refused.text()).toContain('A sitemap capture is running already');
    expect(refused.text()).not.toContain('sitemap.error');
    expect(refused.attributes('role')).toBe('alert');
    expect(wrapper.find('[data-testid="sitemap-start-starting"]').exists()).toBe(false);
    expect(startButton(wrapper).attributes('disabled')).toBeUndefined();
  });

  it('closes its window with Close and with Escape', async () => {
    const { wrapper, asked } = await mounted();
    await wrapper.get('[data-testid="sitemap-start-close"]').trigger('click');
    await wrapper.get('[data-testid="sitemap-start"]').trigger('keydown', { key: 'Escape' });
    await settle();
    expect(asked.filter((message) => message.type === 'sitemapStart' && message.op === 'close')).toHaveLength(2);
  });
});

function nodeIdOf(wrapper: VueWrapper, name: string): string {
  return nodeId(wrapper, name).replace(/^node-/, 'sitemap-node-');
}
