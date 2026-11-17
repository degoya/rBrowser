// The ids the palette shares with the core and the settings (RB-060-11).

import { booleanSetting } from '../settings/booleanSetting';

/** The command the double leader runs (`rb_commands::chord::PALETTE_COMMAND`). */
export const PALETTE_COMMAND = 'app.command_palette';

/** Where the shortcut editor lives (`Ctrl+Enter` on an entry opens it). */
export const SHORTCUTS_SETTINGS_KEY = 'shortcuts.bindings';

/** The palette's settings (`schema/parts/palette.json`). */
export const SETTING_RECENT_COUNT = 'palette.recentCount';
export const SETTING_SHOW_DESCRIPTIONS = 'palette.showDescriptions';
export const SETTING_PREFIXES = 'palette.prefixes';

export interface PaletteOptions {
  recentCount: number;
  showDescriptions: boolean;
  prefixes: boolean;
}

export const DEFAULT_OPTIONS: PaletteOptions = { recentCount: 8, showDescriptions: true, prefixes: true };
/** The schema's maximum of `palette.recentCount`. */
export const MAX_RECENT = 50;

/** The palette's settings from the values the core sent; a missing or odd value is the default. */
export function optionsOf(values: ReadonlyMap<string, unknown>): PaletteOptions {
  const count = values.get(SETTING_RECENT_COUNT);
  const descriptions = values.get(SETTING_SHOW_DESCRIPTIONS);
  const prefixes = values.get(SETTING_PREFIXES);
  return {
    recentCount:
      typeof count === 'number' && Number.isFinite(count)
        ? Math.min(MAX_RECENT, Math.max(0, Math.floor(count)))
        : DEFAULT_OPTIONS.recentCount,
    showDescriptions: booleanSetting(descriptions, DEFAULT_OPTIONS.showDescriptions),
    prefixes: booleanSetting(prefixes, DEFAULT_OPTIONS.prefixes),
  };
}
