import { afterEach, describe, expect, it, vi } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { IpcClient } from '../../ipc/client';
import { IpcErrorCode, type ShellToCore, type SitemapGalleryPage as Run } from '../../ipc/protocol';
import { IpcError } from '../../ipc/transport';
import { mountWithShell, settle } from '../../test/mountShell';
import { galleryRun, RUN_ID } from './fixture';
import SitemapGalleryPage from './SitemapGalleryPage.vue';

let current: VueWrapper | null = null;

afterEach(() => {
  current?.unmount();
  current = null;
  document.body.replaceChildren();
});

/** The page over a client whose `getSitemapGallery` answers `run` (or fails with `failure`). */
async function mounted(run: Run = galleryRun(), failure?: IpcError) {
  const asked: ShellToCore[] = [];
  const client = new IpcClient({
    send: async (message) => {
      asked.push(message);
      if (message.type === 'getSitemapGallery') {
        if (failure !== undefined) throw failure;
        return { type: 'sitemapGallery', ...run };
      }
      return { type: 'ack' };
    },
    subscribe: () => () => undefined,
  });
  const result = await mountWithShell(SitemapGalleryPage, {
    mount: { props: { client, hash: `#/sitemap-gallery?run=${RUN_ID}` } },
  });
  current = result.wrapper;
  await settle();
  return { wrapper: result.wrapper, asked };
}

const rows = (wrapper: VueWrapper): string[] => wrapper.findAll('[data-testid^="row-"]').map((row) => row.attributes('data-testid') ?? '');

async function choose(wrapper: VueWrapper, group: string, value: string): Promise<void> {
  await wrapper.get(`[data-testid="${group}"] [role="radio"][value="${value}"]`).trigger('click');
  await settle();
}

function key(name: string): void {
  window.dispatchEvent(new KeyboardEvent('keydown', { key: name }));
}

const large = (): string | null => document.body.querySelector('[data-testid="shot-large"]')?.getAttribute('src') ?? null;

describe('the sitemap gallery page', () => {
  it('asks the core for the run its route names and shows every page and breakpoint', async () => {
    const { wrapper, asked } = await mounted();
    expect(asked).toContainEqual({ type: 'getSitemapGallery', runId: RUN_ID });
    expect(rows(wrapper)).toEqual(['row-0', 'row-1', 'row-2']);
    const image = wrapper.get('[data-testid="shot-0-mobile"] img');
    expect(image.attributes('src')).toBe(`rbrowser://sitemap/${RUN_ID}/index/mobile.png`);
    expect(image.attributes('style')).toContain('width: 320px');
    expect(wrapper.get('[data-testid="column-desktop"]').text()).toContain('desktop');
    expect(wrapper.get('[data-testid="cell-1-desktop"]').text()).toContain('Not captured yet');
    expect(wrapper.get('[data-testid="cell-2-mobile"]').text()).toContain('Failed: timeout');
    expect(wrapper.get('[data-testid="cell-0-desktop"]').text()).toContain('differs 1.23 %');
    expect(wrapper.get('[data-testid="gallery-state"]').text()).toBe('Finished');
  });

  it('takes the thumbnail width the core names', async () => {
    const { wrapper } = await mounted({ ...galleryRun(), thumbnailWidth: 640 });
    expect(wrapper.get('[data-testid="shot-0-mobile"] img').attributes('style')).toContain('width: 640px');
  });

  it('filters by breakpoint and by status', async () => {
    const { wrapper } = await mounted();
    await choose(wrapper, 'filter-breakpoint', 'desktop');
    expect(wrapper.find('[data-testid="cell-0-mobile"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="cell-0-desktop"]').exists()).toBe(true);
    await choose(wrapper, 'filter-status', 'problems');
    expect(rows(wrapper)).toEqual(['row-1', 'row-2']);
    await choose(wrapper, 'filter-status', 'ok');
    expect(rows(wrapper)).toEqual(['row-0']);
    await choose(wrapper, 'filter-breakpoint', 'all');
    await choose(wrapper, 'filter-status', 'all');
    expect(rows(wrapper)).toEqual(['row-0', 'row-1', 'row-2']);
  });

  it('lists the 404, the redirect and the failed shot as errors', async () => {
    const { wrapper } = await mounted();
    const errors = wrapper
      .findAll('[data-testid="gallery-error"]')
      .map((entry) => [entry.get('[data-testid="gallery-error-kind"]').text(), entry.get('[data-external-link]').text()]);
    expect(errors).toEqual([
      ['HTTP 404', 'http://localhost:3000/missing'],
      ['Redirect to http://localhost:3000/new', 'http://localhost:3000/old'],
      ['Failed: mobile: timeout', 'http://localhost:3000/old'],
    ]);
  });

  it('opens a thumbnail large and steps with the buttons and the arrow keys', async () => {
    const { wrapper } = await mounted();
    expect(large()).toBeNull();
    await wrapper.get('[data-testid="shot-0-mobile"]').trigger('click');
    await settle();
    expect(large()).toBe(`rbrowser://sitemap/${RUN_ID}/index/mobile.png`);
    key('ArrowRight');
    await settle();
    expect(large()).toBe(`rbrowser://sitemap/${RUN_ID}/index/desktop.png`);
    key('ArrowRight');
    await settle();
    expect(large()).toBe(`rbrowser://sitemap/${RUN_ID}/missing/mobile.png`);
    key('ArrowLeft');
    await settle();
    expect(large()).toBe(`rbrowser://sitemap/${RUN_ID}/index/desktop.png`);
    (document.body.querySelector('[data-testid="shot-next"]') as HTMLElement).click();
    await settle();
    expect(large()).toBe(`rbrowser://sitemap/${RUN_ID}/missing/mobile.png`);
    (document.body.querySelector('[data-testid="shot-close"]') as HTMLElement).click();
    await settle();
    expect(large()).toBeNull();
  });

  it('stops at the first and the last shot', async () => {
    const { wrapper } = await mounted();
    await wrapper.get('[data-testid="shot-2-desktop"]').trigger('click');
    await settle();
    key('ArrowRight');
    await settle();
    expect(large()).toBe(`rbrowser://sitemap/${RUN_ID}/old/desktop.png`);
    expect((document.body.querySelector('[data-testid="shot-next"]') as HTMLButtonElement).disabled).toBe(true);
  });

  it('reads the run again when the window shows another one', async () => {
    const asked: ShellToCore[] = [];
    const client = new IpcClient({
      send: async (message) => {
        asked.push(message);
        return message.type === 'getSitemapGallery' ? { type: 'sitemapGallery', ...galleryRun() } : { type: 'ack' };
      },
      subscribe: () => () => undefined,
    });
    window.location.hash = `#/sitemap-gallery?run=${RUN_ID}`;
    const result = await mountWithShell(SitemapGalleryPage, { mount: { props: { client } } });
    current = result.wrapper;
    await settle();
    window.location.hash = '#/sitemap-gallery?run=2026-11-12_10-00-00';
    window.dispatchEvent(new HashChangeEvent('hashchange'));
    await settle();
    expect(asked.filter((message) => message.type === 'getSitemapGallery')).toEqual([
      { type: 'getSitemapGallery', runId: RUN_ID },
      { type: 'getSitemapGallery', runId: '2026-11-12_10-00-00' },
    ]);
    expect(rows(result.wrapper)).toHaveLength(3);
  });

  it('reads a running capture again until it ends, without losing the page', async () => {
    let state = 'running';
    let reads = 0;
    const client = new IpcClient({
      send: async (message) => {
        if (message.type !== 'getSitemapGallery') return { type: 'ack' };
        reads += 1;
        return { type: 'sitemapGallery', ...galleryRun(), state };
      },
      subscribe: () => () => undefined,
    });
    const result = await mountWithShell(SitemapGalleryPage, {
      mount: { props: { client, hash: `#/sitemap-gallery?run=${RUN_ID}`, refreshMs: 10 } },
    });
    current = result.wrapper;
    await vi.waitFor(() => expect(reads).toBeGreaterThanOrEqual(3));
    expect(result.wrapper.find('[data-testid="gallery-loading"]').exists()).toBe(false);
    state = 'finished';
    await vi.waitFor(() => expect(result.wrapper.get('[data-testid="gallery-state"]').text()).toBe('Finished'));
    const done = reads;
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(reads).toBe(done);
  });

  it('says when it shows only the first pages of a long run', async () => {
    const { wrapper } = await mounted({ ...galleryRun(), totalPages: 2500 });
    expect(wrapper.get('[data-testid="gallery-truncated"]').text()).toContain('first 3 of 2500 pages');
  });

  it('says when the run is gone or cannot be read', async () => {
    const gone = await mounted(galleryRun(), new IpcError(IpcErrorCode.NotFound, 'sitemap.error.not_found: no run'));
    expect(gone.wrapper.get('[data-testid="gallery-failure"]').text()).toContain('was not found');
    current?.unmount();
    const broken = await mounted(galleryRun(), new IpcError(IpcErrorCode.Internal, 'sitemap.error.failed: x'));
    expect(broken.wrapper.get('[data-testid="gallery-failure"]').text()).toContain('could not be read');
  });
});
