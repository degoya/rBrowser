// Light, dark or the system's: the `ui.theme` user setting (rb-config `UiSettings.theme`,
// Settings > General > Appearance), rBrowser's own theme - never the pages' colour scheme, which
// is the emulation's. Nuxt UI's colours switch on the `dark` class of the root; the native parts
// follow the same setting in the core (`crate::app_theme`, RB-080-95).

import type { JsonValue } from '../ipc/protocol';

export type ThemeSetting = 'system' | 'light' | 'dark';
export type ResolvedTheme = 'light' | 'dark';

export const THEME_SETTING_KEY = 'ui.theme';

export function parseTheme(value: JsonValue | undefined): ThemeSetting {
  return value === 'light' || value === 'dark' ? value : 'system';
}

/** The theme after `setting` in the toolbar's turn, as `ui.theme.cycle` steps (RB-080-95). */
export function nextTheme(setting: ThemeSetting): ThemeSetting {
  if (setting === 'system') {
    return 'light';
  }
  return setting === 'light' ? 'dark' : 'system';
}

export function resolveTheme(setting: ThemeSetting, systemDark: boolean): ResolvedTheme {
  if (setting === 'system') {
    return systemDark ? 'dark' : 'light';
  }
  return setting;
}

export function applyTheme(root: HTMLElement, theme: ResolvedTheme): void {
  root.classList.toggle('dark', theme === 'dark');
  root.style.colorScheme = theme;
}

interface DarkQuery {
  matches: boolean;
  addEventListener(type: 'change', listener: (event: { matches: boolean }) => void): void;
  removeEventListener(type: 'change', listener: (event: { matches: boolean }) => void): void;
}

/** Calls `onChange` now and whenever the system switches; returns the function that stops. */
export function watchSystemDark(
  matchMedia: (query: string) => DarkQuery,
  onChange: (dark: boolean) => void,
): () => void {
  const query = matchMedia('(prefers-color-scheme: dark)');
  const listener = (event: { matches: boolean }): void => onChange(event.matches);
  query.addEventListener('change', listener);
  onChange(query.matches);
  return () => query.removeEventListener('change', listener);
}
