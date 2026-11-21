import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { sampleState } from '../ipc/mockState';
import type { CoreToShell, SitemapStatus } from '../ipc/protocol';
import { mountWithShell, settle } from '../test/mountShell';
import SitemapBar from './SitemapBar.vue';
import SitemapIndicator from './SitemapIndicator.vue';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

const run: SitemapStatus = {
  runId: 'r1',
  state: 'running',
  total: 76,
  done: 38,
  failed: 1,
  etaSeconds: 40,
  folder: 'C:\\shop\\.rbrowser\\out\\sitemap-capture\\r1',
};

async function bar(sitemap?: SitemapStatus) {
  let push: (message: CoreToShell) => void = () => undefined;
  const subscribe = (listener: (message: CoreToShell) => void) => {
    push = listener;
    return () => undefined;
  };
  const state = sitemap === undefined ? sampleState() : { ...sampleState(), sitemap };
  const result = await mountWithShell(SitemapBar, {
    mock: { state },
    mount: { props: { subscribe } },
  });
  current = result.wrapper;
  await settle();
  push({ type: 'uiCommand', id: 'panels.sitemap' });
  await settle();
  return result;
}

describe('the sitemap bar', () => {
  it('opens on panels.sitemap only and closes with Escape', async () => {
    const { wrapper } = await bar();
    expect(wrapper.get('[data-testid="sitemap-line"]').text()).toContain('No sitemap capture yet');
    await wrapper.get('[data-testid="sitemap-bar"]').trigger('keydown', { key: 'Escape' });
    await settle();
    expect(wrapper.find('[data-testid="sitemap-bar"]').exists()).toBe(false);
  });

  it('starts a capture with the defaults', async () => {
    const { wrapper, mock } = await bar();
    await wrapper.get('[data-testid="sitemap-start"]').trigger('click');
    await settle();
    expect(mock.sent).toContainEqual({ type: 'runCommand', id: 'sitemap.capture', args: {} });
  });

  it('shows a running capture with the time left and cancels it', async () => {
    const { wrapper, mock } = await bar(run);
    expect(wrapper.get('[data-testid="sitemap-line"]').text()).toBe(
      '38 of 76 shots, 1 failed, about 40 s left',
    );
    expect(wrapper.find('[data-testid="sitemap-start"]').exists()).toBe(false);
    await wrapper.get('[data-testid="sitemap-cancel"]').trigger('click');
    await settle();
    expect(mock.sent).toContainEqual({
      type: 'runCommand',
      id: 'sitemap.cancel',
      args: { runId: 'r1' },
    });
  });

  it('opens the gallery of a running capture before its index.html exists (RB-090-88)', async () => {
    // The app's gallery window reads run.json; the core tells the person when the system
    // browser's HTML export comes only at the end.
    const { wrapper, mock } = await bar(run);
    await wrapper.get('[data-testid="sitemap-gallery"]').trigger('click');
    await settle();
    expect(mock.sent).toContainEqual({ type: 'runCommand', id: 'sitemap.gallery', args: { runId: 'r1' } });
  });

  it('offers no gallery before any capture', async () => {
    const { wrapper } = await bar();
    expect(wrapper.find('[data-testid="sitemap-gallery"]').exists()).toBe(false);
  });

  it('opens by itself when a capture starts, without taking the focus (RB-090-83)', async () => {
    const result = await mountWithShell(SitemapBar, { mount: { props: { subscribe: () => () => undefined } } });
    current = result.wrapper;
    await settle();
    expect(result.wrapper.find('[data-testid="sitemap-bar"]').exists()).toBe(false);
    const focused = document.activeElement;
    result.mock.emit({ type: 'state', ...sampleState(), sitemap: { ...run, done: 0, failed: 0 } });
    await settle();
    expect(result.wrapper.find('[data-testid="sitemap-bar"]').exists()).toBe(true);
    expect(result.wrapper.get('[data-testid="sitemap-line"]').text()).toContain('0 of 76 shots');
    expect(document.activeElement).toBe(focused);
    // Closed, it stays closed while the same run goes on.
    await result.wrapper.get('[data-testid="sitemap-close"]').trigger('click');
    result.mock.emit({ type: 'state', ...sampleState(), sitemap: run });
    await settle();
    expect(result.wrapper.find('[data-testid="sitemap-bar"]').exists()).toBe(false);
  });

  it('resumes a cancelled capture and opens its gallery through sitemap.gallery', async () => {
    const cancelled: SitemapStatus = {
      ...run,
      state: 'cancelled',
      etaSeconds: undefined,
      gallery: 'C:\\shop\\.rbrowser\\out\\sitemap-capture\\r1\\index.html',
    };
    const { wrapper, mock } = await bar(cancelled);
    await wrapper.get('[data-testid="sitemap-resume"]').trigger('click');
    await wrapper.get('[data-testid="sitemap-gallery"]').trigger('click');
    await settle();
    expect(mock.sent).toContainEqual({
      type: 'runCommand',
      id: 'sitemap.capture',
      args: { resume: 'r1' },
    });
    expect(mock.sent).toContainEqual({ type: 'runCommand', id: 'sitemap.gallery', args: { runId: 'r1' } });
    expect(mock.sent.some((message) => message.type === 'runCommand' && message.id === 'view.navigate')).toBe(
      false,
    );
  });
});

describe('the sitemap indicator', () => {
  it('shows only while a capture runs and opens the bar', async () => {
    const idle = await mountWithShell(SitemapIndicator);
    current = idle.wrapper;
    await settle();
    expect(idle.wrapper.find('[data-testid="sitemap-indicator"]').exists()).toBe(false);
    idle.wrapper.unmount();
    const state = { ...sampleState(), sitemap: run };
    const { wrapper, mock } = await mountWithShell(SitemapIndicator, { mock: { state } });
    current = wrapper;
    await settle();
    const button = wrapper.get('[data-testid="sitemap-indicator"]');
    expect(button.text()).toBe('Sitemap 38/76');
    // Label in name (RB-090-23, WCAG 2.5.3): the visible text is the name, the title describes.
    expect(button.attributes('aria-label')).toBeUndefined();
    expect(button.attributes('title')).toBe(
      'Sitemap capture: 38 of 76 shots, 1 failed, about 40 s left',
    );
    await button.trigger('click');
    await settle();
    expect(mock.sent).toContainEqual({ type: 'runCommand', id: 'panels.sitemap' });
  });

  it('is drawn with Nuxt UI buttons, the first one focused (RB-080-88)', async () => {
    const { wrapper } = await bar();
    const start = wrapper.get('[data-testid="sitemap-start"]');
    expect(start.attributes('data-slot')).toBe('base');
    expect(document.activeElement).toBe(start.element);
    expect(wrapper.get('[data-testid="sitemap-close"]').attributes('data-slot')).toBe('base');
  });
});
