// The pages window's router: the route is the URL hash (`settings.html#/settings/sync.scroll`,
// `#/trust`, `#/analyzer`), so the window routes without a server and the core opens a deep
// link by loading a URL (`shell::page_url`). Only well-formed keys reach a page. A category with
// sub-tabs keeps its tab in the address as `?tab=<section>` (`#/settings/layout?tab=grid`,
// RB-080-13); with a tab the key names the category.

import { onScopeDispose, ref, type Ref } from 'vue';

export interface PageRoute {
  /** The page: `settings`, `trust`, `analyzer`. */
  page: string;
  /** What the page opens at: a setting key or a category id (`settings/<key>`). */
  key: string | null;
  /** The sub-tab of the category the page opens at (`?tab=`); `null` without one. */
  tab: string | null;
}

export const DEFAULT_PAGE = 'settings';

const PAGE = /^[a-z][a-z0-9-]*$/;
const KEY = /^[A-Za-z][A-Za-z0-9_-]*(\.[A-Za-z][A-Za-z0-9_-]*)*$/;

/** The route of `hash` (`#/settings/sync.scroll`); anything malformed opens the settings. */
export function parseRoute(hash: string): PageRoute {
  const [path = '', query = ''] = hash.replace(/^#\/?/, '').split('?', 2);
  const [page = '', key, ...rest] = path.split('/');
  if (!PAGE.test(page) || rest.length > 0) {
    return { page: DEFAULT_PAGE, key: null, tab: null };
  }
  const parsedKey = key !== undefined && KEY.test(key) ? key : null;
  const tab = new URLSearchParams(query).get('tab');
  return { page, key: parsedKey, tab: parsedKey !== null && tab !== null && PAGE.test(tab) ? tab : null };
}

/** The hash of `route`. */
export function routeHash(route: PageRoute): string {
  if (route.key === null) {
    return `#/${route.page}`;
  }
  const tab = route.tab === null ? '' : `?tab=${route.tab}`;
  return `#/${route.page}/${route.key}${tab}`;
}

/** The current route, following `hashchange` (the core loads the same page with a new hash). */
export function useHashRoute(win: Window = window): Ref<PageRoute> {
  const route = ref<PageRoute>(parseRoute(win.location.hash));
  const update = (): void => {
    route.value = parseRoute(win.location.hash);
  };
  win.addEventListener('hashchange', update);
  onScopeDispose(() => win.removeEventListener('hashchange', update));
  return route;
}
