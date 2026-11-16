// The emulation extras of the toolbar (RB-080-10): geolocation, time zone, CPU throttling,
// offline and the print view. Every choice is one of the core commands `view.emulate.*` or
// `view.add_print` for the active view; with Shift held it targets every view (as RB-010-05).

import type { Emulation, JsonValue, ShellToCore } from '../ipc/protocol';

/** The command ids, as `crates/rbrowser/src/emulation/extras.rs` registers them. */
export const EXTRA_COMMANDS = {
  geolocation: 'view.emulate.geolocation',
  timezone: 'view.emulate.timezone',
  cpu: 'view.emulate.cpu',
  offline: 'view.emulate.offline',
  addPrint: 'view.add_print',
} as const;

/** The built-in geolocation presets (rb-config `emulation_settings`); label key by id. */
export const GEOLOCATION_PRESETS = [
  { id: 'berlin', latitude: 52.52, longitude: 13.405 },
  { id: 'new-york', latitude: 40.7128, longitude: -74.006 },
  { id: 'tokyo', latitude: 35.6762, longitude: 139.6503 },
  { id: 'sydney', latitude: -33.8688, longitude: 151.2093 },
] as const;

/** The i18n keys of the built-in presets. */
export const PRESET_LABEL_KEYS: Record<string, string> = {
  berlin: 'emulation.geolocation.preset.berlin',
  'new-york': 'emulation.geolocation.preset.new-york',
  tokyo: 'emulation.geolocation.preset.tokyo',
  sydney: 'emulation.geolocation.preset.sydney',
};

/** The throttling factors offered; 1 is none. */
export const CPU_RATES = [2, 4, 6, 20] as const;

/** Time zones offered for a quick pick; any IANA name can be typed. */
export const TIMEZONES = [
  'America/Los_Angeles',
  'America/New_York',
  'Europe/London',
  'Europe/Berlin',
  'Asia/Kolkata',
  'Asia/Tokyo',
  'Australia/Sydney',
  'UTC',
] as const;

/** The papers of a print view. */
export const PAPERS = ['A4', 'Letter'] as const;
export type Paper = (typeof PAPERS)[number];

/** A command of the extras for the active view, or every view when `all`. */
export function extraMessage(
  id: (typeof EXTRA_COMMANDS)[keyof typeof EXTRA_COMMANDS],
  args: { [key: string]: JsonValue },
  all: boolean,
): ShellToCore {
  const withTarget: { [key: string]: JsonValue } = { ...args };
  if (all) {
    withTarget.target = 'all';
  }
  return { type: 'runCommand', id, args: withTarget };
}

/** `view.add_print` on `paper`, from the active view. */
export function addPrintMessage(paper: Paper): ShellToCore {
  return { type: 'runCommand', id: EXTRA_COMMANDS.addPrint, args: { paper } };
}

/** A coordinate typed by hand: a finite number in `[min, max]`, else `null`. */
export function parseCoordinate(text: string, min: number, max: number): number | null {
  const trimmed = text.trim().replace(',', '.');
  if (trimmed === '') {
    return null;
  }
  const value = Number(trimmed);
  return Number.isFinite(value) && value >= min && value <= max ? value : null;
}

/** The preset the position is, or `null` for a free one. */
export function presetOf(current: Emulation | null): string | null {
  const position = current?.geolocation;
  if (!position) {
    return null;
  }
  const match = GEOLOCATION_PRESETS.find(
    (preset) => preset.latitude === position.latitude && preset.longitude === position.longitude,
  );
  return match?.id ?? null;
}

/** Whether the view is offline. */
export function isOffline(current: Emulation | null): boolean {
  return current?.network?.offline === true;
}
