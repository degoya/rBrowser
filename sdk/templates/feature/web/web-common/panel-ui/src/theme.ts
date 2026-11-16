// The app's theme in a panel (RB-080-83): the `settings` message carries `ui.theme` (`system`,
// `light`, `dark`) beside the plugin's own keys. Nuxt UI's colours switch on the `dark` class of
// the root, as in the shell (ui/src/theme); `data-theme` stays set for `light` and `dark` as the
// kit panels set it (docs/plugins.md, "Theme"). `system` follows the system's scheme, live.

export type ThemeSetting = 'system' | 'light' | 'dark';

interface DarkQuery {
  matches: boolean;
  addEventListener(type: 'change', listener: (event: { matches: boolean }) => void): void;
}

/** The theme setting of a panel's settings; `system` for anything else. */
export function themeOf(settings: Record<string, unknown> | undefined): ThemeSetting {
  const value = settings?.['ui.theme'];
  return value === 'light' || value === 'dark' ? value : 'system';
}

/** Sets the root's class, colour scheme and `data-theme` for `setting`. */
export function applyTheme(root: HTMLElement, setting: ThemeSetting, systemDark: boolean): void {
  const dark = setting === 'dark' || (setting === 'system' && systemDark);
  root.classList.toggle('dark', dark);
  root.style.colorScheme = dark ? 'dark' : 'light';
  if (setting === 'system') delete root.dataset.theme;
  else root.dataset.theme = setting;
}

/** Follows the theme: `apply(settings)` on every `settings` message, and on a system switch. */
export function themeFollower(
  root: HTMLElement,
  matchMedia: ((query: string) => DarkQuery) | undefined,
): (settings: Record<string, unknown> | undefined) => void {
  let setting: ThemeSetting = 'system';
  const query = matchMedia?.('(prefers-color-scheme: dark)');
  const dark = (): boolean => query?.matches ?? false;
  query?.addEventListener('change', () => applyTheme(root, setting, dark()));
  applyTheme(root, setting, dark());
  return (settings) => {
    setting = themeOf(settings);
    applyTheme(root, setting, dark());
  };
}
