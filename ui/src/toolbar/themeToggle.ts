// The choice of rBrowser's own theme in the toolbar (RB-080-95, RB-090-10, `ThemeToggle.vue`): the
// commands it runs and names, the setting that shows it, its icon and the mark of each theme.

import { ICONS } from '../icons';
import type { ResolvedTheme, ThemeSetting } from '../theme/theme';

/** Sets rBrowser's theme (`theme`: system, light, dark); the menu's choices run it. */
export const THEME_SET_COMMAND = 'ui.theme.set';
/** Steps rBrowser's theme: system, light, dark (`Leader V 0`); the tooltip names its shortcut. */
export const THEME_CYCLE_COMMAND = 'ui.theme.cycle';
/** Whether the toolbar shows the choice. */
export const THEME_TOGGLE_SETTING = 'ui.toolbar.themeToggle';

/**
 * The icon in every state: a sun and a moon in one, read as "light or dark" at a glance - never
 * the page emulation's separate sun and moon.
 */
export const THEME_ICON = ICONS.theme;

/** The small mark on the icon that says the theme in effect: a screen for Windows'. */
export const THEME_MARKS: Record<ThemeSetting, string> = {
  system: ICONS.themeSystem,
  light: ICONS.themeLight,
  dark: ICONS.themeDark,
};

/** The order of the menu's choices. */
export const THEME_CHOICES: readonly ThemeSetting[] = ['system', 'light', 'dark'];

/** The name of the control in each state; System names what Windows uses now. */
type Entry = `system_${ResolvedTheme}` | Exclude<ThemeSetting, 'system'>;

function entryOf(theme: ThemeSetting, windows: ResolvedTheme): Entry {
  return theme === 'system' ? `system_${windows}` : theme;
}

const LABELS: Record<Entry, string> = {
  system_light: 'shell.toolbar.theme.system_light',
  system_dark: 'shell.toolbar.theme.system_dark',
  light: 'theme.name.light',
  dark: 'theme.name.dark',
};

/** The text of each choice in the menu; System names what Windows uses now. */
const CHOICES: Record<Entry, string> = {
  system_light: 'shell.toolbar.theme.option.system_light',
  system_dark: 'shell.toolbar.theme.option.system_dark',
  light: 'shell.toolbar.theme.option.light',
  dark: 'shell.toolbar.theme.option.dark',
};

/** The key of the control's name for `theme`, with Windows' theme `windows` for System. */
export function themeLabelKey(theme: ThemeSetting, windows: ResolvedTheme): string {
  return LABELS[entryOf(theme, windows)];
}

/** The key of the menu's text for `theme`, with Windows' theme `windows` for System. */
export function themeChoiceKey(theme: ThemeSetting, windows: ResolvedTheme): string {
  return CHOICES[entryOf(theme, windows)];
}
