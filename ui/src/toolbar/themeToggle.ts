// rBrowser's own theme in the main menu `≡` (RB-080-95, RB-090-10; App > Theme since RB-090-63):
// the commands its choices run and the text of each choice.

import type { ResolvedTheme, ThemeSetting } from '../theme/theme';

/** Sets rBrowser's theme (`theme`: system, light, dark); the menu's choices run it. */
export const THEME_SET_COMMAND = 'ui.theme.set';
/** Steps rBrowser's theme: system, light, dark (`Leader V 0`). */
export const THEME_CYCLE_COMMAND = 'ui.theme.cycle';

/** The order of the menu's choices. */
export const THEME_CHOICES: readonly ThemeSetting[] = ['system', 'light', 'dark'];

/** The text of each choice in the menu; System names what Windows uses now. */
type Entry = `system_${ResolvedTheme}` | Exclude<ThemeSetting, 'system'>;

const CHOICES: Record<Entry, string> = {
  system_light: 'shell.toolbar.theme.option.system_light',
  system_dark: 'shell.toolbar.theme.option.system_dark',
  light: 'shell.toolbar.theme.option.light',
  dark: 'shell.toolbar.theme.option.dark',
};

/** The key of the menu's text for `theme`, with Windows' theme `windows` for System. */
export function themeChoiceKey(theme: ThemeSetting, windows: ResolvedTheme): string {
  const entry: Entry = theme === 'system' ? `system_${windows}` : theme;
  return CHOICES[entry];
}
