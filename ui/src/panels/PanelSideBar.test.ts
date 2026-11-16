import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { mountWithShell, settle } from '../test/mountShell';
import { sideEntries } from './core';
import PanelSideBar from './PanelSideBar.vue';
import type { PanelInfo } from './protocol';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

const FONTS: PanelInfo = {
  plugin: 'fonts',
  panel: 'fonts',
  title: 'Fonts',
  placement: 'sidebar',
  icon: 'type',
  url: 'rbrowser://plugin/fonts/panel/index.html',
};
const OTHER: PanelInfo = {
  plugin: 'third-party',
  panel: 'main',
  title: 'Third party',
  placement: 'sidebar',
  icon: 'https://example.com/x.svg',
  url: 'rbrowser://plugin/third-party/panel/index.html',
};
const BARE: PanelInfo = { ...OTHER, plugin: 'bare', icon: undefined, title: 'Bare' };

async function mounted(props: Record<string, unknown>) {
  const result = await mountWithShell(PanelSideBar, {
    mount: {
      props: { entries: sideEntries([FONTS, OTHER, BARE], (key) => `<${key}>`), selected: 'fonts.fonts', ...props },
    },
  });
  current = result.wrapper;
  await settle();
  return result.wrapper;
}

const iconOf = (wrapper: VueWrapper, key: string): string =>
  wrapper.get(`[data-testid="panel-tab-${key}"] [data-testid="panel-icon"]`).attributes('data-icon') ?? '';

describe('the panels side bar', () => {
  it('shows Findings first, then every panel with its icon and title, a foreign or missing icon as the fallback', async () => {
    const wrapper = await mounted({ iconsOnly: false });
    const keys = wrapper.findAll('[data-testid^="panel-tab-"]').map((tab) => tab.attributes('data-testid'));
    expect(keys).toEqual(['panel-tab-findings', 'panel-tab-videos', 'panel-tab-fonts.fonts', 'panel-tab-third-party.main', 'panel-tab-bare.main']);
    expect(iconOf(wrapper, 'findings')).toBe('i-lucide-list-checks');
    expect(iconOf(wrapper, 'videos')).toBe('i-lucide-film');
    expect(wrapper.get('[data-testid="panel-tab-findings"]').text()).toBe('<findings.title>');
    expect(iconOf(wrapper, 'fonts.fonts')).toBe('i-lucide-type');
    expect(iconOf(wrapper, 'third-party.main')).toBe('i-lucide-puzzle');
    expect(iconOf(wrapper, 'bare.main')).toBe('i-lucide-puzzle');
    const fonts = wrapper.get('[data-testid="panel-tab-fonts.fonts"]');
    expect(fonts.text()).toBe('Fonts');
    expect(fonts.attributes('aria-current')).toBe('page');
    // Only the current entry; marked by a line too, not by ground alone (docs/design.md, 12).
    expect(wrapper.findAll('[aria-current="page"]')).toHaveLength(1);
    expect(wrapper.findAll('[data-active]').map((tab) => tab.attributes('data-testid'))).toEqual([
      'panel-tab-fonts.fonts',
    ]);
    expect(fonts.classes()).toContain('data-active:after:bg-primary');
    expect(wrapper.get('[data-testid="panel-sidebar-toggle"]').attributes('aria-label')).toBe('Collapse side bar');
  });

  it('icons only: the title is the tooltip and the button name', async () => {
    const wrapper = await mounted({ iconsOnly: true });
    const fonts = wrapper.get('[data-testid="panel-tab-fonts.fonts"]');
    expect(fonts.text()).toBe('');
    expect(fonts.attributes('title')).toBe('Fonts');
    expect(fonts.attributes('aria-label')).toBe('Fonts');
    expect(wrapper.get('[data-testid="panel-sidebar-toggle"]').attributes('aria-label')).toBe('Expand side bar');
  });

  it('the button at the bottom asks for the other mode', async () => {
    const wrapper = await mounted({ iconsOnly: false });
    await wrapper.get('[data-testid="panel-sidebar-toggle"]').trigger('click');
    const icons = await mounted({ iconsOnly: true });
    await icons.get('[data-testid="panel-sidebar-toggle"]').trigger('click');
    expect(wrapper.emitted('sidebar')).toEqual([['icons']]);
    expect(icons.emitted('sidebar')).toEqual([['expanded']]);
  });

  it('a click on a panel opens it', async () => {
    const wrapper = await mounted({ iconsOnly: false });
    await wrapper.get('[data-testid="panel-tab-bare.main"]').trigger('click');
    expect(wrapper.emitted('open')).toEqual([['bare.main']]);
  });

  it('folded beside the views: an expand button on top, icons only, no mode button', async () => {
    const wrapper = await mounted({ iconsOnly: true, strip: 'right' });
    const expand = wrapper.get('[data-testid="panel-strip-expand"]');
    expect(expand.attributes('aria-label')).toBe('Expand the panels');
    expect(wrapper.get('[data-testid="panel-strip-expand"] [data-testid="panel-icon"]').attributes('data-icon')).toBe(
      'i-lucide-panel-right-open',
    );
    expect(wrapper.find('[data-testid="panel-sidebar-toggle"]').exists()).toBe(false);
    await expand.trigger('click');
    expect(wrapper.emitted('expand')).toEqual([[]]);
  });

  it('without room for titles the mode button goes', async () => {
    const wrapper = await mounted({ iconsOnly: true, narrow: true });
    expect(wrapper.find('[data-testid="panel-sidebar-toggle"]').exists()).toBe(false);
  });

  it('is a Nuxt UI navigation menu, vertical beside the views', async () => {
    const wrapper = await mounted({ iconsOnly: false });
    const menu = wrapper.get('[data-testid="panel-sidebar-menu"]');
    expect(menu.attributes('data-orientation')).toBe('vertical');
  });

  it('marks the entry a command opened while folded with a chip, and names it', async () => {
    const wrapper = await mounted({ iconsOnly: true, strip: 'left', attention: 'fonts.fonts' });
    const fonts = wrapper.get('[data-testid="panel-tab-fonts.fonts"]');
    // A Nuxt UI chip on the icon, on that entry only.
    expect(fonts.find('[data-slot="base"]').exists()).toBe(true);
    expect(wrapper.findAll('[data-slot="link"] [data-slot="base"]')).toHaveLength(1);
    expect(fonts.attributes('aria-label')).toBe('Fonts, opened while the panels were collapsed');
  });

  it('folded below the views: the rail is a row of icons after the expand button', async () => {
    const wrapper = await mounted({ iconsOnly: false, strip: 'bottom' });
    expect(wrapper.get('[data-testid="panel-sidebar-menu"]').attributes('data-orientation')).toBe('horizontal');
    const fonts = wrapper.get('[data-testid="panel-tab-fonts.fonts"]');
    expect(fonts.text()).toBe('');
    expect(fonts.attributes('title')).toBe('Fonts');
    expect(wrapper.get('[data-testid="panel-strip-expand"] [data-testid="panel-icon"]').attributes('data-icon')).toBe(
      'i-lucide-panel-bottom-open',
    );
    await fonts.trigger('click');
    expect(wrapper.emitted('open')).toEqual([['fonts.fonts']]);
  });
});
