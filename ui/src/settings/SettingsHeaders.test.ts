import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import SettingsPage from './SettingsPage.vue';
import { mountPage, settle } from './test/mountPage';

let current: VueWrapper | null = null;

afterEach(() => {
  current?.unmount();
  current = null;
});

// Every section opens the same way (RB-080-13, after rDownloader's `SettingsHeaders.test.ts`):
// one `SectionHeader` per card, one page header per page, in every category and every tab.
describe('the section headers of the settings', () => {
  it('gives every section card exactly one header and every page exactly one page header', async () => {
    const { wrapper } = await mountPage(SettingsPage, { routeKey: null });
    current = wrapper;
    let cards = 0;
    for (const button of wrapper.findAll('[data-category]')) {
      await button.trigger('click');
      await settle();
      const tabs = wrapper.findAll('[data-testid="settings-tabs"] button[role="tab"]');
      for (const tab of tabs.length > 0 ? tabs : [null]) {
        await tab?.trigger('mousedown', { button: 0 });
        await settle();
        const main = wrapper.get('[data-testid="settings-main"]');
        expect(main.findAll('[data-testid="section-header"][data-level="page"]')).toHaveLength(1);
        for (const section of main.findAll('[data-section]')) {
          const name = section.attributes('data-section');
          expect(section.findAll('[data-testid="section-header"]'), name).toHaveLength(1);
          expect(section.findAll('h2'), name).toHaveLength(1);
          cards += 1;
        }
      }
    }
    expect(cards).toBeGreaterThan(40);
  });
});
