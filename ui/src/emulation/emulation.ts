// The shell's emulation controls (RB-010-05): every click is the command `emulation.set` (or
// `emulation.reset_view`) for the active view; with Shift held it targets every view. The
// current values come from `getEmulation` through the query the page provides.

import type { InjectionKey } from 'vue';
import type { ColorScheme, Emulation, JsonValue, ShellToCore, ViewId } from '../ipc/protocol';

/** Asks the core for a view's emulation (`getEmulation`); `null` when there is none. */
export type EmulationQuery = (view: ViewId) => Promise<Emulation | null>;

/** Provided by the page that has the IPC client (`bootstrap.ts`). */
export const EMULATION_QUERY_KEY: InjectionKey<EmulationQuery> = Symbol('rb-emulation-query');

/** The fields the toolbar sets. */
export type EmulationFieldsPatch = { colorScheme: ColorScheme } | { reducedMotion: boolean };

/** `emulation.set` for the active view, or for all views when `all`. */
export function setMessage(fields: EmulationFieldsPatch, all: boolean): ShellToCore {
  const args: { [key: string]: JsonValue } = { fields };
  if (all) {
    args.target = 'all';
  }
  return { type: 'runCommand', id: 'emulation.set', args };
}

/** `emulation.reset_view` for the active view, or for all views when `all`. */
export function resetMessage(all: boolean): ShellToCore {
  return all
    ? { type: 'runCommand', id: 'emulation.reset_view', args: { target: 'all' } }
    : { type: 'runCommand', id: 'emulation.reset_view' };
}

/** The reduced-motion value a click sets: the opposite of the active view's. */
export function nextReducedMotion(current: Emulation | null): boolean {
  return current?.reducedMotion !== true;
}

/** The commands behind the controls and their MCP coverage. */
export const EMULATION_COMMANDS = {
  set: { id: 'emulation.set', mcp: 'emulation_set' },
  reset: { id: 'emulation.reset_view', mcp: 'emulation_set' },
} as const;
