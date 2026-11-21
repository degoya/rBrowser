// Where the plugin panels sit (RB-080-57): read from the settings `panels.dock`,
// `panels.dockSize.<side>`, `panels.dockState` (open, the icon rail or hidden, RB-080-93) and
// `panels.sidebar` (RB-080-61), changed only through the core's commands
// (`panels.dock.left|right|bottom`, `panels.undock`, `panels.close`, `panels.dock.toggle`) - the
// same ones the shortcuts, the palette, the toolbar's switch and MCP's `panels_dock` run - and, for
// the side bar, the setting itself. The panels page (`PanelsPage.vue`), its header
// (`PanelDockBar.vue`), its side bar (`PanelSideBar.vue`) and the toolbar's switch
// (`toolbar/PanelsToggle.vue`) use them.

import type { IpcClient } from '../ipc/client';
import type { JsonValue, SettingValue } from '../ipc/protocol';

export type DockSide = 'left' | 'right' | 'bottom';
export type DockPosition = DockSide | 'floating';
/** `panels.sidebar`: icons with titles, or icons only. */
export type SidebarMode = 'expanded' | 'icons';
/** `panels.dockState`: open at their size, folded to the icon rail, or hidden (RB-080-93). */
export type DockShown = 'open' | 'rail' | 'hidden';

/** The sides in the Dock menu's order. */
export const DOCK_SIDES: readonly DockSide[] = ['left', 'bottom', 'right'];

export const DOCK_COMMANDS: Record<DockSide, string> = {
  left: 'panels.dock.left',
  right: 'panels.dock.right',
  bottom: 'panels.dock.bottom',
};
export const UNDOCK_COMMAND = 'panels.undock';
/** Closes the panels: to the icon rail, or hidden, as `panels.closeTo` says; `{to: 'hidden'}`
 * hides them whatever it says (RB-080-93). */
export const CLOSE_COMMAND = 'panels.close';
/** Folds the dock to its strip or opens it again (RB-080-61). */
export const TOGGLE_COMMAND = 'panels.dock.toggle';
export const SIDEBAR_SETTING = 'panels.sidebar';
export const STATE_SETTING = 'panels.dockState';
/** How many hidden panels stay loaded (RB-090-38). */
export const KEEP_FRAMES_SETTING = 'panels.keepFrames';
/** The most `panels.keepFrames` allows. */
const MAX_KEEP_FRAMES = 10;

/** The folded dock's strip in CSS px, as the core sizes it (`panel_dock/collapse.rs`): the icon
 * column beside the views, the icon row below them. */
export const STRIP = { width: 44, height: 36 } as const;
/** A page narrower than this (a narrow dock, a small window) shows the side bar as icons. */
export const NARROW_PAGE = 420;

export interface DockState {
  position: DockPosition;
  sizes: Record<DockSide, number>;
  /** `panels.dockState`. */
  shown: DockShown;
  /** Not open: folded to the rail, or hidden. */
  collapsed: boolean;
  sidebar: SidebarMode;
  /** `panels.keepFrames`: the panels shown last kept loaded, hidden, beside the shown one. */
  keepFrames: number;
}

/** The core's defaults (RB-080-93): docked right, folded to the icon rail. */
export const DEFAULT_STATE: DockState = {
  position: 'right',
  sizes: { left: 520, right: 520, bottom: 320 },
  shown: 'rail',
  collapsed: true,
  sidebar: 'expanded',
  keepFrames: 3,
};

function isShown(value: JsonValue | undefined): value is DockShown {
  return value === 'open' || value === 'rail' || value === 'hidden';
}

function isPosition(value: JsonValue | undefined): value is DockPosition {
  return value === 'floating' || value === 'left' || value === 'right' || value === 'bottom';
}

/** The dock state the settings give; a missing or odd value keeps the default. */
export function stateOf(values: SettingValue[]): DockState {
  const byKey = new Map(values.map((entry) => [entry.key, entry.value]));
  const position = byKey.get('panels.dock');
  const sizes = { ...DEFAULT_STATE.sizes };
  for (const side of DOCK_SIDES) {
    const size = byKey.get(`panels.dockSize.${side}`);
    if (typeof size === 'number' && Number.isFinite(size)) {
      sizes[side] = size;
    }
  }
  const value = byKey.get(STATE_SETTING);
  const shown = isShown(value) ? value : DEFAULT_STATE.shown;
  const sidebar = byKey.get(SIDEBAR_SETTING);
  const keep = byKey.get(KEEP_FRAMES_SETTING);
  return {
    position: isPosition(position) ? position : DEFAULT_STATE.position,
    sizes,
    shown,
    collapsed: shown !== 'open',
    sidebar: sidebar === 'icons' || sidebar === 'expanded' ? sidebar : DEFAULT_STATE.sidebar,
    keepFrames:
      typeof keep === 'number' && Number.isInteger(keep) && keep >= 0 ? Math.min(keep, MAX_KEEP_FRAMES) : DEFAULT_STATE.keepFrames,
  };
}

/** A page that cannot read the settings shows its panels open: hiding them could not be undone
 * from the page. */
export const UNREAD_STATE: DockState = { ...DEFAULT_STATE, shown: 'open', collapsed: false };

/** The dock state as the core's settings say now; [`UNREAD_STATE`] when they cannot be read. */
export async function loadState(client: IpcClient): Promise<DockState> {
  try {
    const answer = await client.expect({ type: 'getSettings' }, 'settings');
    return stateOf(answer.values);
  } catch {
    return UNREAD_STATE;
  }
}

/** The side the panels are docked at; `null` in their own window. */
export function dockedSide(state: DockState): DockSide | null {
  return state.position === 'floating' ? null : state.position;
}

/** Folded to its strip: only a dock folds, the panels' own window never. */
export function folded(state: DockState): boolean {
  return state.collapsed && dockedSide(state) !== null;
}

/** Folded, the side the page is the icon rail at - a column beside the views, a row below
 * them - without its header; `null` while open or in their own window. */
export function railSide(state: DockState): DockSide | null {
  return folded(state) ? dockedSide(state) : null;
}

/** The side bar shows icons only: the setting says so, or the page is narrower than
 * [`NARROW_PAGE`]. */
export function iconsOnly(state: DockState, pageWidth: number): boolean {
  return state.sidebar === 'icons' || pageWidth < NARROW_PAGE;
}

/** `{plugin, panel}` of the selected panel's key (`<plugin>.<panel>`), so the page moves with it. */
export function panelArgs(key: string | null): { plugin?: string; panel?: string } {
  if (key === null) {
    return {};
  }
  const dot = key.indexOf('.');
  if (dot <= 0 || dot === key.length - 1) {
    return {};
  }
  return { plugin: key.slice(0, dot), panel: key.slice(dot + 1) };
}
