// Settings > Plugins > Catalog (RB-091-01): the page lists both origins of one plugin, shows
// the capabilities and the publisher key before an install, and sends the install only from the
// dialog's confirm, with exactly what it showed.
import ui from '@nuxt/ui/vue-plugin';
import { mount, type VueWrapper } from '@vue/test-utils';
import { afterEach, describe, expect, it } from 'vitest';
import { createShellI18n } from '../../i18n';
import type { IpcClient } from '../../ipc/client';
import type { CoreToShell } from '../../ipc/protocol';
import type { PanelRequest } from '../../panels/protocol';
import { settle } from '../test/mountPage';
import CatalogPanel from './CatalogPanel.vue';
import { SAMPLE_CATALOG, SAMPLE_CHANGED_KEY_REVIEW, SAMPLE_REVIEW } from './catalogSample';

let current: VueWrapper | null = null;

afterEach(() => {
  current?.unmount();
  current = null;
});

/** The core's events the page listens to (RB-092-06), sent by the test. */
let emit: (message: CoreToShell) => void = () => undefined;

/** A client answering every catalogue request with the samples, recording what was asked; a
 * change answers `restart` as given. */
function stubClient(failInstall = false, review: unknown = SAMPLE_REVIEW, restart = true) {
  const requests: PanelRequest[] = [];
  const client = {
    async expect(message: { type: string; request: PanelRequest }) {
      requests.push(message.request);
      if (failInstall && message.request.op === 'catalogInstall') {
        throw new Error('plugins.catalog.error.checksum');
      }
      const answers: Record<string, unknown> = { catalog: SAMPLE_CATALOG, catalogPrepare: review };
      return { type: 'pluginPanel', message: answers[message.request.op] ?? { value: { restart } } };
    },
    on(listener: (message: CoreToShell) => void) {
      emit = listener;
      return () => {
        emit = () => undefined;
      };
    },
  };
  return { client: client as unknown as IpcClient, requests };
}

async function open(failInstall = false, review: unknown = SAMPLE_REVIEW, restart = true) {
  const { client, requests } = stubClient(failInstall, review, restart);
  const wrapper = mount(CatalogPanel, {
    props: { client },
    attachTo: document.body,
    global: { plugins: [ui, createShellI18n()] },
  });
  current = wrapper;
  await settle();
  return { wrapper, requests };
}

describe('Settings > Plugins > Catalog', () => {
  it('asks for the catalogue without reaching the network and lists both origins of a plugin', async () => {
    const { wrapper, requests } = await open();
    expect(requests[0]).toEqual({ op: 'catalog', refresh: false });
    const seo = wrapper.findAll('[data-testid="catalog-row"][data-plugin="seo"]');
    expect(seo.map((row) => row.attributes('data-repository'))).toEqual(['official', 'user-0123456789abcdef']);
  });

  it('checks for updates only when asked', async () => {
    const { wrapper, requests } = await open();
    await wrapper.get('[data-testid="catalog-refresh"]').trigger('click');
    await settle();
    expect(requests.at(-1)).toEqual({ op: 'catalog', refresh: true });
  });

  const click = (testid: string) => (document.querySelector(`[data-testid="${testid}"]`) as HTMLElement | null)?.click();
  const installs = (requests: PanelRequest[]) => requests.filter((request) => request.op === 'catalogInstall');

  it('reviews the package first, shows its full grants, and trusts a new key only in its own step', async () => {
    const { wrapper, requests } = await open();
    await wrapper.get('[data-plugin="todo"] [data-testid="catalog-install"]').trigger('click');
    await settle();
    expect(requests).toContainEqual({ op: 'catalogPrepare', repository: 'official', id: 'todo', version: '1.0.0' });
    expect(installs(requests)).toEqual([]);
    expect(document.querySelector('[data-testid="trust-grants"]')?.textContent).toContain('DOM');
    expect(document.querySelector('[data-testid="catalog-fingerprint"]')?.textContent).toContain('ab'.repeat(32));
    click('catalog-confirm-install');
    await settle();
    // The key is not trusted: its own step, still nothing sent.
    expect(document.querySelector('[data-testid="catalog-key-step"]')).not.toBeNull();
    expect(installs(requests)).toEqual([]);
    click('catalog-trust-key');
    await settle();
    expect(installs(requests)).toEqual([
      {
        op: 'catalogInstall',
        repository: 'official',
        id: 'todo',
        version: '1.0.0',
        consent: {
          capabilities: ['views', 'cdp'],
          grants: { evaluate: false, cdpDomains: ['DOM'], projectGlobs: [], sidecarMethods: [], risky: ['cdp:DOM'] },
          keyFingerprint: 'ab'.repeat(32),
        },
      },
    ]);
  });

  it('warns about an update signed by another key and cancels by default', async () => {
    const { wrapper, requests } = await open(false, SAMPLE_CHANGED_KEY_REVIEW);
    await wrapper.get('[data-plugin="todo"] [data-testid="catalog-install"]').trigger('click');
    await settle();
    click('catalog-confirm-install');
    await settle();
    expect(document.querySelector('[data-testid="catalog-key-changed"]')).not.toBeNull();
    click('catalog-cancel');
    await settle();
    expect(installs(requests)).toEqual([]);
  });

  it('removes what the catalogue installed and says that a failure happened', async () => {
    const { wrapper, requests } = await open(true);
    await wrapper.get('[data-plugin="outline"] [data-testid="catalog-remove"]').trigger('click');
    await settle();
    expect(requests).toContainEqual({ op: 'catalogRemove', id: 'outline' });
    await wrapper.get('[data-plugin="todo"] [data-testid="catalog-install"]').trigger('click');
    await settle();
    (document.querySelector('[data-testid="catalog-confirm-install"]') as HTMLElement | null)?.click();
    await settle();
    (document.querySelector('[data-testid="catalog-trust-key"]') as HTMLElement | null)?.click();
    await settle();
    expect(wrapper.find('[data-testid="catalog-failure"]').exists()).toBe(true);
  });

  it('offers no second install of a plugin the start loaded', async () => {
    const { wrapper } = await open();
    const row = wrapper.get('[data-testid="catalog-row"][data-plugin="overlays"]');
    expect(row.find('[data-testid="catalog-install"]').exists()).toBe(false);
    expect(row.find('[data-testid="catalog-update"]').exists()).toBe(false);
  });

  it('says a removal took effect, and "at the next start" only when the core could not load it live (RB-092-06)', async () => {
    const live = await open(false, SAMPLE_REVIEW, false);
    await live.wrapper.get('[data-plugin="outline"] [data-testid="catalog-remove"]').trigger('click');
    await settle();
    const notice = live.wrapper.get('[data-testid="catalog-notice"]').text();
    expect(notice).not.toContain('next start');
    live.wrapper.unmount();
    current = null;

    const later = await open(false, SAMPLE_REVIEW, true);
    await later.wrapper.get('[data-plugin="outline"] [data-testid="catalog-remove"]').trigger('click');
    await settle();
    expect(later.wrapper.get('[data-testid="catalog-notice"]').text()).toContain('next start');
  });

  it('asks for the catalogue again when the installed plugins changed elsewhere (RB-092-06)', async () => {
    const { requests } = await open();
    const asked = () => requests.filter((request) => request.op === 'catalog').length;
    expect(asked()).toBe(1);
    emit({ type: 'pluginPanel', message: { panelsChanged: true } });
    await settle();
    expect(asked()).toBe(1);
    emit({ type: 'pluginPanel', message: { pluginsChanged: true } });
    await settle();
    expect(asked()).toBe(2);
    expect(requests.at(-1)).toEqual({ op: 'catalog', refresh: false });
  });

  it('opens the automatic updates setting above the list', async () => {
    const { wrapper } = await open();
    await wrapper.get('[data-testid="catalog-auto-update-key"]').trigger('click');
    expect(wrapper.emitted('open')).toEqual([['plugins.catalog.autoUpdate']]);
  });

  it('opens the repositories setting', async () => {
    const { wrapper } = await open();
    await wrapper.get('[data-testid="catalog-repositories-key"]').trigger('click');
    expect(wrapper.emitted('open')).toEqual([['plugins.catalog.repositories']]);
  });
});

describe('Settings > Plugins > Catalog, the bundled plugins (RB-091-04)', () => {
  const peek = {
    id: 'peek',
    name: 'Peek',
    description: 'Hold Alt.',
    version: '1.0.0',
    kinds: ['feature-plugin'],
    category: 'overlays',
    sizeBytes: 1024,
    recommended: false,
    grants: [],
    installed: false,
    origin: 'bundle',
    update: null,
  };

  async function openWithBundle(installedPeek: boolean) {
    const requests: PanelRequest[] = [];
    const catalog = { ...SAMPLE_CATALOG, bundled: [{ ...peek, installed: installedPeek }] };
    const client = {
      async expect(message: { type: string; request: PanelRequest }) {
        requests.push(message.request);
        const answer =
          message.request.op === 'catalog'
            ? catalog
            : { value: { results: [{ id: 'peek', ok: true, origin: 'bundle' }], restart: true } };
        return { type: 'pluginPanel', message: answer };
      },
      on: () => () => undefined,
    };
    const wrapper = mount(CatalogPanel, {
      props: { client: client as unknown as IpcClient },
      attachTo: document.body,
      global: { plugins: [ui, createShellI18n()] },
    });
    current = wrapper;
    await settle();
    return { wrapper, requests };
  }

  it('installs a bundled plugin with one click, without a review', async () => {
    const { wrapper, requests } = await openWithBundle(false);
    const row = wrapper.get('[data-testid="catalog-row"][data-plugin="peek"]');
    expect(row.find('[data-testid="catalog-bundled"]').exists()).toBe(true);
    await row.get('[data-testid="catalog-choose"]').trigger('click');
    await settle();
    expect(requests).toContainEqual({ op: 'installChosen', ids: ['peek'] });
    expect(requests.some((request) => request.op === 'catalogPrepare')).toBe(false);
  });

  it('takes a chosen bundled plugin out with Remove', async () => {
    const { wrapper, requests } = await openWithBundle(true);
    await wrapper.get('[data-testid="catalog-row"][data-plugin="peek"] [data-testid="catalog-remove"]').trigger('click');
    await settle();
    expect(requests).toContainEqual({ op: 'removeChosen', ids: ['peek'] });
  });
});
