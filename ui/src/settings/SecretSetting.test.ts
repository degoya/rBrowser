// RB-094-27: a key annotated `x-secret` (`api-client.secrets`) is no setting of the window - it
// has no option to show - and edited where its plugin edits it (the API client's Variables tab).
// The core leaves it out of what it sends; the page holds to it even if one came through: it is
// in no category, no search, not among the changed and not in the JSON view.
import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import SettingsPage from './SettingsPage.vue';
import { BENCH_REPEATS, createFakeCore, SECRETS } from './test/fakeCore';
import { mountPage, settle } from './test/mountPage';

const VALUE = { staging: { TOKEN: 'opensesame99' } };

let current: VueWrapper | null = null;

afterEach(() => {
  current?.unmount();
  current = null;
});

/** The page over a core that still sends the secret key, its value stored in config.local.json. */
async function open(routeKey: string | null = null) {
  const core = createFakeCore();
  core.addDefs([SECRETS, BENCH_REPEATS]);
  core.values.set(SECRETS.key, { key: SECRETS.key, value: VALUE, origin: 'local' });
  const { wrapper } = await mountPage(SettingsPage, { routeKey }, core);
  current = wrapper;
  return wrapper;
}

const keys = (wrapper: VueWrapper) => wrapper.findAll('[data-key]').map((field) => field.attributes('data-key'));

describe('a secret key', () => {
  it('is not in its category, while the section keeps its other keys', async () => {
    const wrapper = await open('api-client.benchRepeats');
    expect(wrapper.get('[aria-current="page"]').attributes('data-category')).toBe('network');
    expect(keys(wrapper)).toContain('api-client.benchRepeats');
    expect(keys(wrapper)).not.toContain('api-client.secrets');
    expect(wrapper.find('[data-testid="secret"]').exists()).toBe(false);
    expect(wrapper.text()).not.toContain('opensesame99');
  });

  it('is found by no search and is not among the changed', async () => {
    const wrapper = await open();
    await wrapper.get('[data-testid="search"]').setValue('secrets');
    await settle();
    expect(keys(wrapper)).not.toContain('api-client.secrets');
    await wrapper.get('[data-testid="search"]').setValue('');
    await wrapper.get('[data-testid="only-changed"]').trigger('click');
    await settle();
    expect(keys(wrapper)).not.toContain('api-client.secrets');
  });

  it('is not in the JSON view, in no scope', async () => {
    const wrapper = await open();
    await wrapper.get('[data-testid="json-toggle"]').trigger('click');
    await settle();
    for (const scope of ['effective', 'local']) {
      await wrapper.get(`[data-scope="${scope}"]`).trigger('click');
      await settle();
      const text = wrapper.get('[data-testid="json-text"]').text();
      expect(text).not.toContain('opensesame99');
      expect(text).not.toContain('secrets');
    }
  });

  it('opens nothing as a link: the page names it as missing', async () => {
    const wrapper = await open('api-client.secrets');
    expect(wrapper.get('[data-testid="missing-link"]').text()).toContain('api-client.secrets');
    expect(keys(wrapper)).not.toContain('api-client.secrets');
  });
});
