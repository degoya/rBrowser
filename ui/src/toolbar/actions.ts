// Every toolbar control triggers a command of the command registry (RB-010-17). Until the core
// routes them through `runCommand`, the thin adapter below sends the dedicated IPC message of the
// contract; the command id stays the toolbar's name for the action.

import type { JsonValue, ShellToCore } from '../ipc/protocol';

export type ToolbarAction =
  | { command: 'view.back' }
  | { command: 'view.forward' }
  | { command: 'reload.all'; ignoreCache?: boolean }
  | { command: 'view.navigate'; url: string }
  | { command: 'sync.toggle'; enabled: boolean }
  | { command: 'layout.select_preset'; id: string }
  | { command: 'environments.select'; id: string }
  | { command: 'settings.open' }
  | { command: 'breakpoints.analyze' }
  /** The footer under every view (RB-080-60, `FooterToggle.vue`). */
  | { command: 'ui.view_footer.toggle' }
  /** rBrowser's own theme: system, light, dark (RB-080-95, `ThemeToggle.vue`). */
  | { command: 'ui.theme.cycle' }
  /** Sets rBrowser's theme, the toolbar's menu (RB-090-10). */
  | { command: 'ui.theme.set'; theme: 'system' | 'light' | 'dark' }
  /** The plugin panels open or at their icon rail (RB-080-79, `PanelsToggle.vue`). */
  | { command: 'panels.dock.toggle' }
  | { command: 'layout.focus_mode' }
  | { command: 'layout.zoom_to_fit' }
  /** `source: 'arrowKey'`: the core ignores it outside focus mode or with the setting off. */
  | { command: 'layout.focus_next' | 'layout.focus_prev'; source?: 'arrowKey' }
  /** The layout modes (RB-080-04); `args` are the command's (`layout/ModeSwitch.vue`). */
  | { command: LayoutModeCommand; args?: JsonValue }
  /** Views added and removed (RB-080-41, `AddViewMenu.vue`, `NewSizeBar.vue`). */
  | { command: 'view.add'; breakpoint?: string; x?: number; y?: number }
  | { command: 'view.remove'; view?: number }
  /** A size, a device preset (`device`, RB-080-46) or `custom` for the size input. */
  | {
      command: 'view.add_size';
      width?: number;
      height?: number;
      dpr?: number;
      name?: string;
      device?: string;
      custom?: boolean;
    };

/** The layout modes' commands the toolbar runs (RB-080-04). */
export type LayoutModeCommand =
  | 'layout.mode.grid'
  | 'layout.mode.horizontal'
  | 'layout.mode.split'
  | 'layout.mode.canvas'
  | 'layout.split.swap'
  | 'layout.minimap.toggle'
  | 'layout.center'
  | 'layout.canvas.place'
  /** Without `args` the core asks for the name (`layout/SavePresetBar.vue`). */
  | 'layout.save_preset';

export type ToolbarCommandId = ToolbarAction['command'] | 'view.focus_address_bar';

export interface ToolbarCommand {
  id: ToolbarCommandId;
  /** The MCP tool, or why there is none (`omitted(...)`). */
  mcp: string;
}

/** The toolbar's commands, as the registry and the MCP coverage table should list them. */
export const TOOLBAR_COMMANDS = {
  back: { id: 'view.back', mcp: 'back' },
  forward: { id: 'view.forward', mcp: 'forward' },
  reload: { id: 'reload.all', mcp: 'reload' },
  navigate: { id: 'view.navigate', mcp: 'navigate' },
  sync: { id: 'sync.toggle', mcp: 'sync_set' },
  layout: { id: 'layout.select_preset', mcp: 'layout_set' },
  environment: { id: 'environments.select', mcp: 'environment_set' },
  settings: { id: 'settings.open', mcp: 'omitted(ui-only)' },
  analyze: { id: 'breakpoints.analyze', mcp: 'breakpoints_detect' },
  focusMode: { id: 'layout.focus_mode', mcp: 'layout_set' },
  focusNext: { id: 'layout.focus_next', mcp: 'layout_set' },
  focusPrev: { id: 'layout.focus_prev', mcp: 'layout_set' },
  zoomToFit: { id: 'layout.zoom_to_fit', mcp: 'omitted(ui-only: grid zoom)' },
  modeGrid: { id: 'layout.mode.grid', mcp: 'layout_set' },
  modeHorizontal: { id: 'layout.mode.horizontal', mcp: 'layout_set' },
  modeSplit: { id: 'layout.mode.split', mcp: 'layout_set' },
  modeCanvas: { id: 'layout.mode.canvas', mcp: 'layout_set' },
  splitSwap: { id: 'layout.split.swap', mcp: 'layout_set' },
  minimap: { id: 'layout.minimap.toggle', mcp: 'omitted(display only: the minimap in the toolbar)' },
  center: { id: 'layout.center', mcp: 'omitted(ui-only)' },
  canvasPlace: { id: 'layout.canvas.place', mcp: 'canvas_place' },
  savePreset: { id: 'layout.save_preset', mcp: 'layout_save_preset' },
  focusAddressBar: { id: 'view.focus_address_bar', mcp: 'omitted(ui-only)' },
  addView: { id: 'view.add', mcp: 'view_add' },
  removeView: { id: 'view.remove', mcp: 'view_remove' },
  addSize: { id: 'view.add_size', mcp: 'view_add_size' },
  footer: { id: 'ui.view_footer.toggle', mcp: 'omitted(display switch; settings_set writes ui.viewFooter.visible)' },
  theme: { id: 'ui.theme.cycle', mcp: 'omitted(app display setting; settings_set writes ui.theme)' },
  themeSet: { id: 'ui.theme.set', mcp: 'omitted(app display setting; settings_set writes ui.theme)' },
  panels: { id: 'panels.dock.toggle', mcp: 'panels_dock' },
} as const satisfies Record<string, ToolbarCommand>;

/** `fields` without the undefined ones, as a command's arguments; `undefined` when none is set. */
function argsOf(fields: Record<string, JsonValue | undefined>): JsonValue | undefined {
  const set = Object.entries(fields).filter((entry): entry is [string, JsonValue] => entry[1] !== undefined);
  return set.length === 0 ? undefined : Object.fromEntries(set);
}

export function toMessage(action: ToolbarAction): ShellToCore {
  switch (action.command) {
    case 'view.back':
      return { type: 'back' };
    case 'view.forward':
      return { type: 'forward' };
    case 'reload.all':
      return { type: 'reload', ignoreCache: action.ignoreCache ?? false };
    case 'view.navigate':
      return { type: 'navigate', url: action.url };
    case 'sync.toggle':
      return { type: 'setSync', feature: 'all', enabled: action.enabled };
    case 'layout.select_preset':
      return { type: 'selectLayoutPreset', id: action.id };
    case 'environments.select':
      return { type: 'selectEnvironment', id: action.id };
    case 'settings.open':
      return { type: 'openSettings' };
    case 'breakpoints.analyze':
    case 'ui.view_footer.toggle':
    case 'ui.theme.cycle':
    case 'panels.dock.toggle':
    case 'layout.focus_mode':
    case 'layout.zoom_to_fit':
      return { type: 'runCommand', id: action.command };
    case 'layout.focus_next':
    case 'layout.focus_prev':
      return action.source === undefined
        ? { type: 'runCommand', id: action.command }
        : { type: 'runCommand', id: action.command, args: { source: action.source } };
    case 'layout.mode.grid':
    case 'layout.mode.horizontal':
    case 'layout.mode.split':
    case 'layout.mode.canvas':
    case 'layout.split.swap':
    case 'layout.minimap.toggle':
    case 'layout.center':
    case 'layout.canvas.place':
    case 'layout.save_preset':
      return action.args === undefined
        ? { type: 'runCommand', id: action.command }
        : { type: 'runCommand', id: action.command, args: action.args };
    case 'ui.theme.set':
      return { type: 'runCommand', id: action.command, args: { theme: action.theme } };
    case 'view.add':
      return withArgs(action.command, argsOf({ breakpoint: action.breakpoint, x: action.x, y: action.y }));
    case 'view.remove':
      return withArgs(action.command, argsOf({ view: action.view }));
    case 'view.add_size':
      return withArgs(
        action.command,
        argsOf({
          width: action.width,
          height: action.height,
          dpr: action.dpr,
          name: action.name,
          device: action.device,
          custom: action.custom,
        }),
      );
  }
}

function withArgs(id: string, args: JsonValue | undefined): ShellToCore {
  return args === undefined ? { type: 'runCommand', id } : { type: 'runCommand', id, args };
}
