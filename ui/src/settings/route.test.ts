import { describe, expect, it } from 'vitest';
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
    expect(pageOf('palette')).toBe(PAGES.palette);
    expect(PAGES.palette).not.toBe(PAGES.settings);
    expect(PAGES.findings).toBeUndefined();
  });

  it('has the setup wizard, opened at its project step by the hint (RB-091-05)', async () => {
    const { PAGES, pageOf } = await import('./routes');
    expect(pageOf('setup')).toBe(PAGES.setup);
    expect(parseRoute('#/setup/project')).toEqual({ page: 'setup', key: 'project', tab: null });
  });
});
