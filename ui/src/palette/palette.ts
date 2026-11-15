// The command palette's state (RB-060-11): open on the double leader, the query with its prefix,
// the argument step, the selection, choosing an entry. Opening shows the palette at once with the
// entries of the last open and refreshes them from the core (`listCommands`, the settings with
// their schema, the project's flows, the device presets) - a plugin's command is there as soon as
// the registry has it. Closing hides the
// palette's popup window and gives the keyboard focus back to the view that had it, or to the
// first view when that one is gone.

import { computed, ref, shallowRef, watch, type InjectionKey } from 'vue';
import type { IpcClient } from '../ipc/client';
import type {
  CommandInfo,
  DevicePreset,
  FlowSummary,
  JsonValue,
  ShellToCore,
  StateSnapshot,
  ViewId,
} from '../ipc/protocol';
import { parseSchema, type SettingsSchema } from '../settings/schema';
import { DEFAULT_OPTIONS, optionsOf, SHORTCUTS_SETTINGS_KEY, type PaletteOptions } from './constants';
import { parseQuery, rank } from './rank';
import { createRecent, type Recent } from './recent';
import { paletteProviders, type ProviderRegistry } from './registry';
import type { PaletteEntry, ProviderContext } from './types';

export interface PaletteDeps {
  client: IpcClient;
  /** Sends a request, reporting a refusal in the status line (`Shell.send`). */
  send: (message: ShellToCore) => Promise<boolean>;
  /** Hides the palette's popup window (`dismissPalette`); before the focus goes back. */
  dismiss?: () => Promise<void>;
  state: () => StateSnapshot | null;
  t: ProviderContext['t'];
  registry?: ProviderRegistry;
  recent?: Recent;
}

export interface PaletteStep {
  id: string;
  title: string;
}

export type Palette = ReturnType<typeof createPalette>;

/** The IPC client the palette asks the core with (`bootstrap.ts` provides the shell's). */
export const PALETTE_CLIENT_KEY: InjectionKey<IpcClient> = Symbol('rb-palette-client');

export function createPalette(deps: PaletteDeps) {
  const registry = deps.registry ?? paletteProviders;
  const recent = deps.recent ?? createRecent();
  const isOpen = ref(false);
  const query = ref('');
  const step = ref<PaletteStep | null>(null);
  const active = ref(0);
  const options = ref<PaletteOptions>({ ...DEFAULT_OPTIONS });
  const recentKeys = ref<string[]>(recent.list());
  const context = shallowRef<ProviderContext>({
    state: null,
    commands: [],
    settings: null,
    values: new Map(),
    t: deps.t,
  });
  const entries = shallowRef<PaletteEntry[]>([]);
  let previousView: ViewId | null = null;
  /** The palette was opened in its step (the new-size picker): Escape closes it. */
  let direct = false;
  let commands: CommandInfo[] = [];
  let settings: SettingsSchema | null = null;
  let values = new Map<string, JsonValue>();
  let devices: readonly DevicePreset[] | undefined;
  let flows: readonly FlowSummary[] | undefined;

  const results = computed<PaletteEntry[]>(() => {
    const parsed = step.value === null ? parseQuery(query.value, options.value.prefixes) : { scope: null, text: query.value.trim() };
    const typed = registry.collectForQuery(parsed.text, context.value);
    return rank([...entries.value, ...typed], parsed.text, {
      recent: recentKeys.value,
      step: step.value?.id ?? null,
      scope: parsed.scope,
    });
  });

  /** The user moved the selection since the query, the step or the open last changed. */
  let moved = false;

  watch(
    [query, step],
    () => {
      active.value = 0;
      moved = false;
    },
    { flush: 'sync' },
  );

  /**
   * Builds the entries again. A selection the user moved stays on its entry, wherever the new
   * rows put it - Enter runs what is highlighted (RB-080-68).
   */
  function refresh(): void {
    const chosen = moved ? results.value[active.value]?.key : undefined;
    context.value = { state: deps.state(), commands, settings, values, devices, flows, t: deps.t };
    entries.value = registry.collect(context.value);
    if (chosen !== undefined) {
      const index = results.value.findIndex((entry) => entry.key === chosen);
      active.value = Math.max(index, 0);
      moved = index !== -1;
    }
  }

  // The core's state can come after an open: the popup's first load mounts the page before the
  // shell has asked for it (`bootstrap.ts`), and the new-size picker stood empty (RB-080-68).
  // The entries are built again once when it comes; its later pushes change nothing here.
  watch(
    () => deps.state() !== null,
    (present) => {
      if (present && isOpen.value) {
        refresh();
      }
    },
  );

  /** Fetches the commands and the settings; what fails keeps the last answer. */
  async function load(): Promise<void> {
    const [listed, read, flowList, deviceList] = await Promise.allSettled([
      deps.client.expect({ type: 'listCommands' }, 'commands'),
      deps.client.expect({ type: 'getSettings', includeSchema: true }, 'settings'),
      deps.client.expect({ type: 'listFlows' }, 'flows'),
      deps.client.expect({ type: 'getDevices' }, 'devices'),
    ]);
    if (flowList.status === 'fulfilled') {
      flows = flowList.value.items;
    }
    if (deviceList.status === 'fulfilled') {
      devices = deviceList.value.items;
    }
    if (listed.status === 'fulfilled') {
      commands = listed.value.commands;
    }
    if (read.status === 'fulfilled') {
      values = new Map(read.value.values.map((v) => [v.key, v.value]));
      options.value = optionsOf(values);
      try {
        settings = parseSchema(read.value.schema);
      } catch {
        settings = null;
      }
    }
    refresh();
  }

  /**
   * Shows the palette at once; the entries refresh when the core has answered. With `initial`
   * it opens in that step (`view.add_size`'s picker, RB-080-46).
   */
  function open(initial: PaletteStep | null = null): Promise<void> {
    // Read again on every open: the view of an earlier open may be gone.
    previousView = deps.state()?.activeView ?? null;
    query.value = '';
    step.value = initial;
    direct = initial !== null;
    active.value = 0;
    moved = false;
    recentKeys.value = recent.list();
    isOpen.value = true;
    refresh();
    return load();
  }

  /**
   * Hides the palette and its popup (unless `dismiss` is off: the popup hid itself when it lost
   * the focus); with `restoreFocus` the view that had the focus gets it back.
   */
  async function close(restoreFocus = true, dismiss = true): Promise<void> {
    if (!isOpen.value) {
      return;
    }
    isOpen.value = false;
    step.value = null;
    query.value = '';
    if (dismiss) {
      await deps.dismiss?.();
    }
    const view = restoreFocus ? focusTarget() : null;
    if (view !== null) {
      await deps.send({ type: 'setActiveView', view });
    }
  }

  /** The view the focus goes back to: the one it came from while it exists, else the first. */
  function focusTarget(): ViewId | null {
    const views = deps.state()?.views ?? [];
    if (previousView !== null && views.some((v) => v.id === previousView)) {
      return previousView;
    }
    return views[0]?.id ?? null;
  }

  /** Escape: out of a step chosen here, else closed (a step the palette opened in included). */
  function escape(): void {
    if (step.value !== null && !direct) {
      back();
      return;
    }
    void close(true);
  }

  /** Out of the step, to the top level (Backspace in the empty field). */
  function back(): void {
    step.value = null;
    query.value = '';
    direct = false;
  }

  function move(delta: number): void {
    const count = results.value.length;
    if (count === 0) {
      active.value = 0;
      return;
    }
    active.value = (((active.value + delta) % count) + count) % count;
    moved = true;
  }

  function remember(entry: PaletteEntry): void {
    recent.push(entry.key, options.value.recentCount);
    recentKeys.value = recent.list();
  }

  /** Runs `entry` (the selected one by default): a step opens, a message goes to the core. */
  async function choose(entry: PaletteEntry | undefined = results.value[active.value]): Promise<void> {
    if (entry === undefined) {
      return;
    }
    remember(entry);
    if (entry.action.kind === 'step') {
      step.value = { id: entry.action.step, title: entry.title };
      query.value = '';
      active.value = 0;
      return;
    }
    const message = entry.action.message;
    // The focus goes back first, so a command that moves it (the address bar) keeps it.
    await close(message.type !== 'setActiveView');
    await deps.send(message);
  }

  /** `Ctrl+Enter`: the entry's command in the shortcut editor. */
  async function editShortcut(entry: PaletteEntry | undefined = results.value[active.value]): Promise<void> {
    if (entry?.commandId === undefined) {
      return;
    }
    await close(false);
    await deps.send({ type: 'openSettings', key: SHORTCUTS_SETTINGS_KEY });
  }

  return { isOpen, query, step, active, options, results, open, close, escape, back, move, choose, editShortcut };
}
