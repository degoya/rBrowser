import { describe, expect, it, vi } from 'vitest';
import { effectScope } from 'vue';
import { parseRoute, routeHash, useHashRoute } from './route';

describe('the pages router', () => {
  it('reads the page and the key of a deep link', () => {
    expect(parseRoute('#/settings/sync.scroll')).toEqual({ page: 'settings', key: 'sync.scroll', tab: null });
    expect(parseRoute('#/settings')).toEqual({ page: 'settings', key: null, tab: null });
    expect(parseRoute('#/trust')).toEqual({ page: 'trust', key: null, tab: null });
    expect(parseRoute('')).toEqual({ page: 'settings', key: null, tab: null });
    expect(routeHash({ page: 'settings', key: 'layout', tab: null })).toBe('#/settings/layout');
  });

  it('reads and writes the sub-tab of a category (RB-080-13)', () => {
    expect(parseRoute('#/settings/layout?tab=grid')).toEqual({ page: 'settings', key: 'layout', tab: 'grid' });
    expect(parseRoute('#/settings/layout?tab=focus-mode')).toEqual({
      page: 'settings',
      key: 'layout',
      tab: 'focus-mode',
    });
    expect(routeHash({ page: 'settings', key: 'layout', tab: 'grid' })).toBe('#/settings/layout?tab=grid');
    // A tab that is no section id is dropped; the key stays.
    expect(parseRoute('#/settings/layout?tab=<b>')).toEqual({ page: 'settings', key: 'layout', tab: null });
    expect(parseRoute('#/settings/layout?other=1')).toEqual({ page: 'settings', key: 'layout', tab: null });
    expect(parseRoute('#/settings?tab=grid')).toEqual({ page: 'settings', key: null, tab: null });
  });

  it('reads the section an area link opens and highlights (RB-090-85)', () => {
    expect(parseRoute('#/settings/capture?section=sitemap')).toEqual({
      page: 'settings',
      key: 'capture',
      tab: null,
      section: 'sitemap',
    });
    expect(parseRoute('#/settings/capture?section=sitemap').section).toBe('sitemap');
    // A section that is no section id is dropped, and so is one without a key.
    expect(parseRoute('#/settings/capture?section=<b>').section).toBeUndefined();
    expect(parseRoute('#/settings?section=sitemap').section).toBeUndefined();
    expect(routeHash({ page: 'settings', key: 'capture', tab: null, section: 'sitemap' })).toBe(
      '#/settings/capture?section=sitemap',
    );
  });

  it('reads the link that opened nothing, named or not (RB-094-26)', () => {
    expect(parseRoute('#/settings?missing=api-client.noSuch')).toEqual({
      page: 'settings',
      key: null,
      tab: null,
      missing: { key: 'api-client.noSuch', waiting: false },
    });
    expect(parseRoute('#/settings?pending=api-client.benchRepeats').missing).toEqual({
      key: 'api-client.benchRepeats',
      waiting: true,
    });
    // The core names no link that is no key; the page then says the link named none.
    expect(parseRoute('#/settings?missing=').missing).toEqual({ key: '', waiting: false });
    // What is not a key's characters is never shown, nor beside a key that opened.
    expect(parseRoute('#/settings?missing=<b>x</b>').missing).toBeUndefined();
    expect(parseRoute('#/settings/sync.scroll?missing=a').missing).toBeUndefined();
    expect(routeHash({ page: 'settings', key: null, tab: null, missing: { key: 'no.such', waiting: false } })).toBe(
      '#/settings?missing=no.such',
    );
    expect(routeHash({ page: 'settings', key: null, tab: null, missing: { key: 'a.b', waiting: true } })).toBe(
      '#/settings?pending=a.b',
    );
  });

  it('drops anything that is no key', () => {
    expect(parseRoute('#/settings/<img onerror=x>')).toEqual({ page: 'settings', key: null, tab: null });
    expect(parseRoute('#/settings/a/b')).toEqual({ page: 'settings', key: null, tab: null });
    expect(parseRoute('#/../../etc')).toEqual({ page: 'settings', key: null, tab: null });
  });

  it('follows the hash', () => {
    window.location.hash = '#/settings/sync.scroll';
    const scope = effectScope();
    const route = scope.run(() => useHashRoute(window))!;
    expect(route.value.key).toBe('sync.scroll');
    window.location.hash = '#/settings/layout';
    window.dispatchEvent(new HashChangeEvent('hashchange'));
    expect(route.value.key).toBe('layout');
    scope.stop();
  });
});

describe('the pages of the page windows', () => {
  it('has a page for the palette popup; the findings live in the panels page (RB-080-69)', async () => {
    const { PAGES, pageOf } = await import('./routes');
    const { default: PalettePage } = await import('../palette/PalettePage.vue');
    expect(pageOf('palette')).toBe(pageOf('palette'));
    expect(pageOf('palette')).not.toBe(pageOf('settings'));
    expect((await PAGES.palette!()).default).toBe(PalettePage);
    expect(PAGES.findings).toBeUndefined();
    expect(pageOf('findings')).toBe(pageOf('settings'));
  });

  it('has the setup wizard, opened at its project step by the hint (RB-091-05)', async () => {
    const { PAGES } = await import('./routes');
    const { default: SetupWizard } = await import('../firstrun/SetupWizard.vue');
    expect((await PAGES.setup!()).default).toBe(SetupWizard);
    expect(parseRoute('#/setup/project')).toEqual({ page: 'setup', key: 'project', tab: null });
  });

  it('has the sitemap gallery under the route its page names (RB-090-86)', async () => {
    const { PAGES } = await import('./routes');
    const { GALLERY_ROUTE } = await import('../sitemap/gallery/gallery');
    const { default: SitemapGalleryPage } = await import('../sitemap/gallery/SitemapGalleryPage.vue');
    expect((await PAGES[GALLERY_ROUTE]!()).default).toBe(SitemapGalleryPage);
  });

  // RB-090-38: the palette's popup and the panels window loaded the whole settings page with
  // them; each page now loads when its route is first shown.
  it('loads no page before its route is shown', async () => {
    vi.resetModules();
    const evaluated: string[] = [];
    vi.doMock('./SettingsPage.vue', () => {
      evaluated.push('settings');
      return { default: { name: 'SettingsPage' } };
    });
    try {
      const { PAGES, pageOf } = await import('./routes');
      pageOf('palette');
      pageOf('settings');
      expect(evaluated).toEqual([]);
      await PAGES.settings!();
      expect(evaluated).toEqual(['settings']);
    } finally {
      vi.doUnmock('./SettingsPage.vue');
      vi.resetModules();
    }
  });
});
