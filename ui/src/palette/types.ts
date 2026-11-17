// The palette's entries and the provider interface (RB-060-11). Every row of the palette is a
// `PaletteEntry` some `PaletteProvider` produced; `README.md` beside this file explains how a
// feature adds its own (RB-010-16's "Set view to ..." sizes, flows, bookmarks).

import type { CommandInfo, DevicePreset, FlowSummary, JsonValue, ShellToCore, StateSnapshot } from '../ipc/protocol';
import type { SettingsSchema } from '../settings/schema';

/** Which prefix shows an entry on its own: `>` commands, `@` views, `#` settings. */
export type EntryScope = 'command' | 'view' | 'setting' | 'other';

/** What choosing an entry does. */
export type PaletteAction =
  /** Sends one message to the core (`runCommand`, `setBreakpoint`, `openSettings`, ...). */
  | { kind: 'send'; message: ShellToCore }
  /** Opens the second step: the entries whose `parent` is `step`. */
  | { kind: 'step'; step: string };

export interface PaletteEntry {
  /** Unique and stable across opens (`command:sync.toggle`): the recent list stores it. */
  key: string;
  scope: EntryScope;
  /** Translated. */
  title: string;
  /** Translated; what the row names as its kind (the command group, "Setting", "Layout"). */
  group?: string;
  /** Translated; shown with `palette.showDescriptions`. */
  description?: string;
  /** Searched too: the command id, synonyms, the setting key. */
  keywords?: string[];
  /** A binding in rb-commands' text form, shown as keys. */
  shortcut?: string;
  /** The registry command behind the entry: `Ctrl+Enter` opens it in the shortcut editor. */
  commandId?: string;
  /**
   * The argument step the entry belongs to (`STEP_SET_VIEW`): listed when that step is open,
   * and at the top level only while a query is typed.
   */
  parent?: string;
  /** Leads its step's list while no query is typed, before the recent entries (RB-080-68). */
  pinned?: boolean;
  action: PaletteAction;
}

/** What a provider sees when the palette opens. */
export interface ProviderContext {
  state: StateSnapshot | null;
  /** The registry's commands (`listCommands`), fresh at every open. */
  commands: CommandInfo[];
  /** The settings schema, when the core sent it. */
  settings: SettingsSchema | null;
  /** The values of the settings by key. */
  values: ReadonlyMap<string, JsonValue>;
  /** The device presets in effect (`getDevices`); absent until the core answered. */
  devices?: readonly DevicePreset[];
  /** The project's flows (`listFlows`); absent until the core answered. */
  flows?: readonly FlowSummary[];
  t: (key: string, args?: Record<string, string | number>) => string;
}

export interface PaletteProvider {
  /** Unique; registering the same id again replaces the provider. */
  id: string;
  /** The entries at open time. */
  entries(context: ProviderContext): PaletteEntry[];
  /**
   * Entries that depend on the typed text (a free size `500x800`); called on every keystroke
   * with the query without its prefix.
   */
  forQuery?(query: string, context: ProviderContext): PaletteEntry[];
}

/** The step "Set view to ...": breakpoints here, device presets and sizes from RB-010-16. */
export const STEP_SET_VIEW = 'set-view';
/** The step "Focus view ...". */
export const STEP_FOCUS_VIEW = 'focus-view';
/** The step "Switch layout preset ...". */
export const STEP_LAYOUT = 'layout';
/** The step "Switch environment ...". */
export const STEP_ENVIRONMENT = 'environment';
/** The step "Run flow ..." (`flowsProvider.ts`). */
export const STEP_RUN_FLOW = 'run-flow';
/** The step "New view size ..." (RB-080-46, `toolbar/newViewProvider.ts`): `view.add_size` opens it. */
export const STEP_NEW_VIEW = 'new-view';
