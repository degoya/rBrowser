// The shell's state: the core's snapshot, the theme and Windows' text size (RB-090-26). Created once per page and provided to the
// components (`useShell`). A message the page raises itself (a refused request, an address it
// does not open) goes to the core (`reportStatus`), which shows it in the status bar with every
// other message (RB-080-15).

import { computed, inject, ref, shallowRef, watchEffect, type InjectionKey } from 'vue';
import type { IpcClient } from '../ipc/client';
import type { BoardState } from '../ipc/layout';
import {
  IpcErrorCode,
  type CommandInfo,
  type CoreToShell,
  type JsonValue,
  type NotificationLevel,
  type SettingValue,
  type ShellToCore,
  type StateSnapshot,
} from '../ipc/protocol';
import { IpcError } from '../ipc/transport';
import { toMessage, type ToolbarAction } from '../toolbar/actions';
import {
  applyTheme,
  parseTheme,
  resolveTheme,
  THEME_SETTING_KEY,
  watchSystemDark,
  type ThemeSetting,
} from '../theme/theme';
import { applyTextScale, parseTextScale } from '../theme/textScale';
import { changedValues } from './settingsChanged';
import { EDIT_ADDRESS, addressEditOf, type AddressEdit } from '../canvas/addressEdit';

export interface ShellStatus {
  key: string;
  args: Record<string, string | number>;
  level?: NotificationLevel;
  /** The setting the notice is about: the status bar links to it (RB-010-08). */
  settingsKey?: string;
}

export interface ShellEnvironment {
  root: HTMLElement;
  matchMedia: Parameters<typeof watchSystemDark>[0];
}

export type Shell = ReturnType<typeof createShell>;

export const SHELL_KEY: InjectionKey<Shell> = Symbol('rb-shell');

export function useShell(): Shell {
  const shell = inject(SHELL_KEY);
  if (shell === undefined) {
    throw new Error('useShell() outside a component tree that provides the shell');
  }
  return shell;
}

function statusOf(error: unknown): ShellStatus {
  if (error instanceof IpcError) {
    if (error.code === IpcErrorCode.NotImplemented) {
      return { key: 'shell.error.not_implemented', args: {}, level: 'warning' };
    }
    return {
      key: 'shell.error.request_failed',
      args: { code: error.code, message: error.message },
      level: 'error',
    };
  }
  return { key: 'shell.error.no_answer', args: {}, level: 'error' };
}

/** `status` as the `reportStatus` the core takes: an error unless it says otherwise. */
export function reportOf(status: ShellStatus): ShellToCore {
  const args = Object.fromEntries(Object.entries(status.args).map(([name, value]) => [name, String(value)]));
  return {
    type: 'reportStatus',
    level: status.level ?? 'error',
    messageKey: status.key,
    ...(Object.keys(args).length === 0 ? {} : { args }),
    ...(status.settingsKey === undefined ? {} : { settingsKey: status.settingsKey }),
  };
}

export function createShell(client: IpcClient, env: ShellEnvironment) {
  /** Replaced whole by each push, never changed inside: a shallow ref (RB-090-36). */
  const state = shallowRef<StateSnapshot | null>(null);
  const theme = ref<ThemeSetting>('system');
  /** The user settings in effect, by key (`getSettings`, again after `settingsChanged`). */
  const settings = ref<Record<string, JsonValue>>({});
  /** The canvas board for a camera a gesture is trying out (`canvasLive`); gone with the next state. */
  const liveBoard = ref<BoardState | null>(null);
  /** The number of the last `canvasLive`: the board says when it painted it (RB-090-55). */
  const liveSeq = ref<number | null>(null);
  const systemDark = ref(false);
  /** Windows' text size the page is drawn at, in percent (RB-090-26). */
  const textScale = ref(100);
  const focusAddressBarRequest = ref(0);
  /** The address field open in a decoupled view's label on the canvas board (RB-092-03). */
  const addressEdit = ref<AddressEdit | null>(null);
  /**
   * The registry's bindings by command id, for tooltips that name a shortcut (RB-080-60): asked
   * for once by the first component that needs them (`loadCommands`), then kept current by the
   * core's `commands` events (a binding edited, a plugin's commands added); `null` until then.
   */
  const bindings = ref<ReadonlyMap<string, readonly string[]> | null>(null);
  /** The registry's commands themselves (titles, groups, plugin commands) for the menus (RB-090-63). */
  const commands = shallowRef<readonly CommandInfo[]>([]);
  let commandsAsked = false;
  /** The last command this page sent and when: what the person started (the error toast, RB-094-20). */
  const lastRun = shallowRef<{ at: number; retry: ShellToCore } | null>(null);
  const resolvedTheme = computed(() => resolveTheme(theme.value, systemDark.value));
  /** What Windows uses now, whatever `ui.theme` says: the toolbar's System entry names it. */
  const systemTheme = computed(() => resolveTheme('system', systemDark.value));
  const stops: (() => void)[] = [];

  const takeSettings = (values: SettingValue[], merge = false): void => {
    const taken = Object.fromEntries(values.map((v) => [v.key, v.value]));
    settings.value = merge ? { ...settings.value, ...taken } : taken;
    // The theme as the settings hold it: the last value of the key, as for every other setting.
    if (THEME_SETTING_KEY in taken) {
      theme.value = parseTheme(taken[THEME_SETTING_KEY]);
    }
  };

  const onEvent = (message: CoreToShell): void => {
    switch (message.type) {
      case 'commands':
        bindings.value = new Map(message.commands.map((command) => [command.id, command.bindings]));
        commands.value = message.commands;
        break;
      case 'state': {
        const { type: _type, ...snapshot } = message;
        state.value = snapshot;
        liveBoard.value = null;
        break;
      }
      case 'metrics':
        // Only the metrics changed (RB-090-36): the rest of the state stays as it is.
        if (state.value) {
          state.value = { ...state.value, metrics: message.metrics };
        }
        break;
      case 'pluginProgress':
        // Only the plugins' progress changed (RB-094-10); none is absent, as in the snapshot.
        if (state.value) {
          const { pluginProgress: _old, ...rest } = state.value;
          state.value = message.progress.length > 0 ? { ...rest, pluginProgress: message.progress } : rest;
        }
        break;
      case 'canvasLive':
        liveBoard.value = message.board;
        liveSeq.value = message.seq;
        break;
      case 'textScale':
        textScale.value = parseTextScale(message.percent);
        break;
      case 'settings':
        takeSettings(message.values);
        break;
      case 'settingsChanged': {
        // Another window changed a setting: take the new values (theme, language) - from the
        // event when it carries them (RB-090-36), else asked for.
        const changed = changedValues(message);
        if (changed) {
          takeSettings([...changed.values()], true);
          break;
        }
        void client
          .expect({ type: 'getSettings' }, 'settings')
          .then(onEvent)
          .catch(() => undefined);
        break;
      }
      case 'uiCommand':
        // A shortcut or MCP resolved to a command that lives in the shell.
        if (message.id === 'view.focus_address_bar') {
          focusAddressBar();
        } else if (message.id === EDIT_ADDRESS) {
          addressEdit.value = addressEditOf(message.args) ?? addressEdit.value;
        }
        break;
      default:
        break;
    }
  };

  async function start(): Promise<void> {
    stops.push(client.on(onEvent));
    // Asked first, so the page grows before it is seen; a core without it leaves 100 %.
    client
      .expect({ type: 'getTextScale' }, 'textScale')
      .then(onEvent)
      .catch(() => undefined);
    stops.push(watchSystemDark(env.matchMedia, (dark) => (systemDark.value = dark)));
    stops.push(watchEffect(() => applyTheme(env.root, resolvedTheme.value)));
    stops.push(watchEffect(() => applyTextScale(env.root, textScale.value)));
    try {
      onEvent(await client.expect({ type: 'getState' }, 'state'));
      onEvent(await client.expect({ type: 'getSettings' }, 'settings'));
    } catch (error) {
      report(statusOf(error));
    }
  }

  function stop(): void {
    stops.splice(0).forEach((fn) => fn());
  }

  /**
   * Sends a request; the core's refusal as text (in the status bar too), `null` when the core
   * took it - for an input that says at its own place why it was refused (RB-080-46).
   */
  async function refused(message: ShellToCore): Promise<string | null> {
    if (message.type === 'runCommand') {
      lastRun.value = { at: Date.now(), retry: message };
    }
    try {
      await client.request(message);
      return null;
    } catch (error) {
      report(statusOf(error));
      return error instanceof Error ? error.message : '';
    }
  }

  /** Sends a request; `false` and a message in the status bar when the core refuses it. */
  async function send(message: ShellToCore): Promise<boolean> {
    return (await refused(message)) === null;
  }

  /**
   * Sends a request whose refusal is no news for the person (the toolbar asking for a second
   * row while the settings change, RB-080-52): `false` when refused, nothing in the status bar.
   */
  async function ask(message: ShellToCore): Promise<boolean> {
    try {
      await client.request(message);
      return true;
    } catch {
      return false;
    }
  }

  /** Asks the core for the registry's bindings, once; a core that cannot say leaves none. */
  function loadCommands(): void {
    if (commandsAsked) {
      return;
    }
    commandsAsked = true;
    client
      .expect({ type: 'listCommands' }, 'commands')
      .then(onEvent)
      .catch(() => undefined);
  }

  /** The first binding of the command `id` in text form; `null` when it has none or none is known. */
  function bindingOf(id: string): string | null {
    return bindings.value?.get(id)?.[0] ?? null;
  }

  /** Sends a toolbar action (a registry command, `actions.ts`). */
  function run(action: ToolbarAction): Promise<boolean> {
    return send(toMessage(action));
  }

  /** Puts `status` into the status bar; a core that cannot take it leaves it in the log. */
  function report(status: ShellStatus): void {
    client.request(reportOf(status)).catch((error: unknown) => {
      console.warn('a message for the status bar was not taken', status.key, error);
    });
  }

  /** The `view.focus_address_bar` command; the address bar watches the counter. */
  function focusAddressBar(): void {
    focusAddressBarRequest.value += 1;
  }

  /** Closes the board's address field: loaded, Escape or a focus elsewhere. */
  function closeAddressEdit(): void {
    addressEdit.value = null;
  }

  return {
    state,
    liveBoard,
    liveSeq,
    theme,
    settings,
    resolvedTheme,
    systemTheme,
    textScale,
    focusAddressBarRequest,
    addressEdit,
    start,
    stop,
    send,
    refused,
    run,
    report,
    focusAddressBar,
    closeAddressEdit,
    ask,
    loadCommands,
    bindingOf,
    commands,
    lastRun,
  };
}
