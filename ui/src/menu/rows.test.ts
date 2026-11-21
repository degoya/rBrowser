// Every row of the main menu `≡` and the split buttons' menus sends what its handler takes
// (RB-090-82): a command of the registry - core or a bundled plugin's - with arguments of the
// shape the core reads, or a setting of the settings schema. A row is chosen in the core's native
// menu and its message sent as it stands (`OverflowMenu.vue`, `SplitButton.vue`), the same way as
// the palette and the toolbar send theirs. And a row whose command fails outside a layout mode or
// a running recording or capture is no choice there.

import { describe, expect, it } from 'vitest';
import type { JsonValue, NativeMenuRow, ShellToCore, StateSnapshot } from '../ipc/protocol';
import { sampleState } from '../ipc/mockState';
import { BUTTONS } from './buttonMenus';
import { MAIN_MENU } from './mainMenu';
import { buildMenu, type BuiltMenu, type MenuContext, type MenuNode } from './menuModel';
import { hasSetting, registryInfos } from './testRegistry';

type Args = JsonValue | undefined;
const isObject = (args: Args): args is Record<string, JsonValue> =>
  typeof args === 'object' && args !== null && !Array.isArray(args);
const only = (args: Args, keys: string[]) => isObject(args) && Object.keys(args).every((key) => keys.includes(key));
const text = (args: Args, key: string) => isObject(args) && typeof args[key] === 'string' && args[key] !== '';

/**
 * The commands a row passes arguments to, and the shape their core handler reads. A row with
 * arguments for a command not listed here fails the test: add the handler's shape.
 */
const ARGS: Readonly<Record<string, (args: Args) => boolean>> = {
  // presets/mod.rs run_command
  'layout.select_preset': (args) => only(args, ['id']) && text(args, 'id'),
  'environments.select': (args) => only(args, ['id']) && text(args, 'id'),
  'layout.focus_mode': (args) => only(args, ['enabled', 'focus']) && isObject(args) && typeof args.enabled === 'boolean',
  // bookmarks/mod.rs: the bookmark by id
  'bookmarks.open': (args) => only(args, ['id']) && text(args, 'id'),
  // emulation/mod.rs: `fields` of the emulation, the colour scheme light or dark
  'emulation.set': (args) =>
    only(args, ['fields', 'target']) &&
    isObject(args) &&
    only(args.fields, ['colorScheme']) &&
    isObject(args.fields) &&
    ['light', 'dark'].includes(String(args.fields.colorScheme)),
  'emulation.reset_view': (args) => only(args, ['target']) && isObject(args) && args.target === 'all',
  // control/handlers.rs plugins_handler: the plugin list's own commands open Settings > Plugins
  ...Object.fromEntries(
    ['plugins.list', 'plugins.install', 'plugins.enable', 'plugins.disable', 'plugins.remove', 'plugins.trust_key'].map(
      (id): [string, (args: Args) => boolean] => [id, (args) => only(args, ['show']) && isObject(args) && args.show === true],
    ),
  ),
  // app_theme/defs.rs theme_of
  'ui.theme.set': (args) => only(args, ['theme']) && isObject(args) && ['system', 'light', 'dark'].includes(String(args.theme)),
  // RB-094-20, devicesMenu.ts: settings_window/mod.rs (a key), engines/android/commands.rs
  // AddArgs, open_link/mod.rs (a web address), engines/set_engine.rs (the engine)
  'settings.open': (args) => only(args, ['key']) && text(args, 'key'),
  'view.add_android': (args) => only(args, ['serial']) && text(args, 'serial'),
  'app.open_link': (args) => only(args, ['url']) && text(args, 'url') && isObject(args) && String(args.url).startsWith('https://'),
  'view.set_engine': (args) => only(args, ['engine']) && isObject(args) && args.engine === 'safari',
};

/** A state with something in every list the menus read: presets, environments, bookmarks, sizes. */
function fullState(mode: StateSnapshot['layout']['mode'] = 'grid', extra: Partial<StateSnapshot> = {}): StateSnapshot {
  const sample = sampleState();
  return {
    ...sample,
    layout: { ...sample.layout, mode },
    detected: [{ width: 640, sources: [] } as unknown as StateSnapshot['detected'][number]],
    bookmarks: [{ id: 'home', label: 'Home', url: 'http://localhost:3000/' }],
    ...extra,
  };
}

function context(state: StateSnapshot): MenuContext {
  return { t: (key) => key, state, settings: {}, commands: registryInfos(), systemTheme: 'light' };
}

const TREES: [string, readonly MenuNode[]][] = [
  ['main', MAIN_MENU],
  ...Object.entries(BUTTONS).map(([name, button]): [string, readonly MenuNode[]] => [name, button.menu]),
];

function built(state: StateSnapshot): [string, BuiltMenu][] {
  return TREES.map(([name, tree]): [string, BuiltMenu] => [name, buildMenu(tree, context(state))]);
}

const items = (rows: NativeMenuRow[]): Extract<NativeMenuRow, { kind: 'item' }>[] =>
  rows.flatMap((row) => (row.kind === 'item' ? [row] : row.kind === 'submenu' ? items(row.rows) : []));

/** What is wrong with `message`, the message of a menu row; `null` when nothing. */
function fault(message: ShellToCore, known: Set<string>): string | null {
  if (message.type === 'setSetting') {
    return hasSetting(message.key) ? null : `the setting ${message.key} is not in the schema`;
  }
  if (message.type !== 'runCommand') {
    return `a ${message.type} message`;
  }
  if (!known.has(message.id)) {
    return `${message.id} is no command of the registry`;
  }
  if (message.args === undefined) {
    return null;
  }
  const shape = ARGS[message.id];
  if (shape === undefined) {
    return `${message.id} gets arguments the test does not know: ${JSON.stringify(message.args)}`;
  }
  return shape(message.args) ? null : `${message.id} gets arguments its handler does not read: ${JSON.stringify(message.args)}`;
}

describe('the rows of the menus', () => {
  it('send a command of the registry with arguments its handler reads, or a known setting', () => {
    const known = new Set(registryInfos().map((info) => info.id));
    let rows = 0;
    for (const [name, menu] of built(fullState())) {
      for (const [id, message] of menu.actions) {
        rows += 1;
        expect(fault(message, known), `${name} ${id}`).toBeNull();
      }
    }
    // Every list filled: the presets, environments, bookmarks, detected sizes, themes, dock modes.
    expect(rows).toBeGreaterThan(250);
  });

  it('ask the plugin list to show itself', () => {
    const menu = buildMenu(MAIN_MENU, context(fullState()));
    const sent = [...menu.actions.values()].flatMap((message) => (message.type === 'runCommand' ? [message] : []));
    for (const id of ['plugins.list', 'plugins.install', 'plugins.enable', 'plugins.disable', 'plugins.remove', 'plugins.trust_key']) {
      expect(sent.find((message) => message.id === id)?.args, id).toEqual({ show: true });
    }
  });

  it('offer a layout mode command only in its mode', () => {
    const modeRows: Record<string, string[]> = {
      'layout.split.swap': ['split'],
      'layout.split.grow': ['split'],
      'layout.split.shrink': ['split'],
      'canvas.zoom.in': ['canvas'],
      'canvas.zoom.out': ['canvas'],
      'canvas.zoom.reset': ['canvas'],
      'layout.canvas.place': ['canvas'],
      'layout.center': ['canvas', 'horizontal'],
      'layout.focus_next': ['focus', 'horizontal'],
      'layout.focus_prev': ['focus', 'horizontal'],
    };
    for (const mode of ['grid', 'horizontal', 'split', 'canvas', 'focus'] as const) {
      const menu = buildMenu(MAIN_MENU, context(fullState(mode)));
      for (const row of items(menu.rows)) {
        const message = menu.actions.get(row.id);
        const modes = message?.type === 'runCommand' ? modeRows[message.id] : undefined;
        if (modes !== undefined) {
          expect(row.disabled === true, `${message?.type === 'runCommand' ? message.id : ''} in ${mode}`).toBe(!modes.includes(mode));
        }
      }
    }
    // The modes themselves are always a choice.
    const grid = buildMenu(MAIN_MENU, context(fullState('grid')));
    const split = items(grid.rows).find((row) => {
      const message = grid.actions.get(row.id);
      return message?.type === 'runCommand' && message.id === 'layout.mode.split';
    });
    expect(split).toMatchObject({ radio: true, checked: false });
    expect(split?.disabled).toBeUndefined();
  });

  it("checks the minimap by the session's switch in every mode, the setting until then", () => {
    const minimap = (state: StateSnapshot, setting: boolean) => {
      const ctx = { ...context(state), settings: { 'layoutModes.minimap': setting } };
      const menu = buildMenu(MAIN_MENU, ctx);
      return items(menu.rows).find((row) => {
        const message = menu.actions.get(row.id);
        return message?.type === 'runCommand' && message.id === 'layout.minimap.toggle';
      });
    };
    for (const mode of ['grid', 'split', 'canvas'] as const) {
      const off = fullState(mode);
      expect(minimap(off, true), mode).toMatchObject({ checked: true });
      expect(minimap(off, true)?.disabled, mode).toBeUndefined();
      const switched = { ...off, layout: { ...off.layout, minimap: false } };
      expect(minimap(switched, true), mode).toMatchObject({ checked: false });
    }
  });

  it("offer a recording's and a capture's own commands only while one runs", () => {
    const enabled = (state: StateSnapshot, id: string): boolean => {
      const menu = buildMenu(MAIN_MENU, context(state));
      const row = items(menu.rows).find((candidate) => {
        const message = menu.actions.get(candidate.id);
        return message?.type === 'runCommand' && message.id === id;
      });
      expect(row, id).toBeDefined();
      return row?.disabled !== true;
    };
    const idle = fullState();
    const busy = fullState('grid', {
      recorder: { phase: 'recording', name: 'checkout', steps: 2 },
      sitemap: { runId: 'r1', state: 'running', total: 4, done: 1, failed: 0, folder: 'out' } as StateSnapshot['sitemap'],
    });
    for (const id of ['recorder.pause', 'recorder.stop', 'recorder.check', 'sitemap.cancel']) {
      expect(enabled(idle, id), `${id} idle`).toBe(false);
      expect(enabled(busy, id), `${id} running`).toBe(true);
    }
    expect(enabled(idle, 'recorder.start')).toBe(true);
    expect(enabled(idle, 'sitemap.capture')).toBe(true);
    // The gallery of any run, also while it runs (RB-090-90); none without a run.
    expect(enabled(idle, 'sitemap.gallery')).toBe(false);
    expect(enabled(busy, 'sitemap.gallery')).toBe(true);
    const done = fullState('grid', {
      sitemap: {
        runId: 'r1',
        state: 'finished',
        total: 4,
        done: 4,
        failed: 0,
        folder: 'out',
        gallery: 'out/index.html',
      } as StateSnapshot['sitemap'],
    });
    expect(enabled(done, 'sitemap.gallery')).toBe(true);
  });
});
