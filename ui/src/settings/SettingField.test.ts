import { afterEach, describe, expect, it, vi } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import SettingField from './SettingField.vue';
import type { JsonValue } from '../ipc/protocol';
import type { SettingDef } from './schema';
import { realDefs } from './test/fakeCore';
import { mountPage } from './test/mountPage';

let current: VueWrapper | null = null;

afterEach(() => {
  current?.unmount();
  current = null;
});

function def(key: string, lockedKey?: string): SettingDef {
  const found = realDefs().find((candidate) => candidate.key === key);
  if (found === undefined) {
    throw new Error(`${key} is no key of the schemas`);
  }
  return lockedKey === undefined ? found : { ...found, lockedKey };
}

async function field(setting: SettingDef, value: JsonValue, origin = 'default', showKeys?: boolean) {
  const { wrapper } = await mountPage(SettingField, {
    def: setting,
    value,
    origin,
    issues: [],
    hasProject: false,
    ...(showKeys === undefined ? {} : { showKeys }),
  });
  current = wrapper;
  return wrapper;
}

describe('a setting field', () => {
  // RB-090-02: General > Updates without self-update in the build.
  it('shows a locked update key with the hint and takes no change or reset', async () => {
    const wrapper = await field(def('updates.enabled', 'settings.updates.inactive'), true, 'user');
    expect(wrapper.get('[data-testid="locked"]').text()).toBe(
      'Updates are inactive until rBrowser moves to GitHub (version 1.1). Until then, update by unpacking the new ZIP.',
    );
    expect(wrapper.get('[data-testid="reset"]').attributes('disabled')).toBeDefined();
    const control = wrapper.get('#setting-updates-enabled');
    expect(control.attributes('disabled') ?? control.attributes('aria-disabled')).toBeDefined();
    await control.trigger('click');
    expect(wrapper.emitted('set')).toBeUndefined();
    expect(wrapper.emitted('reset')).toBeUndefined();
  });

  it('labels the update keys from the catalogue', async () => {
    const wrapper = await field(def('updates.check', 'settings.updates.inactive'), 'off');
    expect(wrapper.text()).toContain('Check for updates');
    expect(wrapper.find('[data-testid="locked"]').exists()).toBe(true);
  });

  it('leaves the old-engine notice of the same section free', async () => {
    const wrapper = await field(def('updates.cefAgeWarning'), true);
    expect(wrapper.find('[data-testid="locked"]').exists()).toBe(false);
  });

  // RB-080-70: the key on request - the title's tooltip and a copy button, not under every title.
  it('keeps the key out of the page, in the title tooltip and behind a copy button', async () => {
    const wrapper = await field(def('updates.check'), 'off');
    expect(wrapper.find('[data-testid="key"]').exists()).toBe(false);
    expect(wrapper.text()).not.toContain('updates.check');
    expect(wrapper.get('label').attributes('title')).toBe('updates.check');
    const copy = wrapper.get('[data-testid="copy-key"]');
    expect(copy.attributes('aria-label')).toBe('Copy the key updates.check');
    expect(copy.attributes('title')).toBe('Copy the key updates.check');
  });

  it('copies the key to the clipboard and says so', async () => {
    const writeText = vi.fn(() => Promise.resolve());
    vi.stubGlobal('navigator', { clipboard: { writeText } });
    try {
      const wrapper = await field(def('updates.check'), 'off');
      await wrapper.get('[data-testid="copy-key"]').trigger('click');
      await Promise.resolve();
      await Promise.resolve();
      expect(writeText).toHaveBeenCalledWith('updates.check');
      expect(wrapper.get('[data-testid="copy-key"]').attributes('aria-label')).toBe('Key copied');
    } finally {
      vi.unstubAllGlobals();
    }
  });

  // RB-090-23 (WCAG 1.3.1): the description under the title is the control's description.
  it.each([
    ['updates.enabled', true, '#setting-updates-enabled'],
    ['updates.check', 'off', '#setting-updates-check'],
    ['sync.hoverThrottleMs', 50, '#setting-sync-hoverThrottleMs'],
    ['sitemap.waitSelector', '', '#setting-sitemap-waitSelector'],
    ['sitemap.include', [], '[role="group"]'],
  ] as const)('describes %s by its description', async (key, value, control) => {
    const setting = def(key);
    const wrapper = await field(setting, value as JsonValue);
    const described = wrapper.get(control).attributes('aria-describedby');
    expect(described).toBe(`setting-${key.replace(/\./g, '-')}-description`);
    const description = wrapper.get(`#${described}`);
    expect(description.text()).not.toBe('');
    expect(wrapper.findAll('[aria-describedby]')).toHaveLength(1);
  });

  it('prints the key under the title when settings.showKeys is on', async () => {
    const wrapper = await field(def('updates.check'), 'off', 'default', true);
    expect(wrapper.get('[data-testid="key"]').text()).toBe('updates.check');
  });
});
