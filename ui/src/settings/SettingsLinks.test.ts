// RB-094-26: a settings link lands on its key, a plugin's key included, also when the window is
// open already and when the plugins' keys come after it opened; a link that opens nothing names
// its key at the top - never silently another page.
import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import SettingsPage from './SettingsPage.vue';
import { BENCH_REPEATS, createFakeCore } from './test/fakeCore';
import { mountPage, settle } from './test/mountPage';

let current: VueWrapper | null = null;

async function open(props: Record<string, unknown> = {}, core = createFakeCore()) {
  const mounted = await mountPage(SettingsPage, { routeKey: null, ...props }, core);
  current = mounted.wrapper;
  return mounted;
}

afterEach(() => {
  current?.unmount();
  current = null;
});

const PLUGINS_CHANGED = { type: 'pluginPanel', message: { pluginsChanged: true } } as const;

describe('a settings link', () => {
  it('opens a plugin key in its category and section with the field highlighted', async () => {
    const core = createFakeCore();
    core.addDefs([BENCH_REPEATS]);
    const { wrapper } = await open({ routeKey: 'api-client.benchRepeats' }, core);
    expect(wrapper.get('[aria-current="page"]').attributes('data-category')).toBe('network');
    expect(wrapper.get('[data-key="api-client.benchRepeats"]').attributes('data-highlighted')).toBe('true');
    expect(wrapper.find('[data-testid="missing-link"]').exists()).toBe(false);
  });

  it('lands on a plugin key that came after the window opened', async () => {
    const core = createFakeCore();
    const { wrapper } = await open({ routeKey: 'sync.scroll' }, core);
    // The plugins load while the window is open: the core says so, the page reads its schema again.
    core.addDefs([BENCH_REPEATS]);
    core.emit(PLUGINS_CHANGED);
    await settle();
    // The schema read again does not move the page from where it is.
    expect(wrapper.get('[aria-current="page"]').attributes('data-category')).toBe('sync');
    // A link from the API client afterwards lands on its key.
    await wrapper.setProps({ routeKey: 'api-client.benchRepeats' });
    await settle();
    expect(wrapper.get('[aria-current="page"]').attributes('data-category')).toBe('network');
    expect(wrapper.get('[data-key="api-client.benchRepeats"]').attributes('data-highlighted')).toBe('true');
  });

  it('opens a key the schema holds only once it is read again, saying so meanwhile', async () => {
    const core = createFakeCore();
    const { wrapper } = await open({ routeKey: 'api-client.benchRepeats' }, core);
    expect(wrapper.get('[data-testid="missing-link"]').text()).toContain('api-client.benchRepeats');
    expect(wrapper.find('[data-key="api-client.benchRepeats"]').exists()).toBe(false);
    core.addDefs([BENCH_REPEATS]);
    core.emit(PLUGINS_CHANGED);
    await settle();
    expect(wrapper.get('[data-key="api-client.benchRepeats"]').attributes('data-highlighted')).toBe('true');
    expect(wrapper.find('[data-testid="missing-link"]').exists()).toBe(false);
  });

  it('waits for the plugins with the key named and asks for it again once they are in', async () => {
    const core = createFakeCore();
    const { wrapper } = await open({ routeMissing: { key: 'api-client.benchRepeats', waiting: true } }, core);
    const notice = wrapper.get('[data-testid="missing-link"]');
    expect(notice.attributes('role')).toBe('status');
    expect(notice.text()).toContain('api-client.benchRepeats');
    expect(notice.text()).toContain('plugins');
    // The installed list says the plugins are loaded: the page asks the core again for the link.
    core.addDefs([BENCH_REPEATS]);
    core.emit(PLUGINS_CHANGED);
    await settle();
    expect(core.sent).toContainEqual({ type: 'openSettings', key: 'api-client.benchRepeats' });
    // The core opens it at its key now.
    await wrapper.setProps({ routeMissing: null, routeKey: 'api-client.benchRepeats' });
    await settle();
    expect(wrapper.get('[data-key="api-client.benchRepeats"]').attributes('data-highlighted')).toBe('true');
    expect(wrapper.find('[data-testid="missing-link"]').exists()).toBe(false);
  });

  it('names a key that is not found and closes on request', async () => {
    const core = createFakeCore();
    const { wrapper } = await open({ routeMissing: { key: 'api-client.noSuch', waiting: false } }, core);
    const notice = wrapper.get('[data-testid="missing-link"]');
    expect(notice.attributes('role')).toBe('alert');
    expect(notice.text()).toContain('api-client.noSuch');
    // Nothing is asked again for a key the core did not find.
    core.emit(PLUGINS_CHANGED);
    await settle();
    expect(core.sent.some((message) => message.type === 'openSettings')).toBe(false);
    await wrapper.get('[data-testid="missing-link-close"]').trigger('click');
    await settle();
    expect(wrapper.find('[data-testid="missing-link"]').exists()).toBe(false);
  });

  it('says a link named no setting when it was no key at all', async () => {
    const { wrapper } = await open({ routeMissing: { key: '', waiting: false } });
    expect(wrapper.get('[data-testid="missing-link"]').text()).toContain('names no setting');
  });

  it('drops the notice once another place opens', async () => {
    const { wrapper } = await open({ routeMissing: { key: 'no.such', waiting: false } });
    expect(wrapper.find('[data-testid="missing-link"]').exists()).toBe(true);
    await wrapper.setProps({ routeMissing: null, routeKey: 'sync.scroll' });
    await settle();
    expect(wrapper.find('[data-testid="missing-link"]').exists()).toBe(false);
    expect(wrapper.get('[data-key="sync.scroll"]').attributes('data-highlighted')).toBe('true');
  });
});
