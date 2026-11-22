// RB-094-23: a key that applies only at the next start (`x-restart`) says so under its field;
// once its change was saved here (`restartPending`, `restartOffers.ts`), "Restart now" asks for
// the restart. Every other key applies at once and says nothing.
import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import SettingField from './SettingField.vue';
import type { SettingDef } from './schema';
import { realDefs } from './test/fakeCore';
import { mountPage, settle } from './test/mountPage';

let current: VueWrapper | null = null;

afterEach(() => {
  current?.unmount();
  current = null;
});

function def(key: string, restart: boolean): SettingDef {
  const found = realDefs().find((candidate) => candidate.key === key);
  if (found === undefined) {
    throw new Error(`${key} is no key of the schemas`);
  }
  const schema = { ...found.schema };
  if (restart) {
    schema['x-restart'] = true;
  } else {
    delete schema['x-restart'];
  }
  return { ...found, schema };
}

async function field(setting: SettingDef, restartPending = false) {
  const { wrapper } = await mountPage(SettingField, {
    def: setting,
    value: true,
    origin: 'default',
    issues: [],
    hasProject: false,
    restartPending,
  });
  current = wrapper;
  return wrapper;
}

describe('the restart notice', () => {
  it('says a restart key applies after a restart, and offers the restart once saved', async () => {
    const wrapper = await field(def('engines.chromium.sandbox', true));
    expect(wrapper.find('[data-testid="restart-notice"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="restart-now"]').exists()).toBe(false);
    await wrapper.get('#setting-engines-chromium-sandbox').trigger('click');
    await settle();
    expect(wrapper.emitted('set')).toBeDefined();
    expect(wrapper.find('[data-testid="restart-now"]').exists(), 'not before the write is saved').toBe(false);
    await wrapper.setProps({ restartPending: true });
    await wrapper.get('[data-testid="restart-now"]').trigger('click');
    expect(wrapper.emitted('restart')).toHaveLength(1);
  });

  it('says nothing for a key that applies at once', async () => {
    const wrapper = await field(def('ui.highlightActiveView', false));
    expect(wrapper.find('[data-testid="restart-notice"]').exists()).toBe(false);
  });
});
