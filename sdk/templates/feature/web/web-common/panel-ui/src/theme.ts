// The app's theme in a panel (RB-080-83): the `settings` message carries `ui.theme` (`system`,
// `light`, `dark`) beside the plugin's own keys. Nuxt UI's colours switch on the `dark` class of
// the root, as in the shell (ui/src/theme); `data-theme` stays set for `light` and `dark` as the
// kit panels set it (docs/plugins.md, "Theme"). `system` follows the system's scheme, live.
// The same message carries the shell's text size as `ui.textScale` (RB-090-39): Windows' text
// size in percent while `ui.followWindowsTextSize` is on, else 100. The panel takes it as its
// root font size, as the shell's pages do (ui/src/theme/textScale.ts), so every rem grows.

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

const MIN_TEXT_PERCENT = 100;
const MAX_TEXT_PERCENT = 225;

/** The text size of a panel's settings in percent, held to 100..225; 100 for anything else. */
export function textScaleOf(settings: Record<string, unknown> | undefined): number {
  const value = settings?.['ui.textScale'];
  if (typeof value !== 'number' || !Number.isFinite(value)) return MIN_TEXT_PERCENT;
  return Math.min(MAX_TEXT_PERCENT, Math.max(MIN_TEXT_PERCENT, Math.round(value)));
}

/** Draws the panel at `percent`: the root's font size; at 100 % the page's own. */
export function applyTextScale(root: HTMLElement, percent: number): void {
  root.style.fontSize = percent === MIN_TEXT_PERCENT ? '' : `${percent}%`;
}

/** Follows the theme and the text size: `apply(settings)` on every `settings` message, and the
 * theme on a system switch. */
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
    applyTextScale(root, textScaleOf(settings));
  };
}
